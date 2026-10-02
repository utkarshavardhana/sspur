use crate::ast::*;

pub fn children(e: &Expr) -> Vec<&Expr> {
    let mut out = Vec::new();
    match &e.kind {
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Unit
        | ExprKind::Name(_)
        | ExprKind::Hole(_)
        | ExprKind::Placeholder => {}
        ExprKind::Str(parts) => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    out.push(x);
                }
            }
        }
        ExprKind::Field(x, _) | ExprKind::Unary(_, x) | ExprKind::Raise(x) | ExprKind::Return(x) => out.push(x),
        ExprKind::Lambda { body, .. } => out.push(body),
        ExprKind::Call(f, args) => {
            out.push(f);
            out.extend(args.iter());
        }
        ExprKind::Method { recv, args, .. } => {
            out.push(recv);
            out.extend(args.iter());
        }
        ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) | ExprKind::Range(a, b) => {
            out.push(a);
            out.push(b);
        }
        ExprKind::If(c, t, f) => {
            out.push(c);
            out.push(t);
            if let Some(f) = f {
                out.push(f);
            }
        }
        ExprKind::Match(s, arms) | ExprKind::Catch(s, arms) => {
            out.push(s);
            for a in arms {
                if let Some(g) = &a.guard {
                    out.push(g);
                }
                out.push(&a.body);
            }
        }
        ExprKind::Block(stmts) => {
            for s in stmts {
                match s {
                    Stmt::Let(_, x) | Stmt::Var(_, x) | Stmt::Assign(_, x, _) | Stmt::Expr(x) => out.push(x),
                    Stmt::For(_, a, b) => {
                        out.push(a);
                        out.push(b);
                    }
                }
            }
        }
        ExprKind::Record { fields, .. } => out.extend(fields.iter().map(|(_, x)| x)),
        ExprKind::List(xs) | ExprKind::Tuple(xs) | ExprKind::Par(xs) => out.extend(xs.iter()),
        ExprKind::With(base, ups) => {
            out.push(base);
            for (p, v) in ups {
                for seg in p {
                    if let PathSeg::Index(i) = seg {
                        out.push(i);
                    }
                }
                out.push(v);
            }
        }
    }
    out
}

pub fn walk_expr(e: &Expr, f: &mut impl FnMut(&Expr) -> bool) {
    if f(e) {
        for c in children(e) {
            walk_expr(c, f);
        }
    }
}

pub fn walk_expr_mut(e: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    f(e);
    match &mut e.kind {
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::Unit
        | ExprKind::Name(_)
        | ExprKind::Hole(_)
        | ExprKind::Placeholder => {}
        ExprKind::Str(parts) => {
            for p in parts {
                if let StrPart::Expr(x) = p {
                    walk_expr_mut(x, f);
                }
            }
        }
        ExprKind::Field(x, _) | ExprKind::Unary(_, x) | ExprKind::Raise(x) | ExprKind::Return(x) => walk_expr_mut(x, f),
        ExprKind::Lambda { body, .. } => walk_expr_mut(body, f),
        ExprKind::Call(g, args) => {
            walk_expr_mut(g, f);
            args.iter_mut().for_each(|a| walk_expr_mut(a, f));
        }
        ExprKind::Method { recv, args, .. } => {
            walk_expr_mut(recv, f);
            args.iter_mut().for_each(|a| walk_expr_mut(a, f));
        }
        ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) | ExprKind::Range(a, b) => {
            walk_expr_mut(a, f);
            walk_expr_mut(b, f);
        }
        ExprKind::If(c, t, e2) => {
            walk_expr_mut(c, f);
            walk_expr_mut(t, f);
            if let Some(x) = e2 {
                walk_expr_mut(x, f);
            }
        }
        ExprKind::Match(s, arms) | ExprKind::Catch(s, arms) => {
            walk_expr_mut(s, f);
            for a in arms {
                if let Some(g) = &mut a.guard {
                    walk_expr_mut(g, f);
                }
                walk_expr_mut(&mut a.body, f);
            }
        }
        ExprKind::Block(stmts) => {
            for s in stmts {
                match s {
                    Stmt::Let(_, x) | Stmt::Var(_, x) | Stmt::Assign(_, x, _) | Stmt::Expr(x) => walk_expr_mut(x, f),
                    Stmt::For(_, a, b) => {
                        walk_expr_mut(a, f);
                        walk_expr_mut(b, f);
                    }
                }
            }
        }
        ExprKind::Record { fields, .. } => fields.iter_mut().for_each(|(_, x)| walk_expr_mut(x, f)),
        ExprKind::List(xs) | ExprKind::Tuple(xs) | ExprKind::Par(xs) => xs.iter_mut().for_each(|x| walk_expr_mut(x, f)),
        ExprKind::With(base, ups) => {
            walk_expr_mut(base, f);
            for (p, v) in ups {
                for seg in p {
                    if let PathSeg::Index(i) = seg {
                        walk_expr_mut(i, f);
                    }
                }
                walk_expr_mut(v, f);
            }
        }
    }
}

pub fn strip_spans(m: &mut Module) {
    let z = Span::default();
    for d in &mut m.defs {
        match d {
            Def::Type(t) => {
                t.span = z;
                match &mut t.body {
                    TypeBody::Record(fs) => fs.iter_mut().for_each(strip_field),
                    TypeBody::Sum(vs) => vs.iter_mut().flat_map(|v| v.fields.iter_mut().flatten()).for_each(strip_field),
                    TypeBody::Alias(ty, r) => {
                        strip_ty(ty);
                        if let Some(r) = r {
                            strip_expr(r);
                        }
                    }
                    TypeBody::New(ty) => strip_ty(ty),
                }
                for p in &mut t.params {
                    strip_tparam(p);
                }
            }
            Def::Fn(f) => {
                f.span = z;
                f.sig_span = z;
                f.tparams.iter_mut().for_each(strip_tparam);
                for p in &mut f.params {
                    strip_ty(&mut p.ty);
                    if let Some(r) = &mut p.refine {
                        strip_expr(r);
                    }
                }
                if let Some(r) = &mut f.ret {
                    strip_ty(r);
                }
                f.effects.iter_mut().for_each(strip_effect);
                f.pres.iter_mut().chain(f.posts.iter_mut()).chain(f.examples.iter_mut()).for_each(strip_expr);
                strip_expr(&mut f.body);
            }
            Def::Test(t) => {
                t.span = z;
                strip_expr(&mut t.body);
            }
        }
    }
}

fn strip_tparam(p: &mut TParam) {
    if let Some(k) = &mut p.kind {
        strip_ty(k);
    }
    if let Some(r) = &mut p.refine {
        strip_expr(r);
    }
}

fn strip_field(f: &mut Field) {
    strip_ty(&mut f.ty);
    if let Some(r) = &mut f.refine {
        strip_expr(r);
    }
}

fn strip_effect(e: &mut Effect) {
    e.span = Span::default();
    e.args.iter_mut().for_each(strip_ty);
}

fn strip_ty(t: &mut Ty) {
    match t {
        Ty::Named { args, span, .. } => {
            *span = Span::default();
            args.iter_mut().for_each(strip_ty);
        }
        Ty::Tuple(xs) => xs.iter_mut().for_each(strip_ty),
        Ty::Fn { params, ret, effects } => {
            params.iter_mut().for_each(strip_ty);
            strip_ty(ret);
            effects.iter_mut().for_each(strip_effect);
        }
    }
}

fn strip_expr(e: &mut Expr) {
    walk_expr_mut(e, &mut |x| {
        x.span = Span::default();
        if let ExprKind::Method { targs, .. } = &mut x.kind {
            targs.iter_mut().for_each(strip_ty);
        }
        if let ExprKind::Block(stmts) = &mut x.kind {
            for s in stmts {
                if let Stmt::Assign(_, _, sp) = s {
                    *sp = Span::default();
                }
            }
        }
    });
}
