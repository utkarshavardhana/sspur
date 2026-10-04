use serde_json::{json, Value as Json};
use sspur_check::Diag;
use sspur_eval::Interp;
use sspur_store::{Change, Store, Tx, TxResult};
use sspur_syntax::{line_col, printer::print_def};
use std::collections::BTreeSet;

pub fn diag_lines(src: &str, diags: &[Diag], only: Option<&BTreeSet<String>>) -> Vec<String> {
    let defs: Vec<(String, u32)> = sspur_syntax::parse(src).map(|m| m.defs.iter().map(|d| (d.name().to_string(), d.span().start)).collect()).unwrap_or_default();
    let mut out = Vec::new();
    for d in diags {
        if d.severity != "error" && only.is_some_and(|s| !d.def.as_ref().is_some_and(|n| s.contains(n))) {
            continue;
        }
        let off = d.span[0];
        let start = d
            .def
            .as_ref()
            .and_then(|n| defs.iter().find(|(m, _)| m == n))
            .or_else(|| defs.iter().filter(|(_, s)| *s <= off).max_by_key(|(_, s)| *s))
            .filter(|_| !src.is_empty() && d.span != [0, 0]);
        let sev = if d.severity == "error" { String::new() } else { format!("{} ", d.severity) };
        let at = match start {
            Some((name, s)) => {
                let (l, c) = line_col(src, off);
                let (dl, _) = line_col(src, *s);
                format!("{name}:{}:{c} ", l + 1 - dl)
            }
            None => d.def.as_ref().map(|n| format!("{n}: ")).unwrap_or_default(),
        };
        out.push(format!("{sev}{at}{} {}", d.code, d.msg));
        if let Some(h) = &d.hint {
            out.push(format!("  hint: {h}"));
        }
    }
    out
}

pub fn change_text(c: &Change) -> String {
    match (&c.old, &c.new, &c.renamed_from) {
        (_, _, Some(f)) => format!("{f}->{}", c.path),
        (None, Some(_), _) => format!("+{}", c.path),
        (Some(_), None, _) => format!("-{}", c.path),
        _ => format!("~{}", c.path),
    }
}

pub fn tx_text(r: &TxResult) -> String {
    tx_text_for(r, None)
}

fn tx_text_for(r: &TxResult, named: Option<&BTreeSet<String>>) -> String {
    let src = r.src.as_deref().unwrap_or("");
    if !r.ok {
        let mut lines = vec!["rejected, nothing changed".to_string()];
        let errors: Vec<Diag> = r.diags.iter().filter(|d| d.is_error()).cloned().collect();
        lines.extend(diag_lines(src, &errors, None));
        for c in &r.conflicts {
            if let Some(t) = &c.theirs {
                lines.push(format!("theirs {}:", c.theirs_name.as_deref().unwrap_or(&c.path)));
                lines.extend(t.lines().map(|l| format!("  {l}")));
            }
        }
        if !r.conflicts.is_empty()
            && let Some(b) = r.rebase.as_ref().and_then(|b| b["base"].as_str()) {
                lines.push(format!("HEAD is now {b}"));
            }
        return lines.join("\n");
    }
    let shown: Vec<&Change> = r.changes.iter().filter(|c| named.is_none_or(|n| c.old.is_none() || c.new.is_none() || c.renamed_from.is_some() || n.contains(&c.path))).collect();
    let touched: BTreeSet<String> = shown.iter().map(|c| c.path.clone()).collect();
    let mut lines = vec![if shown.is_empty() { "ok, no changes".to_string() } else { format!("ok {}", shown.iter().map(|c| change_text(c)).collect::<Vec<_>>().join(" ")) }];
    lines.extend(diag_lines(src, &r.diags, Some(&touched)));
    lines.join("\n")
}

pub fn tests_text(results: &[(String, Result<(), String>)], full: bool) -> (String, bool) {
    let failed = results.iter().filter(|(_, r)| r.is_err()).count();
    let mut lines = Vec::new();
    for (name, r) in results {
        match r {
            Ok(()) if full => lines.push(format!("pass  {name}")),
            Ok(()) => {}
            Err(e) => lines.push(format!("FAIL  {name}: {e}")),
        }
    }
    lines.push(format!("{} passed, {failed} failed", results.len() - failed));
    (lines.join("\n"), failed == 0)
}

fn directive(line: &str) -> Option<Json> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_');
    match w.as_slice() {
        ["rename", a, b] if ident(a) && ident(b) && !line.starts_with(' ') => Some(json!({"op": "rename", "from": a, "to": b})),
        ["remove", a] if ident(a) && !line.starts_with(' ') => Some(json!({"op": "remove", "path": a})),
        _ => None,
    }
}

pub fn edit_ops(store: &Store, input: &str) -> Result<Vec<Json>, String> {
    let mut renames = Vec::new();
    let mut removes = Vec::new();
    let mut body = String::new();
    for line in input.lines() {
        match directive(line) {
            Some(op) if op["op"] == "rename" => renames.push(op),
            Some(op) => removes.push(op),
            None => body.push_str(line),
        }
        body.push('\n');
    }
    let module = sspur_syntax::parse(&body).map_err(|e| {
        let (l, c) = line_col(&body, e.span.start);
        format!("rejected, nothing changed\ninput:{l}:{c} {} {}", e.code, e.msg)
    })?;
    if module.defs.is_empty() && renames.is_empty() && removes.is_empty() {
        return Err("nothing to edit: give definitions, 'rename OLD NEW', or 'remove NAME'".into());
    }
    let mut names: BTreeSet<String> = store.head_root().map(|r| r.names.keys().cloned().collect()).unwrap_or_default();
    for r in &renames {
        let from = r["from"].as_str().unwrap_or_default();
        if names.remove(from) {
            names.insert(r["to"].as_str().unwrap_or_default().to_string());
        }
    }
    let mut ops = renames;
    for d in &module.defs {
        let kind = if names.insert(d.name().to_string()) { "add" } else { "replace" };
        ops.push(json!({"op": kind, "path": d.name(), "src": print_def(d)}));
    }
    ops.extend(removes);
    Ok(ops)
}

pub fn run_edit(store: &Store, ops: Vec<Json>, agent: &str, test: Option<&dyn Fn(&sspur_store::Loaded) -> Interp>) -> (String, bool) {
    let named: BTreeSet<String> = ops.iter().flat_map(|o| ["path", "target", "to"].map(|k| o.get(k).and_then(Json::as_str).map(String::from))).flatten().collect();
    let r = store.apply(Tx { base: None, agent: Some(agent.into()), reason: None, gate: None, merge: false, ops });
    let mut text = tx_text_for(&r, Some(&named));
    if !r.ok {
        return (text, false);
    }
    let Some(make) = test else { return (text, true) };
    match store.load_head() {
        Ok(l) => {
            let (t, ok) = tests_text(&make(&l).run_tests(), false);
            text.push('\n');
            text.push_str(&t);
            (text, ok)
        }
        Err(d) => (diag_lines("", &d, None).join("\n"), false),
    }
}

pub fn parse_tx(raw: &str) -> Result<Tx, String> {
    let v: Json = serde_json::from_str(raw).map_err(|e| format!("invalid transaction JSON: {e}"))?;
    let v = if v.is_array() { json!({"ops": v}) } else { v };
    serde_json::from_value(v).map_err(|e| format!("invalid transaction: {e}"))
}

fn names(v: &Json) -> String {
    let xs: Vec<&str> = v.as_array().map(|a| a.iter().filter_map(Json::as_str).collect()).unwrap_or_default();
    if xs.is_empty() { "(none)".into() } else { xs.join(" ") }
}

pub fn query_text(q: &str, out: &Json, src: &str) -> String {
    if let Some(e) = out.get("error") {
        let mut s = format!("{} {}", e.as_str().unwrap_or("E_QUERY"), out["msg"].as_str().unwrap_or(""));
        if let Some(c) = out.get("close").filter(|c| c.as_array().is_some_and(|a| !a.is_empty())) {
            s.push_str(&format!(" (did you mean: {})", names(c)));
        }
        if let Some(qs) = out.get("queries") {
            s.push_str(&format!("\nqueries: {}", names(qs)));
        }
        return s;
    }
    if let Some(t) = out.get("text").and_then(Json::as_str) {
        return t.to_string();
    }
    let arr = || out.as_array().cloned().unwrap_or_default();
    match q {
        "list" => {
            let (tests, rest): (Vec<Json>, Vec<Json>) = arr().into_iter().partition(|d| d["kind"] == "test");
            let mut lines: Vec<String> = rest.iter().map(|d| d["sig"].as_str().unwrap_or("").to_string()).collect();
            if !tests.is_empty() {
                lines.push(format!("tests: {}", tests.iter().filter_map(|d| d["name"].as_str()).collect::<Vec<_>>().join(" ")));
            }
            lines.join("\n")
        }
        "sig" => out["sig"].as_str().unwrap_or("").to_string(),
        "body" => out["src"].as_str().unwrap_or("").to_string(),
        "callers" | "callees" | "effects" => names(out),
        "find" => arr().iter().map(|h| format!("{}: {}", h["name"].as_str().unwrap_or(""), h["type"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n"),
        "impact" => format!("dependents: {}\ntests: {}", names(&out["dependents"]), names(&out["tests"])),
        "holes" | "diag" => {
            let diags: Vec<Diag> = arr().iter().map(diag_of_json).collect();
            let lines = diag_lines(src, &diags, None);
            if lines.is_empty() { "(none)".into() } else { lines.join("\n") }
        }
        "log" => arr().iter().map(|r| format!("{} {} {} [{}]", r["at"].as_str().unwrap_or(""), r["agent"].as_str().unwrap_or(""), r["reason"].as_str().unwrap_or(""), names(&r["changes"]))).collect::<Vec<_>>().join("\n"),
        "why" => {
            let p = &out["prov"];
            let mut lines = vec![format!("{} by {} at {}: {}", out["name"].as_str().unwrap_or(""), p["agent"].as_str().unwrap_or("?"), p["at"].as_str().unwrap_or("?"), p["reason"].as_str().unwrap_or(""))];
            if let Some(reqs) = p.get("reqs").filter(|r| r.as_array().is_some_and(|a| !a.is_empty())) {
                lines.push(format!("reqs: {}", names(reqs)));
            }
            for h in out["history"].as_array().into_iter().flatten() {
                lines.push(format!("  {} {} {}", h["at"].as_str().unwrap_or(""), h["agent"].as_str().unwrap_or(""), h["reason"].as_str().unwrap_or("")));
            }
            lines.join("\n")
        }
        _ => out.to_string(),
    }
}

fn diag_of_json(d: &Json) -> Diag {
    let severity = match d["severity"].as_str() {
        Some("error") => "error",
        Some("warning") => "warning",
        Some("hole") => "hole",
        _ => "note",
    };
    let span = [d["span"][0].as_u64().unwrap_or(0) as u32, d["span"][1].as_u64().unwrap_or(0) as u32];
    let s = |k: &str| d[k].as_str().map(String::from);
    Diag { code: s("code").unwrap_or_default(), severity, def: s("def"), span, msg: s("msg").unwrap_or_default(), hint: s("hint"), fix: vec![] }
}
