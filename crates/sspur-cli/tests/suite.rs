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

        let interp = Interp::new(&module, out.record_types.clone(), out.user_methods.clone(), out.gen_loops.clone());
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
            let interp = Interp::new(&module, out.record_types.clone(), out.user_methods.clone(), out.gen_loops.clone()).run_tests();
            let mut native = Interp::new(&module, out.record_types.clone(), out.user_methods.clone(), out.gen_loops.clone());
            let compiled = sspur_native::cgen::compile_release(&module, &out, "-O2").unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let effectful = module.defs.iter().any(|d| matches!(d, sspur_syntax::Def::Effect(_))) || src.contains("yield");
            assert!(compiled.skipped.is_empty() || effectful, "{}: not native: {:?}", path.display(), compiled.skipped);
            native.set_native(compiled);
            assert_eq!(interp, native.run_tests(), "{}", path.display());
        }
    });
}
