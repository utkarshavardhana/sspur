use crate::Diag;
use sspur_syntax::visit::walk_expr;
use sspur_syntax::*;
use std::collections::{BTreeSet, HashMap, HashSet};

fn access<'a>(e: &'a Expr, statics: &HashSet<String>) -> Option<(&'a str, &'a str, &'a [Expr])> {
    match &e.kind {
        ExprKind::Method { recv, name, args, .. } => match &recv.kind {
            ExprKind::Name(n) if statics.contains(n) => Some((n.as_str(), name.as_str(), args.as_slice())),
            _ => None,
        },
        ExprKind::Field(recv, name) => match &recv.kind {
            ExprKind::Name(n) if statics.contains(n) => Some((n.as_str(), name.as_str(), &[])),
            _ => None,
        },
        _ => None,
    }
}

fn bodies(f: &FnDef) -> impl Iterator<Item = &Expr> {
    f.pres.iter().chain(&f.posts).chain([&f.body])
}

fn reach(roots: impl Iterator<Item = String>, calls: &HashMap<String, BTreeSet<String>>) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut stack: Vec<String> = roots.collect();
    while let Some(f) = stack.pop() {
        if seen.insert(f.clone()) {
            stack.extend(calls.get(&f).into_iter().flatten().cloned());
        }
    }
    seen
}

/// Code reachable from main can be preempted by handlers, so a static shared with them is updated
/// with `add`, `swap` or `cas` there: `x.store(.. x.load ..)` is `E_STATIC_RACE`.
pub fn check(m: &Module) -> Vec<Diag> {
    let statics: HashSet<String> = m.defs.iter().filter_map(|d| if let Def::Static(s) = d { Some(s.name.clone()) } else { None }).collect();
    if statics.is_empty() {
        return vec![];
    }
    let fns: Vec<&FnDef> = m.defs.iter().filter_map(|d| if let Def::Fn(f) = d { Some(f) } else { None }).collect();
    let names: HashSet<&str> = fns.iter().map(|f| f.name.as_str()).collect();
    let mut calls: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut writes: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut touches: HashMap<String, BTreeSet<String>> = HashMap::new();
    for f in &fns {
        for b in bodies(f) {
            walk_expr(b, &mut |e| {
                if let ExprKind::Call(c, _) = &e.kind
                    && let ExprKind::Name(n) = &c.kind
                    && names.contains(n.as_str())
                {
                    calls.entry(f.name.clone()).or_default().insert(n.clone());
                }
                if let Some((s, op, _)) = access(e, &statics) {
                    touches.entry(f.name.clone()).or_default().insert(s.to_string());
                    if matches!(op, "store" | "swap" | "add" | "cas") {
                        writes.entry(f.name.clone()).or_default().insert(s.to_string());
                    }
                }
                true
            });
        }
    }
    let irq = reach(fns.iter().filter(|f| f.interrupt.is_some() || f.name == "on_trap" || f.name == "core_main").map(|f| f.name.clone()), &calls);
    let has_main = fns.iter().any(|f| f.name == "main");
    let main = reach(fns.iter().filter(|f| if has_main { f.name == "main" || f.name == "core_main" } else { f.interrupt.is_none() && f.name != "on_trap" }).map(|f| f.name.clone()), &calls);
    let any = |set: &HashSet<String>, map: &HashMap<String, BTreeSet<String>>, s: &str| set.iter().any(|f| map.get(f).is_some_and(|x| x.contains(s)));
    let shared: HashSet<&String> = statics.iter().filter(|s| (any(&irq, &writes, s) && any(&main, &touches, s)) || (any(&main, &writes, s) && any(&irq, &touches, s))).collect();
    let mut out = vec![];
    for f in fns.iter().filter(|f| main.contains(&f.name)) {
        for b in bodies(f) {
            walk_expr(b, &mut |e| {
                if let Some((s, op @ ("store" | "swap"), args)) = access(e, &statics)
                    && shared.contains(&s.to_string())
                    && let Some(v) = args.last()
                {
                    let mut reads = false;
                    walk_expr(v, &mut |x| {
                        reads |= access(x, &statics).is_some_and(|(t, o, _)| t == s && o != "len");
                        !reads
                    });
                    if reads {
                        out.push(Diag {
                            code: "E_STATIC_RACE".into(),
                            severity: "error",
                            def: Some(f.name.clone()),
                            span: [e.span.start, e.span.end],
                            msg: format!("'{s}.{op}' writes a value read from '{s}', which is shared between interrupt handlers and other code; an interrupt between the load and the {op} loses its update"),
                            hint: Some(format!("use {s}.add(d), {s}.swap(v) or a {s}.cas(old, new) loop")),
                            fix: vec![],
                        });
                    }
                }
                true
            });
        }
    }
    out
}
