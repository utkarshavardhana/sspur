use crate::{agent, default_interp, interp, AGENT_SPEC, REFERENCE};
use serde_json::{json, Value as Json};
use sspur_eval::fuzz::Options;
use sspur_store::{query::Ctx, Store, Tx};
use std::io::{BufRead, Write};
use std::process::ExitCode;

const PROTOCOL: &str = "2025-06-18";

const INSTRUCTIONS: &str = "SSPUR is an AI-native language stored as a content-addressed codebase. Call sspur_spec once to learn the language and sspur_export to read the code. Change code with sspur_edit (definitions replace same-named ones; atomic and typechecked; test: true also runs the tests).";

fn tools() -> Json {
    json!([
        {"name": "sspur_spec", "description": "Return the compact SSPUR language reference. Read this first. full: true returns the long reference.", "inputSchema": {"type": "object", "properties": {"full": {"type": "boolean"}}}},
        {"name": "sspur_edit", "description": "Replace or add definitions by name. src holds SSP-T definitions, optionally preceded by lines 'rename OLD NEW' or 'remove NAME'. Atomic: rejected with diagnostics if the result doesn't typecheck. test: true also runs all tests.", "inputSchema": {"type": "object", "properties": {"src": {"type": "string"}, "test": {"type": "boolean"}}, "required": ["src"]}},
        {"name": "sspur_query", "description": "Query the codebase. Queries: list, sig, body, callers, callees, effects, find (type shape like 'List[Int] -> Int'), pack (minimal edit context within a token budget), why, impact, holes, diag, log.", "inputSchema": {"type": "object", "properties": {"query": {"type": "string"}, "target": {"type": "string"}, "budget": {"type": "integer"}, "json": {"type": "boolean"}}, "required": ["query"]}},
        {"name": "sspur_apply", "description": "Apply a transaction of ops (add, replace, rename, remove, refine, fill, attach). The transaction applies completely or not at all; it's rejected with diagnostics (including fix ops) if the result doesn't typecheck.", "inputSchema": {"type": "object", "properties": {"ops": {"type": "array", "items": {"type": "object"}}, "reason": {"type": "string"}, "agent": {"type": "string"}, "base": {"type": "string"}, "gate": {"type": "string", "enum": ["check", "tests"], "description": "'tests' rejects the transaction unless all tests pass"}, "test": {"type": "boolean", "description": "run all tests after applying"}, "json": {"type": "boolean"}}, "required": ["ops"]}},
        {"name": "sspur_check", "description": "Typecheck the codebase and return all diagnostics.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "sspur_test", "description": "Run all tests in the codebase.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "sspur_run", "description": "Run main() and return its log output.", "inputSchema": {"type": "object", "properties": {}}},
        {"name": "sspur_fuzz", "description": "Property-test every function from its contracts (pre, post, where). Returns shrunk counterexamples.", "inputSchema": {"type": "object", "properties": {"cases": {"type": "integer"}}}},
        {"name": "sspur_export", "description": "Return the whole codebase as SSP-T source.", "inputSchema": {"type": "object", "properties": {}}}
    ])
}

fn call(store: &Store, name: &str, a: &Json) -> (String, bool) {
    let flag = |k: &str| a.get(k).and_then(Json::as_bool).unwrap_or(false);
    if name == "sspur_spec" {
        return ((if flag("full") { REFERENCE } else { AGENT_SPEC }).to_string(), false);
    }
    let make = |l: &sspur_store::Loaded| default_interp(l);
    let test: Option<&dyn Fn(&sspur_store::Loaded) -> sspur_eval::Interp> = if flag("test") { Some(&make) } else { None };
    if name == "sspur_edit" {
        return match agent::edit_ops(store, a.get("src").and_then(Json::as_str).unwrap_or("")) {
            Ok(ops) => {
                let (t, ok) = agent::run_edit(store, ops, "mcp", test);
                (t, !ok)
            }
            Err(e) => (e, true),
        };
    }
    if name == "sspur_apply" && !flag("json") && a.get("gate").is_none() && a.get("base").is_none() {
        let ops = a.get("ops").and_then(Json::as_array).cloned().unwrap_or_default();
        let (t, ok) = agent::run_edit(store, ops, a.get("agent").and_then(Json::as_str).unwrap_or("mcp"), test);
        return (t, !ok);
    }
    if name == "sspur_apply" {
        let tx = Tx {
            base: a.get("base").and_then(Json::as_str).map(String::from),
            agent: Some(a.get("agent").and_then(Json::as_str).unwrap_or("mcp").to_string()),
            reason: a.get("reason").and_then(Json::as_str).map(String::from),
            gate: a.get("gate").and_then(Json::as_str).map(String::from),
            ops: a.get("ops").and_then(Json::as_array).cloned().unwrap_or_default(),
        };
        let r = store.apply(tx);
        return (serde_json::to_string(&r).unwrap(), !r.ok);
    }
    let loaded = match store.load_head() {
        Ok(l) => l,
        Err(d) => return (serde_json::to_string(&d).unwrap(), true),
    };
    match name {
        "sspur_query" => {
            let q = a.get("query").and_then(Json::as_str).unwrap_or("list");
            let budget = a.get("budget").and_then(Json::as_u64).unwrap_or(2000) as usize;
            let out = Ctx::new(&loaded, Some(store)).run(q, a.get("target").and_then(Json::as_str), budget);
            let err = out.get("error").is_some();
            if flag("json") { (serde_json::to_string(&out).unwrap(), err) } else { (agent::query_text(q, &out, &loaded.src), err) }
        }
        "sspur_check" => {
            let lines = agent::diag_lines(&loaded.src, &loaded.check.diags, None);
            let head = if loaded.check.has_errors() { "errors".to_string() } else { format!("ok {} definitions", loaded.module.defs.len()) };
            (std::iter::once(head).chain(lines).collect::<Vec<_>>().join("\n"), false)
        }
        "sspur_export" => (loaded.src.clone(), false),
        _ if loaded.check.has_errors() => (agent::diag_lines(&loaded.src, &loaded.check.diags, None).join("\n"), true),
        "sspur_test" => {
            let (t, ok) = agent::tests_text(&default_interp(&loaded).run_tests(), false);
            (t, !ok)
        }
        "sspur_run" => {
            let it = default_interp(&loaded);
            *it.output.borrow_mut() = Some(vec![]);
            let r = it.run_main();
            let out = it.output.borrow_mut().take().unwrap_or_default();
            (serde_json::to_string(&json!({"ok": r.is_ok(), "output": out, "error": r.err()})).unwrap(), false)
        }
        "sspur_fuzz" => {
            let cases = a.get("cases").and_then(Json::as_u64).unwrap_or(200) as usize;
            let reports: Vec<Json> = interp(&loaded)
                .fuzz(&Options { cases, seed: 7, edge: false })
                .into_iter()
                .map(|r| json!({"fn": r.name, "cases": r.cases, "skipped": r.skipped, "counterexample": r.failure.map(|(args, msg)| json!({"args": args, "error": msg}))}))
                .collect();
            (serde_json::to_string(&reports).unwrap(), false)
        }
        _ => (format!("unknown tool '{name}'"), true),
    }
}

pub fn serve() -> ExitCode {
    let dir = std::env::current_dir().unwrap();
    let store = match Store::find(&dir) {
        Some(s) => s,
        None => match Store::init(&dir) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cannot create .sspur: {e}");
                return ExitCode::FAILURE;
            }
        },
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Json = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                let resp = json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}});
                let _ = writeln!(stdout, "{resp}");
                continue;
            }
        };
        let Some(id) = msg.get("id").cloned() else { continue };
        let method = msg.get("method").and_then(Json::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": params.get("protocolVersion").and_then(Json::as_str).unwrap_or(PROTOCOL),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "sspur", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => {
                let name = params.get("name").and_then(Json::as_str).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                let (text, is_error) = call(&store, name, &args);
                Ok(json!({"content": [{"type": "text", "text": text}], "isError": is_error}))
            }
            m => Err(json!({"code": -32601, "message": format!("method not found: {m}")})),
        };
        let resp = match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        };
        if writeln!(stdout, "{resp}").is_err() {
            break;
        }
        let _ = stdout.flush();
    }
    ExitCode::SUCCESS
}
