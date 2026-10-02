use sspur_syntax::{parse, print_module, visit::strip_spans};
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs");
    let mut n = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "ssp") {
            roundtrip(&std::fs::read_to_string(&path).unwrap());
            n += 1;
        }
    }
    assert!(n > 0);
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
    let e = parse("store Orders = table").unwrap_err();
    assert_eq!(e.code, "E_UNSUPPORTED");
}
