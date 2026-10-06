use sspur_syntax::{parse, parse_all, syntax_hint};

fn hint(src: &str) -> String {
    let e = parse(src).expect_err("should not parse");
    syntax_hint(src, &e).unwrap_or_default()
}

#[test]
fn foreign_syntax_gets_a_hint() {
    assert_eq!(hint("fn f(a: Bool, b: Bool) -> Bool\n= a && b"), "no '&&' operator: write 'and'");
    assert_eq!(hint("fn f(a: Bool, b: Bool) -> Bool\n= a || b"), "no '||' operator: write 'or'");
    assert_eq!(hint("fn f(a: Bool) -> Bool\n= !a"), "no '!' operator: write 'not x'");
    assert_eq!(hint("fn f() -> Unit ! log fail[E]\n= ()"), "separate effects with commas: '! log, fail[E]'");
    assert!(hint("type E = A{x: Int}\nfn f(e: E) -> Int\n= match e\n  | A{_} => 1").starts_with("a bare 'A' pattern ignores the fields"));
    assert_eq!(hint("fn f(n: Int) -> Int\n= do\n  var x = 0\n  x += n\n  x"), "no '+=': write 'x := x + e' (with 'var x = ..' first)");
    assert_eq!(hint("fn f(xs: List[Int]) -> List[Int]\n= xs.map(x -> x + 1)"), "a lambda is 'x => e' or '(a, b) => e'");
    assert!(hint("fn f() -> Str\n= \"\\d+\"").starts_with("a regex escape doubles the backslash"));
}

#[test]
fn parse_all_reports_every_bad_definition() {
    let src = "fn a(x: Int) -> Bool\n= x > 0 && x < 9\n\nfn b() -> Int\n= 1\n\nfn c(x: Bool) -> Bool\n= !x\n";
    let errs = parse_all(src).expect_err("two bad definitions");
    assert_eq!(errs.len(), 2);
    assert!(errs[1].span.start as usize > src.find("fn c").unwrap());
}
