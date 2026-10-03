mod agent;
mod bind;
mod mcp;
mod verify;

use serde_json::{json, Value as Json};
use sspur_check::Diag;
use sspur_eval::fuzz::Options;
use sspur_eval::Interp;
use sspur_hash::{hash_module_with, Resolution};
use sspur_store::{load_src, query::Ctx, Loaded, Store, Tx};
use sspur_syntax::{line_col, print_module};
use std::io::Read;
use std::process::ExitCode;

pub const REFERENCE: &str = include_str!("../../../docs/07-reference-v0.md");
pub const AGENT_SPEC: &str = include_str!("../../../docs/agent-spec.md");

const USAGE: &str = "usage:
  sspur init [file.ssp]                 create a codebase in .sspur/ (optionally import a file)
  sspur edit [file|-] [-e SRC] [--test] replace or add definitions by name (also 'rename A B', 'remove A' lines)
  sspur apply [tx.json|-] [-e JSON] [--test] apply a transaction of ops
  sspur q <query> [target] [--budget N] query the codebase (list sig body callers callees effects find pack why impact holes diag log)
  sspur src | log | export | spec [--full] | mcp
  sspur bind header.h [--lib NAME] [-o out.ssp]   generate extern declarations from a C header (uses clang)
  sspur export-c file.ssp [-o libfoo] [--shared] [--prefix P]  build a C library and header from the C-compatible functions
  add --json for machine output (apply, edit, q, check)
  sspur check|run|test|fuzz|verify|hash|fmt|native [file.ssp] [--json] [--cases N] [--seed N] [--edge] [--write] [--full]
  verify proves pre/post/where clauses with z3 and reports proved, counterexample, or unknown per clause
  run/test compile to native code by default (cached); --interp forces the interpreter, --native uses the Cranelift JIT,
  --O3 raises the optimization level; fuzz --differential compares native against the interpreter";

fn main() -> ExitCode {
    std::thread::Builder::new().stack_size(1 << 29).spawn(real_main).unwrap().join().unwrap()
}

struct Args {
    pos: Vec<String>,
    flags: Vec<String>,
}

impl Args {
    fn has(&self, f: &str) -> bool {
        self.flags.iter().any(|x| x == f)
    }

    fn val(&self, f: &str) -> Option<&String> {
        self.flags.iter().position(|x| x == f).and_then(|i| self.flags.get(i + 1))
    }

    fn num(&self, f: &str, default: usize) -> usize {
        self.flags.iter().position(|x| x == f).and_then(|i| self.flags.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
}

fn parse_args() -> Args {
    let mut pos = Vec::new();
    let mut flags = Vec::new();
    let mut it = std::env::args().skip(1).peekable();
    while let Some(a) = it.next() {
        if a.starts_with("--") || a == "-e" || a == "-o" {
            let takes = matches!(a.as_str(), "--budget" | "--cases" | "--seed" | "-e" | "-o" | "--lib" | "--prefix");
            flags.push(a);
            if takes
                && let Some(v) = it.next() {
                    flags.push(v);
                }
        } else {
            pos.push(a);
        }
    }
    Args { pos, flags }
}

fn cwd_store() -> Option<Store> {
    Store::find(&std::env::current_dir().ok()?)
}

fn real_main() -> ExitCode {
    let args = parse_args();
    let Some(cmd) = args.pos.first().cloned() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match cmd.as_str() {
        "spec" => {
            print!("{}", if args.has("--full") { REFERENCE } else { AGENT_SPEC });
            ExitCode::SUCCESS
        }
        "init" => init(args.pos.get(1), args.has("--json")),
        "apply" => apply(&args),
        "edit" => edit(&args),
        "q" => {
            let Some(store) = cwd_store() else { return no_store() };
            let loaded = match store.load_head() {
                Ok(l) => l,
                Err(d) => return fail_diags("", "HEAD", &d, true),
            };
            let q = args.pos.get(1).map_or("list", String::as_str);
            let out = Ctx::new(&loaded, Some(&store)).run(q, args.pos.get(2).map(String::as_str), args.num("--budget", 2000));
            if args.has("--json") {
                println!("{out}");
            } else {
                println!("{}", agent::query_text(q, &out, &loaded.src));
                if args.has("--full") && out.get("est_tokens").is_some() {
                    eprintln!("-- {} tokens, included {}, omitted {}", out["est_tokens"], out["included"], out["omitted"]);
                }
            }
            if out.get("error").is_some() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
        }
        "log" => {
            let Some(store) = cwd_store() else { return no_store() };
            for r in store.log() {
                let changes: Vec<String> = r.changes.iter().map(agent::change_text).collect();
                println!("#{}  {}  {}  {}  [{}]", &r.hash[..12], r.at, r.agent, r.reason, changes.join(" "));
            }
            ExitCode::SUCCESS
        }
        "export" | "src" => {
            let Some(store) = cwd_store() else { return no_store() };
            print!("{}", store.head_root().map(|r| store.root_src(&r)).unwrap_or_default());
            ExitCode::SUCCESS
        }
        "mcp" => mcp::serve(),
        "bind" => {
            let Some(h) = args.pos.get(1) else {
                eprintln!("usage: sspur bind header.h [--lib NAME] [-o out.ssp]");
                return ExitCode::from(2);
            };
            match bind::bind(h, args.val("--lib").map(String::as_str)) {
                Ok(out) => match args.val("-o") {
                    Some(p) => match std::fs::write(p, out) {
                        Ok(()) => ExitCode::SUCCESS,
                        Err(e) => {
                            eprintln!("cannot write {p}: {e}");
                            ExitCode::FAILURE
                        }
                    },
                    None => {
                        print!("{out}");
                        ExitCode::SUCCESS
                    }
                },
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        "check" | "run" | "test" | "fuzz" | "verify" | "hash" | "fmt" | "native" | "export-c" => program_cmd(&cmd, &args),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn no_store() -> ExitCode {
    eprintln!("no .sspur codebase here; run 'sspur init'");
    ExitCode::FAILURE
}

fn init(file: Option<&String>, json: bool) -> ExitCode {
    let dir = std::env::current_dir().unwrap();
    let store = match Store::init(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot create .sspur: {e}");
            return ExitCode::FAILURE;
        }
    };
    let Some(path) = file else {
        println!("initialized empty codebase in {}/.sspur", dir.display());
        return ExitCode::SUCCESS;
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let module = match sspur_syntax::parse(&src) {
        Ok(m) => m,
        Err(e) => return fail_diags(&src, path, &[sspur_check::syntax_diag(&e)], false),
    };
    let ops: Vec<Json> = module.defs.iter().map(|d| json!({"op": "add", "path": d.name(), "src": sspur_syntax::printer::print_def(d)})).collect();
    let n = ops.len();
    let r = store.apply(Tx { base: None, agent: Some("import".into()), reason: Some(format!("import {path}")), gate: None, ops });
    if json {
        println!("{}", serde_json::to_string(&r).unwrap());
    } else if r.ok {
        println!("ok {n} definitions");
    } else {
        println!("{}", agent::tx_text(&r));
    }
    if r.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn read_input(args: &Args) -> Result<String, String> {
    if let Some(e) = args.val("-e") {
        return Ok(e.clone());
    }
    let mut raw = String::new();
    match args.pos.get(1).map(String::as_str) {
        None | Some("-") => std::io::stdin().read_to_string(&mut raw).map(|_| raw),
        Some(p) => std::fs::read_to_string(p),
    }
    .map_err(|e| format!("cannot read input: {e}"))
}

fn finish(args: &Args, store: &Store, ops: Vec<Json>, agent_name: &str) -> ExitCode {
    let make = |l: &Loaded| native_interp(l, args);
    let test: Option<&dyn Fn(&Loaded) -> Interp> = if args.has("--test") { Some(&make) } else { None };
    let (text, ok) = agent::run_edit(store, ops, agent_name, test);
    println!("{text}");
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn apply(args: &Args) -> ExitCode {
    let Some(store) = cwd_store() else { return no_store() };
    let tx = match read_input(args).and_then(|raw| agent::parse_tx(&raw)) {
        Ok(t) => t,
        Err(e) => {
            println!("{e}");
            return ExitCode::FAILURE;
        }
    };
    if args.has("--json") {
        let r = store.apply(tx);
        println!("{}", serde_json::to_string(&r).unwrap());
        return if r.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }
    if tx.base.is_some() || tx.reason.is_some() || tx.gate.is_some() {
        let r = store.apply(tx);
        println!("{}", agent::tx_text(&r));
        return if r.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE };
    }
    let name = tx.agent.unwrap_or_else(|| "cli".into());
    finish(args, &store, tx.ops, &name)
}

fn edit(args: &Args) -> ExitCode {
    let Some(store) = cwd_store() else { return no_store() };
    let ops = match read_input(args).and_then(|raw| agent::edit_ops(&store, &raw)) {
        Ok(o) => o,
        Err(e) => {
            println!("{e}");
            return ExitCode::FAILURE;
        }
    };
    finish(args, &store, ops, "edit")
}

fn program_cmd(cmd: &str, args: &Args) -> ExitCode {
    let json = args.has("--json");
    let (label, text, loaded) = match args.pos.get(1) {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(src) => (path.clone(), src.clone(), load_src(src)),
            Err(e) => {
                eprintln!("cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        },
        None => match cwd_store() {
            Some(s) => ("HEAD".to_string(), String::new(), s.load_head()),
            None => return no_store(),
        },
    };
    let loaded = match loaded {
        Ok(l) => l,
        Err(d) => return fail_diags(&text, &label, &d, json),
    };
    if cmd == "fmt" {
        let out = print_module(&loaded.module);
        if args.has("--write") && label != "HEAD" {
            if let Err(e) = std::fs::write(&label, &out) {
                eprintln!("cannot write {label}: {e}");
                return ExitCode::FAILURE;
            }
        } else {
            print!("{out}");
        }
        return ExitCode::SUCCESS;
    }
    report(&loaded.src, &label, &loaded.check.diags, json);
    if loaded.check.has_errors() {
        return ExitCode::FAILURE;
    }
    match cmd {
        "check" => {
            if !json {
                println!("ok {} definitions", loaded.module.defs.len());
            }
            ExitCode::SUCCESS
        }
        "verify" => verify::verify(&loaded, &label, json),
        "export-c" => export_c(&loaded, &label, args),
        "hash" => {
            let res = Resolution { user_methods: Some(&loaded.check.user_methods), record_types: Some(&loaded.check.record_types) };
            for (name, h) in hash_module_with(&loaded.module, &res) {
                println!("#{}  {name}", if args.has("--full") { &h } else { &h[..12] });
            }
            ExitCode::SUCCESS
        }
        "native" if args.has("--emit-c") => {
            print!("{}", sspur_native::cgen::c_source(&loaded.module, &loaded.check));
            ExitCode::SUCCESS
        }
        "native" if args.has("--release") => match sspur_native::cgen::compile_release(&loaded.module, &loaded.check, "-O2") {
            Ok(c) => {
                for f in &c.functions {
                    println!("native  {f}");
                }
                for (f, why) in &c.skipped {
                    println!("interp  {f}  ({why})");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("release compilation failed: {e}");
                ExitCode::FAILURE
            }
        },
        "native" => match sspur_native::compile(&loaded.module) {
            Ok(c) => {
                for f in &c.functions {
                    println!("native  {f}");
                }
                for (f, why) in &c.skipped {
                    println!("interp  {f}  ({why})");
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("native compilation failed: {e}");
                ExitCode::FAILURE
            }
        },
        "run" => match native_interp(&loaded, args).run_main() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("runtime error: {e}");
                ExitCode::FAILURE
            }
        },
        "test" => {
            let (text, ok) = agent::tests_text(&native_interp(&loaded, args).run_tests(), args.has("--full"));
            println!("{text}");
            if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        "fuzz" if args.has("--differential") => {
            let opts = Options { cases: args.num("--cases", 300), seed: args.num("--seed", 7) as u64, edge: args.has("--edge") };
            let it = native_interp(&loaded, args);
            let mut bad = 0;
            for r in it.differential(&opts) {
                match r.mismatch {
                    Some((a, i, n)) => {
                        bad += 1;
                        println!("DIFF  {}({}): interpreter {i} | native {n}", r.name, a.join(", "));
                    }
                    None => println!("same  {} ({} cases)", r.name, r.cases),
                }
            }
            if bad == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        "fuzz" => {
            let opts = Options { cases: args.num("--cases", 200), seed: args.num("--seed", 7) as u64, edge: args.has("--edge") };
            let reports = interp(&loaded).fuzz(&opts);
            let mut failed = 0;
            for r in &reports {
                match (&r.skipped, &r.failure) {
                    (Some(why), _) => println!("skip  {} ({why})", r.name),
                    (_, Some((args, msg))) => {
                        failed += 1;
                        println!("FAIL  {}({}): {msg}", r.name, args.join(", "));
                    }
                    _ => println!("ok    {} ({} cases, {} discarded)", r.name, r.cases, r.discarded),
                }
            }
            if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        _ => unreachable!(),
    }
}

fn export_c(l: &Loaded, label: &str, args: &Args) -> ExitCode {
    let stem = std::path::Path::new(label).file_stem().map_or("out".into(), |s| s.to_string_lossy().into_owned());
    let out = args.val("-o").cloned().unwrap_or_else(|| format!("lib{stem}"));
    let out = out.trim_end_matches(".a").trim_end_matches(".so").trim_end_matches(".dylib").to_string();
    let base = std::path::Path::new(&out).file_name().map_or(String::new(), |s| s.to_string_lossy().into_owned());
    let prefix: String = args.val("--prefix").cloned().unwrap_or_else(|| base.strip_prefix("lib").unwrap_or(&base).to_string()).chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let prefix = if prefix.is_empty() || prefix.starts_with(|c: char| c.is_ascii_digit()) { format!("s{prefix}") } else { prefix };
    let ex = match sspur_native::cgen::export_c(&l.module, &l.check, &prefix) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("export failed: {e}");
            return ExitCode::FAILURE;
        }
    };
    let shared = args.has("--shared");
    let lib = if shared { format!("{out}.{}", std::env::consts::DLL_EXTENSION) } else { format!("{out}.a") };
    let header = format!("{out}.h");
    let csrc = format!("{out}.sspur.c");
    let cc = std::env::var("CC").unwrap_or_else(|_| "clang".into());
    let run = |c: &mut std::process::Command| -> Result<(), String> {
        let o = c.output().map_err(|e| format!("cannot run {cc}: {e}"))?;
        if o.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&o.stderr).lines().take(6).collect::<Vec<_>>().join(" | ")) }
    };
    let built = std::fs::write(&csrc, &ex.source).map_err(|e| e.to_string()).and_then(|_| {
        if shared {
            run(std::process::Command::new(&cc).args(["-O2", "-shared", "-fPIC", "-w", "-o", &lib, &csrc]).args(&ex.links))
        } else {
            let obj = format!("{out}.o");
            let r = run(std::process::Command::new(&cc).args(["-O2", "-c", "-fPIC", "-w", "-o", &obj, &csrc])).and_then(|_| {
                let _ = std::fs::remove_file(&lib);
                run(std::process::Command::new("ar").args(["rcs", &lib, &obj]))
            });
            let _ = std::fs::remove_file(&obj);
            r
        }
    });
    if !args.has("--keep-c") {
        let _ = std::fs::remove_file(&csrc);
    }
    if let Err(e) = built.and_then(|_| std::fs::write(&header, &ex.header).map_err(|e| e.to_string())) {
        eprintln!("export failed: {e}");
        return ExitCode::FAILURE;
    }
    for f in &ex.exported {
        println!("export  {f}");
    }
    for (f, why) in &ex.skipped {
        println!("skip    {f}  ({why})");
    }
    let dir = std::path::Path::new(&out).parent().map(|p| p.display().to_string()).filter(|p| !p.is_empty()).unwrap_or_else(|| ".".into());
    let mut link = format!("-L{dir} -l{}", base.strip_prefix("lib").unwrap_or(&base));
    for a in &ex.links {
        link.push(' ');
        link.push_str(a);
    }
    if cfg!(target_os = "linux") {
        link.push_str(" -lpthread -lm");
    }
    println!("wrote {lib} and {header}; link with {link}");
    ExitCode::SUCCESS
}

fn native_interp(l: &Loaded, args: &Args) -> Interp {
    let mut it = interp(l);
    if args.has("--interp") {
        return it;
    }
    if !args.has("--native") {
        let opt = if args.has("--O3") { "-O3" } else { "-O2" };
        match sspur_native::cgen::compile_release(&l.module, &l.check, opt) {
            Ok(c) => {
                c.set_max_depth(1_000_000);
                it.set_native(c);
                return it;
            }
            Err(e) => {
                eprintln!("native build failed, interpreting: {e}");
                return it;
            }
        }
    }
    if args.has("--native") {
        match sspur_native::compile(&l.module) {
            Ok(c) => {
                c.set_max_depth(1_000_000);
                it.set_native(c)
            }
            Err(e) => eprintln!("native compilation failed, interpreting: {e}"),
        }
    }
    it
}

pub fn default_interp(l: &Loaded) -> Interp {
    native_interp(l, &Args { pos: vec![], flags: vec![] })
}

pub fn interp(l: &Loaded) -> Interp {
    let mut it = Interp::new(&l.module, l.check.record_types.clone(), l.check.user_methods.clone(), l.check.gen_loops.clone());
    it.float_sums = l
        .check
        .expr_types
        .iter()
        .filter(|((_, _, tag), t)| matches!(tag, 4 | 5) && matches!(t, sspur_check::Type::Con(n, a) if n == "F64" && a.is_empty()))
        .map(|((s, e, _), _)| (*s, *e))
        .collect();
    it
}

fn fail_diags(src: &str, label: &str, d: &[Diag], json: bool) -> ExitCode {
    report(src, label, d, json);
    ExitCode::FAILURE
}

fn report(src: &str, path: &str, diags: &[Diag], json: bool) {
    if json {
        for d in diags {
            println!("{}", serde_json::to_string(d).unwrap());
        }
        return;
    }
    if path == "HEAD" {
        for l in agent::diag_lines(src, diags, None) {
            eprintln!("{l}");
        }
        return;
    }
    for d in diags {
        let (line, col) = line_col(src, d.span[0]);
        let sev = if d.is_error() { String::new() } else { format!("{} ", d.severity) };
        eprintln!("{path}:{line}:{col} {sev}{} {}", d.code, d.msg);
        if let Some(h) = &d.hint {
            eprintln!("  hint: {h}");
        }
    }
}
