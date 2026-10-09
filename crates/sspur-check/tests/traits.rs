use sspur_check::{check, elab, Diag};
use sspur_syntax::parse;

fn errors(src: &str) -> Vec<Diag> {
    let m = parse(src).unwrap_or_else(|e| panic!("{e:?}"));
    check(&m).diags.into_iter().filter(|d| d.is_error()).collect()
}

/// The program has exactly one error, with this code and a hint that contains `hint`.
fn one(src: &str, code: &str, hint: &str) {
    let es = errors(src);
    assert_eq!(es.len(), 1, "{src}\n{es:?}");
    assert_eq!(es[0].code, code, "{src}\n{es:?}");
    let h = es[0].hint.clone().unwrap_or_default();
    assert!(h.contains(hint), "{code}: hint {h:?} lacks {hint:?}");
}

const P: &str = "type P = {x: Int}\n";

#[test]
fn clean_trait_program_has_no_diags() {
    let src = "trait Shape\n  fn area(s: Self) -> Int\n  fn label(s: Self) -> Str = \"a{s.area}\"\ntype Sq = {n: Int} derive Eq, Ord, Show, Hash, Json\nimpl Shape for Sq\n  fn area(s: Sq) -> Int = s.n * s.n\nfn total[T: Shape + Show](xs: List[T]) -> Int\n= xs.map(_.area).sum\ntest t = total([Sq{n: 2}]) == 4 and Sq{n: 1} < Sq{n: 2} and Sq{n: 3}.label == \"a9\"";
    let m = parse(src).unwrap();
    let out = check(&m);
    assert!(out.diags.is_empty(), "{:?}", out.diags);
    let (m2, out2) = elab::lower(&m, &out).unwrap().unwrap();
    assert!(!out2.has_errors());
    assert!(m2.defs.iter().all(|d| !matches!(d, sspur_syntax::Def::Trait(_) | sspur_syntax::Def::Impl(_))));
    assert!(m2.defs.iter().any(|d| d.name() == "total__Sq"));
}

#[test]
fn programs_without_traits_are_not_elaborated() {
    let m = parse("fn f(x: Int) -> Int = x + 1\ntest t = f(1) == 2").unwrap();
    let out = check(&m);
    assert!(elab::lower(&m, &out).unwrap().is_none());
}

#[test]
fn bounds_are_checked_at_the_call_site() {
    one(&format!("{P}fn f[T: Ord](xs: List[T]) -> Int = xs.len\ntest t = f([P{{x: 1}}]) == 1"), "E_TRAIT_MISSING", "derive Ord");
    one("fn f[T](a: T, b: T) -> Bool = a < b", "E_OPERATOR", "[T: Ord]");
    one("fn f[T](a: T) -> Str = a.show", "E_TRAIT_MISSING", "[T: Show]");
    one(&format!("{P}fn f(p: P) -> Str = p.show"), "E_TRAIT_MISSING", "impl Show for P");
    one("trait Tr\n  fn m(x: Self) -> Int\nfn f[T: Tr](xs: List[T]) -> Int = xs.len\nfn g() -> Int = f([])", "E_TRAIT_AMBIGUOUS", "known type");
}

#[test]
fn bounds_name_traits() {
    one("fn f[T: Ordd](x: T) -> T = x", "E_TRAIT_UNKNOWN", "Ord");
    one(&format!("{P}fn f[T: P](x: T) -> T = x"), "E_TRAIT_UNKNOWN", "is a type");
    one("fn f[T: Index](x: T) -> T = x", "E_TRAIT_ARGS", "Index[K, V]");
    one(&format!("{P}impl Showw for P\n  fn show(p: P) -> Str = \"\""), "E_TRAIT_UNKNOWN", "Show");
}

#[test]
fn trait_declarations_are_checked() {
    one("trait Tr\n  fn m(x: Int) -> Int", "E_TRAIT_SELF", "fn m(x: Self)");
    one("trait Tr[t]\n  fn m(x: Self) -> Int", "E_TRAIT_PARAMS", "trait Tr[T]");
    let es = errors("trait A\n  fn m(x: Self) -> Int\ntrait B\n  fn m(x: Self) -> Int");
    assert_eq!(es[0].code, "E_DUPLICATE");
    assert!(es[0].hint.as_deref().unwrap().contains("rename"));
    one("fn f(x: Self) -> Int = 1", "E_SELF", "name the type");
}

#[test]
fn impls_match_their_trait() {
    let tr = format!("{P}trait Tr\n  fn m(x: Self) -> Int\n");
    one(&format!("{tr}impl Tr for P\n  fn m(p: P) -> Str = \"a\""), "E_IMPL_SIG", "fn m(p: P) -> Int");
    one(&format!("{tr}impl Tr for P\n  fn m(p: P) -> Int ! log\n  = do\n    log(\"x\")\n    1"), "E_IMPL_EFFECT", "! log");
    one(&format!("{tr}impl Tr for P"), "E_IMPL_MISSING", "fn m(x: P) -> Int");
    one(&format!("{P}impl Show for P\n  fn show(p: P) -> Str = \"\"\n  fn extra(p: P) -> Int = 1"), "E_IMPL_EXTRA", "top-level fns");
    one("type B[T] = {v: T}\nimpl Show for B[Int]\n  fn show(b: B[Int]) -> Str = \"\"", "E_IMPL_TARGET", "impl[T] Show for B[T]");
    one("type G = {x: Int}\nimpl Index for G\n  fn index(g: G, i: Int) -> Int = 1", "E_TRAIT_ARGS", "impl Index[K, V] for G");
}

#[test]
fn one_impl_per_trait_and_type() {
    one(&format!("{P}impl Show for P\n  fn show(p: P) -> Str = \"a\"\nimpl Show for P\n  fn show(p: P) -> Str = \"b\""), "E_IMPL_DUP", "merge");
    one("type Q = {x: Int} derive Show\nimpl Show for Q\n  fn show(p: Q) -> Str = \"b\"", "E_IMPL_DUP", "derive list");
    one("impl Show for Int\n  fn show(p: Int) -> Str = \"b\"", "E_IMPL_ORPHAN", "new Int");
    one("trait Tr\n  fn m(x: Self) -> Int\nimpl Tr for Int\n  fn m(n: Int) -> Int = n\nimpl Tr for Int\n  fn m(n: Int) -> Int = n", "E_IMPL_DUP", "merge");
    one(&format!("{P}impl Copy for P"), "E_IMPL_BUILTIN", "Copy already");
}

#[test]
fn derive_is_checked() {
    one("type Q = {x: Int} derive Foo", "E_DERIVE_UNKNOWN", "Eq, Ord, Show, Hash, Json");
    one(&format!("{P}type Q = {{p: P}} derive Eq"), "E_DERIVE_FIELD", "derive Eq' to type P");
    one("type Q = {x: Int} derive Eq, Eq", "E_DUPLICATE", "once");
}

#[test]
fn operators_on_user_types_suggest_the_trait() {
    one(&format!("{P}fn f(a: P, b: P) -> P = a + b"), "E_OPERATOR", "impl Add for P");
    one(&format!("{P}fn f(a: P, b: P) -> Bool = a < b"), "E_OPERATOR", "derive Ord");
    one(&format!("{P}fn f(a: P) -> P = -a"), "E_OPERATOR", "impl Neg for P");
    one(&format!("{P}fn f(a: P) -> Int = a[0]"), "E_OPERATOR", "impl Index[Int, V] for P");
}

#[test]
fn trait_methods_are_not_values() {
    let es = errors(&format!("{P}impl Show for P\n  fn show(p: P) -> Str = \"\"\nfn f(xs: List[P]) -> List[Str] = xs.map(show)"));
    assert_eq!(es[0].code, "E_UNKNOWN_NAME");
    assert!(es[0].hint.as_deref().unwrap().contains("x => x.show"));
}

#[test]
fn polymorphic_recursion_through_a_bound_is_reported() {
    let m = parse("fn f[T: Show](x: T, n: Int) -> Str = if n == 0 then x.show else f([x], n - 1)\ntest t = f(1, 2) == \"[[1]]\"").unwrap();
    let out = check(&m);
    assert!(!out.has_errors(), "{:?}", out.diags);
    let e = elab::lower(&m, &out).err().unwrap();
    assert!(e.starts_with("E_TRAIT_RECURSION"), "{e}");
}

#[test]
fn impl_bodies_must_parse_as_methods() {
    let e = parse("type P = {x: Int}\nimpl Show for P\n  fn show(p: P) -> Str").unwrap_err();
    assert_eq!(e.code, "E_PARSE_IMPL");
    let e = parse("impl Show P").unwrap_err();
    assert_eq!(e.code, "E_PARSE_IMPL");
}
