mod mcp;

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

const USAGE: &str = "usage:
  sspur init [file.ssp]                 create a codebase in .sspur/ (optionally import a file)
  sspur apply [tx.json|-]               apply a transaction of ops
  sspur q <query> [target] [--budget N] query the codebase (list sig body callers callees effects find pack why impact holes diag log)
  sspur log | export | spec | mcp
  sspur check|run|test|fuzz|hash|fmt [file.ssp] [--json] [--cases N] [--seed N] [--edge] [--write] [--full]";

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

    fn num(&self, f: &str, default: usize) -> usize {
        self.flags.iter().position(|x| x == f).and_then(|i| self.flags.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(default)
    }
}

fn parse_args() -> Args {
    let mut pos = Vec::new();
    let mut flags = Vec::new();
    let mut it = std::env::args().skip(1).peekable();
    while let Some(a) = it.next() {
        if a.starts_with("--") {
            let takes = matches!(a.as_str(), "--budget" | "--cases" | "--seed");
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
            print!("{REFERENCE}");
            ExitCode::SUCCESS
        }
        "init" => init(args.pos.get(1)),
        "apply" => apply(args.pos.get(1)),
        "q" => {
            let Some(store) = cwd_store() else { return no_store() };
            let loaded = match store.load_head() {
                Ok(l) => l,
                Err(d) => return fail_diags("", "HEAD", &d, true),
            };
            let q = args.pos.get(1).map_or("list", String::as_str);
            let out = Ctx::new(&loaded, Some(&store)).run(q, args.pos.get(2).map(String::as_str), args.num("--budget", 2000));
            if let Some(text) = out.get("text").and_then(Json::as_str).filter(|_| !args.has("--json")) {
                println!("{text}");
                eprintln!("-- {} tokens, included {}, omitted {}", out["est_tokens"], out["included"], out["omitted"]);
            } else {
                println!("{}", serde_json::to_string_pretty(&out).unwrap());
            }
            if out.get("error").is_some() { ExitCode::FAILURE } else { ExitCode::SUCCESS }
        }
        "log" => {
            let Some(store) = cwd_store() else { return no_store() };
            for r in store.log() {
                let changes: Vec<String> = r
                    .changes
                    .iter()
                    .map(|c| match (&c.old, &c.new, &c.renamed_from) {
                        (_, _, Some(f)) => format!("{f}->{}", c.path),
                        (None, Some(_), _) => format!("+{}", c.path),
                        (Some(_), None, _) => format!("-{}", c.path),
                        _ => format!("~{}", c.path),
                    })
                    .collect();
                println!("#{}  {}  {}  {}  [{}]", &r.hash[..12], r.at, r.agent, r.reason, changes.join(" "));
            }
            ExitCode::SUCCESS
        }
        "export" => {
            let Some(store) = cwd_store() else { return no_store() };
            print!("{}", store.head_root().map(|r| store.root_src(&r)).unwrap_or_default());
            ExitCode::SUCCESS
        }
        "mcp" => mcp::serve(),
        "check" | "run" | "test" | "fuzz" | "hash" | "fmt" => program_cmd(&cmd, &args),
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

fn init(file: Option<&String>) -> ExitCode {
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
    let r = store.apply(Tx { base: None, agent: Some("import".into()), reason: Some(format!("import {path}")), gate: None, ops });
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
    if r.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn apply(file: Option<&String>) -> ExitCode {
    let Some(store) = cwd_store() else { return no_store() };
    let mut raw = String::new();
    let read = match file.map(String::as_str) {
        None | Some("-") => std::io::stdin().read_to_string(&mut raw).map(|_| ()),
        Some(p) => std::fs::read_to_string(p).map(|s| raw = s),
    };
    if let Err(e) = read {
        eprintln!("cannot read transaction: {e}");
        return ExitCode::FAILURE;
    }
    let tx: Tx = match serde_json::from_str(&raw) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("invalid transaction JSON: {e}");
            return ExitCode::FAILURE;
        }
    };
    let r = store.apply(tx);
    println!("{}", serde_json::to_string_pretty(&r).unwrap());
    if r.ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn program_cmd(cmd: &str, args: &Args) -> ExitCode {
    let json = args.has("--json");
    let (label, loaded) = match args.pos.get(1) {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(src) => (path.clone(), load_src(src)),
            Err(e) => {
                eprintln!("cannot read {path}: {e}");
                return ExitCode::from(2);
            }
        },
        None => match cwd_store() {
            Some(s) => ("HEAD".to_string(), s.load_head()),
            None => return no_store(),
        },
    };
    let loaded = match loaded {
        Ok(l) => l,
        Err(d) => return fail_diags("", &label, &d, json),
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
                println!("ok: {} definitions", loaded.module.defs.len());
            }
            ExitCode::SUCCESS
        }
        "hash" => {
            let res = Resolution { user_methods: Some(&loaded.check.user_methods), record_types: Some(&loaded.check.record_types) };
            for (name, h) in hash_module_with(&loaded.module, &res) {
                println!("#{}  {name}", if args.has("--full") { &h } else { &h[..12] });
            }
            ExitCode::SUCCESS
        }
        "run" => match interp(&loaded).run_main() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("runtime error: {e}");
                ExitCode::FAILURE
            }
        },
        "test" => {
            let results = interp(&loaded).run_tests();
            let failed = results.iter().filter(|(_, r)| r.is_err()).count();
            for (name, r) in &results {
                match r {
                    Ok(()) => println!("pass  {name}"),
                    Err(e) => println!("FAIL  {name}: {e}"),
                }
            }
            println!("{} passed, {failed} failed", results.len() - failed);
            if failed == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
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

pub fn interp(l: &Loaded) -> Interp {
    Interp::new(&l.module, l.check.record_types.clone(), l.check.user_methods.clone())
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
    for d in diags {
        let (line, col) = line_col(src, d.span[0]);
        let def = d.def.as_deref().map(|n| format!(" in {n}")).unwrap_or_default();
        eprintln!("{path}:{line}:{col}: {} {}{def}: {}", d.severity, d.code, d.msg);
        if let Some(h) = &d.hint {
            eprintln!("  hint: {h}");
        }
    }
}
