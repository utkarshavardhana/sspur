use super::*;

fn an(src: &str) -> Analysis {
    let m = parse(src).unwrap();
    let c = sspur_check::check(&m);
    assert!(!c.has_errors(), "{:?}", c.diags);
    analyze(&m, &c)
}

fn clause<'a>(a: &'a Analysis, f: &str, text: &str) -> &'a Clause {
    a.clauses.iter().find(|c| c.func == f && c.text == text).unwrap_or_else(|| panic!("no clause {f}: {text}"))
}

fn verdicts(a: &Analysis, c: &Clause) -> Vec<Option<Answer>> {
    let mut s = Solver::new().uncached();
    c.obligations.iter().map(|o| o.query.as_ref().map(|q| s.check(&a.script(q), &a.model_vars(q)))).collect()
}

fn z3() -> bool {
    Solver::new().available()
}

#[test]
fn literals_and_ranges_encode_as_smt_lib() {
    assert_eq!(lit(-5), "(- 5)");
    assert_eq!(lit(7), "7");
    assert_eq!(in_range("x"), format!("(and (<= {MIN} x) (<= x {MAX}))"));
    assert_eq!(and(&[]), "true");
    assert_eq!(and(&["a".into(), "b".into()]), "(and a b)");
}

#[test]
fn post_query_asserts_negated_goal_after_entry_facts() {
    let a = an("fn inc(x: Int) -> Int\n  pre x < 100\n  post r > x\n= x + 1");
    let c = clause(&a, "inc", "r > x");
    let q = c.obligations[0].query.as_ref().unwrap();
    let s = a.script(q);
    assert!(s.contains("(declare-const"));
    assert!(s.contains("(assert (< k1 100))"), "{s}");
    assert!(s.trim_end().ends_with(&format!("(assert (not {}))", q.goal)));
    assert!(a.sites.keys().any(|k| k.2 == "arith"));
}

#[test]
fn truncated_division_matches_runtime_semantics() {
    if !z3() {
        return;
    }
    let a = an("fn d(a: Int, b: Int) -> Int\n  pre b != 0 and a > -100 and a < 100 and b > -100 and b < 100\n  post r * b + a % b == a\n= a / b\nfn m(a: Int) -> Int\n  post r <= 0\n= -7 % 2 + a * 0");
    assert_eq!(verdicts(&a, clause(&a, "d", "r * b + a % b == a")), vec![Some(Answer::Unsat)]);
    assert_eq!(verdicts(&a, clause(&a, "m", "r <= 0")), vec![Some(Answer::Unsat)]);
}

#[test]
fn relational_contracts_prove_and_refute() {
    if !z3() {
        return;
    }
    let src = "fn clamp(x: Int, lo: Int, hi: Int) -> Int\n  pre lo <= hi\n  post r >= lo and r <= hi\n= if x < lo then lo else if x > hi then hi else x\nfn bad(a: Int, b: Int) -> Int\n  pre a <= b\n  post r >= b\n= a\nfn use(x: Int) -> Int\n= clamp(x, 0, 10) + 1\nfn wrong(x: Int) -> Int\n= clamp(x, 5, x)";
    let a = an(src);
    assert_eq!(verdicts(&a, clause(&a, "clamp", "r >= lo and r <= hi")), vec![Some(Answer::Unsat)]);
    let v = verdicts(&a, clause(&a, "bad", "r >= b"));
    assert!(matches!(&v[0], Some(Answer::Sat(m)) if m.len() == 2), "{v:?}");
    let pre = verdicts(&a, clause(&a, "clamp", "lo <= hi"));
    assert_eq!(pre.len(), 2);
    assert_eq!(pre[0], Some(Answer::Unsat));
    assert!(matches!(pre[1], Some(Answer::Sat(_))));
    let o = Oracle::new(&parse(src).unwrap(), &sspur_check::check(&parse(src).unwrap()));
    assert!(o.post_proved("clamp", 0));
    assert!(!o.post_proved("bad", 0));
}

#[test]
fn overflow_sites_use_path_conditions_and_callee_posts() {
    if !z3() {
        return;
    }
    let a = an("fn small(x: Int) -> Int\n  post r >= 0 and r < 1000\n= if x < 0 then 0 else x % 1000\nfn f(a: Int, b: Int) -> Int\n= if a < b and a >= 0 then b - a else small(a) * 1000\nfn g(a: Int) -> Int\n= a + 1");
    let mut s = Solver::new().uncached();
    let mut by_fn: HashMap<String, Vec<bool>> = HashMap::new();
    for site in a.sites.iter().filter(|(k, _)| k.2 == "arith").map(|(_, v)| v) {
        let ok = site.queries.iter().all(|q| q.as_ref().is_some_and(|q| s.check(&a.script(q), &[]) == Answer::Unsat));
        by_fn.entry(site.func.clone()).or_default().push(ok);
    }
    assert!(by_fn["f"].iter().all(|b| *b), "{by_fn:?}");
    assert!(by_fn["g"].iter().all(|b| !*b));
}

#[test]
fn hidden_returns_and_catch_block_proofs() {
    let a = an("type E = Bad\nfn h(x: Int) -> Int\n  post r > 0\n= do\n  y = catch (if x < 0 then return 0 - 1 else if x == 0 then raise Bad else x)\n    | Bad => 0\n  5 + y * 0");
    assert!(clause(&a, "h", "r > 0").obligations.iter().all(|o| o.query.is_none()));
}

#[test]
fn record_field_invariants_feed_proofs() {
    if !z3() {
        return;
    }
    let a = an("type Acct = {bal: Int where _ >= 0}\nfn dep(a: Acct, n: Int where _ > 0) -> Acct\n  post r.bal == a.bal + n\n= {bal: a.bal + n}");
    assert_eq!(verdicts(&a, clause(&a, "dep", "r.bal == a.bal + n")), vec![Some(Answer::Unsat)]);
    let mut s = Solver::new().uncached();
    let site = a.sites.iter().find(|(k, _)| k.2 == "field:bal").unwrap().1;
    assert!(site.queries.iter().all(|q| s.check(&a.script(q.as_ref().unwrap()), &[]) == Answer::Unsat));
}

#[test]
fn models_parse_negative_values() {
    assert_eq!(parse_model("((k1 5) (k2 (- 3)) (k3 true))"), vec![("k1".into(), "5".into()), ("k2".into(), "-3".into()), ("k3".into(), "true".into())]);
}
