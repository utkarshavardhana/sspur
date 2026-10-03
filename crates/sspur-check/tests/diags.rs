use sspur_check::check;
use sspur_syntax::parse;

fn codes(src: &str) -> Vec<String> {
    let m = parse(src).unwrap_or_else(|e| panic!("{e:?}"));
    check(&m).diags.into_iter().map(|d| d.code).collect()
}

fn diags(src: &str) -> Vec<sspur_check::Diag> {
    check(&parse(src).unwrap()).diags
}

#[test]
fn clean_program_has_no_diags() {
    let src = "type E = Bad\nfn f(x: Int) -> Int ! fail[E], log\n= do\n  log(\"x\")\n  if x < 0 then raise Bad\n  x * 2";
    assert!(codes(src).is_empty(), "{:?}", diags(src));
}

#[test]
fn missing_effect_has_fix_op() {
    let d = diags("fn f() -> Unit\n= log(\"hi\")");
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].code, "E_EFFECT_MISSING");
    assert_eq!(d[0].fix[0]["contract"]["effects"][0], "+log");
}

#[test]
fn unused_effect_warns() {
    let d = diags("fn f() -> Int ! log\n= 1");
    assert_eq!(d[0].code, "W_EFFECT_UNUSED");
    assert!(!d[0].is_error());
}

#[test]
fn effects_flow_through_lambdas() {
    let base = "fn noisy(x: Int) -> Int ! log\n= do\n  log(\"{x}\")\n  x\n";
    assert_eq!(codes(&format!("{base}fn g(xs: List[Int]) -> List[Int]\n= xs.map(x => noisy(x))")), vec!["E_EFFECT_MISSING"]);
    assert!(codes(&format!("{base}fn g(xs: List[Int]) -> List[Int] ! log\n= xs.map(x => noisy(x))")).is_empty());
    assert!(codes("fn g(xs: List[Int]) -> List[Int]\n= xs.map(_ + 1)").is_empty());
}

#[test]
fn user_higher_order_fn_is_effect_polymorphic() {
    let src = "fn twice[A, e](x: A, f: A -> A ! e) -> A ! e\n= f(f(x))\nfn noisy(x: Int) -> Int ! log\n= do\n  log(\"{x}\")\n  x\nfn pure_use() -> Int\n= twice(1, _ + 1)\nfn loud_use() -> Int ! log\n= twice(1, noisy)";
    assert!(codes(src).is_empty(), "{:?}", diags(src));
}

#[test]
fn raise_requires_fail_effect_and_catch_discharges_it() {
    assert_eq!(codes("type E = Bad\nfn f() -> Int\n= raise Bad"), vec!["E_EFFECT_MISSING"]);
    let src = "type E = Bad | Worse\nfn f(x: Int) -> Int ! fail[E]\n= if x > 0 then x else raise Bad\nfn g(x: Int) -> Int\n= catch f(x)\n  | Bad => 0\n  | Worse => 1";
    assert!(codes(src).is_empty(), "{:?}", diags(src));
    let partial = "type E = Bad | Worse\nfn f(x: Int) -> Int ! fail[E]\n= if x > 0 then x else raise Bad\nfn g(x: Int) -> Int\n= catch f(x)\n  | Bad => 0";
    assert_eq!(codes(partial), vec!["E_EFFECT_MISSING"]);
}

#[test]
fn type_mismatch() {
    assert_eq!(codes("fn f() -> Int\n= \"no\""), vec!["E_TYPE_MISMATCH"]);
    assert_eq!(codes("fn f() -> Int\n= 1 + 2.0"), vec!["E_TYPE_MISMATCH"]);
}

#[test]
fn nonexhaustive_match_lists_missing() {
    let d = diags("type S = A | B{x: Int} | C\nfn f(s: S) -> Int\n= match s\n  | A => 1");
    assert_eq!(d[0].code, "E_NONEXHAUSTIVE");
    assert!(d[0].msg.contains("B{..}") && d[0].msg.contains("C"), "{}", d[0].msg);
}

#[test]
fn holes_report_expected_type_and_candidates() {
    let d = diags("fn f(a: Int, b: Str) -> Int\n= ?");
    assert_eq!(d[0].code, "E_HOLE");
    assert!(d[0].msg.contains("Int"));
    assert_eq!(d[0].hint.as_deref(), Some("in scope with type Int: a"));
}

#[test]
fn unknown_name_suggests() {
    let d = diags("fn total(x: Int) -> Int\n= x\nfn g() -> Int\n= totl(1)");
    assert_eq!(d[0].code, "E_UNKNOWN_NAME");
    assert_eq!(d[0].hint.as_deref(), Some("did you mean 'total'?"));
}

#[test]
fn records_and_fields() {
    let ok = "type P = {x: Int, y: Int}\nfn f() -> Int\n= do\n  p = {x: 1, y: 2}\n  p.x + p.y";
    assert!(codes(ok).is_empty(), "{:?}", diags(ok));
    assert_eq!(codes("type P = {x: Int, y: Int}\nfn f() -> P\n= P{x: 1}"), vec!["E_FIELD_MISSING"]);
    assert_eq!(codes("type P = {x: Int}\nfn f(p: P) -> Int\n= p.z"), vec!["E_UNKNOWN_METHOD"]);
}

#[test]
fn contracts_must_be_pure_and_boolean() {
    assert_eq!(codes("fn f(x: Int) -> Int\n  pre x\n= x"), vec!["E_TYPE_MISMATCH"]);
    assert!(codes("fn f(x: Int where _ > 0) -> Int\n  post r > x\n= x + 1").is_empty());
}

#[test]
fn immutable_assignment() {
    assert_eq!(codes("fn f() -> Int\n= do\n  x = 1\n  x := 2\n  x"), vec!["E_ASSIGN_IMMUTABLE"]);
}

#[test]
fn newtypes_do_not_mix() {
    let base = "type UserId = new Str\ntype OrderId = new Str\nfn u(id: UserId) -> Str\n= id.raw\n";
    assert!(codes(&format!("{base}fn g() -> Str\n= u(UserId(\"a\"))")).is_empty());
    assert_eq!(codes(&format!("{base}fn g() -> Str\n= u(OrderId(\"a\"))")), vec!["E_TYPE_MISMATCH"]);
}

#[test]
fn secrets_cannot_leak() {
    let base = "type U = {pw: Secret[Str]}\n";
    assert_eq!(codes(&format!("{base}fn f(u: U) -> Str\n= \"pw={{u.pw}}\"")), vec!["E_SECRET_LEAK"]);
    assert_eq!(codes(&format!("{base}fn f(u: U) -> Str\n= \"{{u}}\"")), vec!["E_SECRET_LEAK"], "secrets nested in records are caught");
    assert_eq!(codes(&format!("{base}fn f(u: U) -> Str\n= u.pw.str")), vec!["E_SECRET_LEAK"]);
    assert_eq!(codes(&format!("{base}fn f(u: U) -> Bool\n= u.pw == secret(\"x\")")), vec!["E_SECRET_COMPARE"]);
    assert_eq!(codes(&format!("{base}fn f(u: U) -> Str\n= u.pw.expose(\"\")")), vec!["E_REASON_REQUIRED"]);
    assert_eq!(codes(&format!("{base}fn f(u: U) -> Str ! log\n= u.pw.map(p => do_log(p))\nfn do_log(p: Str) -> Str ! log\n= do\n  log(p)\n  p"))[0], "E_EFFECT_NOT_ALLOWED");
    let ok = diags(&format!("{base}fn f(u: U) -> Str\n= u.pw.expose(\"hashing for storage\")"));
    assert!(ok.iter().all(|d| !d.is_error()));
    assert_eq!(ok[0].code, "A_DECLASSIFY");
}

#[test]
fn untrusted_and_guess_must_be_handled() {
    assert_eq!(codes("fn q(name: Untrusted[Str]) -> Str\n= \"SELECT * WHERE n = '{name}'\""), vec!["E_UNTRUSTED_INTERP"]);
    assert_eq!(codes("fn q(name: Untrusted[Str]) -> Str\n= name + \"x\"")[0], "E_TYPE_MISMATCH");
    assert_eq!(codes("fn f(g: Guess[Int]) -> Int\n= g + 1")[0], "E_TYPE_MISMATCH");
    assert!(codes("fn f(g: Guess[Int]) -> Int\n= g.at_least(0.8).or(0)").is_empty());
}

const ASK: &str = "effect ask() -> Int\neffect state\n  get() -> Int\n  put(v: Int)\nfn twice() -> Int ! ask\n= ask() + ask()\n";

#[test]
fn performing_an_operation_needs_its_effect() {
    let d = diags(&format!("{ASK}fn f() -> Int\n= ask()"));
    assert_eq!(d[0].code, "E_EFFECT_MISSING");
    assert_eq!(d[0].fix[0]["contract"]["effects"][0], "+ask");
}

#[test]
fn handle_removes_the_effect_and_adds_arm_effects() {
    assert!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | ask() => resume(1)")).is_empty());
    assert_eq!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | ask() => do\n    log(\"asked\")\n    resume(1)")), vec!["E_EFFECT_MISSING"]);
    assert!(codes(&format!("{ASK}fn f() -> Int ! log\n= handle twice()\n  | ask() => do\n    log(\"asked\")\n    resume(1)")).is_empty());
    assert!(codes("fn f() -> Int\n= handle do\n    log(\"x\")\n    1\n  | log(m) => resume()").is_empty(), "log is an ordinary effect");
}

#[test]
fn handle_must_cover_every_operation() {
    let d = diags(&format!("{ASK}fn c() -> Int ! state\n= get()\nfn f() -> Int\n= handle c()\n  | get() => resume(1)"));
    assert_eq!(d[0].code, "E_HANDLE_PARTIAL");
    assert!(d[0].hint.as_ref().unwrap().contains("put"));
    assert_eq!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | akk() => resume(1)"))[0], "E_UNKNOWN_OP");
    assert_eq!(codes(&format!("{ASK}fn f() -> Int\n= handle 1\n  | ask() => resume(2)")), vec!["W_HANDLE_UNUSED"]);
}

#[test]
fn resume_is_typed_and_positioned() {
    assert_eq!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | ask() => resume(\"x\")")), vec!["E_TYPE_MISMATCH"]);
    assert_eq!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | ask() => resume(1) + 1")), vec!["E_RESUME_POSITION"]);
    assert_eq!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | ask() => [1].map(x => resume(x)).sum")), vec!["E_RESUME_POSITION"]);
    assert_eq!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | ask() => do\n    r = resume(1)\n    resume(r)")), vec!["E_RESUME_POSITION"]);
    assert!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | ask() => do\n    r = resume(1)\n    r * 2")).is_empty());
    assert!(codes(&format!("{ASK}fn f(b: Bool) -> Int\n= handle twice()\n  | ask() => if b then resume(1) else 0")).is_empty());
    assert_eq!(codes(&format!("{ASK}fn f() -> Int\n= handle twice()\n  | ask() => return 1")), vec!["E_RETURN_IN_HANDLER"]);
}

#[test]
fn generators_are_typed_through_for() {
    let base = "fn nums() -> Unit ! yield[Int]\n= do\n  yield(1)\n  yield(2)\n";
    assert!(codes(&format!("{base}fn f() -> Int\n= do\n  var s = 0\n  for x in nums()\n    s := s + x\n  s")).is_empty());
    assert_eq!(codes(&format!("{base}fn f() -> Int\n= do\n  var s = 0\n  for x in nums()\n    s := s + x.len\n  s")), vec!["E_UNKNOWN_METHOD"]);
    assert_eq!(codes("fn g() -> Unit ! yield[Int]\n= yield(\"a\")"), vec!["E_EFFECT_MISSING", "W_EFFECT_UNUSED"]);
    assert_eq!(codes("fn g() -> Unit ! yield\n= ()"), vec!["E_EFFECT_ARGS"]);
    let take = "fn take[T, e](n: Int, gen: () -> Unit ! yield[T], e) -> List[T] ! e\n= do\n  var out = []\n  handle gen()\n    | yield(x) => do\n      out := out.push(x)\n      if out.len >= n then () else resume()\n  out\n";
    assert!(codes(&format!("{base}{take}fn f() -> List[Int]\n= take(1, nums)")).is_empty());
    assert_eq!(codes(&format!("{base}{take}fn f() -> List[Str]\n= take(1, nums)")), vec!["E_TYPE_MISMATCH"]);
}
