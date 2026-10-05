use crate::ast::*;

pub fn print_module(m: &Module) -> String {
    let mut out = String::new();
    if let Some(p) = &m.profile {
        out.push_str(&format!("profile {p}\n\n"));
    }
    let defs: Vec<String> = m.defs.iter().map(print_def).collect();
    out.push_str(&defs.join("\n\n"));
    out.push('\n');
    out
}

pub fn print_def(d: &Def) -> String {
    match d {
        Def::Type(t) => print_type_def(t),
        Def::Fn(f) => print_fn(f),
        Def::Test(t) => format!("test {} = {}", t.name, expr(&t.body, 0)),
        Def::Effect(e) => print_effect_def(e),
        Def::Store(st) => format!("store {} = {}[{}, {}]", st.name, st.kind, ty(&st.key), ty(&st.val)),
        Def::Svc(sv) => {
            let mut s = format!("svc {}", sv.name);
            for ep in &sv.eps {
                s.push_str(&format!("\n  ep {} {:?} = {}", ep.method, ep.path, ep.handler));
            }
            s
        }
        Def::Static(s) => format!("static {}: {} = {}", s.name, ty(&s.ty), expr(&s.init, 0)),
    }
}

fn op_sig(op: &OpSig) -> String {
    let params: Vec<String> = op.params.iter().map(|p| format!("{}: {}", p.name, ty(&p.ty))).collect();
    let mut s = format!("({})", params.join(", "));
    if let Some(r) = &op.ret {
        s.push_str(&format!(" -> {}", ty(r)));
    }
    s
}

fn print_effect_def(e: &EffectDef) -> String {
    let head = format!("effect {}{}", e.name, tparams(&e.params));
    if let [op] = e.ops.as_slice()
        && op.name == e.name {
            return format!("{head}{}", op_sig(op));
        }
    let mut s = head;
    for op in &e.ops {
        s.push_str(&format!("\n  {}{}", op.name, op_sig(op)));
    }
    s
}

fn tparams(ps: &[TParam]) -> String {
    if ps.is_empty() {
        return String::new();
    }
    let items: Vec<String> = ps
        .iter()
        .map(|p| {
            let mut s = p.name.clone();
            if let Some(k) = &p.kind {
                s.push_str(&format!(": {}", ty(k)));
            }
            if let Some(r) = &p.refine {
                s.push_str(&format!(" where {}", expr(r, 0)));
            }
            s
        })
        .collect();
    format!("[{}]", items.join(", "))
}

fn fields(fs: &[Field]) -> String {
    let items: Vec<String> = fs
        .iter()
        .map(|f| match &f.refine {
            Some(r) => format!("{}: {} where {}", f.name, ty(&f.ty), expr(r, 0)),
            None => format!("{}: {}", f.name, ty(&f.ty)),
        })
        .collect();
    format!("{{{}}}", items.join(", "))
}

fn print_type_def(t: &TypeDef) -> String {
    let body = match &t.body {
        TypeBody::Record(fs) => fields(fs),
        TypeBody::Sum(vs) => vs
            .iter()
            .map(|v| match &v.fields {
                Some(fs) => format!("{}{}", v.name, fields(fs)),
                None => v.name.clone(),
            })
            .collect::<Vec<_>>()
            .join(" | "),
        TypeBody::Alias(t, None) => ty(t),
        TypeBody::Alias(t, Some(r)) => format!("{} where {}", ty(t), expr(r, 0)),
        TypeBody::New(t) => format!("new {}", ty(t)),
    };
    let res = if t.res { "res " } else { "" };
    let mut s = format!("{res}type {}{} = {}", t.name, tparams(&t.params), body);
    if !t.derives.is_empty() {
        s.push_str(&format!(" derive {}", t.derives.join(", ")));
    }
    if let Some(d) = &t.drop {
        s.push_str(&format!(" drop {d}"));
    }
    s
}

pub fn print_sig(f: &FnDef) -> String {
    let params: Vec<String> = f
        .params
        .iter()
        .map(|p| match &p.refine {
            Some(r) => format!("{}: {} where {}", p.name, ty(&p.ty), expr(r, 0)),
            None => format!("{}: {}", p.name, ty(&p.ty)),
        })
        .collect();
    let kw = if f.ext.is_some() {
        "extern fn"
    } else if f.kernel.is_some() {
        "kernel fn"
    } else {
        "fn"
    };
    let mut s = format!("{kw} {}{}({})", f.name, tparams(&f.tparams), params.join(", "));
    if let Some(r) = &f.ret {
        s.push_str(&format!(" -> {}", ty(r)));
    }
    if !f.effects.is_empty() {
        s.push_str(&format!(" ! {}", effects(&f.effects)));
    }
    if let Some(k) = &f.kernel {
        match &k.y {
            Some((h, gh)) => s.push_str(&format!(" @grid2({}, {}, {}, {})", expr(&k.grid, 0), expr(h, 0), expr(&k.group, 0), expr(gh, 0))),
            None => s.push_str(&format!(" @grid({}, {})", expr(&k.grid, 0), expr(&k.group, 0))),
        }
    }
    if let Some(x) = &f.ext {
        if let Some(l) = &x.lib {
            s.push_str(&format!(" from {l:?}"));
        }
        if x.symbol != f.name {
            s.push_str(&format!(" as {:?}", x.symbol));
        }
    }
    s
}

fn print_fn(f: &FnDef) -> String {
    print_fn_at(f, 0)
}

fn print_fn_at(f: &FnDef, ind: usize) -> String {
    let mut s = print_sig(f);
    if f.ext.is_some() {
        return s;
    }
    let c = pad(ind + 2);
    if let Some(r) = &f.trusted {
        s.push_str(&format!("\n{c}unsafe {}", expr(&Expr::new(ExprKind::Str(vec![StrPart::Lit(r.clone())]), Span::default()), 0)));
    }
    if let Some(v) = &f.interrupt {
        s.push_str(&format!("\n{c}interrupt {v}"));
    }
    for p in &f.pres {
        s.push_str(&format!("\n{c}pre {}", expr(p, ind + 2)));
    }
    for p in &f.posts {
        s.push_str(&format!("\n{c}post {}", expr(p, ind + 2)));
    }
    for p in &f.examples {
        s.push_str(&format!("\n{c}ex {}", expr(p, ind + 2)));
    }
    if let ExprKind::Table(rows) = &f.body.kind {
        s.replace_range(..2, "rule");
        for r in rows {
            let cells: Vec<String> = r
                .cells
                .iter()
                .map(|c| match c {
                    Cell::Any => "_".to_string(),
                    Cell::Pat(p) => pat(p),
                    Cell::Cond(e) => expr(e, ind + 2),
                })
                .collect();
            s.push_str(&format!("\n{c}| {} => {}", cells.join(", "), expr(&r.out, ind + 2)));
        }
        return s;
    }
    s.push_str(&format!("\n{}= {}", pad(ind), expr(&f.body, ind)));
    s
}

pub fn effects(es: &[Effect]) -> String {
    es.iter().map(effect).collect::<Vec<_>>().join(", ")
}

pub fn effect(e: &Effect) -> String {
    if e.args.is_empty() {
        e.name.clone()
    } else {
        let sep = if e.name == "fail" { " | " } else { ", " };
        format!("{}[{}]", e.name, e.args.iter().map(ty).collect::<Vec<_>>().join(sep))
    }
}

pub fn ty(t: &Ty) -> String {
    match t {
        Ty::Named { name, args, .. } if args.is_empty() => name.clone(),
        Ty::Named { name, args, .. } if name == "&" => format!("&{}", ty(&args[0])),
        Ty::Named { name, args, .. } if name == "[]" => format!("[{}]", ty(&args[0])),
        Ty::Named { name, args, .. } if matches!(name.as_str(), "&mut" | "own") => format!("{name} {}", ty(&args[0])),
        Ty::Named { name, args, .. } => format!("{}[{}]", name, args.iter().map(ty).collect::<Vec<_>>().join(", ")),
        Ty::Tuple(xs) => format!("({})", xs.iter().map(ty).collect::<Vec<_>>().join(", ")),
        Ty::Fn { params, ret, effects: es } => {
            let ps = if params.len() == 1 && !matches!(params[0], Ty::Fn { .. } | Ty::Tuple(_)) {
                ty(&params[0])
            } else {
                format!("({})", params.iter().map(ty).collect::<Vec<_>>().join(", "))
            };
            let mut s = format!("{} -> {}", ps, ty(ret));
            if !es.is_empty() {
                s.push_str(&format!(" ! {}", effects(es)));
            }
            s
        }
    }
}

fn prec_of(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Binary(op, ..) => op.prec(),
        ExprKind::Range(..) => 5,
        ExprKind::Unary(UnOp::Not, _) => 3,
        ExprKind::Unary(UnOp::Neg | UnOp::Ref | UnOp::RefMut, _) => 9,
        ExprKind::Int(n) if *n < 0 => 9,
        ExprKind::Float(n) if *n < 0.0 => 9,
        ExprKind::Lambda { implicit: false, .. }
        | ExprKind::If(..)
        | ExprKind::Match(..)
        | ExprKind::Catch(..)
        | ExprKind::Handle(..)
        | ExprKind::Block(_)
        | ExprKind::Raise(_)
        | ExprKind::Return(_)
        | ExprKind::With(..)
        | ExprKind::Table(_) => 0,
        _ => 10,
    }
}

fn operand(e: &Expr, min: u8, ind: usize) -> String {
    let s = expr(e, ind);
    if prec_of(e) < min { format!("({s})") } else { s }
}

fn pad(n: usize) -> String {
    " ".repeat(n)
}

pub fn expr(e: &Expr, ind: usize) -> String {
    match &e.kind {
        ExprKind::Int(n) => n.to_string(),
        ExprKind::Float(n) => {
            let s = format!("{n:?}");
            if s.contains(['.', 'e']) { s } else { format!("{s}.0") }
        }
        ExprKind::Str(parts) => {
            let mut s = String::from("\"");
            for p in parts {
                match p {
                    StrPart::Lit(l) => {
                        for c in l.chars() {
                            match c {
                                '"' => s.push_str("\\\""),
                                '\\' => s.push_str("\\\\"),
                                '\n' => s.push_str("\\n"),
                                '\t' => s.push_str("\\t"),
                                '{' => s.push_str("\\{"),
                                c => s.push(c),
                            }
                        }
                    }
                    StrPart::Expr(x) => s.push_str(&format!("{{{}}}", expr(x, ind))),
                }
            }
            s.push('"');
            s
        }
        ExprKind::Bool(b) => b.to_string(),
        ExprKind::Unit => "()".into(),
        ExprKind::Name(n) => n.clone(),
        ExprKind::Hole(None) => "?".into(),
        ExprKind::Hole(Some(n)) => format!("?{n}"),
        ExprKind::Placeholder => "_".into(),
        ExprKind::Field(x, f) => format!("{}.{}", operand(x, 10, ind), f),
        ExprKind::Call(f, args) => format!("{}({})", operand(f, 10, ind), list(args, ind)),
        ExprKind::Method { recv, name, targs, args } if name == "mmio" && targs.len() == 1 => {
            let rest: String = args.iter().map(|a| format!(", {}", expr(a, ind))).collect();
            format!("mmio[{}]({}{rest})", ty(&targs[0]), expr(recv, ind))
        }
        ExprKind::Method { recv, name, targs, .. } if name == "#array" && targs.len() == 1 => format!("[{}; {}]", expr(recv, ind), ty(&targs[0])),
        ExprKind::Method { recv, name, targs, args } if name == "asm" && args.len() == 3 => {
            let tmpl = match &recv.kind {
                ExprKind::Str(p) => p.iter().map(|x| if let StrPart::Lit(s) = x { s.as_str() } else { "" }).collect::<String>(),
                _ => String::new(),
            };
            let named = |rest: &str| rest.find('}').is_some_and(|j| j > 0 && rest[..j].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !rest.as_bytes()[0].is_ascii_digit());
            let esc: String = tmpl
                .char_indices()
                .map(|(i, c)| match c {
                    '"' => "\\\"".to_string(),
                    '\\' => "\\\\".to_string(),
                    '\n' => "\\n".to_string(),
                    '\t' => "\\t".to_string(),
                    '{' if !named(&tmpl[i + 1..]) => "\\{".to_string(),
                    c => c.to_string(),
                })
                .collect();
            let mut s = format!("asm \"{esc}\"");
            let fields = |a: &Expr| if let ExprKind::Record { fields, .. } = &a.kind { fields.clone() } else { vec![] };
            let ins = fields(&args[0]);
            if !ins.is_empty() {
                s.push_str(&format!(" in({})", ins.iter().map(|(n, e)| format!("{n}: {}", expr(e, ind))).collect::<Vec<_>>().join(", ")));
            }
            let outs = fields(&args[1]);
            if !outs.is_empty() {
                s.push_str(&format!(" out({})", outs.iter().zip(targs).map(|((n, _), t)| format!("{n}: {}", ty(t))).collect::<Vec<_>>().join(", ")));
            }
            let cl = fields(&args[2]);
            if !cl.is_empty() {
                s.push_str(&format!(" clobber({})", cl.iter().map(|(c, _)| format!("\"{c}\"")).collect::<Vec<_>>().join(", ")));
            }
            s
        }
        ExprKind::Method { recv, name, targs, args } => {
            let mut s = format!("{}.{}", operand(recv, 10, ind), name);
            if !targs.is_empty() {
                s.push_str(&format!("[{}]", targs.iter().map(ty).collect::<Vec<_>>().join(", ")));
            }
            if !args.is_empty() || targs.is_empty() {
                s.push_str(&format!("({})", list(args, ind)));
            }
            s
        }
        ExprKind::Index(a, i) => format!("{}[{}]", operand(a, 10, ind), expr(i, ind)),
        ExprKind::Lambda { body, implicit: true, .. } => expr(body, ind),
        ExprKind::Lambda { params, body, .. } => {
            let ps = if params.len() == 1 { params[0].clone() } else { format!("({})", params.join(", ")) };
            format!("{} => {}", ps, expr(body, ind))
        }
        ExprKind::Binary(op, l, r) => {
            let p = op.prec();
            let (lmin, rmin) = if *op == BinOp::Pow { (p + 1, p) } else { (p, p + 1) };
            format!("{} {} {}", operand(l, lmin, ind), op.symbol(), operand(r, rmin, ind))
        }
        ExprKind::Unary(UnOp::Neg, x) => format!("-{}", operand(x, 9, ind)),
        ExprKind::Unary(UnOp::Not, x) => format!("not {}", operand(x, 4, ind)),
        ExprKind::Unary(UnOp::Ref, x) => format!("&{}", operand(x, 10, ind)),
        ExprKind::Unary(UnOp::RefMut, x) => format!("&mut {}", operand(x, 10, ind)),
        ExprKind::Range(a, b) => format!("{}..{}", operand(a, 6, ind), operand(b, 6, ind)),
        ExprKind::If(c, t, Some(f)) if matches!(&t.kind, ExprKind::Block(st) if !matches!(st.as_slice(), [Stmt::Assign(..)])) => {
            format!("if {} then {}\n{}else {}", expr(c, ind), branch(t, ind + 2), pad(ind + 2), branch(f, ind + 2))
        }
        ExprKind::If(c, t, Some(f)) if matches!(t.kind, ExprKind::If(..) | ExprKind::Match(..) | ExprKind::Catch(..) | ExprKind::Handle(..) | ExprKind::Lambda { .. }) => {
            format!("if {} then do\n{}{}\n{}else {}", expr(c, ind), pad(ind + 4), expr(t, ind + 4), pad(ind + 2), branch(f, ind + 2))
        }
        ExprKind::If(c, t, f) => {
            let mut s = format!("if {} then {}", expr(c, ind), branch(t, ind));
            if let Some(f) = f {
                s.push_str(&format!(" else {}", branch(f, ind)));
            }
            s
        }
        ExprKind::Match(s, arms) => format!("match {}{}", arms_head(s, ind), print_arms(arms, ind)),
        ExprKind::Catch(s, arms) => format!("catch {}{}", arms_head(s, ind), print_arms(arms, ind)),
        ExprKind::Handle(s, arms) => format!("handle {}{}", arms_head(s, ind), print_arms(arms, ind)),
        ExprKind::Block(stmts) => {
            let mut s = String::from("do");
            for st in stmts {
                s.push_str(&format!("\n{}{}", pad(ind + 2), stmt(st, ind + 2)));
            }
            s
        }
        ExprKind::Record { ctor, fields } => {
            let items: Vec<String> = fields.iter().map(|(n, v)| format!("{}: {}", n, expr(v, ind))).collect();
            format!("{}{{{}}}", ctor.as_deref().unwrap_or(""), items.join(", "))
        }
        ExprKind::List(xs) => format!("[{}]", list(xs, ind)),
        ExprKind::Tuple(xs) => format!("({})", list(xs, ind)),
        ExprKind::Par(xs) => format!("par({})", list(xs, ind)),
        ExprKind::Raise(x) => format!("raise {}", expr(x, ind)),
        ExprKind::Return(x) => format!("return {}", expr(x, ind)),
        ExprKind::Table(_) => "<rule table>".into(),
        ExprKind::With(base, ups) => {
            let items: Vec<String> = ups.iter().map(|(p, v)| format!("{} := {}", path(p, ind), expr(v, ind))).collect();
            format!("{} with {}", operand(base, 1, ind), items.join(", "))
        }
    }
}

fn iter_expr(e: &Expr, ind: usize) -> String {
    match e.kind {
        ExprKind::Block(_) => expr(e, ind + 2),
        _ => expr(e, ind),
    }
}

fn arms_head(e: &Expr, ind: usize) -> String {
    match e.kind {
        ExprKind::Match(..) | ExprKind::Catch(..) | ExprKind::Handle(..) => format!("do\n{}{}", pad(ind + 2), expr(e, ind + 2)),
        _ => expr(e, ind),
    }
}

fn path(p: &[PathSeg], ind: usize) -> String {
    let mut s = String::new();
    for (i, seg) in p.iter().enumerate() {
        match seg {
            PathSeg::Field(f) if i == 0 => s.push_str(f),
            PathSeg::Field(f) => s.push_str(&format!(".{f}")),
            PathSeg::Index(e) => s.push_str(&format!("[{}]", expr(e, ind))),
        }
    }
    s
}

fn branch(e: &Expr, ind: usize) -> String {
    match &e.kind {
        ExprKind::Block(stmts) if matches!(stmts.as_slice(), [Stmt::Assign(..)]) => stmt(&stmts[0], ind),
        _ => expr(e, ind),
    }
}

fn list(xs: &[Expr], ind: usize) -> String {
    xs.iter().map(|x| expr(x, ind)).collect::<Vec<_>>().join(", ")
}

fn print_arms(arms: &[Arm], ind: usize) -> String {
    let mut s = String::new();
    for a in arms {
        s.push_str(&format!("\n{}| {}", pad(ind), pat(&a.pat)));
        if let Some(g) = &a.guard {
            s.push_str(&format!(" if {}", expr(g, ind)));
        }
        s.push_str(&format!(" => {}", branch(&a.body, ind)));
    }
    s
}

fn stmt(s: &Stmt, ind: usize) -> String {
    match s {
        Stmt::Let(p, e) => format!("{} = {}", pat(p), expr(e, ind)),
        Stmt::Var(n, e) => format!("var {} = {}", n, expr(e, ind)),
        Stmt::Assign(n, e, _) => match &e.kind {
            ExprKind::With(base, ups) if matches!(&base.kind, ExprKind::Name(b) if b == n) && ups.len() == 1 && !ups[0].0.is_empty() => {
                let mut p = n.clone();
                for seg in &ups[0].0 {
                    match seg {
                        PathSeg::Field(f) => p.push_str(&format!(".{f}")),
                        PathSeg::Index(e) => p.push_str(&format!("[{}]", expr(e, ind))),
                    }
                }
                format!("{p} := {}", expr(&ups[0].1, ind))
            }
            _ => format!("{} := {}", n, expr(e, ind)),
        },
        Stmt::Expr(e) => expr(e, ind),
        Stmt::For(p, it, body) => match &body.kind {
            ExprKind::Block(stmts) => {
                let mut s = format!("for {} in {}", pat(p), iter_expr(it, ind));
                for st in stmts {
                    s.push_str(&format!("\n{}{}", pad(ind + 2), stmt(st, ind + 2)));
                }
                s
            }
            _ => format!("for {} in {}\n{}{}", pat(p), iter_expr(it, ind), pad(ind + 2), branch(body, ind + 2)),
        },
        Stmt::While(c, body) => match &body.kind {
            ExprKind::Block(stmts) => {
                let mut s = format!("while {}", expr(c, ind));
                for st in stmts {
                    s.push_str(&format!("\n{}{}", pad(ind + 2), stmt(st, ind + 2)));
                }
                s
            }
            _ => format!("while {}\n{}{}", expr(c, ind), pad(ind + 2), branch(body, ind + 2)),
        },
        Stmt::Fn(f) => print_fn_at(f, ind),
    }
}

pub fn pat(p: &Pat) -> String {
    match p {
        Pat::Wild => "_".into(),
        Pat::Bind(n) => n.clone(),
        Pat::Int(n) => n.to_string(),
        Pat::Str(s) => format!("{s:?}"),
        Pat::Bool(b) => b.to_string(),
        Pat::Tuple(xs) => format!("({})", xs.iter().map(pat).collect::<Vec<_>>().join(", ")),
        Pat::Ctor { name, args: CtorArgs::None } => name.clone(),
        Pat::Ctor { name, args: CtorArgs::Positional(xs) } => {
            format!("{}({})", name, xs.iter().map(pat).collect::<Vec<_>>().join(", "))
        }
        Pat::Ctor { name, args: CtorArgs::Record(fs) } => {
            let items: Vec<String> = fs
                .iter()
                .map(|(f, p)| match p {
                    Pat::Bind(b) if b == f => f.clone(),
                    _ => format!("{}: {}", f, pat(p)),
                })
                .collect();
            format!("{}{{{}}}", name, items.join(", "))
        }
    }
}
