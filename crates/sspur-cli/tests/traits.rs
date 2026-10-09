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
