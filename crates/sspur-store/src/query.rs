use crate::{Loaded, Store};
use serde_json::{json, Value as Json};
use sspur_hash::{dependencies, Resolution};
use sspur_syntax::*;
use std::collections::{BTreeMap, BTreeSet};

pub const QUERIES: &[&str] = &["list", "sig", "body", "callers", "callees", "effects", "find", "pack", "why", "impact", "holes", "diag", "log"];

pub struct Ctx<'a> {
    pub loaded: &'a Loaded,
    pub store: Option<&'a Store>,
    deps: BTreeMap<String, Vec<String>>,
}

pub fn tokens(s: &str) -> usize {
    s.len().div_ceil(4)
}

fn short(h: &str) -> String {
    h.chars().take(12).collect()
}

impl<'a> Ctx<'a> {
    pub fn new(loaded: &'a Loaded, store: Option<&'a Store>) -> Self {
        let res = Resolution { user_methods: Some(&loaded.check.user_methods), record_types: Some(&loaded.check.record_types) };
        let deps = dependencies(&loaded.module, &res);
        Ctx { loaded, store, deps }
    }

    fn def(&self, name: &str) -> Result<&Def, Json> {
        self.loaded.module.defs.iter().find(|d| d.name() == name).ok_or_else(|| {
            let close: Vec<&str> = self.loaded.module.defs.iter().map(Def::name).filter(|n| n.contains(name) || name.contains(*n)).collect();
            json!({"error": "E_NOT_FOUND", "msg": format!("no definition '{name}'"), "close": close})
        })
    }

    fn kind(d: &Def) -> &'static str {
        match d {
            Def::Type(_) => "type",
            Def::Fn(_) => "fn",
            Def::Test(_) => "test",
            Def::Effect(_) => "effect",
            Def::Store(_) => "store",
            Def::Svc(_) => "svc",
            Def::Static(_) => "static",
            Def::Use(_) => "use",
        }
    }

    fn sig_text(d: &Def) -> String {
        match d {
            Def::Fn(f) => {
                let mut s = printer::print_sig(f);
                for p in &f.pres {
                    s.push_str(&format!("\n  pre {}", printer::expr(p, 2)));
                }
                for p in &f.posts {
                    s.push_str(&format!("\n  post {}", printer::expr(p, 2)));
                }
                for p in &f.examples {
                    s.push_str(&format!("\n  ex {}", printer::expr(p, 2)));
                }
                s
            }
            other => printer::print_def(other),
        }
    }

    fn callers_of(&self, name: &str) -> Vec<String> {
        self.deps.iter().filter(|(_, ds)| ds.iter().any(|d| d == name)).map(|(n, _)| n.clone()).collect()
    }

    pub fn run(&self, q: &str, target: Option<&str>, budget: usize) -> Json {
        let need: Result<&str, Json> = target.ok_or_else(|| json!({"error": "E_QUERY_ARGS", "msg": format!("query '{q}' needs a target")}));
        let r: Result<Json, Json> = (|| {
            Ok(match q {
                "list" => {
                    let items: Vec<Json> = self
                        .loaded
                        .module
                        .defs
                        .iter()
                        .map(|d| json!({"name": d.name(), "kind": Self::kind(d), "hash": short(&self.loaded.hashes[d.name()]), "sig": match d { Def::Fn(f) => printer::print_sig(f), Def::Type(_) | Def::Effect(_) | Def::Store(_) | Def::Svc(_) | Def::Static(_) | Def::Use(_) => printer::print_def(d), Def::Test(_) => String::new() }}))
                        .collect();
                    json!(items)
                }
                "sig" => {
                    let d = self.def(need.clone()?)?;
                    json!({"name": d.name(), "kind": Self::kind(d), "hash": self.loaded.hashes[d.name()], "sig": Self::sig_text(d)})
                }
                "body" => {
                    let d = self.def(need.clone()?)?;
                    json!({"name": d.name(), "hash": self.loaded.hashes[d.name()], "src": printer::print_def(d)})
                }
                "callers" => json!(self.callers_of(self.def(need.clone()?)?.name())),
                "callees" => json!(self.deps.get(self.def(need.clone()?)?.name()).cloned().unwrap_or_default()),
                "effects" => match self.def(need.clone()?)? {
                    Def::Fn(f) => json!(f.effects.iter().map(printer::effect).collect::<Vec<_>>()),
                    _ => json!([]),
                },
                "find" => {
                    let want: String = need.clone()?.split_whitespace().collect();
                    let hits: Vec<Json> = self
                        .loaded
                        .module
                        .defs
                        .iter()
                        .filter_map(|d| match d {
                            Def::Fn(f) => {
                                let params: Vec<String> = f.params.iter().map(|p| printer::ty(&p.ty)).collect();
                                let ret = f.ret.as_ref().map_or("Unit".to_string(), printer::ty);
                                let shape = if params.len() == 1 { format!("{} -> {ret}", params[0]) } else { format!("({}) -> {ret}", params.join(", ")) };
                                let flat: String = shape.split_whitespace().collect();
                                flat.contains(&want).then(|| json!({"name": f.name, "type": shape}))
                            }
                            _ => None,
                        })
                        .collect();
                    json!(hits)
                }
                "pack" => self.pack(need.clone()?, budget)?,
                "why" => {
                    let name = self.def(need.clone()?)?.name().to_string();
                    let hash = &self.loaded.hashes[&name];
                    let prov = self.store.and_then(|s| s.prov(hash));
                    let history: Vec<Json> = self
                        .store
                        .map(|s| s.history(&name))
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(r, h)| json!({"root": short(&r.hash), "hash": short(&h), "agent": r.agent, "reason": r.reason, "at": r.at}))
                        .collect();
                    json!({"name": name, "hash": hash, "prov": prov, "history": history})
                }
                "impact" => {
                    let start = self.def(need.clone()?)?.name().to_string();
                    let mut seen = BTreeSet::new();
                    let mut todo = vec![start.clone()];
                    while let Some(n) = todo.pop() {
                        for c in self.callers_of(&n) {
                            if seen.insert(c.clone()) {
                                todo.push(c);
                            }
                        }
                    }
                    let (tests, defs): (Vec<String>, Vec<String>) = seen.into_iter().partition(|n| matches!(self.def(n), Ok(Def::Test(_))));
                    json!({"target": start, "dependents": defs, "tests": tests})
                }
                "holes" => json!(self.loaded.check.diags.iter().filter(|d| d.severity == "hole").collect::<Vec<_>>()),
                "diag" => json!(self.loaded.check.diags),
                "log" => {
                    let roots: Vec<Json> = self
                        .store
                        .map(Store::log)
                        .unwrap_or_default()
                        .into_iter()
                        .take(budget.min(50))
                        .map(|r| json!({"root": short(&r.hash), "at": r.at, "agent": r.agent, "reason": r.reason, "changes": r.changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>()}))
                        .collect();
                    json!(roots)
                }
                _ => return Err(json!({"error": "E_QUERY_UNKNOWN", "msg": format!("unknown query '{q}'"), "queries": QUERIES})),
            })
        })();
        r.unwrap_or_else(|e| e)
    }

    fn pack(&self, target: &str, budget: usize) -> Result<Json, Json> {
        let d = self.def(target)?;
        let name = d.name().to_string();
        let mut parts: Vec<(String, &'static str, String)> = vec![(name.clone(), "full", printer::print_def(d))];
        let callees = self.deps.get(&name).cloned().unwrap_or_default();
        for c in &callees {
            if let Ok(cd) = self.def(c) {
                match cd {
                    Def::Type(_) => parts.push((c.clone(), "type", printer::print_def(cd))),
                    Def::Effect(_) => parts.push((c.clone(), "effect", printer::print_def(cd))),
                    Def::Store(_) => parts.push((c.clone(), "store", printer::print_def(cd))),
                    Def::Static(_) => parts.push((c.clone(), "static", printer::print_def(cd))),
                    Def::Fn(_) => parts.push((c.clone(), "sig", Self::sig_text(cd))),
                    Def::Test(_) | Def::Svc(_) | Def::Use(_) => {}
                }
            }
        }
        let callers = self.callers_of(&name);
        for c in &callers {
            if let Ok(cd @ Def::Test(_)) = self.def(c) {
                parts.push((c.clone(), "test", printer::print_def(cd)));
            }
        }
        for c in &callers {
            if let Ok(cd @ Def::Fn(_)) = self.def(c) {
                parts.push((c.clone(), "caller", printer::print_def(cd)));
            }
        }
        let mut used = 0;
        let mut text = Vec::new();
        let mut included = Vec::new();
        let mut omitted = Vec::new();
        for (i, (n, part, src)) in parts.into_iter().enumerate() {
            let t = tokens(&src) + 1;
            if i == 0 || used + t <= budget {
                used += t;
                text.push(src);
                included.push(json!({"name": n, "part": part}));
            } else if part == "caller" {
                if let Ok(cd) = self.def(&n) {
                    let sig = Self::sig_text(cd);
                    let st = tokens(&sig) + 1;
                    if used + st <= budget {
                        used += st;
                        text.push(sig);
                        included.push(json!({"name": n, "part": "caller_sig"}));
                        continue;
                    }
                }
                omitted.push(json!({"name": n, "part": part}));
            } else {
                omitted.push(json!({"name": n, "part": part}));
            }
        }
        Ok(json!({"target": name, "hash": self.loaded.hashes[&name], "budget": budget, "est_tokens": used, "text": text.join("\n\n"), "included": included, "omitted": omitted}))
    }
}
