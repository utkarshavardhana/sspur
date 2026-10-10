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

fn regressions() -> Vec<(String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fuzz/regressions.ssp");
    let mut cases: Vec<(String, String)> = Vec::new();
    for line in std::fs::read_to_string(path).unwrap().lines() {
        if let Some(name) = line.strip_prefix("// case ") {
            cases.push((name.to_string(), String::new()));
        } else if let Some(c) = cases.last_mut() {
            c.1 += line;
            c.1 += "\n";
        }
    }
    cases
}

#[test]
fn fuzz_regressions_match_the_interpreter_in_every_build() {
    if !has_clang() {
        return;
    }
    let all = regressions();
    assert!(!all.is_empty());
    let dir = std::env::temp_dir().join(format!("sspur-fuzz-regressions-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (name, src) in all {
        let path: PathBuf = dir.join(format!("{name}.ssp"));
        std::fs::write(&path, &src).unwrap();
        let p = path.to_str().unwrap();
        let (out, ok) = sspur(&["test", "--strict-native", p], &[]);
        assert!(ok, "{name}: {out}");
        let (out, ok) = sspur(&["native", "--release", "--strict-native", p], &[]);
        assert!(ok && (src.contains("// interp-ok") || !out.contains("interp ")), "{name}: {out}");
        for (flags, split) in [(vec![], "1"), (vec![], "0"), (vec!["--O3"], "0")] {
            let mut args = vec!["fuzz", "--differential", "--strict-native", p, "--edge", "--cases", "60"];
            args.extend(flags.iter().copied());
            let (out, ok) = sspur(&args, &[("SSPUR_SPLIT", split)]);
            assert!(ok && !out.contains("DIFF"), "{name} {flags:?} split={split}: {out}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
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
