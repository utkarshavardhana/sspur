use serde_json::{json, Value as Json};
use sspur_store::sync::{self, Remote};
use sspur_store::{Store, Tx};
use std::process::ExitCode;

fn show(r: &Json, json: bool, what: &str) -> ExitCode {
    let ok = r["ok"].as_bool().unwrap_or(false);
    if json {
        println!("{r}");
    } else if ok {
        let changes: Vec<String> = r["changes"].as_array().into_iter().flatten().map(|c| c["path"].as_str().unwrap_or("").to_string()).collect();
        let root = r["root"].as_str().map_or("(empty)".to_string(), |h| format!("#{}", &h[..12.min(h.len())]));
        if r["up_to_date"].as_bool() == Some(true) {
            println!("ok, up to date");
        } else {
            println!("ok {what}, HEAD {root}{}", if changes.is_empty() { String::new() } else { format!(" [{}]", changes.join(" ")) });
        }
    } else {
        println!("{what} not merged, HEAD unchanged");
        for d in r["diags"].as_array().into_iter().flatten() {
            println!("{} {}", d["code"].as_str().unwrap_or(""), d["msg"].as_str().unwrap_or(""));
            if let Some(h) = d["hint"].as_str() {
                println!("  hint: {h}");
            }
        }
        for c in r["conflicts"].as_array().into_iter().flatten() {
            println!("conflict {} ({}) with {} commit {}", c["path"].as_str().unwrap_or(""), c["kind"].as_str().unwrap_or(""), c["agent"].as_str().unwrap_or("?"), c["commit"].as_str().unwrap_or(""));
            for (side, k) in [("ours", "ours"), ("theirs", "theirs")] {
                if let Some(t) = c[k].as_str() {
                    println!("  {side}:");
                    for l in t.lines() {
                        println!("    {l}");
                    }
                }
            }
        }
        if let Some(e) = r["error"].as_str() {
            println!("{e}");
        }
    }
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

pub fn run(pos: &[String], port: Option<&str>, max: Option<usize>, agent: &str, json: bool) -> ExitCode {
    let Some(store) = std::env::current_dir().ok().and_then(|d| Store::find(&d)) else {
        eprintln!("no .sspur codebase here; run 'sspur init'");
        return ExitCode::FAILURE;
    };
    let remote = |i: usize| -> Result<Remote, String> { Remote::parse(pos.get(i).ok_or("give a remote: a directory or tcp://host:port")?) };
    match pos.first().map(String::as_str) {
        Some("serve") => {
            let addr = format!("127.0.0.1:{}", port.unwrap_or("7411"));
            match sync::serve(&store, &addr, max, |a| {
                println!("serving {} on tcp://{a}", store.path().display());
            }) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some(cmd @ ("push" | "pull")) => {
            let rep = remote(1).and_then(|r| if cmd == "push" { sync::push(&store, &r, agent) } else { sync::pull(&store, &r, agent) });
            match rep {
                Ok(rep) => show(&rep.result, json, &if cmd == "push" { format!("pushed {} commits", rep.sent) } else { format!("pulled {} commits", rep.received) }),
                Err(e) => {
                    println!("{e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("status") => {
            let ix = store.index().unwrap_or_default();
            let pending = store.pending();
            println!("HEAD {}", store.head().unwrap_or_else(|| "(empty)".into()));
            println!("heads {}  commits {}  definitions {}", ix.heads.join(" "), store.ancestors(&ix.heads).len(), store.head_root().map_or(0, |r| r.names.len()));
            if !pending.is_empty() {
                println!("pending merge of {}; settle it with 'sspur sync resolve ours|theirs'", pending.join(" "));
            }
            ExitCode::SUCCESS
        }
        Some("resolve") => {
            let pick = pos.get(1).map_or("ours", String::as_str);
            if !matches!(pick, "ours" | "theirs") {
                eprintln!("usage: sspur sync resolve ours|theirs [PATH...]");
                return ExitCode::from(2);
            }
            let pending = store.pending();
            if pending.is_empty() {
                println!("nothing to resolve");
                return ExitCode::SUCCESS;
            }
            let paths: Vec<String> = if pos.len() > 2 { pos[2..].to_vec() } else { store.merge_remote(&pending, agent, true).conflicts.into_iter().map(|c| c.path).collect() };
            let ops: Vec<Json> = paths.iter().map(|p| json!({"op": "resolve", "path": p, "pick": pick})).collect();
            let r = store.apply(Tx { merge: true, ..Tx::new(agent, ops) });
            show(&serde_json::to_value(&r).unwrap(), json, "merge")
        }
        _ => {
            eprintln!("usage: sspur sync serve [--port N] [--max N] | push REMOTE | pull REMOTE | status | resolve ours|theirs [PATH...]");
            ExitCode::from(2)
        }
    }
}
