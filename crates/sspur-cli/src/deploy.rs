use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

const USAGE: &str = "usage:
  sspur deploy plan <file.ssp> [--out DIR]   write template.json, iam/*.json, plan.json, bootstrap.c, build.sh, deploy.sh and a host build to DIR (default deploy/)
  sspur deploy local <file.ssp> [--port N]  run the service on 127.0.0.1 with emulated Lambda and DynamoDB (no AWS calls)";

pub fn run(sub: Option<&str>, file: Option<&String>, out: Option<&String>, port: Option<&String>) -> ExitCode {
    let (Some(sub), Some(file)) = (sub, file) else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {file}: {e}");
            return ExitCode::from(2);
        }
    };
    let svc = match sspur_deploy::analyze(&src) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{file}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let plan = match sspur_deploy::plan(&svc) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{file}: {e}");
            return ExitCode::FAILURE;
        }
    };
    match sub {
        "plan" => {
            let dir = PathBuf::from(out.map_or("deploy", String::as_str));
            if let Err(e) = sspur_deploy::write_plan(&plan, &dir) {
                eprintln!("{e}");
                return ExitCode::FAILURE;
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
            for h in &svc.handlers {
                let acts: Vec<String> = sspur_deploy::iam::actions(h).into_iter().flat_map(|(s, a)| a.into_iter().map(move |x| format!("{x}[{s}]"))).collect();
                let routes: Vec<String> = svc.routes.iter().filter(|r| r.handler == h.name).map(|r| r.key()).collect();
                println!("  {:<24} {:<10} {}", routes.join(", "), h.name, if acts.is_empty() { "-".to_string() } else { acts.join(" ") });
            }
            println!("  files: {} | {host}", plan.files.keys().cloned().collect::<Vec<_>>().join(" "));
            println!("  nothing was deployed; build.sh then deploy.sh ship it with your AWS credentials");
            ExitCode::SUCCESS
        }
        "local" => {
            let bin = match sspur_deploy::build_host(&plan.files["bootstrap.c"], &sspur_deploy::cache_dir()) {
                Ok(b) => b,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            let port: u16 = port.and_then(|p| p.parse().ok()).unwrap_or(8080);
            let local = match sspur_deploy::local::start(&svc, &bin, port, Arc::new(|l: &str| eprintln!("{l}"))) {
                Ok(l) => l,
                Err(e) => {
                    eprintln!("{e}");
                    return ExitCode::FAILURE;
                }
            };
            println!("listening on http://127.0.0.1:{}", local.port);
            for r in &svc.routes {
                println!("  {} -> {}", r.key(), r.handler);
            }
            use std::io::Write;
            let _ = std::io::stdout().flush();
            loop {
                std::thread::park();
            }
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
