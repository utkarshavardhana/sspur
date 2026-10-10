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
fn failing_tests_show_the_values_in_both_tiers() {
    let d = fresh("values");
    let src = "type E = Bad{n: Int}\n\nfn g(x: Int) -> Int ! fail[E]\n= if x > 3 then raise Bad{n: x} else x\n\nfn c(lo: Int, hi: Int, s: Str) -> Int\n  pre lo <= hi\n= hi - lo\n\ntest t_eq = two() + 5 == 8\ntest t_and = two() == 2 and [two()].contains(3)\ntest t_catch = catch g(5) == 5\n  | Bad{n} => n == 4\ntest t_raised = catch g(9) == 9\n  | _ => false\ntest t_pre = c(3, 0, \"a\") == 0\ntest t_do = do\n  x = \"a{two()}\"\n  x == \"a3\"\n";
    let want = "FAIL  t_and: [2].contains(3) is false\nFAIL  t_catch: raised Bad{n: 5}: left 5, right 4\nFAIL  t_do: left \"a2\", right \"a3\"\nFAIL  t_eq: left 7, right 8\nFAIL  t_pre: contract violated: pre lo <= hi in c (lo = 3, hi = 0)\nFAIL  t_raised: raised Bad{n: 9}\n1 passed, 6 failed";
    for tier in ["--interp", "--strict-native"] {
        let (out, ok) = sspur(&d, &["edit", "--test", tier], src);
        assert!(!ok);
        assert_eq!(out.split_once('\n').unwrap().1, want, "{tier}");
    }
}

#[test]
fn edit_replaces_by_name_and_runs_tests() {
    let d = fresh("edit");
    let (out, ok) = sspur(&d, &["edit", "--test", "--interp"], "fn one() -> Int\n= 2\n\nfn three() -> Int\n= one() + 1\n");
    assert!(!ok);
    assert_eq!(out, "ok ~one +three\nFAIL  two_t: left 4, right 2\n0 passed, 1 failed");
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
    assert_eq!(sspur(&d, &["q", "list"], "").0, "# fns 2\nfn one() -> Int\nfn two() -> Int\n# tests 1: two_t");
    assert_eq!(sspur(&d, &["q", "list", "--tests"], "").0, "# tests 1\ntwo_t");
    assert_eq!(sspur(&d, &["q", "find", "TWO"], "").0, "fn two() -> Int\ntests: two_t");
    assert_eq!(sspur(&d, &["q", "grep", "one()"], "").0, "fn one() -> Int\nfn two() -> Int\n  = one() + one()");
    assert_eq!(sspur(&d, &["q", "body", "one", "two"], "").0, "fn one() -> Int\n= 1\n\nfn two() -> Int\n= one() + one()");
}

#[test]
fn find_and_grep_cap_their_text_output() {
    let d = fresh("hits_cap");
    let mut src = String::new();
    for i in 0..30 {
        src.push_str(&format!("fn f{i:02}(x: Int) -> Int\n= one() + {i}\n\n"));
    }
    src.push_str("fn f(x: Int) -> Int\n= one()\n");
    let (out, ok) = sspur(&d, &["edit", "-e", &src], "");
    assert!(ok, "{out}");
    let (out, _) = sspur(&d, &["q", "grep", "one()"], "");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.iter().filter(|l| l.starts_with("fn ")).count(), 12, "12 definitions in full: {out}");
    let last = lines.last().unwrap();
    assert!(last.starts_with("-- 21 more: f") && last.contains(" f29"), "{last}");
    let (out, _) = sspur(&d, &["q", "find", "f"], "");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "fn f(x: Int) -> Int", "the exact name first");
    assert_eq!(lines.iter().filter(|l| l.starts_with("fn ")).count(), 25, "25 signatures: {out}");
    assert!(lines.last().unwrap().starts_with("-- 6 more: f"), "{out}");
    let (full, _) = sspur(&d, &["q", "grep", "one()", "--json"], "");
    assert!(full.contains("\"f29\""), "--json is uncapped past the text cap: {full}");
}

#[test]
fn start_prints_the_spec_and_a_small_codebase_whole() {
    let spec = sspur(&std::env::temp_dir(), &["spec"], "").0;
    let d = fresh("start_small");
    let (out, ok) = sspur(&d, &["start", "one"], "");
    assert!(ok);
    let src = sspur(&d, &["src"], "").0;
    assert_eq!(out, format!("{spec}\n## This codebase: 2 fns, 1 test, all of it\n{src}"));
    let empty = std::env::temp_dir().join(format!("sspur-cli-start-none-{}", std::process::id()));
    std::fs::create_dir_all(&empty).unwrap();
    let (out, ok) = sspur(&empty, &["start"], "");
    assert!(ok && out.starts_with(&spec) && out.ends_with("or the first edit creates it."), "{out}");
}

#[test]
fn start_packs_named_definitions_of_a_large_codebase() {
    let d = fresh("start_large");
    let mut src = String::new();
    for i in 0..300 {
        src.push_str(&format!("fn padding_function_number_{i:03}(x: Int) -> Int\n= one() + x * {i}\n\n"));
    }
    let (out, ok) = sspur(&d, &["edit", "-e", &src], "");
    assert!(ok, "{out}");
    let (out, ok) = sspur(&d, &["start", "two,one", "number_00*"], "");
    assert!(ok, "{out}");
    let rest = &out[out.find("## This codebase").expect("a codebase section")..];
    let (head, rest) = rest.split_once("\n\n").unwrap();
    assert!(head.starts_with("## This codebase: 302 fns, 1 test, about ") && head.ends_with("too much to print. Search it with `q find|grep|body|callers|pack`."), "{head}");
    let pack = sspur(&d, &["q", "pack", "two,one"], "").0;
    let find = sspur(&d, &["q", "find", "number_00*"], "").0;
    assert_eq!(rest, format!("## q pack two,one\n{pack}\n\n## q find 'number_00*'\n{find}"));
    let (out, _) = sspur(&d, &["start"], "");
    assert!(out.ends_with("also packs the named definitions (and finds other words) in this output."), "{out}");
}

#[test]
fn spec_runs_queries_after_the_spec() {
    let spec = sspur(&std::env::temp_dir(), &["spec"], "").0;
    let d = fresh("spec_q");
    let (out, ok) = sspur(&d, &["spec", "src", "find", "tw*", "callers", "one"], "");
    assert!(ok, "{out}");
    let src = sspur(&d, &["src"], "").0;
    assert_eq!(out, format!("{spec}\n\n## src\n{src}\n\n## q find 'tw*'\nfn two() -> Int\ntests: two_t\n\n## q callers one\ntwo"));
    let (out, ok) = sspur(&d, &["spec", "pack"], "");
    assert!(!ok && out.trim_end().ends_with("query 'pack' needs a target: spec pack TARGET"), "{out}");
}

#[test]
fn explain_opt_lists_proven_rewrites() {
    let d = fresh("explain");
    let prog = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs/pipelines.ssp");
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

#[cfg(unix)]
#[test]
fn failed_native_builds_warn_fall_back_and_fail_under_strict() {
    use std::os::unix::fs::PermissionsExt;
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let d = std::env::temp_dir().join(format!("sspur-cli-fallback-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("p.ssp"), "fn victim(n: Int) -> Int\n= n * 3 + 1\n\nfn other(n: Int) -> Int\n= victim(n) - 1\n\ntest t = other(2) == 6\n").unwrap();
    let cc = d.join("cc.sh");
    std::fs::write(&cc, "#!/bin/sh\nfor a in \"$@\"; do case \"$a\" in *.c) n=$(grep -n 'f_victim(.*) {$' \"$a\" | head -n1 | cut -d: -f1); if [ -n \"$n\" ] && { [ \"$MODE\" = all ] || echo \" $* \" | grep -q ' -c '; }; then echo \"$a:$n:1: error: injected failure\" >&2; exit 1; fi;; esac; done\nexec clang \"$@\"\n").unwrap();
    std::fs::set_permissions(&cc, std::fs::Permissions::from_mode(0o755)).unwrap();
    let run = |mode: &str, extra: &[&str], strict: bool| {
        let mut c = Command::new(env!("CARGO_BIN_EXE_sspur"));
        c.arg("test").arg(d.join("p.ssp")).args(extra).env("CC", &cc).env("MODE", mode).env("SSPUR_CACHE", d.join(format!("cache-{mode}"))).env_remove("SSPUR_STRICT_NATIVE").env_remove("SSPUR_SPLIT");
        if strict {
            c.env("SSPUR_STRICT_NATIVE", "1");
        }
        let o = c.output().unwrap();
        (String::from_utf8_lossy(&o.stdout).to_string(), String::from_utf8_lossy(&o.stderr).to_string(), o.status.success())
    };
    let (out, err, ok) = run("split", &[], false);
    assert!(ok && out.contains("1 passed"), "{out}{err}");
    assert_eq!(err.trim(), "warning: per-definition native build failed in fn victim: error: injected failure; using the whole-program build");
    let (_, err, ok) = run("split", &["--quiet"], false);
    assert!(ok && err.is_empty(), "{err}");
    let (out, err, ok) = run("split", &["--strict-native"], false);
    assert!(!ok && !out.contains("passed"), "{out}");
    assert_eq!(err.trim(), "error: per-definition native build failed in fn victim: error: injected failure");
    let (out, err, ok) = run("all", &[], false);
    assert!(ok && out.contains("1 passed"), "{out}{err}");
    assert_eq!(err.trim(), "warning: native build failed in fn victim: error: injected failure; interpreting");
    let (_, err, ok) = run("all", &[], true);
    assert!(!ok);
    assert_eq!(err.trim(), "error: native build failed in fn victim: error: injected failure");
    let _ = std::fs::remove_dir_all(&d);
}
