use sspur_syntax::{parse, parse_all, parse_noted, print_module, syntax_hint};

fn hint(src: &str) -> String {
    let e = parse(src).expect_err("should not parse");
    syntax_hint(src, &e).unwrap_or_default()
}

#[test]
fn foreign_syntax_gets_a_hint() {
    assert_eq!(hint("fn f(a: Bool, b: Bool) -> Bool\n= a & b"), "no '&&' operator: write 'and'");
    assert_eq!(hint("fn f() -> Unit ! log fail[E]\n= ()"), "separate effects with commas: '! log, fail[E]'");
    assert!(hint("type E = A{x: Int}\nfn f(e: E) -> Int\n= match e\n  | A{_, y} => 1").starts_with("a bare 'A' pattern ignores the fields"));
    assert_eq!(hint("fn f(xs: List[Int]) -> List[Int]\n= xs.map(x -> x + 1)"), "a lambda is 'x => e' or '(a, b) => e'");
    assert!(hint("fn f() -> Str\n= \"\\d+\"").starts_with("a regex escape doubles the backslash"));
}

#[test]
fn parse_all_reports_every_bad_definition() {
    let src = "fn a(x: Int) -> Bool\n= x > 0 ? 1 : 2\n\nfn b() -> Int\n= 1\n\nfn c(x: Bool) -> Bool\n= x -> 2\n";
    let errs = parse_all(src).expect_err("two bad definitions");
    assert_eq!(errs.len(), 2);
    assert!(errs[1].span.start as usize > src.find("fn c").unwrap());
}

#[test]
fn foreign_operators_parse_to_the_canonical_form() {
    for (src, canon, note) in [
        ("fn f(a: Bool, b: Bool) -> Bool\n= a && b || !a\n", "fn f(a: Bool, b: Bool) -> Bool\n= a and b or not a\n", "&& -> and"),
        ("fn f(n: Int) -> Int\n= do\n  var x = 0\n  x += n\n  x\n", "fn f(n: Int) -> Int\n= do\n  var x = 0\n  x := x + n\n  x\n", "x += e -> x := x + e"),
        ("type E = A{x: Int}\n\nfn f(e: E) -> Int\n= match e\n  | A{_} => 1\n", "type E = A{x: Int}\n\nfn f(e: E) -> Int\n= match e\n| A => 1\n", "Ctor{_} pattern -> Ctor"),
        ("fn f(n: Int) -> Int\n= if n > 0 then 1\n  elif n < 0 then 2\n  else 3\n", "fn f(n: Int) -> Int\n= if n > 0 then 1 else if n < 0 then 2 else 3\n", "elif -> else if"),
        ("fn f(pre: Int, post: Int, effect: Int) -> Int\n= pre + post + effect\n", "fn f(pre: Int, post: Int, effect: Int) -> Int\n= pre + post + effect\n", ""),
    ] {
        let (m, notes) = parse_noted(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
        let printed = print_module(&m);
        assert_eq!(printed, canon, "{src}");
        assert!(note.is_empty() && notes.is_empty() || notes.iter().any(|n| n == note), "{src}: {notes:?}");
        let (m2, again) = parse_noted(&printed).unwrap();
        assert!(again.is_empty() && print_module(&m2) == printed, "the canonical form is a fixed point: {printed}");
    }
}
