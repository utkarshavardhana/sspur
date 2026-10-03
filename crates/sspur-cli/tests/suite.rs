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

        let interp = Interp::new(&module, out.record_types.clone(), out.user_methods.clone());
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
    let results = Interp::new(&m, out.record_types, out.user_methods).run_tests();
    assert!(results[0].1.as_ref().unwrap_err().contains("post r * 2 == x"));
    assert!(results[1].1.as_ref().unwrap_err().contains("type Pos"));
}

#[test]
fn integer_overflow_traps_instead_of_wrapping() {
    let src = "fn big() -> Int\n= 9223372036854775807 + 1\ntest t = big() == 0";
    let m = parse(src).unwrap();
    let out = check(&m);
    let results = Interp::new(&m, out.record_types, out.user_methods).run_tests();
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
            let interp = Interp::new(&module, out.record_types.clone(), out.user_methods.clone()).run_tests();
            let mut native = Interp::new(&module, out.record_types.clone(), out.user_methods.clone());
            let compiled = sspur_native::cgen::compile_release(&module, &out, "-O2").unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!(compiled.skipped.is_empty(), "{}: not native: {:?}", path.display(), compiled.skipped);
            native.set_native(compiled);
            assert_eq!(interp, native.run_tests(), "{}", path.display());
        }
    });
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
        "pass  big_ok",
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
        outputs.push(out);
    }
    std::fs::remove_file(&path).ok();
    assert!(outputs.iter().all(|o| *o == outputs[0]));
}
