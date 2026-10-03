use sspur_check::check;
use sspur_eval::Interp;
use sspur_hash::{hash_module_with, Resolution};
use sspur_syntax::parse;
use std::path::Path;

fn big_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(1 << 29).spawn(f).unwrap().join().unwrap();
}

#[test]
fn every_suite_program_checks_and_passes_its_tests() {
    big_stack(run_suite);
}

fn run_suite() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs");
    let mut programs = 0;
    let mut tests = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "ssp") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let module = parse(&src).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
        let out = check(&module);
        let errors: Vec<_> = out.diags.iter().filter(|d| d.is_error()).collect();
        assert!(errors.is_empty(), "{}: {errors:?}", path.display());

        let res = Resolution { user_methods: Some(&out.user_methods), record_types: Some(&out.record_types) };
        assert_eq!(hash_module_with(&module, &res), hash_module_with(&module, &res));

        let mut interp = Interp::new(&module, out.record_types.clone(), out.user_methods.clone(), out.gen_loops.clone());
        interp.set_check(&out);
        let results = interp.run_tests();
        assert!(!results.is_empty(), "{} has no tests", path.display());
        for (name, r) in &results {
            assert!(r.is_ok(), "{}::{name}: {:?}", path.display(), r);
        }
        programs += 1;
        tests += results.len();
    }
    assert!(programs >= 10, "suite needs at least 10 programs, found {programs}");
    assert!(tests >= 40);
}

#[test]
fn contract_violations_trap_with_context() {
    let src = "type Pos = Int where _ > 0\nfn half(x: Pos) -> Int\n  post r * 2 == x\n= x / 2\ntest odd = half(3) == 1\ntest neg = half(-2) == -1";
    let m = parse(src).unwrap();
    let out = check(&m);
    let results = Interp::new(&m, out.record_types, out.user_methods, out.gen_loops).run_tests();
    assert!(results[0].1.as_ref().unwrap_err().contains("post r * 2 == x"));
    assert!(results[1].1.as_ref().unwrap_err().contains("type Pos"));
}

#[test]
fn integer_overflow_traps_instead_of_wrapping() {
    let src = "fn big() -> Int\n= 9223372036854775807 + 1\ntest t = big() == 0";
    let m = parse(src).unwrap();
    let out = check(&m);
    let results = Interp::new(&m, out.record_types, out.user_methods, out.gen_loops).run_tests();
    assert_eq!(results[0].1.as_ref().unwrap_err(), "integer overflow");
}

#[test]
fn native_release_matches_interpreter_on_every_suite_program() {
    if std::process::Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    big_stack(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "ssp") {
                continue;
            }
            let src = std::fs::read_to_string(&path).unwrap();
            let module = parse(&src).unwrap();
            let out = check(&module);
            let mut first = Interp::new(&module, out.record_types.clone(), out.user_methods.clone(), out.gen_loops.clone());
            first.set_check(&out);
            let interp = first.run_tests();
            let mut native = Interp::new(&module, out.record_types.clone(), out.user_methods.clone(), out.gen_loops.clone());
            native.set_check(&out);
            let compiled = sspur_native::cgen::compile_release(&module, &out, "-O2").unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let effectful = module.defs.iter().any(|d| matches!(d, sspur_syntax::Def::Effect(_))) || src.contains("yield");
            assert!(compiled.skipped.is_empty() || effectful, "{}: not native: {:?}", path.display(), compiled.skipped);
            native.set_native(compiled);
            assert_eq!(interp, native.run_tests(), "{}", path.display());
        }
    });
}

#[test]
fn verify_reports_proofs_counterexamples_and_unknowns() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs/contracts.ssp");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_sspur")).arg("verify").arg(&path).arg("--json").output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let status = |f: &str, prefix: &str| v["clauses"].as_array().unwrap().iter().find(|c| c["fn"] == f && c["clause"].as_str().unwrap().starts_with(prefix)).map(|c| c["status"].as_str().unwrap().to_string()).unwrap();
    if v["z3"] == false {
        assert_eq!(status("clamp", "post"), "unknown");
        assert!(out.status.success());
        return;
    }
    assert_eq!((v["proved"].as_u64(), v["counterexample"].as_u64(), v["unknown"].as_u64()), (Some(19), Some(1), Some(1)), "{v}");
    assert_eq!(status("search", "pre"), "proved");
    assert_eq!(status("midpoint", "post"), "proved");
    assert_eq!(status("mean2", "post"), "counterexample");
    assert_eq!(status("total_len", "post"), "unknown");
    assert!(!out.status.success());
}

#[test]
fn parallel_pipelines_report_the_sequential_trap_at_any_thread_count() {
    if std::process::Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let base = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs/parallel.ssp")).unwrap();
    let extra = "
fn combo(i: Int, a: Int, b: Int) -> Int
= if i == a or i == a + 1 then 9000000000000000000 else if i == b then 1 / (i - i) else 1

fn combo_sum(n: Int, a: Int, b: Int) -> Int
= (0..n).map(i => combo(i, a, b)).sum

fn refine_at(n: Int, at: Int) -> Int ! div
= (0..n).map(i => collatz_len(at - i)).sum

test excursion = spiky_sum(2000000, 10, 1500000) == 0
test div_first = first_fault(2000000, 1200000, 1800000) == 0
test ovf_first = first_fault(2000000, 1800000, 1200000) == 0
test deep = deep_at(200000, 150000) == 0
test refine = refine_at(30000, 20000) == 0
test deferred = combo_sum(2000000, 1500000, 1800000) == 0
test deferred_ovf = combo_sum(2000000, 1500000, -1) == 0
test big_ok = prime_count(2000000) == 148933 and first_fault(2000000, -1, -1) == 1999999000000
";
    let path = std::env::temp_dir().join(format!("sspur_parallel_traps_{}.ssp", std::process::id()));
    std::fs::write(&path, format!("{base}\n{extra}")).unwrap();
    let expected = [
        "FAIL  excursion: integer overflow",
        "FAIL  div_first: division by zero",
        "FAIL  ovf_first: integer overflow",
        "FAIL  deep: stack overflow in down",
        "FAIL  refine: contract violated: parameter 'n' of collatz_len where _ > 0 (value = 0)",
        "FAIL  deferred: division by zero",
        "FAIL  deferred_ovf: integer overflow",
    ];
    let mut outputs = Vec::new();
    for threads in [None, Some("1"), Some("3")] {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_sspur"));
        cmd.arg("test").arg(&path);
        if let Some(t) = threads {
            cmd.env("SSPUR_THREADS", t);
        }
        let out = String::from_utf8(cmd.output().unwrap().stdout).unwrap();
        for line in expected {
            assert!(out.lines().any(|l| l == line), "threads {threads:?}: missing {line:?} in\n{out}");
        }
        assert!(!out.contains("big_ok"), "threads {threads:?}: big_ok failed in\n{out}");
        outputs.push(out);
    }
    std::fs::remove_file(&path).ok();
    assert!(outputs.iter().all(|o| *o == outputs[0]));
}

#[test]
fn concurrency_failures_are_identical_in_both_tiers() {
    if std::process::Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let src = "type E = Boom{n: Int}
fn waiter(c: Chan[Int]) -> Int ! conc
= c.recv.or(-1)
fn dead() -> Int ! conc
= do
  c = chan()
  (a, b) = par(waiter(c), waiter(c))
  a + b
fn failing(n: Int) -> Int ! fail[E]
= if n > 2 then raise Boom{n} else n
fn fails() -> Int ! fail[E]
= do
  (a, b, c) = par(failing(1), failing(5), failing(3))
  a + b + c
fn trapper(c: Chan[Int]) -> Int ! conc
= do
  c.send(1)
  1 / 0
fn mixed() -> Int ! conc
= do
  c = chan()
  (a, b) = par(waiter(c) + waiter(c), trapper(c))
  a + b
fn closed() -> Unit ! conc
= do
  c = chan()
  c.close
  c.send(1)
test t_dead = dead() == 0
test t_fail = catch fails() == 0
  | Boom{n} => n == 5
test t_mixed = mixed() == 0
test t_closed = closed() == ()
";
    let path = std::env::temp_dir().join(format!("sspur_conc_fail_{}.ssp", std::process::id()));
    std::fs::write(&path, src).unwrap();
    let expected = ["FAIL  t_dead: deadlock: every task is blocked on recv", "FAIL  t_mixed: division by zero", "FAIL  t_closed: send on a closed channel", "1 passed, 3 failed"];
    let mut outputs = Vec::new();
    for mode in ["--interp", "--release"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_sspur")).arg("test").arg(mode).arg(&path).output().unwrap();
        let out = String::from_utf8(out.stdout).unwrap();
        for line in expected {
            assert!(out.lines().any(|l| l == line), "{mode}: missing {line:?} in\n{out}");
        }
        outputs.push(out);
    }
    std::fs::remove_file(&path).ok();
    assert_eq!(outputs[0], outputs[1]);
}

#[test]
fn vectorized_pipelines_trap_exactly_like_the_interpreter() {
    if std::process::Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let base = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs/simd.ssp")).unwrap();
    let extra = "
fn mixed_fault(xs: List[Int], n: Int, a: Int) -> Int
= (0..n).map(i => xs[i] * 0 + (if i == a then 9223372036854775807 else 1)).sum

test add_63 = bump(200, 63, 9223372036854775807) == 0
test add_64 = bump(200, 64, 9223372036854775807) == 0
test sum_127 = ramp_sum(200, 126, 4611686018427387904) == 0
test sum_128 = spike_sum(200, 128, 9223372036854775807) == 0
test neg_64 = negs(200, 64) == 0
test mul_65 = tripled(200, 65, 3074457345618258603) == 0
test mul_neg = tripled(200, 65, -3074457345618258603) == 0
test sq_big = sq_sum([5, 3037000500]) == 0
test idx_far = idx_sum(ints(100), 130) == 0
test ovf_before_idx = mixed_fault(ints(100), 200, 70) == 0
test idx_before_ovf = mixed_fault(ints(100), 200, 150) == 0
test big_add = bump(2000000, 1500001, 9223372036854775807) == 0
test big_sum = spike_sum(2000000, 1999999, 9223372036854775807) == 0
test big_masked = masked(2000000, 1234567) == 2000001000000 - 1234568
test big_affine = affine_pos(ints(2000000)) == ints(2000000).map(x => 3 * x + 1).filter(y => y > 0).sum
";
    let path = std::env::temp_dir().join(format!("sspur_simd_traps_{}.ssp", std::process::id()));
    std::fs::write(&path, format!("{base}\n{extra}")).unwrap();
    let run = |interp: bool, threads: Option<&str>| {
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_sspur"));
        cmd.arg("test").arg(&path);
        if interp {
            cmd.arg("--interp");
        }
        if let Some(t) = threads {
            cmd.env("SSPUR_THREADS", t);
        }
        String::from_utf8(cmd.output().unwrap().stdout).unwrap()
    };
    let expected = run(true, None);
    assert_eq!(expected.lines().filter(|l| l.starts_with("FAIL")).count(), 13, "{expected}");
    for threads in [None, Some("1"), Some("3")] {
        assert_eq!(run(false, threads), expected, "threads {threads:?}");
    }
    std::fs::remove_file(&path).ok();
}
