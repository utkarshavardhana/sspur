use std::path::{Path, PathBuf};
use std::process::Command;

fn sspur(args: &[&str], env: &[(&str, &str)]) -> (String, bool) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sspur"));
    c.args(args);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    (format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)), o.status.success())
}

fn has_clang() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}

fn files(dir: &str) -> Vec<PathBuf> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests").join(dir);
    let mut out: Vec<PathBuf> = std::fs::read_dir(base).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "ssp")).collect();
    out.sort();
    out
}

#[test]
fn fuzz_regressions_match_the_interpreter_in_every_build() {
    if !has_clang() {
        return;
    }
    let all = files("fuzz");
    assert!(!all.is_empty());
    for path in all {
        let p = path.to_str().unwrap();
        let src = std::fs::read_to_string(&path).unwrap();
        let (out, ok) = sspur(&["test", "--strict-native", p], &[]);
        assert!(ok, "{p}: {out}");
        let (out, ok) = sspur(&["native", "--release", "--strict-native", p], &[]);
        assert!(ok && (src.contains("// interp-ok") || !out.contains("interp ")), "{p}: {out}");
        for (flags, split) in [(vec![], "1"), (vec![], "0"), (vec!["--O3"], "0")] {
            let mut args = vec!["fuzz", "--differential", "--strict-native", p, "--edge", "--cases", "60"];
            args.extend(flags.iter().copied());
            let (out, ok) = sspur(&args, &[("SSPUR_SPLIT", split)]);
            assert!(ok && !out.contains("DIFF"), "{p} {flags:?} split={split}: {out}");
        }
    }
}

#[test]
fn generated_programs_match_the_interpreter() {
    if !has_clang() {
        return;
    }
    let (out, ok) = sspur(&["fuzz", "--gen", "20", "--seed", "11", "--cases", "6"], &[]);
    assert!(ok, "{out}");
    assert!(out.contains("20 programs (0 rejected)"), "{out}");
}
