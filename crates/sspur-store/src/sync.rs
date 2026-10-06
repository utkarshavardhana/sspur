//! Replica sync. Replicas exchange commits and the objects they reference by hash, then merge
//! with the same register rules as local transactions. A remote is another codebase directory or
//! a `sspur sync serve` endpoint (`tcp://host:port`), spoken to with one JSON line per request.

use crate::crdt::Commit;
use crate::{Prov, Store, TxResult};
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;

pub enum Remote {
    Dir(Store),
    Tcp(String),
}

impl Remote {
    pub fn parse(spec: &str) -> Result<Remote, String> {
        if let Some(a) = spec.strip_prefix("tcp://") {
            return Ok(Remote::Tcp(a.to_string()));
        }
        let p = Path::new(spec);
        if p.join(crate::DIR).is_dir() {
            return Store::find(p).map(Remote::Dir).ok_or_else(|| format!("no codebase at {spec}"));
        }
        if spec.contains(':') && !p.exists() {
            return Ok(Remote::Tcp(spec.to_string()));
        }
        Err(format!("no codebase at {spec} (give a directory with .sspur or tcp://host:port)"))
    }

    pub fn call(&self, req: &Json) -> Result<Json, String> {
        match self {
            Remote::Dir(s) => Ok(handle(s, req)),
            Remote::Tcp(addr) => {
                let mut c = TcpStream::connect(addr).map_err(|e| format!("cannot connect to {addr}: {e}"))?;
                let mut line = serde_json::to_string(req).unwrap();
                line.push('\n');
                c.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
                let mut out = String::new();
                BufReader::new(c).read_line(&mut out).map_err(|e| e.to_string())?;
                serde_json::from_str(&out).map_err(|e| format!("bad reply from {addr}: {e}"))
            }
        }
    }
}

fn heads(s: &Store) -> Vec<String> {
    s.recover();
    s.index().map(|ix| ix.heads).unwrap_or_default()
}

pub fn have(s: &Store) -> HashSet<String> {
    s.ancestors(&heads(s))
}

pub fn bundle(s: &Store, ids: &[String]) -> Json {
    let mut commits = Vec::new();
    let mut texts = BTreeMap::new();
    let mut nodes = BTreeMap::new();
    for id in ids {
        let Some(c) = s.commit(id) else { continue };
        for w in &c.writes {
            if let Some(Some(b)) = &w.body {
                if let Some(t) = s.text(&b.text) {
                    texts.insert(b.text.clone(), t);
                }
                if let Some(p) = s.prov(&b.hash) {
                    nodes.insert(b.hash.clone(), p);
                }
            }
        }
        commits.push(c);
    }
    json!({"commits": commits, "texts": texts, "nodes": nodes})
}

/// Stores the objects of a bundle. Every object is checked against its content address.
pub fn ingest(s: &Store, b: &Json) -> Result<usize, String> {
    let texts: BTreeMap<String, String> = serde_json::from_value(b.get("texts").cloned().unwrap_or(json!({}))).map_err(|e| e.to_string())?;
    for (id, t) in &texts {
        if crate::text_id(t) != *id {
            return Err(format!("text {id} does not match its hash"));
        }
        s.put_text(t).map_err(|e| e.to_string())?;
    }
    let nodes: BTreeMap<String, Prov> = serde_json::from_value(b.get("nodes").cloned().unwrap_or(json!({}))).map_err(|e| e.to_string())?;
    for (h, p) in &nodes {
        if s.prov(h).is_none() {
            s.put_prov(h, p).map_err(|e| e.to_string())?;
        }
    }
    let commits: Vec<Commit> = serde_json::from_value(b.get("commits").cloned().unwrap_or(json!([]))).map_err(|e| e.to_string())?;
    for c in &commits {
        if c.clone().seal().id != c.id {
            return Err(format!("commit {} does not match its hash", c.id));
        }
        for w in &c.writes {
            if let Some(Some(b)) = &w.body
                && s.text(&b.text).is_none() {
                    return Err(format!("commit {} needs text {} which was not sent", c.id, b.text));
                }
        }
    }
    for (i, c) in commits.iter().enumerate() {
        s.put_commit(c).map_err(|e| e.to_string())?;
        if i == 0 {
            crate::crash_point("ingest");
        }
    }
    Ok(commits.len())
}

fn result(r: &TxResult) -> Json {
    json!({"ok": r.ok, "root": r.root, "changes": r.changes, "diags": r.diags, "conflicts": r.conflicts})
}

pub fn handle(s: &Store, req: &Json) -> Json {
    match req.get("op").and_then(Json::as_str).unwrap_or_default() {
        "heads" => json!({"heads": heads(s), "root": s.head()}),
        "have" => {
            let mut v: Vec<String> = have(s).into_iter().collect();
            v.sort();
            json!({"heads": heads(s), "have": v})
        }
        "fetch" => {
            let ids: Vec<String> = req.get("ids").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
            bundle(s, &ids)
        }
        "deps" => crate::pkg::bundle(&s.root_dir()),
        "push" => {
            if let Some(d) = req.get("pkgs")
                && let Err(e) = crate::pkg::ingest_bundle(&s.root_dir(), d) {
                    return json!({"ok": false, "error": e});
                }
            if let Err(e) = ingest(s, req) {
                return json!({"ok": false, "error": e});
            }
            let hs: Vec<String> = req.get("heads").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
            let agent = req.get("agent").and_then(Json::as_str).unwrap_or("sync");
            result(&s.merge_remote(&hs, agent, false))
        }
        op => json!({"ok": false, "error": format!("unknown sync op '{op}'")}),
    }
}

#[derive(Debug)]
pub struct Report {
    pub sent: usize,
    pub received: usize,
    pub result: Json,
}

pub fn pull(local: &Store, remote: &Remote, agent: &str) -> Result<Report, String> {
    let info = remote.call(&json!({"op": "have"}))?;
    let theirs: Vec<String> = serde_json::from_value(info["have"].clone()).map_err(|e| e.to_string())?;
    let rheads: Vec<String> = serde_json::from_value(info["heads"].clone()).map_err(|e| e.to_string())?;
    let mine = have(local);
    let want: Vec<String> = theirs.into_iter().filter(|c| !mine.contains(c)).collect();
    if want.is_empty() {
        return Ok(Report { sent: 0, received: 0, result: json!({"ok": true, "root": local.head(), "up_to_date": true}) });
    }
    let d = remote.call(&json!({"op": "deps"}))?;
    if d.get("error").is_none() {
        crate::pkg::ingest_bundle(&local.root_dir(), &d)?;
    }
    let b = remote.call(&json!({"op": "fetch", "ids": want}))?;
    let n = ingest(local, &b)?;
    Ok(Report { sent: 0, received: n, result: result(&local.merge_remote(&rheads, agent, true)) })
}

pub fn push(local: &Store, remote: &Remote, agent: &str) -> Result<Report, String> {
    let info = remote.call(&json!({"op": "have"}))?;
    let theirs: HashSet<String> = serde_json::from_value(info["have"].clone()).map_err(|e| e.to_string())?;
    let mut send: Vec<String> = have(local).into_iter().filter(|c| !theirs.contains(c)).collect();
    send.sort();
    if send.is_empty() {
        return Ok(Report { sent: 0, received: 0, result: json!({"ok": true, "up_to_date": true}) });
    }
    let mut req = bundle(local, &send);
    req["op"] = json!("push");
    req["heads"] = json!(heads(local));
    req["agent"] = json!(agent);
    req["pkgs"] = crate::pkg::bundle(&local.root_dir());
    let r = remote.call(&req)?;
    if let Some(e) = r.get("error") {
        return Err(e.as_str().unwrap_or_default().to_string());
    }
    Ok(Report { sent: send.len(), received: 0, result: r })
}

/// Serves `s` until `max` requests were handled (forever with `None`). Prints the bound address.
pub fn serve(s: &Store, addr: &str, max: Option<usize>, on_bind: impl FnOnce(&str)) -> Result<(), String> {
    let l = TcpListener::bind(addr).map_err(|e| format!("cannot listen on {addr}: {e}"))?;
    let local = l.local_addr().map_err(|e| e.to_string())?.to_string();
    on_bind(&local);
    let mut served = 0;
    for conn in l.incoming() {
        let Ok(mut c) = conn else { continue };
        let mut line = String::new();
        if BufReader::new(&c).read_line(&mut line).is_err() {
            continue;
        }
        let reply = match serde_json::from_str::<Json>(&line) {
            Ok(req) => handle(s, &req),
            Err(e) => json!({"ok": false, "error": format!("bad request: {e}")}),
        };
        let mut out = serde_json::to_string(&reply).unwrap();
        out.push('\n');
        let _ = c.write_all(out.as_bytes());
        served += 1;
        if max.is_some_and(|m| served >= m) {
            break;
        }
    }
    Ok(())
}
