use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

const USAGE: &str = "usage (nothing here calls AWS):
  sspur deploy plan <file.ssp> [--out DIR]          write template.json, iam/*.json, plan.json, bootstrap.c, build.sh, deploy.sh, rollback.sh, backfill.sh and a host build to DIR (default deploy/)
  sspur deploy local <file.ssp> [--port N] [--record FILE]   serve on 127.0.0.1 with emulated Lambda and DynamoDB; --record appends requests, responses and db calls as JSON lines
  sspur deploy migrate <old.ssp> <new.ssp> [--out DIR] [--json]   classify store schema changes, check migrate_/unmigrate_ functions, write migrate.json and backfill.sh
  sspur deploy replay <recording> <new.ssp> [--strict] [--json]   re-run a recording against new.ssp and report behavior differences (exit 1 if any)
  sspur deploy swap <new.ssp> [--weight P] [--port N]   hot swap a running deploy local to new.ssp (P% canary, default 100)
  sspur deploy promote|rollback|status [--port N]   promote the canary, roll back to the previous version, or show versions
  sspur deploy backfill [--store S] [--port N]       migrate every stored item of the live version's migrated stores in the emulated table";

fn val<'a>(flags: &'a [String], f: &str) -> Option<&'a String> {
    flags.iter().position(|x| x == f).and_then(|i| flags.get(i + 1))
}

fn read(file: &str) -> Result<String, ExitCode> {
    std::fs::read_to_string(file).map_err(|e| {
        eprintln!("cannot read {file}: {e}");
        ExitCode::from(2)
    })
}

fn service(file: &str) -> Result<sspur_deploy::Service, ExitCode> {
    let src = read(file)?;
    sspur_deploy::analyze(&src).map_err(|e| {
        eprintln!("{file}: {e}");
        ExitCode::FAILURE
    })
}

fn built(svc: &sspur_deploy::Service, file: &str) -> Result<(sspur_deploy::Plan, PathBuf), ExitCode> {
    let plan = sspur_deploy::plan(svc).map_err(|e| {
        eprintln!("{file}: {e}");
        ExitCode::FAILURE
    })?;
    let bin = sspur_deploy::build_host(&plan.files["bootstrap.c"], &sspur_deploy::cache_dir()).map_err(|e| {
        eprintln!("{e}");
        ExitCode::FAILURE
    })?;
    Ok((plan, bin))
}

fn control(flags: &[String], method: &str, cmd: &str, body: Value) -> ExitCode {
    let port: u16 = val(flags, "--port").and_then(|p| p.parse().ok()).unwrap_or(8080);
    match sspur_deploy::local::request(port, method, &format!("/_sspur/{cmd}"), Some(&body.to_string())) {
        Ok((200, b)) => {
            println!("{b}");
            ExitCode::SUCCESS
        }
        Ok((s, b)) => {
            let v: Value = serde_json::from_str(&b).unwrap_or(json!({"error": b}));
            eprintln!("{cmd} failed ({s}): {}", v["error"].as_str().unwrap_or(&b));
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("no deploy local on 127.0.0.1:{port}: {e}");
            ExitCode::FAILURE
        }
    }
}

pub fn run(pos: &[String], flags: &[String]) -> ExitCode {
    match run_inner(pos, flags) {
        Ok(c) | Err(c) => c,
    }
}

fn run_inner(pos: &[String], flags: &[String]) -> Result<ExitCode, ExitCode> {
    let sub = pos.first().map(String::as_str).unwrap_or("");
    let file = pos.get(1).map(String::as_str);
    let json_out = flags.iter().any(|f| f == "--json");
    match (sub, file) {
        ("plan", Some(file)) => {
            let svc = service(file)?;
            let plan = sspur_deploy::plan(&svc).map_err(|e| {
                eprintln!("{file}: {e}");
                ExitCode::FAILURE
            })?;
            let dir = PathBuf::from(val(flags, "--out").map_or("deploy", String::as_str));
            if let Err(e) = sspur_deploy::write_plan(&plan, &dir) {
                eprintln!("{e}");
                return Ok(ExitCode::FAILURE);
            }
            let host = match sspur_deploy::build_host(&plan.files["bootstrap.c"], &sspur_deploy::cache_dir()) {
                Ok(bin) => {
                    let dst = dir.join("local/bootstrap");
                    let _ = std::fs::create_dir_all(dir.join("local"));
                    match std::fs::copy(&bin, &dst) {
                        Ok(_) => format!("local/bootstrap ({} build)", std::env::consts::OS),
                        Err(e) => format!("host build not copied: {e}"),
                    }
                }
                Err(e) => format!("host build skipped: {}", e.lines().next().unwrap_or("")),
            };
            println!("svc {} #{} -> {}", svc.name, svc.hash, dir.display());
            for h in svc.handlers.iter().chain(&svc.backfills) {
                let acts: Vec<String> = sspur_deploy::iam::actions(h).into_iter().flat_map(|(s, a)| a.into_iter().map(move |x| format!("{x}[{s}]"))).collect();
                let routes: Vec<String> = svc.routes.iter().filter(|r| r.handler == h.name).map(|r| r.key()).collect();
                println!("  {:<24} {:<10} {}", if routes.is_empty() { "(invoked)".to_string() } else { routes.join(", ") }, h.name, if acts.is_empty() { "-".to_string() } else { acts.join(" ") });
            }
            println!("  files: {} | {host}", plan.files.keys().cloned().collect::<Vec<_>>().join(" "));
            println!("  nothing was deployed; build.sh then deploy.sh ship it with your AWS credentials");
            Ok(ExitCode::SUCCESS)
        }
        ("local", Some(file)) => {
            let svc = service(file)?;
            let (_, bin) = built(&svc, file)?;
            let port: u16 = val(flags, "--port").and_then(|p| p.parse().ok()).unwrap_or(8080);
            let opts = sspur_deploy::local::Options { record: val(flags, "--record").map(PathBuf::from) };
            let local = sspur_deploy::local::start_with(&svc, &bin, port, Arc::new(|l: &str| eprintln!("{l}")), opts).map_err(|e| {
                eprintln!("{e}");
                ExitCode::FAILURE
            })?;
            println!("listening on http://127.0.0.1:{}", local.port);
            println!("  #{} live; control: sspur deploy swap|promote|rollback|backfill|status --port {}", svc.hash, local.port);
            for r in &svc.routes {
                println!("  {} -> {}", r.key(), r.handler);
            }
            use std::io::Write;
            let _ = std::io::stdout().flush();
            loop {
                std::thread::park();
            }
        }
        ("migrate", Some(old)) => {
            let Some(new) = pos.get(2) else {
                eprintln!("{USAGE}");
                return Ok(ExitCode::from(2));
            };
            let (a, b) = (service(old)?, service(new)?);
            let rep = sspur_deploy::migrate::compare(&a.stores, &b.stores);
            let plan = sspur_deploy::migrate::plan_json(&a, &b, &rep);
            if let Some(dir) = val(flags, "--out") {
                let dir = PathBuf::from(dir);
                let mut files = std::collections::BTreeMap::new();
                files.insert("migrate.json".to_string(), format!("{}\n", serde_json::to_string_pretty(&plan).unwrap()));
                if !b.backfills.is_empty() {
                    files.insert("backfill.sh".to_string(), sspur_deploy::cfn::backfill_script(&b));
                }
                files.insert("rollback.sh".to_string(), sspur_deploy::cfn::rollback_script(&b));
                if let Err(e) = sspur_deploy::write_plan(&sspur_deploy::Plan { files }, &dir) {
                    eprintln!("{e}");
                    return Ok(ExitCode::FAILURE);
                }
            }
            if json_out {
                println!("{}", serde_json::to_string_pretty(&plan).unwrap());
            } else {
                println!("migrate svc {} #{} -> #{}", b.name, a.hash, b.hash);
                print!("{}", rep.text());
                for f in &b.backfills {
                    println!("  backfill: {} (dynamodb:Scan, dynamodb:PutItem on {}); reads migrate lazily until it has run", f.name, f.name.trim_start_matches("backfill_"));
                }
                println!(
                    "  {}",
                    if !rep.ok() {
                        "breaking: deploy refused until the errors are fixed"
                    } else if rep.side_by_side() {
                        "ok: old and new versions can run side by side"
                    } else {
                        "ok, but all at once only: the old version cannot read what the new one writes"
                    }
                );
            }
            Ok(if rep.ok() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        ("replay", Some(rec)) => {
            let Some(new) = pos.get(2) else {
                eprintln!("{USAGE}");
                return Ok(ExitCode::from(2));
            };
            let recording = read(rec)?;
            let svc = service(new)?;
            let (_, bin) = built(&svc, new)?;
            let rep = sspur_deploy::replay::replay(&recording, &svc, &bin).map_err(|e| {
                eprintln!("{e}");
                ExitCode::FAILURE
            })?;
            let strict = flags.iter().any(|f| f == "--strict");
            if json_out {
                let diffs: Vec<Value> = rep.diffs.iter().map(|d| json!({"id": d.id, "request": d.request, "what": d.what, "recorded": d.old, "new": d.new})).collect();
                println!("{}", json!({"total": rep.total, "same": rep.same, "extended": rep.extended, "diffs": diffs, "passed": rep.passed(strict)}));
            } else {
                print!("{}", rep.text());
            }
            Ok(if rep.passed(strict) { ExitCode::SUCCESS } else { ExitCode::FAILURE })
        }
        ("swap", Some(file)) => {
            let path = std::fs::canonicalize(file).map_err(|e| {
                eprintln!("cannot read {file}: {e}");
                ExitCode::from(2)
            })?;
            let weight: u64 = val(flags, "--weight").and_then(|w| w.parse().ok()).unwrap_or(100);
            Ok(control(flags, "POST", "swap", json!({"file": path, "weight": weight})))
        }
        ("promote" | "rollback" | "backfill", _) => Ok(control(flags, "POST", sub, json!({"store": val(flags, "--store")}))),
        ("status", _) => Ok(control(flags, "GET", "status", json!({}))),
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(2))
        }
    }
}
