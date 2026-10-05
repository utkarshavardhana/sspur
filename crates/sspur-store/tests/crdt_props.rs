use sspur_store::crdt::{view, Body, Commit, Index, Write};
use std::collections::{BTreeSet, HashSet};

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n.max(1)
    }
}

fn body(text: &str) -> Body {
    Body { hash: text.into(), text: text.into(), refs: Default::default(), um: vec![], test: false }
}

fn state(ix: &Index) -> String {
    format!("{}|{:?}", serde_json::to_string(&ix.defs).unwrap(), ix.dead)
}

fn run(seed: u64, replicas: usize, steps: usize) {
    let mut rng = Rng(seed | 1);
    let defs = ["d0", "d1", "d2"];
    let mut ixs: Vec<Index> = (0..replicas).map(|_| Index::default()).collect();
    let mut known: Vec<HashSet<usize>> = vec![HashSet::new(); replicas];
    let mut commits: Vec<Commit> = vec![];
    for step in 0..steps {
        let r = rng.below(replicas as u64) as usize;
        if commits.is_empty() || rng.below(100) < 55 {
            let mut writes = vec![];
            for _ in 0..1 + rng.below(2) {
                let def = defs[rng.below(defs.len() as u64) as usize];
                let regs = ixs[r].defs.get(def).cloned().unwrap_or_default();
                let (wn, wb) = match rng.below(3) {
                    0 => (true, false),
                    1 => (false, true),
                    _ => (true, true),
                };
                let mut sup = vec![];
                if wn {
                    sup.extend(regs.name.iter().map(|(d, _)| d.clone()));
                }
                if wb {
                    sup.extend(regs.body.iter().map(|(d, _)| d.clone()));
                }
                sup.sort();
                sup.dedup();
                let text = format!("fn {def}() -> Int\n= {step}");
                writes.push(Write {
                    def: def.into(),
                    name: wn.then(|| format!("n{}", rng.below(3))),
                    body: wb.then(|| if rng.below(10) == 0 { None } else { Some(body(&text)) }),
                    sup,
                });
            }
            let c = Commit { id: String::new(), parents: vec![], agent: format!("r{r}"), reason: String::new(), at: step.to_string(), writes }.seal();
            ixs[r].apply(&c);
            known[r].insert(commits.len());
            commits.push(c);
        } else {
            let k = rng.below(commits.len() as u64) as usize;
            ixs[r].apply(&commits[k]);
            known[r].insert(k);
        }
    }
    for r in 0..replicas {
        let mut order: Vec<usize> = (0..commits.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, rng.below(i as u64 + 1) as usize);
        }
        for k in order {
            ixs[r].apply(&commits[k]);
            if rng.below(4) == 0 {
                ixs[r].apply(&commits[k]);
            }
        }
    }
    let first = state(&ixs[0]);
    for (r, ix) in ixs.iter().enumerate() {
        assert_eq!(state(ix), first, "seed {seed}: replica {r} diverged");
        let (a, b) = (view(ix), view(&ixs[0]));
        assert_eq!(format!("{:?}", a.0.iter().map(|d| (&d.id, &d.name, &d.body.text)).collect::<Vec<_>>()), format!("{:?}", b.0.iter().map(|d| (&d.id, &d.name, &d.body.text)).collect::<Vec<_>>()));
    }
    let mut killed: BTreeSet<String> = BTreeSet::new();
    for c in &commits {
        for w in &c.writes {
            for (reg, on) in [("n", w.name.is_some()), ("b", w.body.is_some())] {
                if on {
                    killed.extend(w.sup.iter().map(|d| format!("{d}/{reg}")));
                }
            }
        }
    }
    let ix = &ixs[0];
    for c in &commits {
        for (i, w) in c.writes.iter().enumerate() {
            let dot = c.dot(i);
            let regs = &ix.defs[&w.def];
            if let Some(n) = &w.name {
                let live = regs.name.iter().any(|(d, v)| d == &dot && v == n);
                assert_eq!(live, !killed.contains(&format!("{dot}/n")), "seed {seed}: name write {dot} lost or resurrected");
            }
            if let Some(b) = &w.body {
                let live = regs.body.iter().any(|(d, v)| d == &dot && v == b);
                assert_eq!(live, !killed.contains(&format!("{dot}/b")), "seed {seed}: body write {dot} lost or resurrected");
            }
        }
    }
}

#[test]
fn random_concurrent_writes_converge_and_keep_every_unsuperseded_edit() {
    for seed in 1..=300u64 {
        run(seed * 7919, 2 + (seed % 4) as usize, 10 + (seed % 40) as usize);
    }
}

mod store_level {
    use serde_json::{json, Value as Json};
    use sspur_store::sync::{self, Remote};
    use sspur_store::{Store, Tx};

    fn fresh(name: &str) -> Store {
        let dir = std::env::temp_dir().join(format!("sspur-props-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Store::init(&dir).unwrap()
    }

    fn send(s: &Store, ops: Json) -> bool {
        s.apply(Tx::new("t", serde_json::from_value(ops).unwrap())).ok
    }

    fn head_checks(s: &Store) -> bool {
        match s.load_head() {
            Ok(l) => !l.check.has_errors(),
            Err(_) => s.head().is_none(),
        }
    }

    fn pair(name: &str) -> (Store, Store) {
        let a = fresh(&format!("{name}-a"));
        let b = fresh(&format!("{name}-b"));
        assert!(send(&a, json!([
            {"op": "add", "path": "helper", "src": "fn helper(x: Int) -> Int\n= x + 1"},
            {"op": "add", "path": "T", "src": "type T = X | Y"},
            {"op": "add", "path": "base", "src": "fn base() -> Int\n= helper(1)"}
        ])));
        assert!(sync::pull(&b, &Remote::Dir(a.clone()), "b").unwrap().result["ok"] == json!(true));
        assert_eq!(a.head(), b.head());
        (a, b)
    }

    fn settle(to: &Store, from: &Store, pick: &str) {
        let r = sync::pull(to, &Remote::Dir(from.clone()), "s").unwrap();
        if r.result["ok"] == json!(true) {
            return;
        }
        let paths: Vec<String> = r.result["conflicts"].as_array().map(|cs| cs.iter().filter_map(|c| c["path"].as_str().map(String::from)).collect()).unwrap_or_default();
        let ops: Vec<Json> = paths.iter().map(|p| json!({"op": "resolve", "path": p, "pick": pick})).collect();
        let _ = to.apply(Tx { merge: true, ..Tx::new("resolver", ops) });
    }

    #[test]
    fn random_adds_removes_and_edits_keep_every_head_checking_and_converge() {
        for seed in 1..=6u64 {
            let mut rng = super::Rng(seed * 0x9e37_79b9 + 7);
            let (a, b) = pair(&format!("rand{seed}"));
            let c = fresh(&format!("rand{seed}-c"));
            settle(&c, &a, "theirs");
            let rs = [a, b, c];
            let mut alive: Vec<Vec<String>> = vec![vec![]; 3];
            let mut k = 0;
            for _ in 0..40 {
                let i = rng.below(3) as usize;
                let s = &rs[i];
                match rng.below(8) {
                    0..=2 => {
                        let name = format!("u{i}_{k}");
                        k += 1;
                        let body = if rng.below(5) == 0 { "nope(1)".to_string() } else { format!("helper({k}) + base()") };
                        let ok = send(s, json!([{"op": "add", "path": name, "src": format!("fn {name}() -> Int\n= {body}")}]));
                        assert_eq!(ok, !body.starts_with("nope"), "seed {seed}: add {name}");
                        if ok {
                            alive[i].push(name);
                        }
                    }
                    3 if !alive[i].is_empty() => {
                        let at = rng.below(alive[i].len() as u64) as usize;
                        let name = alive[i].remove(at);
                        assert!(send(s, json!([{"op": "remove", "path": name}])), "seed {seed}: remove {name}");
                    }
                    4 => {
                        let _ = send(s, json!([{"op": "replace", "path": "helper", "src": format!("fn helper(x: Int) -> Int\n= x + {}", rng.below(9))}]));
                    }
                    _ => {
                        let j = (i + 1 + rng.below(2) as usize) % 3;
                        settle(s, &rs[j], if rng.below(2) == 0 { "ours" } else { "theirs" });
                    }
                }
                for (n, r) in rs.iter().enumerate() {
                    assert!(head_checks(r), "seed {seed}: replica {n} HEAD does not typecheck");
                }
            }
            for _ in 0..4 {
                for i in 0..3 {
                    for j in 0..3 {
                        if i != j {
                            settle(&rs[i], &rs[j], "theirs");
                        }
                    }
                }
            }
            let srcs: Vec<String> = rs.iter().map(|r| r.load_head().unwrap().src).collect();
            assert!(srcs.iter().all(|x| *x == srcs[0]), "seed {seed}: replicas diverged");
            for names in &alive {
                for n in names {
                    assert!(srcs[0].contains(&format!("fn {n}()")), "seed {seed}: acknowledged {n} lost");
                }
            }
        }
    }

    #[test]
    fn removals_sync_and_survive_an_index_rebuild() {
        let (a, b) = pair("remove");
        assert!(send(&a, json!([{"op": "add", "path": "spare", "src": "fn spare() -> Int\n= 7"}])));
        assert_eq!(sync::pull(&b, &Remote::Dir(a.clone()), "b").unwrap().result["ok"], json!(true));
        assert!(send(&a, json!([{"op": "remove", "path": "spare"}])));
        let r = sync::pull(&b, &Remote::Dir(a.clone()), "b").unwrap();
        assert_eq!(r.result["ok"], json!(true), "{}", r.result);
        assert_eq!(a.head(), b.head());
        assert!(!b.load_head().unwrap().src.contains("spare"));
        std::fs::remove_file(a.path().join("index.json")).unwrap();
        assert!(send(&a, json!([{"op": "add", "path": "later", "src": "fn later() -> Int\n= 8"}])));
        assert!(!a.load_head().unwrap().src.contains("spare"), "a rebuilt index resurrected a removed definition");
    }

    #[test]
    fn a_merge_that_does_not_typecheck_never_becomes_head() {
        let cases: [(Json, Json); 3] = [
            (json!([{"op": "remove", "path": "helper"}, {"op": "replace", "path": "base", "src": "fn base() -> Int\n= 2"}]), json!([{"op": "add", "path": "user", "src": "fn user() -> Int\n= helper(3)"}])),
            (json!([{"op": "replace", "path": "helper", "src": "fn helper(x: Str) -> Int\n= x.len"}, {"op": "replace", "path": "base", "src": "fn base() -> Int\n= helper(\"a\")"}]), json!([{"op": "add", "path": "user", "src": "fn user() -> Int\n= helper(3)"}])),
            (json!([{"op": "replace", "path": "T", "src": "type T = X | Y | Z"}]), json!([{"op": "add", "path": "f", "src": "fn f(t: T) -> Int\n= match t\n  | X => 1\n  | Y => 2"}])),
        ];
        for (k, (ours, theirs)) in cases.into_iter().enumerate() {
            let (a, b) = pair(&format!("sem{k}"));
            assert!(send(&a, ours), "case {k}: a's edit");
            assert!(send(&b, theirs), "case {k}: b's edit");
            let (ha, hb) = (a.head(), b.head());
            for (to, from, before) in [(&a, &b, &ha), (&b, &a, &hb)] {
                let r = sync::pull(to, &Remote::Dir(from.clone()), "x").unwrap();
                assert_eq!(r.result["ok"], json!(false), "case {k}: a union that does not typecheck merged: {}", r.result);
                assert_eq!(&to.head(), before, "case {k}: HEAD moved");
                assert!(head_checks(to), "case {k}: HEAD does not typecheck");
                assert!(!to.pending().is_empty(), "case {k}: remote heads not kept pending");
            }
            let p = sync::push(&a, &Remote::Dir(b.clone()), "a").unwrap();
            assert_eq!(p.result["ok"], json!(false), "case {k}: push accepted");
            assert!(head_checks(&a) && head_checks(&b));
        }
    }
}
