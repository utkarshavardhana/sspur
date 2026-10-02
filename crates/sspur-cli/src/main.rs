use sspur_check::{check, syntax_diag, Diag};
use sspur_syntax::{line_col, parse, print_module, Module};
use std::process::ExitCode;

const USAGE: &str = "usage: sspur <check|run|test|hash|fmt> <file.ssp> [--json] [--write] [--full]";

fn main() -> ExitCode {
    std::thread::Builder::new().stack_size(1 << 29).spawn(real_main).unwrap().join().unwrap()
}

fn real_main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let write = args.iter().any(|a| a == "--write");
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let (Some(cmd), Some(path)) = (pos.first(), pos.get(1)) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let module = match parse(&src) {
        Ok(m) => m,
        Err(e) => {
            report(&src, path, &[syntax_diag(&e)], json);
            return ExitCode::FAILURE;
        }
    };
    match cmd.as_str() {
        "fmt" => fmt(&module, path, write),
        "check" => checked(&src, path, &module, json).map_or(ExitCode::FAILURE, |_| {
            if !json {
                println!("ok: {} definitions", module.defs.len());
            }
            ExitCode::SUCCESS
        }),
        "hash" => {
            let Some(out) = checked(&src, path, &module, json) else { return ExitCode::FAILURE };
            let res = sspur_hash::Resolution { user_methods: Some(&out.user_methods), record_types: Some(&out.record_types) };
            let full = args.iter().any(|a| a == "--full");
            for (name, h) in sspur_hash::hash_module_with(&module, &res) {
                println!("#{}  {name}", if full { &h } else { &h[..12] });
            }
            ExitCode::SUCCESS
        }
        "run" | "test" => {
            let Some(out) = checked(&src, path, &module, json) else { return ExitCode::FAILURE };
            let interp = sspur_eval::Interp::new(&module, out.record_types, out.user_methods);
            if cmd.as_str() == "run" {
                match interp.run_main() {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(e) => {
                        eprintln!("runtime error: {e}");
                        ExitCode::FAILURE
                    }
                }
            } else {
                let results = interp.run_tests();
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
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn fmt(module: &Module, path: &str, write: bool) -> ExitCode {
    let out = print_module(module);
    if write {
        if let Err(e) = std::fs::write(path, &out) {
            eprintln!("cannot write {path}: {e}");
            return ExitCode::FAILURE;
        }
    } else {
        print!("{out}");
    }
    ExitCode::SUCCESS
}

fn checked(src: &str, path: &str, module: &Module, json: bool) -> Option<sspur_check::CheckOutput> {
    let out = check(module);
    report(src, path, &out.diags, json);
    if out.has_errors() { None } else { Some(out) }
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
