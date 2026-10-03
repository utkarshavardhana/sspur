pub mod query;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value as Json};
use sspur_check::{check, syntax_diag, CheckOutput, Diag};
use sspur_hash::{base32, hash_module_with, root_hash, Resolution};
use sspur_syntax::*;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const DIR: &str = ".sspur";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub hash: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub old: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub renamed_from: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Root {
    pub hash: String,
    pub parent: Option<String>,
    pub at: String,
    pub agent: String,
    pub reason: String,
    pub names: BTreeMap<String, Entry>,
    pub changes: Vec<Change>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Prov {
    pub agent: String,
    pub reason: String,
    pub at: String,
    #[serde(default)]
    pub reqs: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Tx {
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub gate: Option<String>,
    pub ops: Vec<Json>,
}

#[derive(Debug, Serialize)]
pub struct TxResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<Change>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diags: Vec<Diag>,
    #[serde(skip)]
    pub src: Option<String>,
}

impl TxResult {
    fn fail(code: &str, msg: String) -> Self {
        TxResult {
            ok: false,
            root: None,
            changes: vec![],
            diags: vec![Diag { code: code.into(), severity: "error", def: None, span: [0, 0], msg, hint: None, fix: vec![] }],
            src: None,
        }
    }
}

pub struct Loaded {
    pub src: String,
    pub module: Module,
    pub check: CheckOutput,
    pub hashes: BTreeMap<String, String>,
}

pub struct Store {
    dir: PathBuf,
}

pub fn now() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn def_rank(d: &Def) -> u8 {
    match d {
        Def::Type(_) => 0,
        Def::Fn(_) => 1,
        Def::Test(_) => 2,
    }
}

pub fn render(defs: &[Def]) -> String {
    let mut sorted: Vec<&Def> = defs.iter().collect();
    sorted.sort_by(|a, b| def_rank(a).cmp(&def_rank(b)).then_with(|| a.name().cmp(b.name())));
    let parts: Vec<String> = sorted.into_iter().map(printer::print_def).collect();
    if parts.is_empty() { String::new() } else { parts.join("\n\n") + "\n" }
}

pub fn load_src(src: String) -> Result<Loaded, Vec<Diag>> {
    let module = parse(&src).map_err(|e| vec![syntax_diag(&e)])?;
    let check = check(&module);
    let res = Resolution { user_methods: Some(&check.user_methods), record_types: Some(&check.record_types) };
    let hashes = hash_module_with(&module, &res).into_iter().collect();
    Ok(Loaded { src, module, check, hashes })
}

impl Store {
    pub fn init(dir: &Path) -> std::io::Result<Store> {
        let root = dir.join(DIR);
        for sub in ["text", "nodes", "roots"] {
            std::fs::create_dir_all(root.join(sub))?;
        }
        Ok(Store { dir: root })
    }

    pub fn find(start: &Path) -> Option<Store> {
        let mut cur = Some(start);
        while let Some(d) = cur {
            if d.join(DIR).is_dir() {
                return Some(Store { dir: d.join(DIR) });
            }
            cur = d.parent();
        }
        None
    }

    fn read(&self, rel: &str) -> Option<String> {
        std::fs::read_to_string(self.dir.join(rel)).ok()
    }

    fn write(&self, rel: &str, data: &str) -> std::io::Result<()> {
        let p = self.dir.join(rel);
        let tmp = p.with_extension("tmp");
        std::fs::write(&tmp, data)?;
        std::fs::rename(tmp, p)
    }

    pub fn head(&self) -> Option<String> {
        self.read("HEAD").map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }

    pub fn root(&self, hash: &str) -> Option<Root> {
        serde_json::from_str(&self.read(&format!("roots/{hash}.json"))?).ok()
    }

    pub fn head_root(&self) -> Option<Root> {
        self.root(&self.head()?)
    }

    pub fn prov(&self, node: &str) -> Option<Prov> {
        serde_json::from_str(&self.read(&format!("nodes/{node}.json"))?).ok()
    }

    pub fn text(&self, id: &str) -> Option<String> {
        self.read(&format!("text/{id}"))
    }

    pub fn root_src(&self, r: &Root) -> String {
        let mut parts: Vec<(u8, String, String)> = r
            .names
            .iter()
            .map(|(n, e)| {
                let t = self.text(&e.text).unwrap_or_default();
                let rank = if t.starts_with("type ") { 0 } else if t.starts_with("fn ") { 1 } else { 2 };
                (rank, n.clone(), t)
            })
            .collect();
        parts.sort();
        let texts: Vec<String> = parts.into_iter().map(|(_, _, t)| t).collect();
        if texts.is_empty() { String::new() } else { texts.join("\n\n") + "\n" }
    }

    pub fn load_head(&self) -> Result<Loaded, Vec<Diag>> {
        let src = self.head_root().map(|r| self.root_src(&r)).unwrap_or_default();
        load_src(src)
    }

    pub fn log(&self) -> Vec<Root> {
        let mut out = Vec::new();
        let mut cur = self.head();
        while let Some(h) = cur {
            let Some(r) = self.root(&h) else { break };
            cur = r.parent.clone();
            out.push(r);
        }
        out
    }

    pub fn history(&self, name: &str) -> Vec<(Root, String)> {
        let mut out = Vec::new();
        let mut want = name.to_string();
        for r in self.log() {
            if let Some(c) = r.changes.iter().find(|c| c.path == want) {
                let h = c.new.clone().unwrap_or_default();
                let from = c.renamed_from.clone();
                out.push((r, h));
                if let Some(f) = from {
                    want = f;
                }
            }
        }
        out
    }

    pub fn apply(&self, tx: Tx) -> TxResult {
        let head = self.head();
        if let Some(b) = &tx.base
            && Some(b) != head.as_ref() && !(b.is_empty() && head.is_none()) {
                return TxResult::fail("E_STALE_BASE", format!("base {b} is not HEAD {}; re-query and retry", head.unwrap_or_default()));
            }
        let current = match self.load_head() {
            Ok(l) => l,
            Err(d) => return TxResult { ok: false, root: None, changes: vec![], diags: d, src: None },
        };
        let agent = tx.agent.unwrap_or_else(|| "unknown".into());
        let reason = tx.reason.unwrap_or_default();
        let mut defs = current.module.defs.clone();
        let mut reqs: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (i, op) in tx.ops.iter().enumerate() {
            if let Err(msg) = apply_op(op, &mut defs, &mut reqs) {
                let mut r = TxResult::fail(&msg.0, format!("op {i}: {}", msg.1));
                r.diags[0].hint = msg.2;
                return r;
            }
        }
        let next = match load_src(render(&defs)) {
            Ok(l) => l,
            Err(d) => return TxResult { ok: false, root: None, changes: vec![], diags: d, src: None },
        };
        if next.check.has_errors() {
            return TxResult { ok: false, root: None, changes: vec![], diags: next.check.diags, src: Some(next.src) };
        }
        if tx.gate.as_deref() == Some("tests") {
            let it = sspur_eval::Interp::new(&next.module, next.check.record_types.clone(), next.check.user_methods.clone());
            let failed: Vec<Diag> = it
                .run_tests()
                .into_iter()
                .filter_map(|(n, r)| r.err().map(|e| Diag { code: "E_TEST_FAILED".into(), severity: "error", def: Some(n.clone()), span: [0, 0], msg: format!("test {n} failed: {e}"), hint: None, fix: vec![] }))
                .collect();
            if !failed.is_empty() {
                return TxResult { ok: false, root: None, changes: vec![], diags: failed, src: None };
            }
        }
        let at = now();
        let mut names = BTreeMap::new();
        for d in &next.module.defs {
            let text = printer::print_def(d);
            let text_id = base32(blake3::hash(text.as_bytes()).as_bytes());
            if self.text(&text_id).is_none()
                && let Err(e) = self.write(&format!("text/{text_id}"), &text) {
                    return TxResult::fail("E_IO", e.to_string());
                }
            let hash = next.hashes[d.name()].clone();
            let mut prov = self.prov(&hash).unwrap_or(Prov { agent: agent.clone(), reason: reason.clone(), at: at.clone(), reqs: vec![] });
            if let Some(extra) = reqs.get(d.name()) {
                for r in extra {
                    if !prov.reqs.contains(r) {
                        prov.reqs.push(r.clone());
                    }
                }
            }
            if let Err(e) = self.write(&format!("nodes/{hash}.json"), &serde_json::to_string(&prov).unwrap()) {
                return TxResult::fail("E_IO", e.to_string());
            }
            names.insert(d.name().to_string(), Entry { hash, text: text_id });
        }
        let old: BTreeMap<String, String> = current.hashes.clone();
        let changes = diff(&old, &names);
        if changes.is_empty() && reqs.is_empty() {
            return TxResult { ok: true, root: head, changes, diags: next.check.diags, src: Some(next.src) };
        }
        let entries: Vec<(String, String)> = names.iter().map(|(n, e)| (n.clone(), e.hash.clone())).collect();
        let hash = root_hash(&entries);
        let root = Root { hash: hash.clone(), parent: head, at, agent, reason, names, changes: changes.clone() };
        if let Err(e) = self.write(&format!("roots/{hash}.json"), &serde_json::to_string_pretty(&root).unwrap()) {
            return TxResult::fail("E_IO", e.to_string());
        }
        if let Err(e) = self.write("HEAD", &hash) {
            return TxResult::fail("E_IO", e.to_string());
        }
        TxResult { ok: true, root: Some(hash), changes, diags: next.check.diags, src: Some(next.src) }
    }
}

fn diff(old: &BTreeMap<String, String>, new: &BTreeMap<String, Entry>) -> Vec<Change> {
    let mut out = Vec::new();
    let removed: Vec<(&String, &String)> = old.iter().filter(|(n, _)| !new.contains_key(*n)).collect();
    let mut used = HashSet::new();
    for (n, e) in new {
        match old.get(n) {
            Some(h) if *h == e.hash => {}
            Some(h) => out.push(Change { path: n.clone(), old: Some(h.clone()), new: Some(e.hash.clone()), renamed_from: None }),
            None => {
                let from = removed.iter().find(|(rn, rh)| **rh == e.hash && !used.contains(*rn)).map(|(rn, _)| (*rn).clone());
                if let Some(f) = &from {
                    used.insert(f.clone());
                }
                out.push(Change { path: n.clone(), old: from.as_ref().map(|_| e.hash.clone()), new: Some(e.hash.clone()), renamed_from: from });
            }
        }
    }
    for (n, h) in removed {
        if !used.contains(n) {
            out.push(Change { path: n.clone(), old: Some(h.clone()), new: None, renamed_from: None });
        }
    }
    out
}

type OpErr = (String, String, Option<String>);

fn op_err<T>(code: &str, msg: impl Into<String>) -> Result<T, OpErr> {
    Err((code.into(), msg.into(), None))
}

fn str_field<'a>(op: &'a Json, key: &str) -> Result<&'a str, OpErr> {
    op.get(key).and_then(Json::as_str).map_or_else(|| op_err("E_OP_SHAPE", format!("missing string field '{key}'")), Ok)
}

fn parse_one(src: &str, path: &str) -> Result<Def, OpErr> {
    let m = parse(src).map_err(|e| (e.code.to_string(), format!("in src for '{path}': {}", e.msg), None))?;
    if m.defs.len() != 1 {
        return op_err("E_OP_SRC", format!("src for '{path}' must contain exactly one definition, found {}", m.defs.len()));
    }
    let d = m.defs.into_iter().next().unwrap();
    if d.name() != path {
        return Err(("E_OP_NAME".into(), format!("src defines '{}' but path is '{path}'", d.name()), Some("the definition name must equal the path".into())));
    }
    Ok(d)
}

fn find_def<'a>(defs: &'a mut [Def], name: &str) -> Result<&'a mut Def, OpErr> {
    let names: Vec<String> = defs.iter().map(|d| d.name().to_string()).collect();
    match defs.iter_mut().find(|d| d.name() == name) {
        Some(d) => Ok(d),
        None => {
            let close = names.iter().find(|n| n.contains(name) || name.contains(n.as_str())).cloned();
            Err(("E_OP_MISSING".into(), format!("no definition '{name}'"), close.map(|c| format!("did you mean '{c}'?"))))
        }
    }
}

fn parse_effect(src: &str) -> Result<Effect, OpErr> {
    let m = parse(&format!("fn eff_probe() ! {src}\n= ()")).map_err(|e| (e.code.to_string(), format!("bad effect '{src}': {}", e.msg), None))?;
    let Def::Fn(f) = &m.defs[0] else { unreachable!() };
    f.effects.first().cloned().map_or_else(|| op_err("E_OP_SHAPE", format!("bad effect '{src}'")), Ok)
}

fn apply_op(op: &Json, defs: &mut Vec<Def>, reqs: &mut BTreeMap<String, Vec<String>>) -> Result<(), OpErr> {
    let kind = str_field(op, "op")?;
    match kind {
        "add" => {
            let path = str_field(op, "path")?;
            let d = parse_one(str_field(op, "src")?, path)?;
            if defs.iter().any(|x| x.name() == path) {
                return Err(("E_OP_EXISTS".into(), format!("'{path}' already exists"), Some("use replace".into())));
            }
            defs.push(d);
        }
        "replace" => {
            let path = str_field(op, "path")?;
            let d = parse_one(str_field(op, "src")?, path)?;
            *find_def(defs, path)? = d;
        }
        "remove" => {
            let path = str_field(op, "path")?;
            find_def(defs, path)?;
            defs.retain(|d| d.name() != path);
        }
        "rename" => {
            let (from, to) = (str_field(op, "from")?, str_field(op, "to")?);
            if defs.iter().any(|d| d.name() == to) {
                return op_err("E_OP_EXISTS", format!("'{to}' already exists"));
            }
            let src = render(defs);
            let mut m = parse(&src).map_err(|e| (e.code.to_string(), e.msg.clone(), None))?;
            let is_ctor = m.defs.iter().any(|d| matches!(d, Def::Type(TypeDef { body: TypeBody::Sum(vs), .. }) if vs.iter().any(|v| v.name == from)));
            if !is_ctor && !m.defs.iter().any(|d| d.name() == from) {
                return op_err("E_OP_MISSING", format!("no definition or constructor '{from}'"));
            }
            let out = check(&m);
            rename::rename_module(&mut m, from, to, &out.user_methods);
            *defs = m.defs;
        }
        "refine" => {
            let target = str_field(op, "target")?;
            let contract = op.get("contract").map_or_else(|| op_err("E_OP_SHAPE", "missing 'contract'"), Ok)?;
            let Def::Fn(f) = find_def(defs, target)? else { return op_err("E_OP_KIND", format!("'{target}' is not a function")) };
            for (key, list) in [("pre", &mut f.pres), ("post", &mut f.posts)] {
                if let Some(src) = contract.get(key).and_then(Json::as_str) {
                    list.push(parse_expr(src).map_err(|e| (e.code.to_string(), format!("bad {key}: {}", e.msg), None))?);
                }
            }
            if let Some(effs) = contract.get("effects").and_then(Json::as_array) {
                for e in effs {
                    let e = e.as_str().unwrap_or_default();
                    if let Some(add) = e.strip_prefix('+') {
                        let eff = parse_effect(add)?;
                        if !f.effects.iter().any(|x| printer::effect(x) == printer::effect(&eff)) {
                            f.effects.push(eff);
                        }
                    } else if let Some(rm) = e.strip_prefix('-') {
                        let eff = parse_effect(rm)?;
                        f.effects.retain(|x| printer::effect(x) != printer::effect(&eff));
                    } else {
                        return op_err("E_OP_SHAPE", format!("effect '{e}' must start with + or -"));
                    }
                }
            }
        }
        "fill" => {
            let hole = str_field(op, "hole")?;
            let want = hole.strip_prefix('?').unwrap_or(hole);
            let expr = parse_expr(str_field(op, "expr")?).map_err(|e| (e.code.to_string(), format!("bad expr: {}", e.msg), None))?;
            let target = op.get("target").and_then(Json::as_str);
            let mut filled = false;
            for d in defs.iter_mut().filter(|d| target.is_none_or(|t| d.name() == t)) {
                let mut exprs: Vec<&mut Expr> = match d {
                    Def::Fn(f) => vec![&mut f.body],
                    Def::Test(t) => vec![&mut t.body],
                    Def::Type(_) => vec![],
                };
                for e in exprs.iter_mut() {
                    visit::walk_expr_mut(e, &mut |x| {
                        if filled {
                            return;
                        }
                        let hit = match &x.kind {
                            ExprKind::Hole(Some(n)) => n == want,
                            ExprKind::Hole(None) => want.is_empty(),
                            _ => false,
                        };
                        if hit {
                            x.kind = expr.kind.clone();
                            filled = true;
                        }
                    });
                }
            }
            if !filled {
                return op_err("E_OP_MISSING", format!("no hole '{hole}' found"));
            }
        }
        "attach" => {
            let target = str_field(op, "target")?;
            find_def(defs, target)?;
            let value = str_field(op, "value")?;
            match str_field(op, "kind")? {
                "req" => reqs.entry(target.to_string()).or_default().push(value.to_string()),
                "test" => {
                    let body = parse_expr(value).map_err(|e| (e.code.to_string(), format!("bad test: {}", e.msg), None))?;
                    let n = (1..).find(|i| !defs.iter().any(|d| d.name() == format!("{target}_t{i}"))).unwrap();
                    defs.push(Def::Test(TestDef { name: format!("{target}_t{n}"), body, span: Span::default() }));
                }
                k => return op_err("E_UNSUPPORTED", format!("attach kind '{k}' is not supported yet")),
            }
        }
        "patch" | "resolve" => return op_err("E_UNSUPPORTED", format!("op '{kind}' is not supported yet; use replace")),
        k => return op_err("E_OP_UNKNOWN", format!("unknown op '{k}'")),
    }
    Ok(())
}

pub fn result_json(r: &TxResult) -> Json {
    json!(r)
}
