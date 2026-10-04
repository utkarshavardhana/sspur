use crate::local::{start_replay, Known, Req};
use crate::Service;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

#[derive(Debug)]
pub struct Diff {
    pub id: String,
    pub request: String,
    pub what: String,
    pub old: String,
    pub new: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub total: usize,
    pub same: usize,
    pub extended: usize,
    pub diffs: Vec<Diff>,
}

impl Report {
    pub fn passed(&self, strict: bool) -> bool {
        self.diffs.is_empty() && (!strict || self.extended == 0)
    }

    pub fn text(&self) -> String {
        let mut s = format!("replayed {} requests: {} identical, {} extended (only new null fields), {} differ\n", self.total, self.same, self.extended, self.diffs.iter().map(|d| &d.id).collect::<BTreeSet<_>>().len());
        for d in &self.diffs {
            s.push_str(&format!("  {} {} {}\n    recorded: {}\n    new:      {}\n", d.id, d.request, d.what, d.old, d.new));
        }
        s
    }
}

fn parse(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string()))
}

pub fn extends(old: &Value, new: &Value) -> bool {
    match (old, new) {
        (Value::Object(a), Value::Object(b)) => a.iter().all(|(k, v)| b.get(k).is_some_and(|w| extends(v, w))) && b.iter().all(|(k, v)| a.contains_key(k) || v.is_null()),
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| extends(x, y)),
        _ => old == new,
    }
}

fn store_of(table: &str, svc: &str) -> String {
    table.strip_prefix(&format!("{svc}-")).unwrap_or(table).to_string()
}

fn writes(db: &[Value], svc: &str) -> Vec<Value> {
    db.iter()
        .filter_map(|e| {
            let store = store_of(e["table"].as_str().unwrap_or(""), svc);
            match e["op"].as_str()? {
                "PutItem" => Some(json!({"op": "put", "store": store, "key": e["req"]["Item"]["pk"], "value": parse(e["req"]["Item"]["v"]["S"].as_str().unwrap_or("null"))})),
                "DeleteItem" => Some(json!({"op": "del", "store": store, "key": e["req"]["Key"]["pk"]})),
                _ => None,
            }
        })
        .collect()
}

type Seed = (HashMap<String, BTreeMap<String, Value>>, HashMap<String, Known>);

fn seed(db: &[Value], rec_svc: &str, svc: &str) -> Seed {
    let mut items: HashMap<String, BTreeMap<String, Value>> = HashMap::new();
    let mut known: HashMap<String, Known> = HashMap::new();
    let mut written: BTreeSet<(String, String)> = BTreeSet::new();
    for e in db {
        let table = format!("{svc}-{}", store_of(e["table"].as_str().unwrap_or(""), rec_svc));
        let k = known.entry(table.clone()).or_default();
        let t = items.entry(table.clone()).or_default();
        let mut see = |pk: String, item: Option<&Value>, k: &mut Known| {
            if written.contains(&(table.clone(), pk.clone())) || k.keys.contains(&pk) {
                return;
            }
            k.keys.insert(pk.clone());
            if let Some(it) = item {
                t.insert(pk, it.clone());
            }
        };
        match e["op"].as_str().unwrap_or("") {
            "GetItem" => see(e["req"]["Key"]["pk"].to_string(), e["resp"].get("Item"), k),
            "DeleteItem" => see(e["req"]["Key"]["pk"].to_string(), e["resp"].get("Attributes"), k),
            "Scan" => {
                for it in e["resp"]["Items"].as_array().into_iter().flatten() {
                    see(it["pk"].to_string(), Some(it), k);
                }
                if e["resp"].get("LastEvaluatedKey").is_none() {
                    k.complete = true;
                }
            }
            "PutItem" => {
                written.insert((table.clone(), e["req"]["Item"]["pk"].to_string()));
            }
            _ => {}
        }
        if e["op"] == "DeleteItem" {
            written.insert((table, e["req"]["Key"]["pk"].to_string()));
        }
    }
    (items, known)
}

pub fn replay(recording: &str, svc: &Service, bootstrap: &Path) -> Result<Report, String> {
    let recs: Vec<Value> = recording.lines().filter(|l| !l.trim().is_empty()).enumerate().map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("recording line {}: {e}", i + 1))).collect::<Result<_, _>>()?;
    let local = start_replay(svc, bootstrap)?;
    let mut rep = Report::default();
    for r in &recs {
        rep.total += 1;
        let id = r["id"].as_str().unwrap_or("?").to_string();
        let method = r["method"].as_str().unwrap_or("GET").to_string();
        let path = r["path"].as_str().unwrap_or("/").to_string();
        let request = format!("{method} {path}");
        let db: Vec<Value> = r["db"].as_array().cloned().unwrap_or_default();
        let rec_svc = r["svc"].as_str().unwrap_or(&svc.name).to_string();
        let (items, known) = seed(&db, &rec_svc, &svc.name);
        let req = Req { method, path, query: r["query"].as_str().unwrap_or("").to_string(), headers: vec![("content-type".into(), "application/json".into())], body: r["body"].as_str().unwrap_or("").as_bytes().to_vec() };
        let (a, new_db, misses) = local.replay_one(&req, items, known);
        let mut diffs = Vec::new();
        let mut extended = false;
        let status = r["status"].as_u64().unwrap_or(0) as u16;
        if status != a.resp.status {
            diffs.push(("status".to_string(), status.to_string(), a.resp.status.to_string()));
        }
        let (old_b, new_b) = (parse(r["response"].as_str().unwrap_or("")), parse(&String::from_utf8_lossy(&a.resp.body)));
        if old_b != new_b {
            if extends(&old_b, &new_b) {
                extended = true;
            } else {
                diffs.push(("response".into(), old_b.to_string(), new_b.to_string()));
            }
        }
        let (ow, nw) = (Value::Array(writes(&db, &rec_svc)), Value::Array(writes(&new_db, &svc.name)));
        if ow != nw {
            if extends(&ow, &nw) {
                extended = true;
            } else {
                diffs.push(("writes".into(), ow.to_string(), nw.to_string()));
            }
        }
        for m in misses {
            diffs.push(("read outside the recording".into(), "-".into(), m));
        }
        if diffs.is_empty() {
            if extended {
                rep.extended += 1;
            } else {
                rep.same += 1;
            }
        }
        rep.diffs.extend(diffs.into_iter().map(|(what, old, new)| Diff { id: id.clone(), request: request.clone(), what, old, new }));
    }
    local.stop();
    Ok(rep)
}
