//! Definitions as CRDT registers. Every definition has a stable id; its name and its body are
//! multi-value registers. A write supersedes the dots it observed, so the state of a replica is a
//! pure function of the set of commits it holds, whatever order they arrived in.

use crate::cache::idents;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use sspur_syntax::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub type Dot = String;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Body {
    pub hash: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refs: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub um: Vec<[u32; 2]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub test: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Write {
    pub def: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Option<Body>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sup: Vec<Dot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commit {
    pub id: String,
    pub parents: Vec<String>,
    pub agent: String,
    #[serde(default)]
    pub reason: String,
    pub at: String,
    pub writes: Vec<Write>,
}

impl Commit {
    pub fn seal(mut self) -> Commit {
        self.id = String::new();
        let raw = serde_json::to_string(&self).unwrap();
        self.id = sspur_hash::base32(blake3::hash(raw.as_bytes()).as_bytes())[..20].to_string();
        self
    }

    pub fn dot(&self, i: usize) -> Dot {
        format!("{}.{i}", self.id)
    }
}

pub fn dot_commit(d: &str) -> &str {
    d.split('.').next().unwrap_or(d)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Regs {
    #[serde(default)]
    pub name: Vec<(Dot, String)>,
    #[serde(default)]
    pub body: Vec<(Dot, Option<Body>)>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Index {
    pub heads: Vec<String>,
    #[serde(default)]
    pub root: String,
    #[serde(default)]
    pub seq: u64,
    pub defs: BTreeMap<String, Regs>,
}

impl Index {
    pub fn apply(&mut self, c: &Commit) {
        for (i, w) in c.writes.iter().enumerate() {
            let r = self.defs.entry(w.def.clone()).or_default();
            let dot = c.dot(i);
            if r.name.iter().any(|x| x.0 == dot) || r.body.iter().any(|x| x.0 == dot) {
                continue;
            }
            if let Some(n) = &w.name {
                r.name.retain(|(d, _)| !w.sup.contains(d));
                r.name.push((c.dot(i), n.clone()));
                r.name.sort_by(|a, b| a.0.cmp(&b.0));
            }
            if let Some(b) = &w.body {
                r.body.retain(|(d, _)| !w.sup.contains(d));
                r.body.push((c.dot(i), b.clone()));
                r.body.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct VDef {
    pub id: String,
    pub name: String,
    pub body: Body,
    pub ndot: Dot,
    pub bdot: Dot,
}

#[derive(Clone, Debug, Serialize)]
pub struct Clash {
    pub path: String,
    pub kind: &'static str,
    pub ids: Vec<String>,
    pub dots: Vec<Dot>,
}

/// The program a register state denotes, plus every place where it does not denote one.
pub fn view(ix: &Index) -> (Vec<VDef>, Vec<Clash>) {
    let mut defs = Vec::new();
    let mut clashes = Vec::new();
    for (id, r) in &ix.defs {
        if r.body.len() > 1 || r.name.len() > 1 {
            let path = r.name.first().map(|n| n.1.clone()).unwrap_or_default();
            let kind = if r.body.iter().any(|b| b.1.is_none()) { "removed" } else if r.body.len() > 1 { "body" } else { "rename" };
            let dots = r.body.iter().map(|b| b.0.clone()).chain(r.name.iter().map(|n| n.0.clone())).collect();
            clashes.push(Clash { path, kind, ids: vec![id.clone()], dots });
            continue;
        }
        let (Some((bdot, Some(body))), Some((ndot, name))) = (r.body.first(), r.name.first()) else { continue };
        defs.push(VDef { id: id.clone(), name: name.clone(), body: body.clone(), ndot: ndot.clone(), bdot: bdot.clone() });
    }
    let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, d) in defs.iter().enumerate() {
        by_name.entry(d.name.clone()).or_default().push(i);
    }
    let mut drop = HashSet::new();
    for (name, is) in by_name.iter().filter(|(_, is)| is.len() > 1) {
        if is.iter().all(|i| defs[*i].body.test) {
            for i in &is[1..] {
                let id = defs[*i].id.clone();
                defs[*i].name = format!("{name}_{}", &id[..6.min(id.len())]);
            }
        } else {
            clashes.push(Clash { path: name.clone(), kind: "name", ids: is.iter().map(|i| defs[*i].id.clone()).collect(), dots: is.iter().map(|i| defs[*i].ndot.clone()).collect() });
            drop.extend(is.iter().copied());
        }
    }
    let defs = defs.into_iter().enumerate().filter(|(i, _)| !drop.contains(i)).map(|(_, d)| d).collect();
    (defs, clashes)
}

fn rename_def(text: &str, pairs: &[(String, String)], um: &[[u32; 2]]) -> Option<String> {
    let mut m = parse(text).ok()?;
    if m.defs.len() != 1 {
        return None;
    }
    let um: HashSet<(u32, u32)> = um.iter().map(|[a, b]| (*a, *b)).collect();
    for (i, (from, _)) in pairs.iter().enumerate() {
        rename::rename_module(&mut m, from, &format!("__sspur_fix{i}"), &um);
    }
    for (i, (_, to)) in pairs.iter().enumerate() {
        rename::rename_module(&mut m, &format!("__sspur_fix{i}"), to, &um);
    }
    Some(printer::print_def(&m.defs[0]))
}

/// The text of `d` with every reference to a renamed definition (and its own head) brought up to
/// the names in `names` (id to current name).
pub fn fixed_text(src: &str, head: &str, d: &VDef, names: &HashMap<String, String>) -> Option<String> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    if head != d.name {
        pairs.push((head.to_string(), d.name.clone()));
    }
    for (n, id) in &d.body.refs {
        if let Some(cur) = names.get(id)
            && cur != n && n != head {
                pairs.push((n.clone(), cur.clone()));
            }
    }
    if pairs.is_empty() {
        return None;
    }
    rename_def(src, &pairs, &d.body.um)
}

/// Top-level names a definition's text refers to, mapped to ids.
pub fn refs_of(text: &str, ids: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    idents(text).into_iter().filter_map(|n| ids.get(n).map(|id| (n.to_string(), id.clone()))).collect()
}

pub fn head_name(text: &str) -> Option<String> {
    let mut words = text.split_whitespace();
    let mut w = words.next()?;
    while matches!(w, "res" | "kernel" | "extern" | "interrupt" | "pub") {
        w = words.next()?;
    }
    let n = words.next()?;
    let end = n.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(n.len());
    Some(n[..end].to_string())
}

/// Commits in `want` that are not in `have`, parents first.
pub fn topo(commits: &HashMap<String, Commit>, want: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut done: HashSet<String> = HashSet::new();
    let mut ids: Vec<&String> = want.iter().collect();
    ids.sort();
    for id in ids {
        let mut stack = vec![(id.clone(), false)];
        while let Some((c, expanded)) = stack.pop() {
            if done.contains(&c) || !want.contains(&c) {
                continue;
            }
            if expanded {
                done.insert(c.clone());
                out.push(c);
                continue;
            }
            stack.push((c.clone(), true));
            if let Some(cm) = commits.get(&c) {
                for p in cm.parents.iter().rev() {
                    if !done.contains(p) {
                        stack.push((p.clone(), false));
                    }
                }
            }
        }
    }
    out
}

pub fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let raw = format!("{}:{t}:{}:{:?}", std::process::id(), N.fetch_add(1, Ordering::Relaxed), std::thread::current().id());
    sspur_hash::base32(blake3::hash(raw.as_bytes()).as_bytes())[..16].to_string()
}

pub fn patch_like(op: &Json) -> bool {
    matches!(op.get("op").and_then(Json::as_str), Some("refine" | "fill" | "attach"))
}
