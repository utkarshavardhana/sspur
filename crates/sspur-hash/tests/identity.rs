use sspur_hash::hash_module;
use sspur_syntax::parse;
use std::collections::HashMap;

fn hashes(src: &str) -> HashMap<String, String> {
    hash_module(&parse(src).unwrap()).into_iter().collect()
}

#[test]
fn renaming_a_definition_keeps_its_hash() {
    let a = hashes("fn double(x: Int) -> Int\n= x * 2");
    let b = hashes("fn twice(x: Int) -> Int\n= x * 2");
    assert_eq!(a["double"], b["twice"]);
}

#[test]
fn renaming_locals_and_params_keeps_hash() {
    let a = hashes("fn f(x: Int) -> Int\n= do\n  y = x + 1\n  y * y");
    let b = hashes("fn f(n: Int) -> Int\n= do\n  m = n + 1\n  m * m");
    assert_eq!(a["f"], b["f"]);
}

#[test]
fn formatting_does_not_matter() {
    let a = hashes("fn f() -> Int ! log\n= log(\"a\"); 1");
    let b = hashes("fn f() -> Int ! log\n= do\n  log(\"a\")\n  1");
    assert_eq!(a["f"], b["f"]);
}

#[test]
fn changing_meaning_changes_hash() {
    let a = hashes("fn f(x: Int) -> Int\n= x * 2");
    let b = hashes("fn f(x: Int) -> Int\n= x * 3");
    assert_ne!(a["f"], b["f"]);
    let c = hashes("fn f(x: Int) -> Int ! log\n= x * 2");
    assert_ne!(a["f"], c["f"]);
}

#[test]
fn dependents_change_when_dependencies_change() {
    let a = hashes("fn g(x: Int) -> Int\n= x + 1\nfn f(x: Int) -> Int\n= g(x) * 2");
    let b = hashes("fn g(x: Int) -> Int\n= x + 2\nfn f(x: Int) -> Int\n= g(x) * 2");
    assert_ne!(a["f"], b["f"]);
    let c = hashes("fn h(x: Int) -> Int\n= x + 1\nfn f(x: Int) -> Int\n= h(x) * 2");
    assert_eq!(a["f"], c["f"], "renaming the dependency must not change the dependent");
}

#[test]
fn mutual_recursion_is_stable_under_renaming() {
    let a = hashes("fn even(n: Int) -> Bool\n= if n == 0 then true else odd(n - 1)\nfn odd(n: Int) -> Bool\n= if n == 0 then false else even(n - 1)");
    let b = hashes("fn is_e(n: Int) -> Bool\n= if n == 0 then true else is_o(n - 1)\nfn is_o(n: Int) -> Bool\n= if n == 0 then false else is_e(n - 1)");
    assert_eq!(a["even"], b["is_e"]);
    assert_eq!(a["odd"], b["is_o"]);
    assert_ne!(a["even"], a["odd"]);
}

#[test]
fn constructor_renames_do_not_change_hashes() {
    let a = hashes("type S = A | B\nfn f(s: S) -> Int\n= match s\n  | A => 1\n  | B => 2");
    let b = hashes("type T = X | Y\nfn f(s: T) -> Int\n= match s\n  | X => 1\n  | Y => 2");
    assert_eq!(a["S"], b["T"]);
    assert_eq!(a["f"], b["f"]);
}

#[test]
fn hashes_are_52_char_base32() {
    let a = hashes("fn f() -> Int\n= 1");
    assert_eq!(a["f"].len(), 52);
    assert!(a["f"].chars().all(|c| c.is_ascii_lowercase() || ('2'..='7').contains(&c)));
}
