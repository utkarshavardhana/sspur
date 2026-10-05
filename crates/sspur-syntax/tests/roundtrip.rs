use sspur_syntax::{parse, print_module, visit::strip_spans, Def, Expr, ExprKind, Stmt};
use std::path::Path;

fn roundtrip(src: &str) {
    let mut a = parse(src).unwrap_or_else(|e| panic!("parse failed: {e:?}"));
    let printed = print_module(&a);
    let mut b = parse(&printed).unwrap_or_else(|e| panic!("reparse failed: {e:?}\n{printed}"));
    assert_eq!(printed, print_module(&b), "printer is not idempotent");
    strip_spans(&mut a);
    strip_spans(&mut b);
    assert_eq!(a, b, "AST changed through print:\n{printed}");
}

#[test]
fn roundtrip_programs() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests");
    let mut n = 0;
    for dir in ["programs", "ownership/accept", "ownership/reject"] {
        for entry in std::fs::read_dir(base.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "ssp") {
                roundtrip(&std::fs::read_to_string(&path).unwrap());
                n += 1;
            }
        }
    }
    assert!(n > 100);
}

#[test]
fn field_assignment_is_sugar_for_with() {
    let m = parse("fn f(p: P) -> P\n= do\n  var q = p\n  q.a.b := 1\n  q").unwrap();
    let printed = print_module(&m);
    assert!(printed.contains("  q.a.b := 1\n"), "{printed}");
    let Def::Fn(f) = &m.defs[0] else { panic!() };
    let ExprKind::Block(stmts) = &f.body.kind else { panic!() };
    assert!(matches!(&stmts[1], Stmt::Assign(n, Expr { kind: ExprKind::With(..), .. }, _) if n == "q"));
}

#[test]
fn placeholder_becomes_lambda() {
    let m = parse("fn f(xs: List[Int]) -> List[Int]\n= xs.map(_ * _ + 1)").unwrap();
    let printed = print_module(&m);
    assert!(printed.contains("xs.map(_ * _ + 1)"), "{printed}");
}

#[test]
fn semicolons_canonicalize_to_block() {
    let m = parse("fn f() -> Int ! log\n= log(\"a\"); 1").unwrap();
    assert_eq!(print_module(&m), "fn f() -> Int ! log\n= do\n  log(\"a\")\n  1\n");
}

#[test]
fn errors_have_codes() {
    let e = parse("fn f( -> Int\n= 1").unwrap_err();
    assert_eq!(e.code, "E_PARSE_EXPECTED");
    let e = parse("queue Ship = fifo[Order]").unwrap_err();
    assert_eq!(e.code, "E_UNSUPPORTED");
    let e = parse("store Orders = blob[Str, Str]").unwrap_err();
    assert_eq!(e.code, "E_UNSUPPORTED");
}

#[test]
fn store_and_svc_roundtrip() {
    let src = "store Orders = table[OrderId, Order]\n\nsvc shop\n  ep post \"/orders\" = place\n  ep get \"/orders/{id}\" = find\n";
    let m = parse(src).unwrap();
    assert_eq!(print_module(&m), src);
    let Def::Svc(s) = &m.defs[1] else { panic!() };
    assert_eq!(s.eps[1].path_params(), ["id"]);
}

#[test]
fn tolerant_input_canonicalizes() {
    let natural = "fn f(xs: List[Int], n: Int) -> Int\n= do\n  var t = 0\n  for x in xs do\n    if x < n then\n      t := t + x\n    else\n      t := t - 1\n  t\n";
    let canonical = "fn f(xs: List[Int], n: Int) -> Int\n= do\n  var t = 0\n  for x in xs\n    if x < n then t := t + x else t := t - 1\n  t\n";
    let mut a = parse(natural).unwrap();
    let mut b = parse(canonical).unwrap();
    strip_spans(&mut a);
    strip_spans(&mut b);
    assert_eq!(a, b);
    assert_eq!(print_module(&parse(natural).unwrap()), canonical);
}

#[test]
fn else_on_next_line_and_single_quotes() {
    let a = parse("fn g(n: Int) -> Str\n= if n < 0 then 'neg'\n  else if n == 0 then 'zero'\n  else 'pos'").unwrap();
    assert_eq!(print_module(&a), "fn g(n: Int) -> Str\n= if n < 0 then \"neg\" else if n == 0 then \"zero\" else \"pos\"\n");
}

#[test]
fn blocks_inside_parentheses_use_layout() {
    let src = "fn h(xs: List[Int]) -> Int\n= xs.fold(0, (acc, x) =>\n    do\n    y = x * 2\n    acc + y\n  )";
    let m = parse(src).unwrap_or_else(|e| panic!("{e:?}"));
    let printed = print_module(&m);
    let mut a = m;
    let mut b = parse(&printed).unwrap_or_else(|e| panic!("{e:?}\n{printed}"));
    strip_spans(&mut a);
    strip_spans(&mut b);
    assert_eq!(a, b);
}

#[test]
fn nested_strings_in_interpolation() {
    let m = parse("fn f(x: Str) -> Str\n= \"a {x.replace(\"b\", 'c')} d\"").unwrap();
    assert!(print_module(&m).contains("x.replace(\"b\", \"c\")"));
}

#[test]
fn index_after_method_with_constructor_expression() {
    let src = "type S = A{x: Int} | C\nfn g(m: Map[Int, Int]) -> Int = m.keys[A{x: 3}.x]\nfn h(j: Json) -> Int = j.as[Int]\n";
    let m = parse(src).unwrap_or_else(|e| panic!("{e:?}"));
    let Def::Fn(f) = &m.defs[1] else { panic!() };
    assert!(matches!(&f.body.kind, ExprKind::Index(..)), "{:?}", f.body.kind);
    let Def::Fn(h) = &m.defs[2] else { panic!() };
    assert!(matches!(&h.body.kind, ExprKind::Method { targs, .. } if targs.len() == 1));
    roundtrip(src);
}
