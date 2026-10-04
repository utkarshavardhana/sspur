use sspur_store::cache::{load_src_cached, HITS, MISSES};
use sspur_store::load_src;
use std::sync::atomic::Ordering;
use std::time::Instant;

fn key(l: &sspur_store::Loaded) -> (Vec<(String, [u32; 2], String)>, Vec<(String, String)>) {
    let mut d: Vec<_> = l.check.diags.iter().map(|d| (d.code.clone(), d.span, d.msg.clone())).collect();
    d.sort();
    (d, l.hashes.clone().into_iter().collect())
}

fn program(n: usize, tweak: usize) -> String {
    let mut s = String::from("type Item = {sku: Str, qty: Int}\n\n");
    for i in 0..n {
        let prev = if i == 0 { "x.qty".to_string() } else { format!("f{}(x) + {}", i - 1, i % 7) };
        let c = if i == tweak { 99 } else { 1 };
        s.push_str(&format!("fn f{i}(x: Item) -> Int\n= do\n  ys = [x.qty, {c}, {i}].map(_ * 2).filter(_ > 0)\n  ys.sum + {prev}\n\n"));
    }
    for i in (0..n).step_by(10) {
        s.push_str(&format!("test t{i} = f{i}(Item{{sku: \"a\", qty: 1}}) > 0\n\n"));
    }
    s
}

#[test]
fn cached_checks_match_full_checks_and_skip_unchanged_definitions() {
    let dir = std::env::temp_dir().join(format!("sspur-cache-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    unsafe { std::env::set_var("SSPUR_CACHE", &dir) };
    let progs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/programs");
    let mut n = 0;
    for e in std::fs::read_dir(progs).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "ssp") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        let Ok(full) = load_src(src.clone()) else { continue };
        for round in ["cold", "warm"] {
            let c = load_src_cached(src.clone()).unwrap();
            assert_eq!(key(&full), key(&c), "{} ({round})", p.display());
        }
        n += 1;
    }
    assert!(n >= 10);

    let big = program(1500, usize::MAX);
    let t = Instant::now();
    let full = load_src(big.clone()).unwrap();
    let t_full = t.elapsed();
    assert!(!full.check.has_errors(), "{:?}", &full.check.diags[..1]);
    let t = Instant::now();
    let m = sspur_syntax::parse(&big).unwrap();
    let res = sspur_hash::Resolution { user_methods: Some(&full.check.user_methods), record_types: Some(&full.check.record_types) };
    let _ = sspur_hash::hash_module_with(&m, &res);
    let t_fixed = t.elapsed();
    let t = Instant::now();
    let cold = load_src_cached(big.clone()).unwrap();
    let t_cold = t.elapsed();
    let h0 = HITS.load(Ordering::Relaxed);
    let t = Instant::now();
    let warm = load_src_cached(big).unwrap();
    let t_warm = t.elapsed();
    assert_eq!(key(&full), key(&cold));
    assert_eq!(key(&full), key(&warm));
    assert_eq!(HITS.load(Ordering::Relaxed) - h0, 1650, "every definition is a hit");
    let edited = program(1500, 700);
    let m0 = MISSES.load(Ordering::Relaxed);
    let t = Instant::now();
    let one = load_src_cached(edited.clone()).unwrap();
    let t_one = t.elapsed();
    assert_eq!(key(&load_src(edited).unwrap()), key(&one));
    assert_eq!(MISSES.load(Ordering::Relaxed) - m0, 1, "a body edit rechecks only that definition");
    eprintln!(
        "cache: 1650 definitions; full check {:.1} ms, cold cached {:.1} ms, warm {:.1} ms ({:.1}x), one body edited {:.1} ms ({:.1}x); parse plus hashing alone {:.1} ms, so the checking part drops from {:.1} to {:.1} ms",
        t_full.as_secs_f64() * 1e3,
        t_cold.as_secs_f64() * 1e3,
        t_warm.as_secs_f64() * 1e3,
        t_full.as_secs_f64() / t_warm.as_secs_f64(),
        t_one.as_secs_f64() * 1e3,
        t_full.as_secs_f64() / t_one.as_secs_f64(),
        t_fixed.as_secs_f64() * 1e3,
        (t_full - t_fixed).as_secs_f64() * 1e3,
        t_warm.saturating_sub(t_fixed).as_secs_f64() * 1e3
    );
    let _ = std::fs::remove_dir_all(&dir);
}
