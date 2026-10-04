use std::io::Write;
use std::process::{Command, Stdio};

fn sspur(dir: &std::path::Path, args: &[&str], stdin: &str) -> (String, bool) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sspur")).args(args).current_dir(dir).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    c.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    let o = c.wait_with_output().unwrap();
    (String::from_utf8_lossy(&o.stdout).trim_end().to_string() + &String::from_utf8_lossy(&o.stderr), o.status.success())
}

fn fresh(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("sspur-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.ssp"), "fn one() -> Int\n= 1\n\nfn two() -> Int\n= one() + one()\n\ntest two_t = two() == 2\n").unwrap();
    assert_eq!(sspur(&d, &["init", "a.ssp"], ""), ("ok 3 definitions".into(), true));
    d
}

#[test]
fn edit_replaces_by_name_and_runs_tests() {
    let d = fresh("edit");
    let (out, ok) = sspur(&d, &["edit", "--test", "--interp"], "fn one() -> Int\n= 2\n\nfn three() -> Int\n= one() + 1\n");
    assert!(!ok);
    assert_eq!(out, "ok ~one +three\nFAIL  two_t: evaluated to false\n0 passed, 1 failed");
    let (out, ok) = sspur(&d, &["edit", "-e", "rename three tri\nremove two_t"], "");
    assert!(ok, "{out}");
    assert_eq!(out, "ok three->tri -two_t");
    assert_eq!(sspur(&d, &["q", "body", "tri"], "").0, "fn tri() -> Int\n= one() + 1");
}

#[test]
fn rejected_edit_reports_def_relative_errors() {
    let d = fresh("reject");
    let (out, ok) = sspur(&d, &["edit"], "fn two() -> Int\n= do\n  x = one()\n  x + \"a\"\n");
    assert!(!ok);
    assert!(out.starts_with("rejected, nothing changed\ntwo:4:"), "{out}");
    let (out, _) = sspur(&d, &["edit", "-e", "fn x( -> Int\n= 1"], "");
    assert!(out.contains("input:1:"), "{out}");
}

#[test]
fn apply_accepts_inline_ops_and_keeps_json() {
    let d = fresh("apply");
    let (out, ok) = sspur(&d, &["apply", "-e", r#"[{"op": "attach", "target": "one", "kind": "test", "value": "one() == 1"}]"#, "--test", "--interp"], "");
    assert!(ok, "{out}");
    assert_eq!(out, "ok +one_t1\n2 passed, 0 failed");
    let (out, ok) = sspur(&d, &["apply", "--json"], r#"{"ops": [{"op": "remove", "path": "one_t1"}]}"#);
    assert!(ok && out.starts_with(r#"{"ok":true,"root":""#), "{out}");
    assert_eq!(sspur(&d, &["q", "list"], "").0, "fn one() -> Int\nfn two() -> Int\ntests: two_t");
}

#[test]
fn explain_opt_lists_proven_rewrites() {
    let d = fresh("explain");
    let prog = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs/rewrites.ssp");
    let (out, ok) = sspur(&d, &["explain-opt", prog.to_str().unwrap()], "");
    assert!(ok, "{out}");
    for want in ["proof: intervals: condition 'n < 0' is always false", "spread  map-map", "total  loop-fusion", "sum_sq  inline", "scaled  fold"] {
        assert!(out.contains(want), "missing {want}:\n{out}");
    }
    if out.contains("width  dead-branch") {
        assert!(out.contains("proof: z3: condition 'lo > hi' is always false"), "{out}");
    }
    assert!(!out.contains("shaky  map-map"), "{out}");
    let (all, _) = sspur(&d, &["explain-opt", prog.to_str().unwrap(), "--all"], "");
    assert!(all.contains("note  shaky: not fusing maps: trap order could change"), "{all}");
}

#[test]
fn profile_counts_guide_inlining() {
    let d = fresh("profile");
    std::fs::write(d.join("p.ssp"), "fn hot(x: Int) -> Int\n= x * 3 + 1\n\nfn cold(x: Int) -> Int\n= x - 1\n\nfn run(n: Int) -> Int\n= do\n  var s = 0\n  for i in 0..n\n    s := s + hot(i)\n  if s < 0 then cold(s) else s\n\nfn main() -> Unit ! log\n= log(\"{run(100)}\")\n").unwrap();
    let go = |args: &[&str]| {
        let o = Command::new(env!("CARGO_BIN_EXE_sspur")).args(args).current_dir(&d).env("SSPUR_CACHE", d.join("cache")).output().unwrap();
        String::from_utf8_lossy(&o.stdout).to_string() + &String::from_utf8_lossy(&o.stderr)
    };
    let before = go(&["explain-opt", "p.ssp", "--all"]);
    assert!(before.contains("run  inline\n    cold(s)"), "{before}");
    let out = go(&["run", "--profile", "p.ssp"]);
    assert!(out.starts_with("14950\nprofile: 3 functions, 102 calls"), "{out}");
    let after = go(&["explain-opt", "p.ssp", "--all"]);
    assert!(after.contains("run  inline\n    hot(i)") && after.contains("note  run: not inlining cold: cost: profile: never called"), "{after}");
}
