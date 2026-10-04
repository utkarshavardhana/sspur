use serde_json::{json, Value as Json};
use sspur_store::sync::{self, Remote};
use sspur_store::{load_src, Store, Tx};
use std::collections::HashMap;
use std::path::PathBuf;

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

fn fresh(name: &str) -> (Store, PathBuf) {
    let dir = std::env::temp_dir().join(format!("sspur-sync-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (Store::init(&dir).unwrap(), dir)
}

fn send(s: &Store, agent: &str, ops: Json) -> sspur_store::TxResult {
    s.apply(Tx::new(agent, serde_json::from_value(ops).unwrap()))
}

/// Pulls `from` into `to`; settles any conflict by picking one side.
fn pull(to: &Store, from: &Store, pick: &str) -> bool {
    let rep = sync::pull(to, &Remote::Dir(from.clone()), "sync").unwrap();
    if rep.result["ok"].as_bool() == Some(true) {
        return rep.received > 0;
    }
    let paths: Vec<String> = rep.result["conflicts"].as_array().unwrap().iter().map(|c| c["path"].as_str().unwrap().to_string()).collect();
    assert!(!paths.is_empty(), "a merge without conflicts failed: {}", rep.result);
    assert!(paths.iter().all(|p| p == "shared"), "only the shared definition is ever written by two replicas: {paths:?}");
    RESOLVED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ops: Vec<Json> = paths.iter().map(|p| json!({"op": "resolve", "path": p, "pick": pick})).collect();
    let r = to.apply(Tx { merge: true, ..Tx::new("resolver", ops) });
    assert!(r.ok, "{:?}", r.diags);
    true
}

static RESOLVED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn hub_name(s: &Store, hub_id: &str) -> String {
    s.head_root().unwrap().ids.iter().find(|(_, i)| i.id == hub_id).map(|(n, _)| n.clone()).unwrap()
}

fn seeded(name: &str, n: usize) -> (Vec<Store>, String) {
    let replicas: Vec<Store> = (0..n).map(|i| fresh(&format!("{name}{i}")).0).collect();
    let r = send(&replicas[0], "seed", json!([
        {"op": "add", "path": "hub", "src": "fn hub(x: Int) -> Int\n= 0"},
        {"op": "add", "path": "shared", "src": "fn shared(x: Int) -> Int\n= x + 0"},
        {"op": "attach", "target": "shared", "kind": "test", "value": "shared(1) - shared(0) == 1"}
    ]));
    assert!(r.ok);
    for r in &replicas[1..] {
        pull(r, &replicas[0], "ours");
    }
    let hub_id = replicas[0].head_root().unwrap().ids["hub"].id.clone();
    (replicas, hub_id)
}

fn random_run(seed: u64) -> usize {
    let n = 4;
    let (rs, hub_id) = seeded(&format!("rand{seed}-"), n);
    let mut rng = Rng(seed * 0x9e37_79b9 + 1);
    let mut owned: Vec<HashMap<String, (String, u64)>> = vec![HashMap::new(); n];
    let mut k = 0;
    let mut conflicts = 0;
    for _ in 0..120 {
        let i = rng.below(n as u64) as usize;
        let s = &rs[i];
        let hub = hub_name(s, &hub_id);
        match rng.below(10) {
            0..=3 => {
                let j = rng.below(n as u64) as usize;
                if j != i {
                    pull(s, &rs[j], if rng.below(2) == 0 { "ours" } else { "theirs" });
                }
            }
            4 if i == 0 => {
                assert!(send(s, "r0", json!([{"op": "rename", "from": hub, "to": format!("hub{}", rng.below(1000))}])).ok);
            }
            4 | 5 => {
                let c = rng.below(100);
                let name = format!("r{i}_f{k}");
                k += 1;
                let r = send(s, &format!("r{i}"), json!([{"op": "add", "path": name, "src": format!("fn {name}(x: Int) -> Int\n= x + {c} + {hub}(0)")}]));
                assert!(r.ok, "{:?}", r.diags);
                owned[i].insert(name.clone(), (name, c));
            }
            6 if !owned[i].is_empty() => {
                let key = owned[i].keys().nth(rng.below(owned[i].len() as u64) as usize).unwrap().clone();
                let (name, _) = owned[i][&key].clone();
                let c = rng.below(100);
                let r = send(s, &format!("r{i}"), json!([{"op": "replace", "path": name, "src": format!("fn {name}(x: Int) -> Int\n= x + {c} + {hub}(0)")}]));
                assert!(r.ok, "{:?}", r.diags);
                owned[i].get_mut(&key).unwrap().1 = c;
            }
            7 if !owned[i].is_empty() => {
                let key = owned[i].keys().nth(rng.below(owned[i].len() as u64) as usize).unwrap().clone();
                let (name, c) = owned[i][&key].clone();
                let to = format!("{key}_v{}", rng.below(1000));
                if to != name {
                    let r = send(s, &format!("r{i}"), json!([{"op": "rename", "from": name, "to": to}]));
                    assert!(r.ok, "{:?}", r.diags);
                    owned[i].insert(key, (to, c));
                }
            }
            8 if !owned[i].is_empty() => {
                let (name, _) = owned[i].values().nth(rng.below(owned[i].len() as u64) as usize).unwrap().clone();
                let r = send(s, &format!("r{i}"), json!([{"op": "attach", "target": name, "kind": "test", "value": format!("{name}(1) - {name}(0) == 1")}]));
                assert!(r.ok, "{:?}", r.diags);
            }
            _ => {
                let c = rng.below(100);
                let r = send(s, &format!("r{i}"), json!([{"op": "replace", "path": "shared", "src": format!("fn shared(x: Int) -> Int\n= x + {c}")}]));
                conflicts += usize::from(!r.ok);
            }
        }
    }
    for _round in 0..6 {
        let mut moved = false;
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    moved |= pull(&rs[i], &rs[j], "theirs");
                }
            }
        }
        if !moved {
            break;
        }
    }
    let roots: Vec<Option<String>> = rs.iter().map(Store::head).collect();
    assert!(roots.iter().all(|r| *r == roots[0]), "replicas diverged: {roots:?}");
    let fin = load_src(rs[0].load_head().unwrap().src).unwrap();
    assert!(!fin.check.has_errors(), "{:?}", fin.check.diags);
    let hub = hub_name(&rs[0], &hub_id);
    for o in &owned {
        for (name, c) in o.values() {
            let d = fin.module.defs.iter().find(|d| d.name() == name).unwrap_or_else(|| panic!("{name} lost"));
            assert!(sspur_syntax::printer::print_def(d).contains(&format!("x + {c} + {hub}(0)")), "{name}: {} (hub {hub})", sspur_syntax::printer::print_def(d));
        }
    }
    let mut it = sspur_eval::Interp::new(&fin.module, fin.check.record_types.clone(), fin.check.user_methods.clone(), fin.check.gen_loops.clone());
    it.set_check(&fin.check);
    assert!(it.run_tests().iter().all(|(_, r)| r.is_ok()));
    conflicts
}

#[test]
fn replicas_converge_under_random_op_and_sync_orders() {
    for seed in 1..=4 {
        random_run(seed);
    }
    eprintln!("sync: 4 runs x 4 replicas x 120 random steps converged; {} conflicting pulls resolved", RESOLVED.load(std::sync::atomic::Ordering::Relaxed));
}

#[test]
fn merge_result_is_independent_of_arrival_order() {
    let (rs, _) = seeded("order-", 3);
    for (i, s) in rs.iter().enumerate() {
        for k in 0..3 {
            let name = format!("o{i}_{k}");
            assert!(send(s, "w", json!([{"op": "add", "path": name, "src": format!("fn {name}() -> Int\n= {k}")}])).ok);
        }
        assert!(send(s, "w", json!([{"op": "attach", "target": "shared", "kind": "test", "value": format!("shared({i}) == {i}")}])).ok);
    }
    assert!(send(&rs[0], "w", json!([{"op": "rename", "from": "hub", "to": "center"}])).ok);
    assert!(send(&rs[1], "w", json!([{"op": "replace", "path": "hub", "src": "fn hub(x: Int) -> Int\n= x - x"}])).ok);
    let mut roots = Vec::new();
    for (n, order) in [[0, 1, 2], [2, 1, 0], [1, 0, 2], [2, 0, 1]].iter().enumerate() {
        let (t, _) = fresh(&format!("order-target{n}"));
        for &i in order {
            assert!(sync::pull(&t, &Remote::Dir(rs[i].clone()), "sync").unwrap().result["ok"].as_bool().unwrap());
        }
        roots.push(t.head().unwrap());
        let src = t.load_head().unwrap().src;
        assert!(src.contains("fn center(x: Int) -> Int\n= x - x"), "{src}");
    }
    assert!(roots.iter().all(|r| *r == roots[0]), "{roots:?}");
}

#[test]
fn tcp_push_and_pull() {
    let (a, _) = fresh("tcp-a");
    let (server, _) = fresh("tcp-srv");
    let (b, _) = fresh("tcp-b");
    assert!(send(&a, "a", json!([{"op": "add", "path": "f", "src": "fn f() -> Int\n= 1"}])).ok);
    let (tx, rx) = std::sync::mpsc::channel();
    let srv = server.clone();
    let h = std::thread::spawn(move || sync::serve(&srv, "127.0.0.1:0", Some(4), |addr| tx.send(addr.to_string()).unwrap()).unwrap());
    let remote = Remote::Tcp(rx.recv().unwrap());
    let p = sync::push(&a, &remote, "a").unwrap();
    assert_eq!(p.sent, 1);
    assert_eq!(p.result["ok"], json!(true), "{}", p.result);
    let q = sync::pull(&b, &remote, "b").unwrap();
    assert_eq!(q.received, 1);
    h.join().unwrap();
    assert_eq!(a.head(), b.head());
    assert_eq!(server.head(), b.head());
}

#[test]
fn conflicting_push_is_refused_until_pulled_and_resolved() {
    let (rs, _) = seeded("refuse-", 2);
    let (a, b) = (&rs[0], &rs[1]);
    assert!(send(a, "a", json!([{"op": "replace", "path": "shared", "src": "fn shared(x: Int) -> Int\n= x + 1"}])).ok);
    assert!(send(b, "b", json!([{"op": "replace", "path": "shared", "src": "fn shared(x: Int) -> Int\n= x + 2"}])).ok);
    let p = sync::push(b, &Remote::Dir(a.clone()), "b").unwrap();
    assert_eq!(p.result["ok"], json!(false));
    assert_eq!(p.result["conflicts"][0]["path"], json!("shared"));
    assert!(a.load_head().unwrap().src.contains("x + 1"), "a refused push changes nothing");
    let q = sync::pull(b, &Remote::Dir(a.clone()), "b").unwrap();
    assert_eq!(q.result["ok"], json!(false));
    assert_eq!(b.pending().len(), 1);
    let c = &q.result["conflicts"][0];
    assert!(c["ours"].as_str().unwrap().contains("x + 2") && c["theirs"].as_str().unwrap().contains("x + 1"), "{c}");
    let r = b.apply(Tx { merge: true, ..Tx::new("b", vec![json!({"op": "resolve", "path": "shared", "pick": "ours"})]) });
    assert!(r.ok, "{:?}", r.diags);
    assert!(b.pending().is_empty());
    let p = sync::push(b, &Remote::Dir(a.clone()), "b").unwrap();
    assert_eq!(p.result["ok"], json!(true), "{}", p.result);
    assert_eq!(a.head(), b.head());
    assert!(a.load_head().unwrap().src.contains("x + 2"));
}
