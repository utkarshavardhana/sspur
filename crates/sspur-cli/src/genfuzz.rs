use sspur_eval::fuzz::Options;
use sspur_eval::progen;
use sspur_store::{load_src, Loaded};
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::process::ExitCode;

pub struct Cfg {
    pub n: usize,
    pub from: usize,
    pub seed: u64,
    pub cases: usize,
    pub group: usize,
    pub keep: PathBuf,
    pub modes: Vec<&'static str>,
}

pub const MODES: &[&str] = &["O2", "O2-whole", "O3"];

fn prog_seed(seed: u64, id: usize) -> u64 {
    seed.wrapping_mul(1_000_003).wrapping_add(id as u64)
}

pub fn source(seed: u64, id: usize) -> String {
    progen::program(prog_seed(seed, id), id)
}

fn set_mode(mode: &str) -> &'static str {
    let split = if mode == "O2" { "1" } else { "0" };
    unsafe { std::env::set_var("SSPUR_SPLIT", split) };
    if mode == "O3" { "-O3" } else { "-O2" }
}

#[derive(Default)]
struct Stats {
    programs: usize,
    rejected: usize,
    functions: usize,
    cases: usize,
    native_cases: usize,
    skipped: BTreeMap<String, (usize, String)>,
    findings: Vec<String>,
}

pub struct Outcome {
    pub mode: &'static str,
    pub func: String,
    pub args: Vec<String>,
    pub interp: String,
    pub native: String,
}

pub struct ModuleRun {
    pub diffs: Vec<Outcome>,
    pub funcs: usize,
    pub cases: usize,
    pub native: usize,
    pub skipped: BTreeMap<String, String>,
}

pub fn diff_module(l: &Loaded, mode: &'static str, cases: usize, seed: u64) -> Result<ModuleRun, String> {
    let opt = set_mode(mode);
    *sspur_native::cgen::SPLIT_FALLBACK.lock().unwrap() = None;
    let c = sspur_native::cgen::compile_release(&l.module, &l.check, opt).map_err(|e| format!("native build failed: {}", e.chars().take(400).collect::<String>()))?;
    if let Some(e) = sspur_native::cgen::SPLIT_FALLBACK.lock().unwrap().take() {
        return Err(format!("per-definition build failed: {}", e.lines().take(6).collect::<Vec<_>>().join(" | ")));
    }
    let skipped = c.skipped.clone();
    c.set_max_depth(1_000_000);
    let mut it = crate::interp(l);
    it.set_native(c);
    let reps = it.differential(&Options { cases, seed, edge: true });
    let funcs = reps.len();
    let mut out = vec![];
    let (mut n, mut nn) = (0, 0);
    for r in reps {
        n += r.cases;
        nn += r.native_cases;
        if let Some((args, i, nat)) = r.mismatch {
            out.push(Outcome { mode, func: r.name, args, interp: i, native: nat });
        }
    }
    Ok(ModuleRun { diffs: out, funcs, cases: n, native: nn, skipped })
}

fn prog_of(func: &str) -> Option<usize> {
    func.strip_prefix('p')?.split('_').next()?.parse().ok()
}

pub fn run(cfg: &Cfg) -> ExitCode {
    let mut st = Stats::default();
    let _ = std::fs::create_dir_all(&cfg.keep);
    let mut id = cfg.from;
    while id < cfg.from + cfg.n {
        let ids: Vec<usize> = (id..(id + cfg.group).min(cfg.from + cfg.n)).collect();
        id += cfg.group;
        let mut srcs: BTreeMap<usize, String> = ids.iter().map(|i| (*i, source(cfg.seed, *i))).collect();
        st.programs += srcs.len();
        let mut loaded = load_src(srcs.values().cloned().collect::<Vec<_>>().join("\n"));
        if loaded.as_ref().map_or(true, |l| l.check.has_errors()) {
            for i in &ids {
                let bad = match load_src(srcs[i].clone()) {
                    Ok(l) if !l.check.has_errors() => None,
                    Ok(l) => Some(l.check.diags.iter().find(|d| d.is_error()).map(|d| format!("{} {}: {}", d.code, d.def.clone().unwrap_or_default(), d.msg)).unwrap_or_default()),
                    Err(d) => Some(d.first().map(|d| d.msg.clone()).unwrap_or_default()),
                };
                if let Some(why) = bad {
                    st.rejected += 1;
                    println!("rejected p{i}: {why}");
                    let _ = std::fs::write(cfg.keep.join(format!("rejected-{}-{i}.ssp", cfg.seed)), &srcs[i]);
                    srcs.remove(i);
                }
            }
            loaded = load_src(srcs.values().cloned().collect::<Vec<_>>().join("\n"));
        }
        let Ok(l) = loaded else { continue };
        if srcs.is_empty() {
            continue;
        }
        for mode in &cfg.modes {
            let r = catch_unwind(AssertUnwindSafe(|| diff_module(&l, mode, cfg.cases, cfg.seed ^ ids[0] as u64)));
            match r {
                Err(_) => {
                    st.findings.push(format!("PANIC {mode} programs {:?}", srcs.keys().collect::<Vec<_>>()));
                    for (i, s) in &srcs {
                        let _ = std::fs::write(cfg.keep.join(format!("panic-{}-{i}.ssp", cfg.seed)), s);
                    }
                }
                Ok(Err(e)) => {
                    let culprits: Vec<usize> = srcs
                        .iter()
                        .filter(|(_, s)| {
                            load_src((*s).clone()).is_ok_and(|l| {
                                set_mode(mode);
                                sspur_native::cgen::compile_release(&l.module, &l.check, if *mode == "O3" { "-O3" } else { "-O2" }).is_err()
                            })
                        })
                        .map(|(i, _)| *i)
                        .collect();
                    st.findings.push(format!("BUILD {mode} programs {culprits:?}: {e}"));
                    for i in &culprits {
                        let _ = std::fs::write(cfg.keep.join(format!("build-{}-{i}.ssp", cfg.seed)), &srcs[i]);
                    }
                }
                Ok(Ok(ModuleRun { diffs, funcs, cases, native, skipped })) => {
                    if *mode == "O2" {
                        st.functions += funcs;
                        for (f, why) in &skipped {
                            let key: String = why.chars().take(60).map(|c| if c.is_ascii_digit() { 'N' } else { c }).collect();
                            let e = st.skipped.entry(key).or_insert((0, f.clone()));
                            e.0 += 1;
                        }
                    }
                    st.cases += cases;
                    st.native_cases += native;
                    for d in diffs {
                        let p = prog_of(&d.func);
                        let msg = format!("DIFF {} p{} {}({}): interpreter {} | native {}", d.mode, p.map_or("?".into(), |p| p.to_string()), d.func, d.args.join(", "), d.interp, d.native);
                        if let Some(s) = p.and_then(|p| srcs.get(&p)) {
                            let _ = std::fs::write(cfg.keep.join(format!("diff-{}-{}.ssp", cfg.seed, p.unwrap())), format!("// {msg}\n{s}"));
                        }
                        st.findings.push(msg);
                    }
                }
            }
        }
        println!("group {}..{}: {} findings so far", ids[0], ids[ids.len() - 1], st.findings.len());
    }
    for f in &st.findings {
        println!("{f}");
    }
    for (why, (n, f)) in &st.skipped {
        println!("not native ({n}, e.g. {f}): {why}");
    }
    println!(
        "{} programs ({} rejected), {} functions, {} cases ({} ran native), {} findings",
        st.programs, st.rejected, st.functions, st.cases, st.native_cases, st.findings.len()
    );
    if st.findings.is_empty() && st.rejected == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

pub enum Want {
    Diff(Vec<&'static str>),
    Skip(String),
    Build(String),
    Hang(Vec<String>),
}

fn interesting(src: &str, want: &Want, cases: usize, seed: u64) -> bool {
    let Ok(l) = load_src(src.to_string()) else { return false };
    if l.check.has_errors() {
        return false;
    }
    match want {
        Want::Skip(s) => {
            set_mode("O2-whole");
            sspur_native::cgen::compile_release(&l.module, &l.check, "-O2").is_ok_and(|c| c.skipped.values().any(|w| w.contains(s.as_str())))
        }
        Want::Hang(cmd) => {
            let path = std::env::temp_dir().join(format!("sspur-hang-{}.ssp", std::process::id()));
            if std::fs::write(&path, src).is_err() {
                return false;
            }
            let Ok(mut child) = std::process::Command::new(std::env::current_exe().unwrap()).args(cmd.iter().map(|a| if a == "FILE" { path.to_string_lossy().to_string() } else { a.clone() })).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn() else { return false };
            for _ in 0..50 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if let Some(st) = child.try_wait().ok().flatten() {
                    return st.code().is_none();
                }


            }
            let _ = child.kill();
            let _ = child.wait();
            true
        }
        Want::Build(s) => {
            set_mode("O2-whole");
            sspur_native::cgen::compile_release(&l.module, &l.check, "-O2").err().is_some_and(|e| e.contains(s.as_str()))
        }
        Want::Diff(modes) => modes.iter().any(|m| {
            let r = catch_unwind(AssertUnwindSafe(|| diff_module(&l, m, cases, seed)));
            matches!(r, Ok(Ok(run)) if !run.diffs.is_empty())
        }),
    }
}

fn blocks(lines: &[String]) -> Vec<(usize, usize)> {
    let ind = |s: &str| s.len() - s.trim_start().len();
    let mut out = vec![];
    for i in 0..lines.len() {
        if lines[i].trim().is_empty() {
            continue;
        }
        let mut j = i + 1;
        while j < lines.len() && !lines[j].trim().is_empty() && (ind(&lines[j]) > ind(&lines[i]) || lines[j].trim_start().starts_with('|') || lines[j].starts_with('=')) {
            j += 1;
        }
        out.push((i, j));
    }
    out
}

pub fn reduce(src: &str, want: &Want, cases: usize, seed: u64) -> Option<String> {
    if !interesting(src, want, cases, seed) {
        return None;
    }
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let mut changed = true;
    while changed {
        changed = false;
        for (i, j) in blocks(&lines).into_iter().rev() {
            if j > lines.len() || lines[i].trim().is_empty() {
                continue;
            }
            let mut trial = lines.clone();
            trial.drain(i..j);
            if interesting(&trial.join("\n"), want, cases, seed) {
                lines = trial;
                changed = true;
            }
        }
        for i in (0..lines.len()).rev() {
            if i >= lines.len() || lines[i].trim().is_empty() {
                continue;
            }
            let mut trial = lines.clone();
            trial.remove(i);
            if interesting(&trial.join("\n"), want, cases, seed) {
                lines = trial;
                changed = true;
            }
        }
    }
    lines.retain(|l| !l.trim().is_empty());
    let mut out = String::new();
    for l in &lines {
        if (l.starts_with("fn ") || l.starts_with("type ") || l.starts_with("effect ") || l.starts_with("test ")) && !out.is_empty() {
            out.push('\n');
        }
        out.push_str(l);
        out.push('\n');
    }
    Some(out)
}
