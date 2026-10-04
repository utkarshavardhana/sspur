use serde_json::json;
use sspur_store::{Store, Tx};
use std::path::PathBuf;

fn fresh(name: &str) -> (Store, PathBuf) {
    let dir = std::env::temp_dir().join(format!("sspur-conc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (Store::init(&dir).unwrap(), dir)
}

fn tx(base: &Option<String>, agent: &str, ops: serde_json::Value) -> Tx {
    let mut t: Tx = serde_json::from_value(json!({"agent": agent, "reason": "r", "ops": ops})).unwrap();
    t.base = base.clone();
    t
}

fn seed(s: &Store) -> Option<String> {
    let r = s.apply(tx(&None, "seed", json!([
        {"op": "add", "path": "double", "src": "fn double(x: Int) -> Int\n= x * 2"},
        {"op": "add", "path": "quad", "src": "fn quad(x: Int) -> Int\n= double(double(x))"},
        {"op": "add", "path": "inc", "src": "fn inc(x: Int) -> Int\n= x + 1"},
        {"op": "attach", "target": "quad", "kind": "test", "value": "quad(2) == 8"}
    ])));
    assert!(r.ok, "{:?}", r.diags);
    s.head()
}

fn src(s: &Store) -> String {
    let l = s.load_head().unwrap();
    assert!(!l.check.has_errors(), "{:?}", l.check.diags);
    l.src
}

#[test]
fn different_definitions_merge() {
    let (s, _d) = fresh("disjoint");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "replace", "path": "inc", "src": "fn inc(x: Int) -> Int\n= x + 10"}]))).ok);
    let r = s.apply(tx(&base, "b", json!([{"op": "add", "path": "tri", "src": "fn tri(x: Int) -> Int\n= x * 3"}])));
    assert!(r.ok, "{:?}", r.diags);
    let out = src(&s);
    assert!(out.contains("x + 10") && out.contains("fn tri"));
}

#[test]
fn same_body_edits_conflict_with_a_precise_report() {
    let (s, _d) = fresh("samebody");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "replace", "path": "inc", "src": "fn inc(x: Int) -> Int\n= x + 10"}]))).ok);
    let head = s.head();
    let r = s.apply(tx(&base, "b", json!([{"op": "replace", "path": "inc", "src": "fn inc(x: Int) -> Int\n= x + 20"}])));
    assert!(!r.ok);
    assert_eq!(s.head(), head);
    let c = &r.conflicts[0];
    assert_eq!((c.path.as_str(), c.kind.as_str(), c.agent.as_str()), ("inc", "body", "a"));
    assert!(c.theirs.as_deref().unwrap().contains("x + 10"));
    assert_eq!(r.diags[0].code, "E_CONFLICT");
    assert!(r.diags[0].hint.as_deref().unwrap().contains("rebase"));
    assert_eq!(r.rebase.as_ref().unwrap()["base"], json!(head));
    let fix = r.diags[0].fix.clone();
    assert!(s.apply(tx(&head, "b", json!(fix))).ok, "the fix forces our version");
    assert!(src(&s).contains("x + 20"));
}

#[test]
fn identical_concurrent_edits_are_not_conflicts() {
    let (s, _d) = fresh("idem");
    let base = seed(&s);
    let op = json!([{"op": "replace", "path": "inc", "src": "fn inc(x: Int) -> Int\n= x + 5"}]);
    assert!(s.apply(tx(&base, "a", op.clone())).ok);
    assert!(s.apply(tx(&base, "b", op)).ok);
}

#[test]
fn rename_and_body_edit_commute() {
    let (s, _d) = fresh("rename");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "rename", "from": "double", "to": "twice"}]))).ok);
    let r = s.apply(tx(&base, "b", json!([
        {"op": "replace", "path": "double", "src": "fn double(x: Int) -> Int\n= x + x"},
        {"op": "add", "path": "oct", "src": "fn oct(x: Int) -> Int\n= double(quad(x))"}
    ])));
    assert!(r.ok, "{:?}", r.diags);
    let out = src(&s);
    assert!(out.contains("fn twice(x: Int) -> Int\n= x + x"), "{out}");
    assert!(out.contains("twice(quad(x))"), "new callers follow the rename: {out}");
    assert!(!out.contains("double"));
}

#[test]
fn renames_to_the_same_name_merge_and_different_names_conflict() {
    let (s, _d) = fresh("rename2");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "rename", "from": "inc", "to": "succ"}]))).ok);
    assert!(s.apply(tx(&base, "b", json!([{"op": "rename", "from": "inc", "to": "succ"}]))).ok);
    let r = s.apply(tx(&base, "c", json!([{"op": "rename", "from": "inc", "to": "next"}])));
    assert_eq!(r.conflicts[0].kind, "rename");
    assert_eq!(r.conflicts[0].theirs_name.as_deref(), Some("succ"));
}

#[test]
fn refine_commutes_with_a_concurrent_replace() {
    let (s, _d) = fresh("refine");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "replace", "path": "inc", "src": "fn inc(x: Int) -> Int\n= x + 2"}]))).ok);
    let r = s.apply(tx(&base, "b", json!([{"op": "refine", "target": "inc", "contract": {"pre": "x >= 0"}}])));
    assert!(r.ok, "{:?}", r.diags);
    let out = src(&s);
    assert!(out.contains("x + 2") && out.contains("pre x >= 0"), "{out}");
}

#[test]
fn concurrent_tests_on_one_target_both_land() {
    let (s, _d) = fresh("tests");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "attach", "target": "inc", "kind": "test", "value": "inc(1) == 2"}]))).ok);
    let r = s.apply(tx(&base, "b", json!([{"op": "attach", "target": "inc", "kind": "test", "value": "inc(2) == 3"}])));
    assert!(r.ok, "{:?}", r.diags);
    let out = src(&s);
    assert!(out.contains("test inc_t1 = inc(1) == 2") && out.contains("test inc_t2 = inc(2) == 3"), "{out}");
}

#[test]
fn merges_that_do_not_typecheck_are_rejected() {
    let (s, _d) = fresh("semantic");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "remove", "path": "inc"}]))).ok);
    let head = s.head();
    let r = s.apply(tx(&base, "b", json!([{"op": "add", "path": "inc2", "src": "fn inc2(x: Int) -> Int\n= inc(inc(x))"}])));
    assert!(!r.ok);
    assert_eq!(s.head(), head);
    assert_eq!(r.diags[0].code, "E_MERGE");
    assert!(r.diags[0].msg.contains("inc"));
}

#[test]
fn edit_versus_remove_conflicts() {
    let (s, _d) = fresh("remove");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "remove", "path": "inc"}]))).ok);
    let r = s.apply(tx(&base, "b", json!([{"op": "replace", "path": "inc", "src": "fn inc(x: Int) -> Int\n= x + 3"}])));
    assert_eq!(r.conflicts[0].kind, "removed");
}

#[test]
fn concurrent_adds_of_one_name_conflict() {
    let (s, _d) = fresh("addadd");
    let base = seed(&s);
    assert!(s.apply(tx(&base, "a", json!([{"op": "add", "path": "z", "src": "fn z() -> Int\n= 1"}]))).ok);
    let r = s.apply(tx(&base, "b", json!([{"op": "add", "path": "z", "src": "fn z() -> Int\n= 2"}])));
    assert_eq!(r.conflicts[0].kind, "name");
    assert_eq!(r.conflicts[0].agent, "a");
}

#[test]
fn threads_on_one_store_lose_nothing() {
    let (s, dir) = fresh("threads");
    seed(&s);
    let hs: Vec<_> = (0..8)
        .map(|t| {
            let dir = dir.clone();
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || {
                    let s = Store::find(&dir).unwrap();
                    for k in 0..5 {
                        let name = format!("t{t}_{k}");
                        let r = s.apply(tx(&None, &format!("t{t}"), json!([{"op": "add", "path": name, "src": format!("fn {name}() -> Int\n= {k}")}])));
                        assert!(r.ok, "{:?}", r.diags);
                    }
                })
                .unwrap()
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    let out = src(&s);
    for t in 0..8 {
        for k in 0..5 {
            assert!(out.contains(&format!("fn t{t}_{k}()")));
        }
    }
}
