use sspur_check::check;
use sspur_syntax::parse;
use std::path::{Path, PathBuf};
use std::process::Command;

fn suite() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/gpu")
}

fn run(args: &[&str], path: &Path, env: &[(&str, &str)]) -> (bool, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sspur"));
    c.args(args).arg(path);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

#[test]
fn kernel_violations_are_rejected_with_their_codes() {
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
}

#[test]
fn accepted_kernels_check_cleanly_and_round_trip() {
    for name in ["basic.ssp", "traps.ssp"] {
        let src = std::fs::read_to_string(suite().join(name)).unwrap();
        let module = parse(&src).unwrap();
        let out = check(&module);
        let bad: Vec<_> = out.diags.iter().filter(|d| d.is_error() || d.severity == "warning").collect();
        assert!(bad.is_empty(), "{name}: {bad:?}");
        assert!(!out.kernels.is_empty());
        let printed = sspur_syntax::print_module(&module);
        assert_eq!(parse(&printed).unwrap(), parse(&sspur_syntax::print_module(&parse(&printed).unwrap())).unwrap());
    }
}

fn f32_saxpy_line() -> String {
    let xs: Vec<f32> = (0..1000).map(|i| (i as f64 * 0.5) as f32).collect();
    let ys: Vec<f32> = (0..1000).map(|i| (1.0 / (i + 1) as f64) as f32).collect();
    let r: Vec<f64> = xs.iter().zip(&ys).map(|(x, y)| (1.5f32 * x + y) as f64).collect();
    let mut sum = -0.0;
    for v in &r {
        sum += v;
    }
    format!("saxpy {:?} {:?} {:?} {:?}", r[0], r[1], r[999], sum)
}

#[test]
fn interpreter_runs_kernels_sequentially_with_f32_rounding() {
    let (ok, out, err) = run(&["run", "--interp"], &suite().join("basic.ssp"), &[]);
    assert!(ok, "{err}");
    assert_eq!(out.lines().next().unwrap(), f32_saxpy_line());
    assert!(out.contains("rows [36, 33, 30"), "{out}");
}
