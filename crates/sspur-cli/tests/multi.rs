use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn sspur(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> (String, bool) {
    let o = Command::new(env!("CARGO_BIN_EXE_sspur")).args(args).envs(env.iter().copied()).current_dir(dir).stdin(Stdio::null()).output().unwrap();
    (String::from_utf8_lossy(&o.stdout).trim_end().to_string() + &String::from_utf8_lossy(&o.stderr), o.status.success())
}

fn fresh(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sspur-multi-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.ssp"), "fn one() -> Int\n= 1\n\ntest one_t = one() == 1\n").unwrap();
    assert!(sspur(&d, &["init", "a.ssp"], &[]).1);
    d
}

#[test]
fn concurrent_processes_lose_no_edits() {
    let d = fresh("procs");
    let hs: Vec<_> = (0..8)
        .map(|p| {
            let d = d.clone();
            std::thread::spawn(move || {
                for k in 0..5 {
                    let src = format!("fn p{p}_{k}() -> Int\n= one() + {k}");
                    let (out, ok) = sspur(&d, &["edit", "-e", &src], &[]);
                    assert!(ok, "{out}");
                }
            })
        })
        .collect();
    for h in hs {
        h.join().unwrap();
    }
    let (src, ok) = sspur(&d, &["src"], &[]);
    assert!(ok);
    for p in 0..8 {
        for k in 0..5 {
            assert!(src.contains(&format!("fn p{p}_{k}()")), "p{p}_{k} lost");
        }
    }
    assert_eq!(sspur(&d, &["check"], &[]), ("ok 42 definitions".into(), true));
}

#[test]
fn a_crash_mid_commit_never_corrupts_the_codebase() {
    for (i, point) in ["commit", "root", "index"].iter().enumerate() {
        let d = fresh(&format!("crash-{point}"));
        let src = format!("fn c{i}() -> Int\n= 1");
        let (_, ok) = sspur(&d, &["edit", "-e", &src], &[("SSPUR_CRASH_AT", point)]);
        assert!(!ok, "the process aborts at {point}");
        let (out, ok) = sspur(&d, &["src"], &[]);
        assert!(ok, "{out}");
        assert_eq!(out.contains(&format!("fn c{i}()")), *point == "index", "after a crash at {point}: {out}");
        assert!(sspur(&d, &["check"], &[]).1);
        let (out, ok) = sspur(&d, &["edit", "-e", "fn after() -> Int\n= one()"], &[]);
        assert!(ok, "{out}");
        assert!(sspur(&d, &["test", "--interp"], &[]).1);
    }
}

#[test]
fn crashes_at_every_write_step_of_edits_and_pulls_recover() {
    for point in ["objects", "commit", "root", "index", "head"] {
        let d = fresh(&format!("crash2-{point}"));
        assert!(sspur(&d, &["edit", "-e", "fn two() -> Int\n= one() + 1"], &[]).1);
        let (_, ok) = sspur(&d, &["edit", "-e", "fn three() -> Int\n= two() + 1"], &[("SSPUR_CRASH_AT", point)]);
        assert!(!ok, "the process aborts at {point}");
        let (out, ok) = sspur(&d, &["src"], &[]);
        assert!(ok && out.contains("fn two()"), "acknowledged edit lost after a crash at {point}: {out}");
        assert_eq!(out.contains("fn three()"), matches!(point, "index" | "head"), "after a crash at {point}: {out}");
        assert!(sspur(&d, &["check"], &[]).1, "HEAD does not check after a crash at {point}");
        assert!(sspur(&d, &["edit", "-e", "fn three() -> Int\n= two() + 2"], &[]).1);
        assert!(sspur(&d, &["edit", "-e", "fn four() -> Int\n= three() + 1"], &[]).1);
        assert!(sspur(&d, &["test", "--interp"], &[]).1);
    }
    for point in ["ingest", "objects", "commit", "root", "index", "head"] {
        let a = fresh(&format!("crash-pull-a-{point}"));
        let b = std::env::temp_dir().join(format!("sspur-multi-crash-pull-b-{point}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&b);
        std::fs::create_dir_all(&b).unwrap();
        assert!(sspur(&b, &["init"], &[]).1);
        assert!(sspur(&a, &["edit", "-e", "fn two() -> Int\n= one() + 1"], &[]).1);
        assert!(sspur(&a, &["edit", "-e", "fn three() -> Int\n= two() + 1"], &[]).1);
        let remote = a.to_str().unwrap();
        let (_, ok) = sspur(&b, &["sync", "pull", remote], &[("SSPUR_CRASH_AT", point)]);
        assert!(!ok, "the pull aborts at {point}");
        let (out, ok) = sspur(&b, &["check"], &[]);
        assert!(ok || out.contains("no definitions") || out.contains("empty"), "HEAD does not check after a pull crash at {point}: {out}");
        let (out, ok) = sspur(&b, &["sync", "pull", remote], &[]);
        assert!(ok, "pull after a crash at {point}: {out}");
        assert_eq!(sspur(&b, &["src"], &[]).0, sspur(&a, &["src"], &[]).0, "replicas differ after a pull crash at {point}");
        assert!(sspur(&b, &["edit", "-e", "fn four() -> Int\n= three() + 1"], &[]).1);
        assert!(sspur(&a, &["sync", "pull", b.to_str().unwrap()], &[]).1);
        assert_eq!(sspur(&b, &["src"], &[]).0, sspur(&a, &["src"], &[]).0);
        assert!(sspur(&a, &["check"], &[]).1 && sspur(&b, &["check"], &[]).1);
    }
}

#[test]
fn sync_serve_push_and_pull_over_tcp() {
    let a = fresh("sync-a");
    let b = std::env::temp_dir().join(format!("sspur-multi-sync-b-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&b);
    std::fs::create_dir_all(&b).unwrap();
    assert!(sspur(&b, &["init"], &[]).1);
    let mut srv = Command::new(env!("CARGO_BIN_EXE_sspur")).args(["sync", "serve", "--port", "0", "--max", "4"]).current_dir(&a).stdout(Stdio::piped()).spawn().unwrap();
    let mut line = String::new();
    BufReader::new(srv.stdout.as_mut().unwrap()).read_line(&mut line).unwrap();
    let url = line.split_whitespace().last().unwrap().to_string();
    assert!(url.starts_with("tcp://"), "{line}");
    let (out, ok) = sspur(&b, &["sync", "pull", &url], &[]);
    assert!(ok && out.starts_with("ok pulled 1 commits"), "{out}");
    assert!(sspur(&b, &["edit", "-e", "fn two() -> Int\n= one() + 1"], &[]).1);
    let (out, ok) = sspur(&b, &["sync", "push", &url], &[]);
    assert!(ok && out.starts_with("ok pushed 1 commits"), "{out}");
    srv.wait().unwrap();
    assert_eq!(sspur(&a, &["src"], &[]), sspur(&b, &["src"], &[]));
    let (out, _) = sspur(&a, &["sync", "status"], &[]);
    assert!(out.contains("commits 2"), "{out}");
}
