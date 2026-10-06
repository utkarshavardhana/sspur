use serde_json::json;
use sspur_store::{query::Ctx, Store, Tx};
use std::path::PathBuf;

fn fresh(name: &str) -> (Store, PathBuf) {
    let dir = std::env::temp_dir().join(format!("sspur-store-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    (Store::init(&dir).unwrap(), dir)
}

fn tx(ops: serde_json::Value) -> Tx {
    serde_json::from_value(json!({"agent": "t", "reason": "r", "ops": ops})).unwrap()
}

fn seed(s: &Store) {
    let r = s.apply(tx(json!([
        {"op": "add", "path": "double", "src": "fn double(x: Int) -> Int\n= x * 2"},
        {"op": "add", "path": "quad", "src": "fn quad(x: Int) -> Int\n= double(double(x))"},
        {"op": "attach", "target": "quad", "kind": "test", "value": "quad(2) == 8"}
    ])));
    assert!(r.ok, "{:?}", r.diags);
}

#[test]
fn transactions_are_atomic_and_rejected_when_ill_typed() {
    let (s, _d) = fresh("atomic");
    seed(&s);
    let head = s.head();
    let r = s.apply(tx(json!([
        {"op": "add", "path": "ok_fn", "src": "fn ok_fn() -> Int\n= 1"},
        {"op": "replace", "path": "double", "src": "fn double(x: Int) -> Int\n= log(\"x\")"}
    ])));
    assert!(!r.ok);
    assert_eq!(s.head(), head, "a rejected transaction must not move HEAD");
    assert!(!s.load_head().unwrap().module.defs.iter().any(|d| d.name() == "ok_fn"));
}

#[test]
fn rename_keeps_hashes_and_rewrites_callers() {
    let (s, _d) = fresh("rename");
    seed(&s);
    let before = s.load_head().unwrap().hashes;
    let r = s.apply(tx(json!([{"op": "rename", "from": "double", "to": "twice"}])));
    assert!(r.ok, "{:?}", r.diags);
    let after = s.load_head().unwrap();
    assert_eq!(before["double"], after.hashes["twice"]);
    assert_eq!(before["quad"], after.hashes["quad"]);
    assert!(after.src.contains("twice(twice(x))"));
    assert_eq!(r.changes[0].renamed_from.as_deref(), Some("double"));
}

#[test]
fn replace_propagates_to_dependents() {
    let (s, _d) = fresh("propagate");
    seed(&s);
    let before = s.load_head().unwrap().hashes;
    let r = s.apply(tx(json!([{"op": "replace", "path": "double", "src": "fn double(x: Int) -> Int\n= x + x"}])));
    assert!(r.ok);
    let after = s.load_head().unwrap().hashes;
    assert_ne!(before["double"], after["double"]);
    assert_ne!(before["quad"], after["quad"], "callers get new hashes");
    let paths: Vec<&str> = r.changes.iter().map(|c| c.path.as_str()).collect();
    assert!(paths.contains(&"quad") && paths.contains(&"quad_t1"));
}

#[test]
fn test_gate_blocks_regressions() {
    let (s, _d) = fresh("gate");
    seed(&s);
    let mut t = tx(json!([{"op": "replace", "path": "double", "src": "fn double(x: Int) -> Int\n= x * 3"}]));
    t.gate = Some("tests".into());
    let r = s.apply(t);
    assert!(!r.ok);
    assert_eq!(r.diags[0].code, "E_TEST_FAILED");
}

#[test]
fn holes_can_be_committed_and_filled() {
    let (s, _d) = fresh("holes");
    let r = s.apply(tx(json!([{"op": "add", "path": "f", "src": "fn f(a: Int, b: Str) -> Int\n= ?todo + 1"}])));
    assert!(r.ok, "{:?}", r.diags);
    let l = s.load_head().unwrap();
    let holes = Ctx::new(&l, Some(&s)).run("holes", None, 100);
    assert!(holes[0]["hint"].as_str().unwrap().contains('a'));
    let r = s.apply(tx(json!([{"op": "fill", "hole": "?todo", "expr": "a * 2"}])));
    assert!(r.ok);
    assert!(s.load_head().unwrap().src.contains("a * 2 + 1"));
}

#[test]
fn refine_applies_effect_fix_ops() {
    let (s, _d) = fresh("refine");
    let bad = s.apply(tx(json!([{"op": "add", "path": "g", "src": "fn g() -> Unit\n= log(\"hi\")"}])));
    assert!(!bad.ok);
    let mut fix = bad.diags[0].fix[0].clone();
    let r = s.apply(tx(json!([
        {"op": "add", "path": "g", "src": "fn g() -> Unit ! log\n= log(\"hi\")"},
        {"op": "refine", "target": "g", "contract": {"effects": ["-log"]}},
        {"op": "refine", "target": "g", "contract": fix["contract"].take()}
    ])));
    assert!(r.ok, "{:?}", r.diags);
}

#[test]
fn queries_answer_from_the_graph() {
    let (s, _d) = fresh("query");
    seed(&s);
    let l = s.load_head().unwrap();
    let c = Ctx::new(&l, Some(&s));
    assert_eq!(c.run("callers", Some("double"), 0), json!(["quad"]));
    assert_eq!(c.run("callees", Some("quad"), 0), json!(["double"]));
    assert_eq!(c.run("find", Some("Int -> Int"), 0).as_array().unwrap().len(), 2);
    let impact = c.run("impact", Some("double"), 0);
    assert_eq!(impact["tests"], json!(["quad_t1"]));
    let pack = c.run("pack", Some("quad"), 1000);
    assert!(pack["text"].as_str().unwrap().contains("fn double(x: Int) -> Int"));
    assert!(!pack["text"].as_str().unwrap().contains("x * 2"), "callee bodies are never packed");
    assert_eq!(c.run("why", Some("quad"), 0)["history"].as_array().unwrap().len(), 1);
    let found = c.run("find", Some("QUAD|dou*"), 0);
    let names: Vec<&str> = found["hits"].as_array().unwrap().iter().map(|h| h["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["quad", "double", "quad_t1"], "exact names first, tests last");
    assert_eq!(found["hits"][1]["sig"], "fn double(x: Int) -> Int");
    let anchored = c.run("find", Some("^quad$|^dou"), 0);
    assert_eq!(anchored["total"], 2, "{anchored}");
    let grep = c.run("grep", Some("double("), 0);
    assert_eq!(grep["total"], 2);
    assert_eq!(grep["hits"][1]["lines"], json!(["= double(double(x))"]));
    assert_eq!(c.run("list", Some("tests"), 0).as_array().unwrap().len(), 1);
}

#[test]
fn pack_caps_callers_in_large_codebases() {
    let (s, _d) = fresh("pack_cap");
    let mut ops = vec![json!({"op": "add", "path": "base", "src": "fn base(x: Int) -> Int\n= x + 1"})];
    for i in 0..20 {
        ops.push(json!({"op": "add", "path": format!("user{i:02}"), "src": format!("fn user{i:02}(x: Int) -> Int\n= base(x) * {i}")}));
    }
    assert!(s.apply(tx(json!(ops))).ok);
    let l = s.load_head().unwrap();
    let pack = Ctx::new(&l, Some(&s)).run("pack", Some("base"), 2000);
    let text = pack["text"].as_str().unwrap();
    assert!(text.contains("= base(x) * 2") && !text.contains("= base(x) * 3"), "{text}");
    assert!(text.contains("fn user07(x: Int) -> Int") && !text.contains("user08"), "{text}");
    assert!(text.ends_with("-- not shown: 12 callers (q callers base, q grep base)"), "{text}");
}

#[test]
fn stale_base_is_rejected() {
    let (s, _d) = fresh("stale");
    seed(&s);
    let mut t = tx(json!([{"op": "add", "path": "z", "src": "fn z() -> Int\n= 0"}]));
    t.base = Some("notthehead".into());
    assert_eq!(s.apply(t).diags[0].code, "E_STALE_BASE");
}
