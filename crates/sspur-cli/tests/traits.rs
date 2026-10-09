use std::path::PathBuf;
use std::process::{Command, Stdio};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sspur-traits-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sspur(cwd: &PathBuf, args: &[&str]) -> (String, bool) {
    let o = Command::new(env!("CARGO_BIN_EXE_sspur")).args(args).env("SSPUR_CACHE", cwd.join("cache")).current_dir(cwd).stdin(Stdio::null()).output().unwrap();
    (String::from_utf8_lossy(&o.stdout).trim_end().to_string() + &String::from_utf8_lossy(&o.stderr), o.status.success())
}

#[test]
fn builtin_operators_keep_their_traps_through_bounds() {
    let d = dir("traps");
    let src = "type M = {c: Int}\n\nimpl Add for M\n  fn add(a: M, b: M) -> M = M{c: a.c + b.c}\n\nfn sum_all[T: Add](xs: List[T], z: T) -> T = xs.fold(z, (a, b) => a + b)\n\nfn main() -> Unit ! log\n= do\n  log(sum_all([M{c: 1}], M{c: 2}).c.str)\n  log(sum_all([9223372036854775807, 1], 0).str)\n";
    std::fs::write(d.join("t.ssp"), src).unwrap();
    let (i, ok_i) = sspur(&d, &["run", "--interp", "t.ssp"]);
    let (n, ok_n) = sspur(&d, &["run", "--strict-native", "t.ssp"]);
    let (o3, _) = sspur(&d, &["run", "--strict-native", "--O3", "t.ssp"]);
    assert!(!ok_i && !ok_n, "{i}\n{n}");
    assert!(i.contains("3") && i.contains("integer overflow"), "{i}");
    assert_eq!(i, n);
    assert_eq!(i, o3);
}

#[test]
fn trait_programs_are_identical_in_both_tiers_and_specialized_per_type() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs");
    let d = dir("tiers");
    for name in ["traits", "constraints", "operators", "derive"] {
        let p = root.join(format!("{name}.ssp"));
        let p = p.to_str().unwrap();
        let (i, ok) = sspur(&d, &["test", "--interp", p]);
        assert!(ok && i.ends_with(" passed, 0 failed"), "{name}: {i}");
        assert_eq!(sspur(&d, &["test", "--strict-native", p]).0, i, "{name}");
        assert_eq!(sspur(&d, &["test", "--strict-native", "--O3", p]).0, i, "{name} --O3");
        let (n, ok) = sspur(&d, &["native", "--release", "--strict-native", p]);
        assert!(ok && !n.contains("interp  "), "{name}: {n}");
    }
    let n = sspur(&d, &["native", "--release", root.join("constraints.ssp").to_str().unwrap()]).0;
    for f in ["max_of__Int", "max_of__Str", "best__Player", "best__Team", "score__Player"] {
        assert!(n.lines().any(|l| l == format!("native  {f}")), "{f} missing:\n{n}");
    }
}

#[test]
fn codebase_edits_store_traits_and_impls() {
    let d = dir("codebase");
    assert!(sspur(&d, &["init"]).1);
    std::fs::write(d.join("c.ssp"), "type P = {x: Int} derive Eq, Ord\n\ntrait Named\n  fn nm(x: Self) -> Str\n\nimpl Named for P\n  fn nm(p: P) -> Str = \"p{p.x}\"\n\nfn top[T: Ord + Named](xs: List[T]) -> Str = match xs.sort.last\n  | some(x) => x.nm\n  | none => \"\"\n\ntest t = top([P{x: 2}, P{x: 5}]) == \"p5\"\n").unwrap();
    let (out, ok) = sspur(&d, &["edit", "--test", "c.ssp"]);
    assert!(ok && out.contains("+impl Named for P") && out.contains("1 passed, 0 failed"), "{out}");
    std::fs::write(d.join("c.ssp"), "impl Named for P\n  fn nm(p: P) -> Str = \"q{p.x}\"\n\ntest t = top([P{x: 5}]) == \"q5\"\n").unwrap();
    let (out, ok) = sspur(&d, &["edit", "--test", "c.ssp"]);
    assert!(ok && out.contains("~impl Named for P") && out.contains("1 passed, 0 failed"), "{out}");
    std::fs::write(d.join("c.ssp"), "remove impl Named for P\nremove top\nremove t\n").unwrap();
    let (out, ok) = sspur(&d, &["edit", "c.ssp"]);
    assert!(ok && out.contains("-impl Named for P"), "{out}");
    assert_eq!(sspur(&d, &["check"]).0, "ok 2 definitions");
}
