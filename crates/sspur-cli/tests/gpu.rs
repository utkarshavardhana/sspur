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
    for name in ["basic.ssp", "traps.ssp", "exact.ssp", "devbuf.ssp"] {
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

#[test]
fn portable_backends_compile_every_kernel() {
    let dir = std::env::temp_dir().join(format!("sspur-gpu-emit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["basic.ssp", "traps.ssp", "exact.ssp", "devbuf.ssp"] {
        let path = suite().join(name);
        let module = parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let kernels: Vec<String> = check(&module).kernels.keys().cloned().collect();
        let (ok, cl, err) = run(&["gpu", "--emit", "opencl"], &path, &[]);
        assert!(ok, "{name}: {err}");
        let (ok, msl, err) = run(&["gpu", "--emit", "metal"], &path, &[]);
        assert!(ok, "{name}: {err}");
        assert!(msl.contains("#pragma METAL fp contract(off)"));
        for k in &kernels {
            assert!(cl.contains(&format!("__kernel void k_{k}(")), "{name}: {k} missing from OpenCL");
        }
        for (target, ext) in [("spirv", "spv"), ("ptx", "ptx")] {
            let out = dir.join(format!("{name}.{ext}"));
            let (ok, _, err) = run(&["gpu", "--emit", target, "-o", out.to_str().unwrap()], &path, &[]);
            if !ok && err.contains("no clang with") {
                eprintln!("skipping {target}: {err}");
                continue;
            }
            assert!(ok, "{name} {target}: {err}");
            let bytes = std::fs::read(&out).unwrap();
            if target == "spirv" {
                assert_eq!(&bytes[..4], &[0x03, 0x02, 0x23, 0x07], "{name}: not a SPIR-V module");
            } else {
                let ptx = String::from_utf8_lossy(&bytes);
                assert!(!ptx.contains(".extern"), "{name}: PTX has unresolved calls");
                for k in &kernels {
                    assert!(ptx.contains(&format!(".entry k_{k}(")), "{name}: {k} missing from PTX");
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn metal_available() -> bool {
    let (_, _, err) = run(&["run"], &suite().join("basic.ssp"), &[("SSPUR_GPU_TRACE", "1")]);
    err.lines().any(|l| l.starts_with("gpu: "))
}

fn have_clang() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}

#[test]
fn native_kernels_match_the_interpreter_on_gpu_and_cpu() {
    if !have_clang() {
        return;
    }
    let metal = metal_available();
    for name in ["basic.ssp", "exact.ssp", "devbuf.ssp"] {
        let path = suite().join(name);
        let interp = run(&["run", "--interp"], &path, &[]);
        assert!(interp.0, "{name}: {}", interp.2);
        let gpu = run(&["run"], &path, &[("SSPUR_GPU_TRACE", "1")]);
        assert_eq!(interp.1, gpu.1, "{name}: GPU tier differs");
        let cpu = run(&["run"], &path, &[("SSPUR_GPU", "0"), ("SSPUR_GPU_TRACE", "1")]);
        assert_eq!(interp.1, cpu.1, "{name}: CPU fallback differs");
        assert!(!cpu.2.contains(": metal"), "{name}: SSPUR_GPU=0 still used the GPU");
        if metal {
            assert!(gpu.2.contains(": metal"), "{name}: no kernel ran on the GPU: {}", gpu.2);
        } else {
            eprintln!("Metal is unavailable; {name} checked on the CPU fallback only");
        }
    }
    if metal {
        let (_, _, err) = run(&["run"], &suite().join("exact.ssp"), &[("SSPUR_GPU_TRACE", "1")]);
        assert!(err.contains("gpu ops: cpu rerun"), "subnormal data must fall back: {err}");
        assert!(err.contains("gpu ops: metal"), "normal data must stay on the GPU: {err}");
    }
}

#[test]
fn kernel_traps_are_identical_in_every_tier() {
    if !have_clang() {
        return;
    }
    let path = suite().join("traps.ssp");
    let interp = run(&["test", "--interp"], &path, &[]);
    assert!(interp.1.contains("FAIL  oob_read: index 3 out of bounds for list of length 3"), "{}", interp.1);
    assert!(interp.1.contains("dev: element 2 of m is out of range for I32 (value = 3000000000)"), "{}", interp.1);
    assert!(interp.1.contains("5 passed, 8 failed"), "{}", interp.1);
    for env in [vec![], vec![("SSPUR_GPU", "0")]] {
        let native = run(&["test"], &path, &env);
        assert_eq!(interp.1, native.1, "{env:?}");
    }
    let path = suite().join("devbuf.ssp");
    let interp = run(&["test", "--interp"], &path, &[]);
    assert!(interp.1.contains("FAIL  alias_trap: dev: arguments x and y of half are the same buffer"), "{}", interp.1);
    assert!(interp.1.contains("FAIL  wrap_trap: integer overflow"), "{}", interp.1);
    assert!(interp.1.contains("3 passed, 3 failed"), "{}", interp.1);
    for env in [vec![], vec![("SSPUR_GPU", "0")]] {
        let native = run(&["test"], &path, &env);
        assert_eq!(interp.1, native.1, "{env:?}");
    }
}
