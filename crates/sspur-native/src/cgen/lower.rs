use sspur_check::{CheckOutput, Type};
use sspur_syntax::visit::{strip_spans, walk_expr};
use sspur_syntax::*;
use std::collections::{BTreeMap, HashMap, HashSet};

struct Op {
    eparams: Vec<String>,
    params: Vec<Ty>,
    ret: Option<Ty>,
}

#[derive(Clone)]
enum Ev {
    Param(String),
    Inline(Vec<Pat>, Expr),
}

type Sc = HashMap<String, Ev>;

struct Lower<'a> {
    check: &'a CheckOutput,
    ops: HashMap<String, Op>,
    effects: HashMap<String, Vec<String>>,
    sigs: HashMap<String, Vec<(String, Ty)>>,
    fresh: usize,
}

const SUFFIX: &str = "__ev";

pub(super) fn original_name(n: &str) -> &str {
    n.strip_suffix(SUFFIX).unwrap_or(n)
}

fn sp() -> Span {
    Span::default()
}

fn ex(kind: ExprKind) -> Expr {
    Expr::new(kind, sp())
}

fn named(n: &str) -> Ty {
    Ty::Named { name: n.into(), args: vec![], span: sp() }
}

fn subst_ty(t: &Ty, map: &HashMap<&str, &Ty>) -> Ty {
    match t {
        Ty::Named { name, args, .. } if args.is_empty() && map.contains_key(name.as_str()) => map[name.as_str()].clone(),
        Ty::Named { name, args, span } => Ty::Named { name: name.clone(), args: args.iter().map(|a| subst_ty(a, map)).collect(), span: *span },
        Ty::Tuple(xs) => Ty::Tuple(xs.iter().map(|a| subst_ty(a, map)).collect()),
        Ty::Fn { params, ret, effects } => Ty::Fn { params: params.iter().map(|a| subst_ty(a, map)).collect(), ret: Box::new(subst_ty(ret, map)), effects: effects.clone() },
    }
}

fn irrefutable(p: &Pat) -> bool {
    match p {
        Pat::Wild | Pat::Bind(_) => true,
        Pat::Tuple(xs) => xs.iter().all(irrefutable),
        _ => false,
    }
}

fn is_resume(e: &Expr) -> bool {
    matches!(&e.kind, ExprKind::Call(f, _) if matches!(&f.kind, ExprKind::Name(n) if n == "resume"))
}

fn tail_resumes(e: &Expr) -> bool {
    match &e.kind {
        _ if is_resume(e) => true,
        ExprKind::Block(stmts) => matches!(stmts.last(), Some(Stmt::Expr(x)) if tail_resumes(x)) && !stmts[..stmts.len() - 1].iter().any(|s| matches!(s, Stmt::Expr(x) | Stmt::Let(_, x) if is_resume(x))),
        ExprKind::If(_, t, Some(f)) => tail_resumes(t) && tail_resumes(f),
        ExprKind::Match(_, arms) => arms.iter().all(|a| tail_resumes(&a.body)),
        _ => false,
    }
}

fn has_post(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Block(stmts) => stmts.iter().enumerate().any(|(i, s)| match s {
            Stmt::Let(_, x) => is_resume(x),
            Stmt::Expr(x) if i + 1 < stmts.len() => is_resume(x),
            Stmt::Expr(x) => has_post(x),
            _ => false,
        }),
        ExprKind::If(_, t, f) => has_post(t) || f.as_ref().is_some_and(|f| has_post(f)),
        ExprKind::Match(_, arms) => arms.iter().any(|a| has_post(&a.body)),
        _ => false,
    }
}

fn strip_resume(e: &Expr) -> Expr {
    if let ExprKind::Call(_, args) = &e.kind
        && is_resume(e) {
            return args.first().cloned().unwrap_or_else(|| ex(ExprKind::Unit));
        }
    let mut e = e.clone();
    match &mut e.kind {
        ExprKind::Block(stmts) => {
            if let Some(Stmt::Expr(x)) = stmts.last_mut() {
                *x = strip_resume(x);
            }
        }
        ExprKind::If(_, t, Some(f)) => {
            **t = strip_resume(t);
            **f = strip_resume(f);
        }
        ExprKind::Match(_, arms) => arms.iter_mut().for_each(|a| a.body = strip_resume(&a.body)),
        _ => {}
    }
    e
}

fn pat_binds(p: &Pat, out: &mut HashSet<String>) {
    match p {
        Pat::Bind(n) => {
            out.insert(n.clone());
        }
        Pat::Tuple(xs) => xs.iter().for_each(|x| pat_binds(x, out)),
        Pat::Ctor { args: CtorArgs::Positional(xs), .. } => xs.iter().for_each(|x| pat_binds(x, out)),
        Pat::Ctor { args: CtorArgs::Record(fs), .. } => fs.iter().for_each(|(_, x)| pat_binds(x, out)),
        _ => {}
    }
}

fn binders(e: &Expr) -> HashSet<String> {
    let mut out = HashSet::new();
    walk_expr(e, &mut |x| {
        match &x.kind {
            ExprKind::Lambda { params, .. } => out.extend(params.iter().cloned()),
            ExprKind::Match(_, arms) | ExprKind::Catch(_, arms) | ExprKind::Handle(_, arms) => arms.iter().for_each(|a| pat_binds(&a.pat, &mut out)),
            ExprKind::Block(stmts) => {
                for s in stmts {
                    match s {
                        Stmt::Let(p, _) | Stmt::For(p, _, _) => pat_binds(p, &mut out),
                        Stmt::Var(n, _) => {
                            out.insert(n.clone());
                        }
                        Stmt::Fn(f) => {
                            out.insert(f.name.clone());
                            out.extend(f.params.iter().map(|p| p.name.clone()));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        true
    });
    out
}

fn names(e: &Expr) -> HashSet<String> {
    let mut out = HashSet::new();
    walk_expr(e, &mut |x| {
        match &x.kind {
            ExprKind::Name(n) => {
                out.insert(n.clone());
            }
            ExprKind::Block(stmts) => {
                for s in stmts {
                    if let Stmt::Assign(n, _, _) = s {
                        out.insert(n.clone());
                    }
                }
            }
            _ => {}
        }
        true
    });
    out
}

fn has_return(e: &Expr) -> bool {
    let mut found = false;
    walk_expr(e, &mut |x| {
        if matches!(x.kind, ExprKind::Return(_)) {
            found = true;
        }
        !found
    });
    found
}

fn ty_mentions(t: &Ty, user: &HashMap<String, Vec<String>>) -> bool {
    match t {
        Ty::Named { args, .. } => args.iter().any(|a| ty_mentions(a, user)),
        Ty::Tuple(xs) => xs.iter().any(|a| ty_mentions(a, user)),
        Ty::Fn { params, ret, effects } => effects.iter().any(|e| user.contains_key(&e.name)) || params.iter().any(|a| ty_mentions(a, user)) || ty_mentions(ret, user),
    }
}

fn single(stmts: Vec<Stmt>) -> Expr {
    match stmts.as_slice() {
        [Stmt::Expr(_)] => {
            let Some(Stmt::Expr(e)) = stmts.into_iter().next() else { unreachable!() };
            e
        }
        _ => ex(ExprKind::Block(stmts)),
    }
}

impl Lower<'_> {
    fn fresh(&mut self, base: &str) -> String {
        self.fresh += 1;
        format!("{base}__{}", self.fresh)
    }

    fn user_atom(&self, a: &str) -> bool {
        self.effects.contains_key(a.split('[').next().unwrap_or(a))
    }

    fn ev_params(&self, f: &FnDef) -> Vec<(String, Ty)> {
        let mut out = Vec::new();
        for e in f.effects.iter().filter(|e| self.effects.contains_key(&e.name)) {
            for op in &self.effects[&e.name] {
                let info = &self.ops[op];
                let map: HashMap<&str, &Ty> = info.eparams.iter().map(String::as_str).zip(e.args.iter()).collect();
                let params = info.params.iter().map(|t| subst_ty(t, &map)).collect();
                let ret = info.ret.as_ref().map_or_else(|| named("Unit"), |t| subst_ty(t, &map));
                out.push((op.clone(), Ty::Fn { params, ret: Box::new(ret), effects: vec![] }));
            }
        }
        out
    }

    fn local_ok(&self, f: &FnDef, effectful: &HashSet<String>) -> Result<(), String> {
        if f.params.iter().any(|p| ty_mentions(&p.ty, &self.effects)) {
            return Err("takes a function with a declared effect".into());
        }
        let bound = binders(&f.body);
        let taken: HashSet<&String> = self.ops.keys().chain(effectful.iter()).collect();
        if bound.iter().chain(f.params.iter().map(|p| &p.name)).any(|n| taken.contains(n) || n.contains("__")) {
            return Err("shadows an operation".into());
        }
        let mut callees = HashSet::new();
        walk_expr(&f.body, &mut |x| {
            if let ExprKind::Call(g, _) = &x.kind
                && matches!(g.kind, ExprKind::Name(_)) {
                    callees.insert((g.span.start, g.span.end));
                }
            true
        });
        let mut err = None;
        walk_expr(&f.body, &mut |x| {
            let why = match &x.kind {
                ExprKind::Name(n) => ((self.ops.contains_key(n) || effectful.contains(n)) && !callees.contains(&(x.span.start, x.span.end))).then_some("passes an effectful function as a value"),
                ExprKind::Lambda { .. } => match self.check.expr_types.get(&(x.span.start, x.span.end, 1)) {
                    Some(Type::Fn(_, _, row)) if !row.atoms.iter().any(|a| self.user_atom(a)) => None,
                    _ => Some("has a lambda that performs a declared effect"),
                },
                ExprKind::Handle(body, arms) => self.handle_why(body, arms),
                ExprKind::Block(stmts) => stmts.iter().find_map(|s| match s {
                    Stmt::Fn(g) if g.effects.iter().any(|e| self.effects.contains_key(&e.name)) => Some("has a local function with a declared effect"),
                    Stmt::For(_, it, body) if self.check.gen_loops.contains(&(it.span.start, it.span.end)) => {
                        if has_return(body) {
                            Some("returns from inside a generator loop")
                        } else if self.raises(body) {
                            Some("a generator loop body may raise")
                        } else if !binders(it).is_disjoint(&names(body)) {
                            Some("a generator loop body uses a name bound in its source")
                        } else {
                            None
                        }
                    }
                    _ => None,
                }),
                _ => None,
            };
            if err.is_none() {
                err = why;
            }
            err.is_none()
        });
        err.map_or(Ok(()), |e| Err(e.into()))
    }

    fn raises(&self, body: &Expr) -> bool {
        self.check.clause_effects.get(&(body.span.start, body.span.end)).is_none_or(|fx| fx.iter().any(|a| a.starts_with("fail[")))
    }

    fn handle_why(&self, body: &Expr, arms: &[Arm]) -> Option<&'static str> {
        let arm_names: HashSet<String> = arms.iter().flat_map(|a| names(&a.body)).collect();
        if !binders(body).is_disjoint(&arm_names) {
            return Some("a handled body shadows a name its handler uses");
        }
        arms.iter().find_map(|a| match &a.pat {
            Pat::Ctor { name, args: CtorArgs::Positional(ps) } if name == "return" => (!ps.iter().all(irrefutable)).then_some("has a refutable return arm"),
            Pat::Ctor { name, .. } if name == "log" => Some("handles log"),
            Pat::Ctor { name, args: CtorArgs::Positional(ps) } if self.ops.contains_key(name) && ps.iter().all(irrefutable) => {
                if has_post(&a.body) {
                    Some("a handler arm runs code after resume")
                } else if !tail_resumes(&a.body) {
                    Some("a handler arm can finish without resuming")
                } else if self.raises(&a.body) {
                    Some("a handler arm may raise")
                } else {
                    None
                }
            }
            _ => Some("uses an unsupported handler arm"),
        })
    }

    fn evidence(&mut self, g: &str, sc: &Sc) -> Result<Vec<Expr>, String> {
        let mut out = Vec::new();
        for (op, _) in self.sigs[g].clone() {
            out.push(match sc.get(&op) {
                Some(Ev::Param(v)) => ex(ExprKind::Name(v.clone())),
                Some(Ev::Inline(pats, body)) => {
                    let mut params = Vec::new();
                    let mut lets = Vec::new();
                    for p in pats {
                        match p {
                            Pat::Bind(n) => params.push(n.clone()),
                            _ => {
                                let t = self.fresh("a");
                                lets.push(Stmt::Let(p.clone(), ex(ExprKind::Name(t.clone()))));
                                params.push(t);
                            }
                        }
                    }
                    lets.push(Stmt::Expr(body.clone()));
                    ex(ExprKind::Lambda { params, body: Box::new(single(lets)), implicit: false })
                }
                None => return Err(format!("no handler for '{op}'")),
            });
        }
        Ok(out)
    }

    fn inline(&mut self, pats: &[Pat], args: Vec<Expr>, body: &Expr) -> Expr {
        let mut stmts = Vec::new();
        let mut temps = Vec::new();
        for a in args {
            let t = self.fresh("a");
            stmts.push(Stmt::Let(Pat::Bind(t.clone()), a));
            temps.push(t);
        }
        for (p, t) in pats.iter().zip(temps) {
            stmts.push(Stmt::Let(p.clone(), ex(ExprKind::Name(t))));
        }
        stmts.push(Stmt::Expr(body.clone()));
        single(stmts)
    }

    fn list(&mut self, xs: &[Expr], sc: &Sc) -> Result<Vec<Expr>, String> {
        xs.iter().map(|x| self.tr(x, sc)).collect()
    }

    fn tr(&mut self, e: &Expr, sc: &Sc) -> Result<Expr, String> {
        let user_method = self.check.user_methods.contains(&(e.span.start, e.span.end));
        let kind = match &e.kind {
            ExprKind::Call(f, args) if matches!(&f.kind, ExprKind::Name(n) if self.ops.contains_key(n)) => {
                let ExprKind::Name(n) = &f.kind else { unreachable!() };
                let args = self.list(args, sc)?;
                return match sc.get(n).cloned() {
                    Some(Ev::Param(v)) => Ok(Expr::new(ExprKind::Call(Box::new(ex(ExprKind::Name(v))), args), e.span)),
                    Some(Ev::Inline(pats, body)) => Ok(self.inline(&pats, args, &body)),
                    None => Err(format!("no handler for '{n}'")),
                };
            }
            ExprKind::Call(f, args) if matches!(&f.kind, ExprKind::Name(n) if self.sigs.contains_key(n)) => {
                let ExprKind::Name(n) = &f.kind else { unreachable!() };
                let mut args = self.list(args, sc)?;
                args.extend(self.evidence(n, sc)?);
                ExprKind::Call(Box::new(Expr::new(ExprKind::Name(format!("{n}{SUFFIX}")), f.span)), args)
            }
            ExprKind::Method { recv, name, targs, args } if user_method && self.sigs.contains_key(name) => {
                let mut args = self.list(args, sc)?;
                args.extend(self.evidence(name, sc)?);
                ExprKind::Method { recv: Box::new(self.tr(recv, sc)?), name: format!("{name}{SUFFIX}"), targs: targs.clone(), args }
            }
            ExprKind::Field(x, name) if user_method && self.sigs.contains_key(name) => {
                let args = self.evidence(name, sc)?;
                ExprKind::Method { recv: Box::new(self.tr(x, sc)?), name: format!("{name}{SUFFIX}"), targs: vec![], args }
            }
            ExprKind::Handle(body, arms) => {
                let mut inner = sc.clone();
                let mut ret = None;
                for a in arms {
                    let Pat::Ctor { name, args: CtorArgs::Positional(ps) } = &a.pat else { return Err("bad arm".into()) };
                    if name == "return" {
                        ret = Some((ps[0].clone(), self.tr(&a.body, sc)?));
                        continue;
                    }
                    let body = strip_resume(&self.tr(&a.body, sc)?);
                    inner.insert(name.clone(), Ev::Inline(ps.clone(), body));
                }
                let body = self.tr(body, &inner)?;
                return Ok(match ret {
                    Some((p, r)) => single(vec![Stmt::Let(p, body), Stmt::Expr(r)]),
                    None => body,
                });
            }
            ExprKind::Block(stmts) => {
                let mut out = Vec::new();
                for s in stmts {
                    self.stmt(s, sc, &mut out)?;
                }
                return Ok(match out.as_slice() {
                    [Stmt::Expr(_)] => single(out),
                    _ => Expr::new(ExprKind::Block(out), e.span),
                });
            }
            ExprKind::Str(parts) => ExprKind::Str(
                parts
                    .iter()
                    .map(|p| match p {
                        StrPart::Lit(l) => Ok(StrPart::Lit(l.clone())),
                        StrPart::Expr(x) => Ok(StrPart::Expr(self.tr(x, sc)?)),
                    })
                    .collect::<Result<_, String>>()?,
            ),
            ExprKind::Field(x, f) => ExprKind::Field(Box::new(self.tr(x, sc)?), f.clone()),
            ExprKind::Call(f, args) => ExprKind::Call(Box::new(self.tr(f, sc)?), self.list(args, sc)?),
            ExprKind::Method { recv, name, targs, args } => ExprKind::Method { recv: Box::new(self.tr(recv, sc)?), name: name.clone(), targs: targs.clone(), args: self.list(args, sc)? },
            ExprKind::Index(a, b) => ExprKind::Index(Box::new(self.tr(a, sc)?), Box::new(self.tr(b, sc)?)),
            ExprKind::Lambda { params, body, implicit } => ExprKind::Lambda { params: params.clone(), body: Box::new(self.tr(body, sc)?), implicit: *implicit },
            ExprKind::Binary(op, a, b) => ExprKind::Binary(*op, Box::new(self.tr(a, sc)?), Box::new(self.tr(b, sc)?)),
            ExprKind::Unary(op, a) => ExprKind::Unary(*op, Box::new(self.tr(a, sc)?)),
            ExprKind::Range(a, b) => ExprKind::Range(Box::new(self.tr(a, sc)?), Box::new(self.tr(b, sc)?)),
            ExprKind::If(c, t, f) => ExprKind::If(Box::new(self.tr(c, sc)?), Box::new(self.tr(t, sc)?), match f {
                Some(f) => Some(Box::new(self.tr(f, sc)?)),
                None => None,
            }),
            ExprKind::Match(s, arms) | ExprKind::Catch(s, arms) => {
                let s2 = Box::new(self.tr(s, sc)?);
                let mut out = Vec::new();
                for a in arms {
                    let guard = match &a.guard {
                        Some(g) => Some(self.tr(g, sc)?),
                        None => None,
                    };
                    out.push(Arm { pat: a.pat.clone(), guard, body: self.tr(&a.body, sc)? });
                }
                if matches!(e.kind, ExprKind::Match(..)) { ExprKind::Match(s2, out) } else { ExprKind::Catch(s2, out) }
            }
            ExprKind::Record { ctor, fields } => ExprKind::Record { ctor: ctor.clone(), fields: fields.iter().map(|(n, x)| Ok((n.clone(), self.tr(x, sc)?))).collect::<Result<_, String>>()? },
            ExprKind::List(xs) => ExprKind::List(self.list(xs, sc)?),
            ExprKind::Tuple(xs) => ExprKind::Tuple(self.list(xs, sc)?),
            ExprKind::Par(xs) => ExprKind::Par(self.list(xs, sc)?),
            ExprKind::Raise(x) => ExprKind::Raise(Box::new(self.tr(x, sc)?)),
            ExprKind::Return(x) => ExprKind::Return(Box::new(self.tr(x, sc)?)),
            ExprKind::With(base, ups) => {
                let mut out = Vec::new();
                for (path, v) in ups {
                    let mut p2 = Vec::new();
                    for seg in path {
                        p2.push(match seg {
                            PathSeg::Field(f) => PathSeg::Field(f.clone()),
                            PathSeg::Index(i) => PathSeg::Index(self.tr(i, sc)?),
                        });
                    }
                    out.push((p2, self.tr(v, sc)?));
                }
                ExprKind::With(Box::new(self.tr(base, sc)?), out)
            }
            ExprKind::Table(rows) => {
                let mut out = Vec::new();
                for r in rows {
                    let mut cells = Vec::new();
                    for c in &r.cells {
                        cells.push(match c {
                            Cell::Cond(x) => Cell::Cond(self.tr(x, sc)?),
                            c => c.clone(),
                        });
                    }
                    out.push(TableRow { cells, out: self.tr(&r.out, sc)? });
                }
                ExprKind::Table(out)
            }
            k => k.clone(),
        };
        Ok(Expr::new(kind, e.span))
    }

    fn stmt(&mut self, s: &Stmt, sc: &Sc, out: &mut Vec<Stmt>) -> Result<(), String> {
        let st = match s {
            Stmt::For(p, it, body) if self.check.gen_loops.contains(&(it.span.start, it.span.end)) => {
                let body = self.tr(body, sc)?;
                let ev = if irrefutable(p) {
                    Ev::Inline(vec![p.clone()], body)
                } else {
                    let t = self.fresh("x");
                    let arms = vec![Arm { pat: p.clone(), guard: None, body }, Arm { pat: Pat::Wild, guard: None, body: ex(ExprKind::Unit) }];
                    Ev::Inline(vec![Pat::Bind(t.clone())], ex(ExprKind::Match(Box::new(ex(ExprKind::Name(t))), arms)))
                };
                let mut inner = sc.clone();
                inner.insert("yield".into(), ev);
                Stmt::Expr(self.tr(it, &inner)?)
            }
            Stmt::For(p, it, body) => Stmt::For(p.clone(), self.tr(it, sc)?, self.tr(body, sc)?),
            Stmt::Let(p, x) => Stmt::Let(p.clone(), self.tr(x, sc)?),
            Stmt::Var(n, x) => Stmt::Var(n.clone(), self.tr(x, sc)?),
            Stmt::Assign(n, x, span) => Stmt::Assign(n.clone(), self.tr(x, sc)?, *span),
            Stmt::Expr(x) => Stmt::Expr(self.tr(x, sc)?),
            Stmt::While(c, body) => Stmt::While(self.tr(c, sc)?, self.tr(body, sc)?),
            Stmt::Fn(f) => {
                let mut g = (**f).clone();
                g.body = self.tr(&f.body, sc)?;
                Stmt::Fn(Box::new(g))
            }
        };
        out.push(st);
        Ok(())
    }

    fn lower_fn(&mut self, f: &FnDef) -> Result<Vec<FnDef>, String> {
        let evs = self.ev_params(f);
        if evs.is_empty() {
            let mut g = f.clone();
            g.body = self.tr(&f.body, &Sc::new())?;
            return Ok(vec![g]);
        }
        let mut g = f.clone();
        let mut sc = Sc::new();
        g.effects.retain(|e| !self.effects.contains_key(&e.name));
        g.examples.clear();
        for (i, (op, ty)) in evs.into_iter().enumerate() {
            let row = format!("e{i}__");
            let Ty::Fn { params, ret, .. } = ty else { unreachable!() };
            let pname = format!("ev__{op}");
            g.params.push(Param { name: pname.clone(), ty: Ty::Fn { params, ret, effects: vec![Effect { name: row.clone(), args: vec![], span: sp() }] }, refine: None });
            g.tparams.push(TParam { name: row.clone(), kind: None, refine: None });
            g.effects.push(Effect { name: row, args: vec![], span: sp() });
            sc.insert(op, Ev::Param(pname));
        }
        g.name = format!("{}{SUFFIX}", f.name);
        g.body = self.tr(&f.body, &sc)?;
        Ok(vec![f.clone(), g])
    }
}

pub(super) type Lowered = (Module, CheckOutput, BTreeMap<String, String>);

pub(super) fn lower(m: &Module, check: &CheckOutput) -> Option<Lowered> {
    let mut ops = HashMap::new();
    let mut effects: HashMap<String, Vec<String>> = HashMap::new();
    for d in &m.defs {
        if let Def::Effect(e) = d {
            for op in &e.ops {
                ops.insert(op.name.clone(), Op { eparams: e.params.iter().map(|p| p.name.clone()).collect(), params: op.params.iter().map(|p| p.ty.clone()).collect(), ret: op.ret.clone() });
            }
            effects.insert(e.name.clone(), e.ops.iter().map(|o| o.name.clone()).collect());
        }
    }
    if effects.is_empty() && check.gen_loops.is_empty() {
        return None;
    }
    ops.insert("yield".into(), Op { eparams: vec!["T".into()], params: vec![named("T")], ret: None });
    effects.insert("yield".into(), vec!["yield".into()]);
    let fns: Vec<&FnDef> = m.defs.iter().filter_map(|d| if let Def::Fn(f) = d { Some(f) } else { None }).collect();
    let mut lw = Lower { check, ops, effects, sigs: HashMap::new(), fresh: 0 };
    let effectful: HashSet<String> = fns.iter().filter(|f| !lw.ev_params(f).is_empty()).map(|f| f.name.clone()).collect();
    if fns.iter().any(|f| f.name.ends_with(SUFFIX)) {
        return None;
    }
    let mut why: BTreeMap<String, String> = BTreeMap::new();
    let mut ok: HashSet<String> = HashSet::new();
    for f in &fns {
        match lw.local_ok(f, &effectful) {
            Ok(()) => {
                ok.insert(f.name.clone());
            }
            Err(e) => {
                why.insert(f.name.clone(), e);
            }
        }
    }
    for _ in 0..8 {
        loop {
            let before = ok.len();
            for f in &fns {
                let mut calls_bad = None;
                walk_expr(&f.body, &mut |x| {
                    let callee = match &x.kind {
                        ExprKind::Call(g, _) => match &g.kind {
                            ExprKind::Name(n) => Some(n),
                            _ => None,
                        },
                        ExprKind::Method { name, .. } | ExprKind::Field(_, name) if check.user_methods.contains(&(x.span.start, x.span.end)) => Some(name),
                        _ => None,
                    };
                    if let Some(n) = callee.filter(|n| effectful.contains(*n) && !ok.contains(*n)) {
                        calls_bad.get_or_insert_with(|| n.clone());
                    }
                    true
                });
                if let Some(n) = calls_bad
                    && ok.remove(&f.name) {
                        why.insert(f.name.clone(), format!("calls '{n}', which is not native"));
                    }
            }
            if ok.len() == before {
                break;
            }
        }
        lw.sigs = fns.iter().filter(|f| ok.contains(&f.name) && effectful.contains(&f.name)).map(|f| (f.name.clone(), lw.ev_params(f))).collect();
        let mut defs = Vec::new();
        let mut origin: BTreeMap<String, String> = BTreeMap::new();
        let mut failed = None;
        for d in &m.defs {
            match d {
                Def::Fn(f) if ok.contains(&f.name) => match lw.lower_fn(f) {
                    Ok(gs) => {
                        for g in gs {
                            origin.insert(g.name.clone(), f.name.clone());
                            defs.push(Def::Fn(g));
                        }
                    }
                    Err(e) => {
                        why.insert(f.name.clone(), e);
                        failed = Some(f.name.clone());
                        break;
                    }
                },
                Def::Test(_) => {}
                d => defs.push(d.clone()),
            }
        }
        if let Some(n) = failed {
            ok.remove(&n);
            continue;
        }
        let lowered = Module { profile: m.profile.clone(), defs };
        let text = printer::print_module(&lowered);
        let Ok(reparsed) = parse(&text) else { return None };
        let (mut a, mut b) = (lowered.clone(), reparsed.clone());
        strip_spans(&mut a);
        strip_spans(&mut b);
        if a.defs.len() != b.defs.len() {
            return None;
        }
        let differ: Vec<&Def> = a.defs.iter().zip(&b.defs).filter(|(x, y)| x != y).map(|(x, _)| x).collect();
        if differ.iter().any(|d| !origin.contains_key(d.name())) {
            return None;
        }
        let bad: Vec<String> = differ.iter().filter_map(|d| origin.get(d.name()).cloned()).collect();
        if !bad.is_empty() {
            for n in bad {
                ok.remove(&n);
            }
            continue;
        }
        let c2 = sspur_check::check(&reparsed);
        let errs: Vec<String> = c2.diags.iter().filter(|d| d.is_error()).filter_map(|d| d.def.as_ref().and_then(|n| origin.get(n)).cloned()).collect();
        if c2.has_errors() && errs.is_empty() {
            return None;
        }
        if !errs.is_empty() {
            for n in errs {
                ok.remove(&n);
            }
            continue;
        }
        why.retain(|n, _| !ok.contains(n));
        return Some((reparsed, c2, why));
    }
    None
}
