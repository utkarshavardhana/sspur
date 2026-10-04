use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sspur-llvm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sspur(args: &[&str], cache: &Path) -> (String, String, bool) {
    let o = Command::new(env!("CARGO_BIN_EXE_sspur")).args(args).env("SSPUR_CACHE", cache).current_dir(root()).output().unwrap();
    (String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned(), o.status.success())
}

fn llvm_clang() -> bool {
    let cc = sspur_native::llvm::llvm_cc();
    Command::new(&cc).arg("--version").output().is_ok_and(|o| o.status.success())
}

#[test]
fn direct_ir_matches_the_interpreter_on_the_subset() {
    if !llvm_clang() {
        eprintln!("skip: no clang");
        return;
    }
    let d = scratch("subset");
    let exe = d.join("subset");
    let (_, err, ok) = sspur(&["build", "--backend", "llvm", "tests/llvm/subset.ssp", "-o", exe.to_str().unwrap()], &d);
    assert!(ok, "{err}");
    let ir = Command::new(&exe).output().unwrap();
    let (want, _, ok) = sspur(&["run", "--interp", "tests/llvm/subset.ssp"], &d);
    assert!(ok);
    assert_eq!(String::from_utf8_lossy(&ir.stdout), want);
    let src = d.join("ovf.ssp");
    std::fs::write(&src, "fn grow(n: Int) -> Int\n= do\n  var x = 1\n  for i in 0..n\n    x := x * 3\n  x\n\nfn main() -> Unit ! log\n= log(\"{grow(50)}\")\n").unwrap();
    let (_, err, ok) = sspur(&["build", "--backend", "llvm", src.to_str().unwrap(), "-o", d.join("ovf").to_str().unwrap()], &d);
    assert!(ok, "{err}");
    let o = Command::new(d.join("ovf")).output().unwrap();
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("integer overflow"));
    let (_, err, ok) = sspur(&["build", "--backend", "llvm", "bench/native/simd.ssp", "-o", d.join("x").to_str().unwrap()], &d);
    assert!(!ok && err.contains("outside the IR subset"), "{err}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn pgo_and_lto_builds_give_identical_output() {
    let d = scratch("pgo");
    let prog = "bench/llvm/simd_loops.ssp";
    let small = d.join("small.ssp");
    std::fs::write(&small, std::fs::read_to_string(root().join(prog)).unwrap().replace("1000000", "20000")).unwrap();
    let small = small.to_str().unwrap();
    let (want, _, ok) = sspur(&["run", small], &d);
    assert!(ok);
    for lto in ["thin", "full", "off"] {
        let (got, err, ok) = sspur(&["run", "--lto", lto, small], &d);
        assert!(ok, "{err}");
        assert_eq!(got, want, "--lto {lto}");
    }
    if sspur_native::cgen::flags::profdata_tool().is_err() {
        eprintln!("skip: no llvm-profdata");
        return;
    }
    let (out, err, ok) = sspur(&["build", "--pgo", small], &d);
    assert!(ok, "{err}");
    assert!(err.contains("pgo: training run") && out.contains("PGO from"), "{out}{err}");
    let (got, err, ok) = sspur(&["run", "--pgo", small], &d);
    assert!(ok, "{err}");
    assert_eq!(got, want);
    assert!(!err.contains("training run"), "the profile is reused: {err}");
    let (got, err, ok) = sspur(&["test", "--pgo", "--retrain", "tests/programs/simd.ssp"], &d);
    assert!(ok, "{got}{err}");
    let _ = std::fs::remove_dir_all(&d);
}
