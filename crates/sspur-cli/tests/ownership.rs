use sspur_check::check;
use sspur_eval::Interp;
use sspur_syntax::parse;
use std::path::{Path, PathBuf};
use std::process::Command;

fn suite() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/ownership")
}

#[test]
fn every_unsound_program_is_rejected_with_its_code() {
    let expected = std::fs::read_to_string(suite().join("reject.txt")).unwrap();
    let mut n = 0;
    for line in expected.lines().filter(|l| !l.trim().is_empty()) {
        let (name, code) = line.split_once(' ').unwrap();
        let path = suite().join("reject").join(format!("{name}.ssp"));
        let src = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let module = parse(&src).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let out = check(&module);
        let codes: Vec<&str> = out.diags.iter().filter(|d| d.is_error()).map(|d| d.code.as_str()).collect();
        assert!(codes.contains(&code), "{name}: expected {code}, got {codes:?}");
        n += 1;
    }
    let files = std::fs::read_dir(suite().join("reject")).unwrap().count();
    assert_eq!(n, files, "every reject program needs an expected code in reject.txt");
    assert!(n >= 30, "soundness suite needs at least 30 rejected programs, found {n}");
}

fn accepted() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(suite().join("accept")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "ssp")).collect();
    v.sort();
    v
}

#[test]
fn every_sound_program_checks_and_passes_in_the_interpreter() {
    let progs = accepted();
    assert!(progs.len() >= 15, "soundness suite needs at least 15 accepted programs");
    for path in progs {
        let src = std::fs::read_to_string(&path).unwrap();
        let module = parse(&src).unwrap();
        let out = check(&module);
        let errors: Vec<_> = out.diags.iter().filter(|d| d.is_error() || d.severity == "warning").collect();
        assert!(errors.is_empty(), "{}: {errors:?}", path.display());
        let mut it = Interp::new(&module, out.record_types.clone(), out.user_methods.clone(), out.gen_loops.clone());
        it.set_ownership(out.own.moves.clone(), out.own.inplace.clone());
        let results = it.run_tests();
        assert!(!results.is_empty(), "{} has no tests", path.display());
        for (name, r) in &results {
            assert!(r.is_ok(), "{}::{name}: {r:?}", path.display());
        }
    }
}

#[test]
fn every_sound_program_runs_identically_in_native_code() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let bin = env!("CARGO_BIN_EXE_sspur");
    for path in accepted() {
        let src = std::fs::read_to_string(&path).unwrap();
        let module = parse(&src).unwrap();
        let out = check(&module);
        let compiled = sspur_native::cgen::compile_release(&module, &out, "-O2").unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(compiled.skipped.is_empty(), "{}: not native: {:?}", path.display(), compiled.skipped);
        let run = |args: &[&str]| {
            let o = Command::new(bin).args(args).arg(&path).output().unwrap();
            (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned())
        };
        let interp = run(&["run", "--interp"]);
        let native = run(&["run"]);
        assert!(interp.0, "{}: main failed", path.display());
        assert!(interp.1.lines().count() > 0, "{}: main printed nothing", path.display());
        assert_eq!(interp, native, "{}", path.display());
        let summary = |s: String| s.lines().filter(|l| l.starts_with("FAIL") || l.contains(" passed, ")).map(str::to_string).collect::<Vec<_>>();
        let (ti, tn) = (summary(run(&["test", "--interp"]).1), summary(run(&["test"]).1));
        assert_eq!(ti, tn, "{}", path.display());
        assert!(ti.iter().all(|l| !l.starts_with("FAIL")), "{}: {ti:?}", path.display());
    }
}
