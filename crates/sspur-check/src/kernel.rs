use sspur_syntax::*;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KTy {
    Int,
    I32,
    U32,
    F32,
    F64,
    Bool,
}

impl KTy {
    pub fn name(self) -> &'static str {
        match self {
            KTy::Int => "Int",
            KTy::I32 => "I32",
            KTy::U32 => "U32",
            KTy::F32 => "F32",
            KTy::F64 => "F64",
            KTy::Bool => "Bool",
        }
    }

    pub fn from_name(n: &str) -> Option<KTy> {
        Some(match n {
            "Int" => KTy::Int,
            "I32" => KTy::I32,
            "U32" => KTy::U32,
            "F32" => KTy::F32,
            "F64" => KTy::F64,
            "Bool" => KTy::Bool,
            _ => return None,
        })
    }

    pub fn is_float(self) -> bool {
        matches!(self, KTy::F32 | KTy::F64)
    }

    pub fn is_int(self) -> bool {
        matches!(self, KTy::Int | KTy::I32 | KTy::U32)
    }

    pub fn range(self) -> (i128, i128) {
        match self {
            KTy::I32 => (i32::MIN as i128, i32::MAX as i128),
            KTy::U32 => (0, u32::MAX as i128),
            _ => (i64::MIN as i128, i64::MAX as i128),
        }
    }

    pub fn host(self) -> &'static str {
        match self {
            KTy::F32 | KTy::F64 => "F64",
            KTy::Bool => "Bool",
            _ => "Int",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KParam {
    pub name: String,
    pub ty: KTy,
    pub slice: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KIntr {
    Gid,
    Lid,
    GroupId,
    GroupSize,
    GridSize,
}

pub const INTRINSICS: &[(&str, KIntr)] = &[("gid", KIntr::Gid), ("lid", KIntr::Lid), ("group_id", KIntr::GroupId), ("group_size", KIntr::GroupSize), ("grid_size", KIntr::GridSize)];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KFn {
    Sqrt,
    Abs,
    Floor,
    Ceil,
    Round,
    Min,
    Max,
}

#[derive(Clone, Debug, PartialEq)]
pub enum KExpr {
    Lit(KTy, u64),
    Local(usize),
    Param(usize),
    Len(usize),
    Intr(KIntr),
    Load(usize, Box<KExpr>),
    Bin(BinOp, KTy, Box<KExpr>, Box<KExpr>),
    Neg(KTy, Box<KExpr>),
    Not(Box<KExpr>),
    Cast(KTy, KTy, Box<KExpr>),
    Call(KFn, KTy, Vec<KExpr>),
    If(Box<KExpr>, Box<KExpr>, Box<KExpr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum KStmt {
    Let(usize, KExpr),
    Set(usize, KExpr),
    Store(usize, KExpr, KExpr),
    For(usize, KExpr, KExpr, Vec<KStmt>),
    If(KExpr, Vec<KStmt>, Vec<KStmt>),
    Eval(KExpr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Kernel {
    pub name: String,
    pub params: Vec<KParam>,
    pub grid: KExpr,
    pub group: u32,
    pub pres: Vec<KExpr>,
    pub body: Vec<KStmt>,
    pub locals: Vec<KTy>,
    pub writes: Vec<bool>,
}

impl Kernel {
    pub fn uses_f64(&self) -> bool {
        self.params.iter().any(|p| p.ty == KTy::F64) || self.locals.contains(&KTy::F64) || self.body.iter().any(|s| stmt_has(s, &|e| matches!(e, KExpr::Lit(KTy::F64, _) | KExpr::Cast(KTy::F64, _, _) | KExpr::Cast(_, KTy::F64, _) | KExpr::Bin(_, KTy::F64, ..))))
    }

    pub fn uses_group(&self) -> bool {
        self.body.iter().any(|s| stmt_has(s, &|e| matches!(e, KExpr::Intr(KIntr::Lid | KIntr::GroupId | KIntr::GroupSize))))
    }
}

pub fn expr_has(e: &KExpr, p: &dyn Fn(&KExpr) -> bool) -> bool {
    if p(e) {
        return true;
    }
    match e {
        KExpr::Load(_, i) | KExpr::Neg(_, i) | KExpr::Not(i) | KExpr::Cast(_, _, i) => expr_has(i, p),
        KExpr::Bin(_, _, a, b) => expr_has(a, p) || expr_has(b, p),
        KExpr::Call(_, _, xs) => xs.iter().any(|x| expr_has(x, p)),
        KExpr::If(c, a, b) => expr_has(c, p) || expr_has(a, p) || expr_has(b, p),
        _ => false,
    }
}

pub fn stmt_has(s: &KStmt, p: &dyn Fn(&KExpr) -> bool) -> bool {
    match s {
        KStmt::Let(_, e) | KStmt::Set(_, e) | KStmt::Eval(e) => expr_has(e, p),
        KStmt::Store(_, i, v) => expr_has(i, p) || expr_has(v, p),
        KStmt::For(_, a, b, body) => expr_has(a, p) || expr_has(b, p) || body.iter().any(|s| stmt_has(s, p)),
        KStmt::If(c, t, f) => expr_has(c, p) || t.iter().chain(f).any(|s| stmt_has(s, p)),
    }
}

#[derive(Clone, Debug)]
pub struct KErr {
    pub code: &'static str,
    pub span: Span,
    pub msg: String,
    pub hint: Option<String>,
}

type KR<T> = Result<T, KErr>;

fn err<T>(code: &'static str, span: Span, msg: impl Into<String>) -> KR<T> {
    Err(KErr { code, span, msg: msg.into(), hint: None })
}

#[derive(Clone)]
struct Local {
    slot: usize,
    ty: KTy,
    mutable: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Def {
    Value(KExpr),
    Loop(KExpr, KExpr),
    Var,
}

struct Lower<'a> {
    f: &'a FnDef,
    params: Vec<KParam>,
    scopes: Vec<HashMap<String, Local>>,
    locals: Vec<KTy>,
    defs: Vec<Def>,
    body: bool,
    accesses: Vec<(usize, KExpr, Span, bool)>,
    errs: Vec<KErr>,
    float: KTy,
}

pub fn param_type(t: &Ty) -> Result<(KTy, Option<bool>), String> {
    let elem = |t: &Ty| match t {
        Ty::Named { name, args, .. } if args.is_empty() => KTy::from_name(name).filter(|k| *k != KTy::Bool),
        _ => None,
    };
    match t {
        Ty::Named { name, args, .. } if (name == "&" || name == "&mut") && args.len() == 1 => match &args[0] {
            Ty::Named { name: s, args: inner, .. } if s == "[]" && inner.len() == 1 => match elem(&inner[0]) {
                Some(k) => Ok((k, Some(name == "&mut"))),
                None => Err(format!("slice elements must be Int, I32, U32, F32 or F64, not {}", printer::ty(&inner[0]))),
            },
            other => Err(format!("kernel parameters borrow slices like &[F32] or &mut [F32], not {}{}", if name == "&" { "&" } else { "&mut " }, printer::ty(other))),
        },
        Ty::Named { name, args, .. } if args.is_empty() && KTy::from_name(name).is_some() => Ok((KTy::from_name(name).unwrap(), None)),
        other => Err(format!("kernel parameters are scalars (Int I32 U32 F32 F64 Bool) or slice borrows, not {}", printer::ty(other))),
    }
}

pub fn host_ty(p: &KParam) -> Ty {
    let span = Span::default();
    let named = |n: &str, args: Vec<Ty>| Ty::Named { name: n.into(), args, span };
    match p.slice {
        Some(_) => named("List", vec![named(p.ty.host(), vec![])]),
        None => named(p.ty.host(), vec![]),
    }
}

pub fn host_view(f: &FnDef) -> FnDef {
    let mut v = f.clone();
    for p in &mut v.params {
        p.ty = match param_type(&p.ty) {
            Ok((ty, slice)) => host_ty(&KParam { name: p.name.clone(), ty, slice }),
            Err(_) => Ty::Named { name: "Unit".into(), args: vec![], span: f.sig_span },
        };
    }
    v.ret = None;
    if !v.effects.iter().any(|e| e.name == "dev") {
        v.effects.push(Effect { name: "dev".into(), args: vec![], span: f.sig_span });
    }
    v.pres.clear();
    v.body = Expr::new(ExprKind::Unit, f.body.span);
    v.kernel = None;
    v
}

fn f32_bits(x: f64) -> u64 {
    (x as f32).to_bits() as u64
}

fn subnormal32(x: f32) -> bool {
    x != 0.0 && x.abs() < f32::MIN_POSITIVE
}

pub fn lower(f: &FnDef) -> Result<Kernel, Vec<KErr>> {
    let spec = f.kernel.as_ref().expect("kernel spec");
    let mut errs = Vec::new();
    let mut params = Vec::new();
    for p in &f.params {
        match param_type(&p.ty) {
            Ok((ty, slice)) => params.push(KParam { name: p.name.clone(), ty, slice }),
            Err(m) => errs.push(KErr { code: "E_KERNEL_SIG", span: f.sig_span, msg: format!("parameter '{}' of {}: {m}", p.name, f.name), hint: None }),
        }
        if p.refine.is_some() {
            errs.push(KErr { code: "E_KERNEL_SIG", span: f.sig_span, msg: format!("parameter '{}' of {} can't have a 'where' refinement; use 'pre'", p.name, f.name), hint: None });
        }
        if INTRINSICS.iter().any(|(n, _)| *n == p.name) {
            errs.push(KErr { code: "E_KERNEL_SIG", span: f.sig_span, msg: format!("parameter '{}' of {} shadows a thread index intrinsic", p.name, f.name), hint: None });
        }
    }
    if !f.tparams.is_empty() {
        errs.push(KErr { code: "E_KERNEL_SIG", span: f.sig_span, msg: format!("kernel {} can't be generic", f.name), hint: None });
    }
    if f.ret.is_some() {
        errs.push(KErr { code: "E_KERNEL_SIG", span: f.sig_span, msg: format!("kernel {} can't return a value; write results through a &mut slice", f.name), hint: None });
    }
    for e in &f.effects {
        if e.name != "dev" || !e.args.is_empty() {
            errs.push(KErr {
                code: "E_KERNEL_EFFECT",
                span: e.span,
                msg: format!("kernel {} performs '{}': kernels run on the device and may only declare 'dev'", f.name, printer::effect(e)),
                hint: Some("move host work (logging, errors, I/O) into the function that launches the kernel".into()),
            });
        }
    }
    if !f.posts.is_empty() || !f.examples.is_empty() || f.trusted.is_some() || f.interrupt.is_some() {
        errs.push(KErr { code: "E_KERNEL_SIG", span: f.sig_span, msg: format!("kernel {} can only have 'pre' clauses", f.name), hint: None });
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    let group = match spec.group.kind {
        ExprKind::Int(g) if (1..=1024).contains(&g) => g as u32,
        _ => return Err(vec![KErr { code: "E_KERNEL_GRID", span: spec.group.span, msg: format!("the group size of {} must be an Int literal from 1 to 1024", f.name), hint: None }]),
    };
    let float = if params.iter().any(|p| p.ty == KTy::F64) && !params.iter().any(|p| p.ty == KTy::F32) { KTy::F64 } else { KTy::F32 };
    let mut lw = Lower { f, params, scopes: vec![HashMap::new()], locals: vec![], defs: vec![], body: false, accesses: vec![], errs: vec![], float };
    let grid = match lw.expr(&spec.grid, Some(KTy::Int)) {
        Ok((g, KTy::Int)) => Some(g),
        Ok((_, t)) => {
            lw.errs.push(KErr { code: "E_KERNEL_TYPE", span: spec.grid.span, msg: format!("the thread count of {} must be Int, got {}", f.name, t.name()), hint: None });
            None
        }
        Err(e) => {
            lw.errs.push(e);
            None
        }
    };
    let mut pres = Vec::new();
    for p in &f.pres {
        match lw.expr(p, Some(KTy::Bool)) {
            Ok((x, KTy::Bool)) => pres.push(x),
            Ok((_, t)) => lw.errs.push(KErr { code: "E_KERNEL_TYPE", span: p.span, msg: format!("pre must be Bool, got {}", t.name()), hint: None }),
            Err(e) => lw.errs.push(e),
        }
    }
    lw.body = true;
    let body = match lw.block(&f.body) {
        Ok(b) => b,
        Err(e) => {
            lw.errs.push(e);
            vec![]
        }
    };
    lw.race_check();
    if !lw.errs.is_empty() {
        return Err(lw.errs);
    }
    let mut writes = vec![false; lw.params.len()];
    for (p, _, _, w) in &lw.accesses {
        if *w {
            writes[*p] = true;
        }
    }
    Ok(Kernel { name: f.name.clone(), params: lw.params, grid: grid.unwrap(), group, pres, body, locals: lw.locals, writes })
}

fn is_lit(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) => true,
        ExprKind::Unary(UnOp::Neg, x) => is_lit(x),
        _ => false,
    }
}

impl Lower<'_> {
    fn lookup(&self, n: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|s| s.get(n))
    }

    fn param(&self, n: &str) -> Option<usize> {
        self.params.iter().position(|p| p.name == n)
    }

    fn bind(&mut self, n: &str, ty: KTy, mutable: bool, def: Def, span: Span) -> KR<usize> {
        if self.param(n).is_some() || INTRINSICS.iter().any(|(i, _)| *i == n) || self.lookup(n).is_some() {
            return err("E_KERNEL", span, format!("'{n}' is already defined; kernel locals can't shadow parameters, intrinsics or other locals"));
        }
        let slot = self.locals.len();
        self.locals.push(ty);
        self.defs.push(def);
        self.scopes.last_mut().unwrap().insert(n.to_string(), Local { slot, ty, mutable });
        Ok(slot)
    }

    fn block(&mut self, e: &Expr) -> KR<Vec<KStmt>> {
        self.scopes.push(HashMap::new());
        let r = self.block_inner(e);
        self.scopes.pop();
        r
    }

    fn block_inner(&mut self, e: &Expr) -> KR<Vec<KStmt>> {
        match &e.kind {
            ExprKind::Block(stmts) => {
                let mut out = Vec::new();
                for s in stmts {
                    match self.stmt(s) {
                        Ok(Some(k)) => out.push(k),
                        Ok(None) => {}
                        Err(e) => self.errs.push(e),
                    }
                }
                Ok(out)
            }
            ExprKind::Unit => Ok(vec![]),
            _ => Ok(self.stmt(&Stmt::Expr(e.clone()))?.into_iter().collect()),
        }
    }

    fn stmt(&mut self, s: &Stmt) -> KR<Option<KStmt>> {
        match s {
            Stmt::Let(Pat::Bind(n), e) => {
                let (x, t) = self.expr(e, None)?;
                let slot = self.bind(n, t, false, Def::Value(x.clone()), e.span)?;
                Ok(Some(KStmt::Let(slot, x)))
            }
            Stmt::Let(Pat::Wild, e) => {
                let (x, _) = self.expr(e, None)?;
                Ok(Some(KStmt::Eval(x)))
            }
            Stmt::Let(_, e) => err("E_KERNEL", e.span, "kernels bind single names; tuples and patterns are not available on the device"),
            Stmt::Var(n, e) => {
                let (x, t) = self.expr(e, None)?;
                let slot = self.bind(n, t, true, Def::Var, e.span)?;
                Ok(Some(KStmt::Let(slot, x)))
            }
            Stmt::Assign(n, e, span) => {
                if let ExprKind::With(base, ups) = &e.kind
                    && matches!(&base.kind, ExprKind::Name(b) if b == n)
                    && let Some(p) = self.param(n)
                {
                    let [(path, value)] = ups.as_slice() else { return err("E_KERNEL", e.span, "assign one element at a time") };
                    let [PathSeg::Index(i)] = path.as_slice() else { return err("E_KERNEL", e.span, format!("'{n}' is a slice; assign elements with {n}[i] := v")) };
                    if self.params[p].slice != Some(true) {
                        return err("E_KERNEL_WRITE", *span, format!("'{n}' is a read-only slice; declare it '&mut [{}]' to write it", self.params[p].ty.name()));
                    }
                    let (ix, it) = self.expr(i, Some(KTy::Int))?;
                    if it != KTy::Int {
                        return err("E_KERNEL_TYPE", i.span, format!("slice indices are Int, got {}", it.name()));
                    }
                    let et = self.params[p].ty;
                    let (v, vt) = self.expr(value, Some(et))?;
                    if vt != et {
                        return err("E_KERNEL_TYPE", value.span, format!("'{n}' holds {}, got {}", et.name(), vt.name()));
                    }
                    self.accesses.push((p, ix.clone(), i.span, true));
                    return Ok(Some(KStmt::Store(p, ix, v)));
                }
                if self.param(n).is_some() {
                    return err("E_KERNEL_WRITE", *span, format!("kernel parameter '{n}' can't be reassigned"));
                }
                let Some(l) = self.lookup(n).cloned() else { return err("E_KERNEL", *span, format!("unknown name '{n}'")) };
                if !l.mutable {
                    return err("E_KERNEL_WRITE", *span, format!("'{n}' is not a var"));
                }
                let (x, t) = self.expr(e, Some(l.ty))?;
                if t != l.ty {
                    return err("E_KERNEL_TYPE", e.span, format!("'{n}' is {}, got {}", l.ty.name(), t.name()));
                }
                Ok(Some(KStmt::Set(l.slot, x)))
            }
            Stmt::For(Pat::Bind(i), it, body) => {
                let ExprKind::Range(a, b) = &it.kind else { return err("E_KERNEL", it.span, "kernel loops run over Int ranges 'a..b'") };
                let (lo, lt) = self.expr(a, Some(KTy::Int))?;
                let (hi, ht) = self.expr(b, Some(KTy::Int))?;
                if lt != KTy::Int || ht != KTy::Int {
                    return err("E_KERNEL_TYPE", it.span, "range bounds must be Int");
                }
                self.scopes.push(HashMap::new());
                let slot = self.bind(i, KTy::Int, false, Def::Loop(lo.clone(), hi.clone()), it.span);
                let r = slot.and_then(|slot| self.block_inner(body).map(|b| (slot, b)));
                self.scopes.pop();
                let (slot, b) = r?;
                Ok(Some(KStmt::For(slot, lo, hi, b)))
            }
            Stmt::For(_, it, _) => err("E_KERNEL", it.span, "kernel loops bind one Int name"),
            Stmt::While(c, _) => err("E_KERNEL", c.span, "kernels can't use while loops: every thread must finish, so loop over a range instead"),
            Stmt::Fn(f) => err("E_KERNEL", f.sig_span, "kernels can't define local functions"),
            Stmt::Expr(e) => match &e.kind {
                ExprKind::If(c, t, f) if f.is_none() || self.is_stmt_branch(t) => {
                    let (cx, ct) = self.expr(c, Some(KTy::Bool))?;
                    if ct != KTy::Bool {
                        return err("E_KERNEL_TYPE", c.span, format!("if conditions are Bool, got {}", ct.name()));
                    }
                    let tb = self.block(t)?;
                    let fb = match f {
                        Some(f) => self.block(f)?,
                        None => vec![],
                    };
                    Ok(Some(KStmt::If(cx, tb, fb)))
                }
                ExprKind::Block(_) => {
                    let b = self.block(e)?;
                    Ok(Some(KStmt::If(KExpr::Lit(KTy::Bool, 1), b, vec![])))
                }
                ExprKind::Unit => Ok(None),
                _ => {
                    let (x, _) = self.expr(e, None)?;
                    Ok(Some(KStmt::Eval(x)))
                }
            },
        }
    }

    fn is_stmt_branch(&self, e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Block(stmts) => stmts.iter().any(|s| !matches!(s, Stmt::Expr(_))) || stmts.last().is_some_and(|s| matches!(s, Stmt::Expr(x) if self.is_stmt_branch(x))),
            ExprKind::If(_, t, f) => f.is_none() || self.is_stmt_branch(t),
            ExprKind::Unit => true,
            _ => false,
        }
    }

    fn lit(&self, e: &Expr, hint: Option<KTy>) -> KR<(KExpr, KTy)> {
        let (neg, inner) = match &e.kind {
            ExprKind::Unary(UnOp::Neg, x) => (true, &**x),
            _ => (false, e),
        };
        match (&inner.kind, hint) {
            (ExprKind::Int(n), Some(t)) if t.is_int() || t.is_float() => {
                let v = if neg { -(*n as i128) } else { *n as i128 };
                if t.is_float() {
                    let x = v as f64;
                    if x as i128 != v || (t == KTy::F32 && (x as f32) as f64 != x) {
                        return err("E_KERNEL_TYPE", e.span, format!("{v} is not exactly representable as {}", t.name()));
                    }
                    let bits = if t == KTy::F32 { f32_bits(x) } else { x.to_bits() };
                    return Ok((KExpr::Lit(t, bits), t));
                }
                let (lo, hi) = t.range();
                if v < lo || v > hi {
                    return err("E_KERNEL_TYPE", e.span, format!("{v} is out of range for {}", t.name()));
                }
                Ok((KExpr::Lit(t, v as i64 as u64), t))
            }
            (ExprKind::Int(n), _) => {
                let v = if neg { n.checked_neg() } else { Some(*n) };
                match v {
                    Some(v) => Ok((KExpr::Lit(KTy::Int, v as u64), KTy::Int)),
                    None => err("E_KERNEL_TYPE", e.span, "integer literal overflows Int"),
                }
            }
            (ExprKind::Float(x), h) => {
                let x = if neg { -*x } else { *x };
                match h.unwrap_or(self.float) {
                    KTy::F32 if subnormal32(x as f32) => err("E_KERNEL_TYPE", e.span, format!("{x} is subnormal as F32; GPUs flush subnormals, so kernels don't accept them as literals")),
                    KTy::F32 => Ok((KExpr::Lit(KTy::F32, f32_bits(x)), KTy::F32)),
                    KTy::F64 => Ok((KExpr::Lit(KTy::F64, x.to_bits()), KTy::F64)),
                    t => err("E_KERNEL_TYPE", e.span, format!("expected {}, got a float literal", t.name())),
                }
            }
            _ => unreachable!(),
        }
    }

    fn expr(&mut self, e: &Expr, hint: Option<KTy>) -> KR<(KExpr, KTy)> {
        if is_lit(e) {
            return self.lit(e, hint);
        }
        match &e.kind {
            ExprKind::Bool(b) => Ok((KExpr::Lit(KTy::Bool, *b as u64), KTy::Bool)),
            ExprKind::Name(n) => {
                if let Some(l) = self.lookup(n) {
                    return Ok((KExpr::Local(l.slot), l.ty));
                }
                if let Some(p) = self.param(n) {
                    if self.params[p].slice.is_some() {
                        return err("E_KERNEL", e.span, format!("slice '{n}' can only be indexed ({n}[i]) or measured ({n}.len)"));
                    }
                    return Ok((KExpr::Param(p), self.params[p].ty));
                }
                if let Some((_, k)) = INTRINSICS.iter().find(|(i, _)| i == n) {
                    if !self.body && *k != KIntr::GroupSize {
                        return err("E_KERNEL", e.span, format!("'{n}' is only defined inside the kernel body"));
                    }
                    if !self.body {
                        return Ok((KExpr::Lit(KTy::Int, self.f.kernel.as_ref().map_or(1, |k| match k.group.kind {
                            ExprKind::Int(g) => g as u64,
                            _ => 1,
                        })), KTy::Int));
                    }
                    return Ok((KExpr::Intr(*k), KTy::Int));
                }
                err("E_KERNEL", e.span, format!("unknown name '{n}' in a kernel; kernels see their parameters, locals and gid lid group_id group_size grid_size"))
            }
            ExprKind::Field(r, m) | ExprKind::Method { recv: r, name: m, .. } if m == "len" && matches!(&r.kind, ExprKind::Name(n) if self.param(n).is_some_and(|p| self.params[p].slice.is_some())) => {
                let ExprKind::Name(n) = &r.kind else { unreachable!() };
                if matches!(&e.kind, ExprKind::Method { args, .. } if !args.is_empty()) {
                    return err("E_KERNEL", e.span, "len takes no arguments");
                }
                Ok((KExpr::Len(self.param(n).unwrap()), KTy::Int))
            }
            ExprKind::Field(r, m) => self.method(e, r, m, &[], hint),
            ExprKind::Method { recv, name, args, targs } if targs.is_empty() => self.method(e, recv, name, args, hint),
            ExprKind::Index(a, i) => {
                let ExprKind::Name(n) = &a.kind else { return err("E_KERNEL", a.span, "only slice parameters can be indexed") };
                let Some(p) = self.param(n).filter(|p| self.params[*p].slice.is_some()) else { return err("E_KERNEL", a.span, format!("'{n}' is not a slice parameter")) };
                if !self.body {
                    return err("E_KERNEL", e.span, "slices can only be read in the kernel body");
                }
                let (ix, it) = self.expr(i, Some(KTy::Int))?;
                if it != KTy::Int {
                    return err("E_KERNEL_TYPE", i.span, format!("slice indices are Int, got {}", it.name()));
                }
                self.accesses.push((p, ix.clone(), i.span, false));
                Ok((KExpr::Load(p, Box::new(ix)), self.params[p].ty))
            }
            ExprKind::Binary(op, l, r) => self.binary(*op, l, r, hint, e.span),
            ExprKind::Unary(UnOp::Neg, x) => {
                let (v, t) = self.expr(x, hint)?;
                if !(t.is_float() || matches!(t, KTy::Int | KTy::I32)) {
                    return err("E_KERNEL_TYPE", e.span, format!("cannot negate {}", t.name()));
                }
                Ok((KExpr::Neg(t, Box::new(v)), t))
            }
            ExprKind::Unary(UnOp::Not, x) => {
                let (v, t) = self.expr(x, Some(KTy::Bool))?;
                if t != KTy::Bool {
                    return err("E_KERNEL_TYPE", e.span, format!("'not' needs Bool, got {}", t.name()));
                }
                Ok((KExpr::Not(Box::new(v)), KTy::Bool))
            }
            ExprKind::If(c, t, Some(f)) => {
                let (cx, ct) = self.expr(c, Some(KTy::Bool))?;
                if ct != KTy::Bool {
                    return err("E_KERNEL_TYPE", c.span, format!("if conditions are Bool, got {}", ct.name()));
                }
                let (tx, tt, fx, ft) = if is_lit(t) && !is_lit(f) {
                    let (fx, ft) = self.expr(f, hint)?;
                    let (tx, tt) = self.expr(t, Some(ft))?;
                    (tx, tt, fx, ft)
                } else {
                    let (tx, tt) = self.expr(t, hint)?;
                    let (fx, ft) = self.expr(f, Some(tt))?;
                    (tx, tt, fx, ft)
                };
                if tt != ft {
                    return err("E_KERNEL_TYPE", e.span, format!("if branches have different types: {} and {}", tt.name(), ft.name()));
                }
                Ok((KExpr::If(Box::new(cx), Box::new(tx), Box::new(fx)), tt))
            }
            ExprKind::Block(stmts) if stmts.len() == 1 && matches!(&stmts[0], Stmt::Expr(_)) => {
                let Stmt::Expr(x) = &stmts[0] else { unreachable!() };
                self.expr(x, hint)
            }
            ExprKind::Call(f, _) => match &f.kind {
                ExprKind::Name(n) => err("E_KERNEL", e.span, format!("kernels can't call '{n}': device code has no calls, so no recursion or allocation; inline the computation")),
                _ => err("E_KERNEL", e.span, "kernels can't call function values"),
            },
            ExprKind::Str(_) => err("E_KERNEL", e.span, "kernels can't use strings"),
            ExprKind::List(_) | ExprKind::Record { .. } | ExprKind::Tuple(_) | ExprKind::Lambda { .. } | ExprKind::With(..) => err("E_KERNEL", e.span, "kernels can't allocate: no lists, records, tuples or closures on the device"),
            ExprKind::Raise(_) | ExprKind::Catch(..) | ExprKind::Handle(..) | ExprKind::Par(_) => err("E_KERNEL_EFFECT", e.span, "kernels can't raise, handle effects or start tasks"),
            ExprKind::Range(..) => err("E_KERNEL", e.span, "ranges are only for 'for' loops in kernels"),
            _ => err("E_KERNEL", e.span, "this expression is not available in kernels"),
        }
    }

    fn method(&mut self, e: &Expr, recv: &Expr, name: &str, args: &[Expr], hint: Option<KTy>) -> KR<(KExpr, KTy)> {
        let conv = match name {
            "to_int" => Some(KTy::Int),
            "to_i32" => Some(KTy::I32),
            "to_u32" => Some(KTy::U32),
            "to_f32" => Some(KTy::F32),
            "to_f64" => Some(KTy::F64),
            _ => None,
        };
        if let Some(to) = conv {
            if !args.is_empty() {
                return err("E_KERNEL", e.span, format!("{name} takes no arguments"));
            }
            let (v, from) = self.expr(recv, None)?;
            if from == KTy::Bool || (from.is_float() && to.is_int()) {
                return err("E_KERNEL_TYPE", e.span, format!("{} has no .{name} in kernels", from.name()));
            }
            if from == to {
                return Ok((v, to));
            }
            return Ok((KExpr::Cast(to, from, Box::new(v)), to));
        }
        let f = match name {
            "sqrt" => KFn::Sqrt,
            "abs" => KFn::Abs,
            "floor" => KFn::Floor,
            "ceil" => KFn::Ceil,
            "round" => KFn::Round,
            "min" => KFn::Min,
            "max" => KFn::Max,
            _ => {
                self.expr(recv, None)?;
                return err("E_KERNEL", e.span, format!("'.{name}' is not available in kernels"));
            }
        };
        let binary = matches!(f, KFn::Min | KFn::Max);
        if args.len() != usize::from(binary) {
            return err("E_KERNEL", e.span, format!("{name} takes {} argument(s)", usize::from(binary)));
        }
        let (a, t, b) = if binary {
            let (a, t, b, _) = self.pair(recv, &args[0], hint, e.span)?;
            (a, t, Some(b))
        } else {
            let (a, t) = self.expr(recv, hint)?;
            (a, t, None)
        };
        let ok = match f {
            KFn::Sqrt | KFn::Floor | KFn::Ceil | KFn::Round => t.is_float(),
            KFn::Abs => t.is_float() || matches!(t, KTy::Int | KTy::I32),
            KFn::Min | KFn::Max => t != KTy::Bool,
        };
        if !ok {
            return err("E_KERNEL_TYPE", e.span, format!("{} has no .{name}", t.name()));
        }
        Ok((KExpr::Call(f, t, std::iter::once(a).chain(b).collect()), t))
    }

    fn pair(&mut self, l: &Expr, r: &Expr, hint: Option<KTy>, span: Span) -> KR<(KExpr, KTy, KExpr, KTy)> {
        let (a, at, b, bt) = if is_lit(l) && !is_lit(r) {
            let (b, bt) = self.expr(r, hint)?;
            let (a, at) = self.expr(l, Some(bt))?;
            (a, at, b, bt)
        } else {
            let (a, at) = self.expr(l, hint)?;
            let (b, bt) = self.expr(r, Some(at))?;
            (a, at, b, bt)
        };
        if at != bt {
            return err("E_KERNEL_TYPE", span, format!("operands have different types: {} and {}; convert with .to_f32, .to_i32 and friends", at.name(), bt.name()));
        }
        Ok((a, at, b, bt))
    }

    fn binary(&mut self, op: BinOp, l: &Expr, r: &Expr, hint: Option<KTy>, span: Span) -> KR<(KExpr, KTy)> {
        use BinOp::*;
        match op {
            And | Or => {
                let (a, at) = self.expr(l, Some(KTy::Bool))?;
                let (b, bt) = self.expr(r, Some(KTy::Bool))?;
                if at != KTy::Bool || bt != KTy::Bool {
                    return err("E_KERNEL_TYPE", span, format!("'{}' needs Bool operands", op.symbol()));
                }
                Ok((KExpr::Bin(op, KTy::Bool, Box::new(a), Box::new(b)), KTy::Bool))
            }
            Eq | Ne | Lt | Le | Gt | Ge => {
                let (a, t, b, _) = self.pair(l, r, None, span)?;
                if t == KTy::Bool && !matches!(op, Eq | Ne) {
                    return err("E_KERNEL_TYPE", span, "Bool values are compared with == and != only");
                }
                Ok((KExpr::Bin(op, t, Box::new(a), Box::new(b)), KTy::Bool))
            }
            Pow => err("E_KERNEL", span, "'**' is not available in kernels; multiply explicitly"),
            Add | Sub | Mul | Div | Rem => {
                let h = hint.filter(|t| *t != KTy::Bool);
                let (a, t, b, _) = self.pair(l, r, h, span)?;
                if t == KTy::Bool {
                    return err("E_KERNEL_TYPE", span, format!("'{}' needs numbers, got Bool", op.symbol()));
                }
                if op == Rem && t.is_float() {
                    return err("E_KERNEL_TYPE", span, "'%' on floats is not available in kernels");
                }
                Ok((KExpr::Bin(op, t, Box::new(a), Box::new(b)), t))
            }
        }
    }

    fn deref<'b>(&'b self, e: &'b KExpr) -> &'b KExpr {
        match e {
            KExpr::Local(s) => match &self.defs[*s] {
                Def::Value(x) => self.deref(x),
                _ => e,
            },
            _ => e,
        }
    }

    fn uniform(&self, e: &KExpr) -> Option<String> {
        Some(match self.deref(e) {
            KExpr::Lit(t, b) => format!("{}:{b}", t.name()),
            KExpr::Param(p) => format!("p{p}"),
            KExpr::Len(p) => format!("len{p}"),
            KExpr::Intr(KIntr::GroupSize) => "group_size".into(),
            KExpr::Intr(KIntr::GridSize) => "grid_size".into(),
            KExpr::Bin(op, _, a, b) => format!("({} {} {})", self.uniform(a)?, op.symbol(), self.uniform(b)?),
            KExpr::Neg(_, a) => format!("(-{})", self.uniform(a)?),
            KExpr::Cast(t, _, a) => format!("{}({})", t.name(), self.uniform(a)?),
            _ => return None,
        })
    }

    fn is_gid(&self, e: &KExpr) -> bool {
        matches!(self.deref(e), KExpr::Intr(KIntr::Gid))
    }

    fn shape(&self, e: &KExpr) -> Option<(bool, String)> {
        let e = self.deref(e);
        if self.is_gid(e) {
            return Some((false, "0".into()));
        }
        match e {
            KExpr::Bin(BinOp::Add, KTy::Int, a, b) => {
                if self.is_gid(a)
                    && let Some(u) = self.uniform(b)
                {
                    return Some((false, u));
                }
                if self.is_gid(b)
                    && let Some(u) = self.uniform(a)
                {
                    return Some((false, u));
                }
                for (m, j) in [(a, b), (b, a)] {
                    if let Some(s) = self.stride(m)
                        && let KExpr::Local(js) = self.deref(j)
                        && let Def::Loop(lo, hi) = &self.defs[*js]
                        && matches!(self.deref(lo), KExpr::Lit(KTy::Int, 0))
                        && self.uniform(hi).as_ref() == Some(&s)
                    {
                        return Some((true, s));
                    }
                }
                None
            }
            KExpr::Bin(BinOp::Sub, KTy::Int, a, b) if self.is_gid(a) => self.uniform(b).map(|u| (false, format!("(-{u})"))),
            _ => self.stride(e).map(|s| (true, s)),
        }
    }

    fn stride(&self, e: &KExpr) -> Option<String> {
        match self.deref(e) {
            KExpr::Bin(BinOp::Mul, KTy::Int, a, b) if self.is_gid(a) => self.uniform(b),
            KExpr::Bin(BinOp::Mul, KTy::Int, a, b) if self.is_gid(b) => self.uniform(a),
            _ => None,
        }
    }

    fn race_check(&mut self) {
        for p in 0..self.params.len() {
            let acc: Vec<(KExpr, Span, bool)> = self.accesses.iter().filter(|a| a.0 == p).map(|a| (a.1.clone(), a.2, a.3)).collect();
            if !acc.iter().any(|a| a.2) {
                continue;
            }
            let name = self.params[p].name.clone();
            let mut first: Option<(bool, String)> = None;
            for (ix, span, write) in &acc {
                let what = if *write { "writes" } else { "reads" };
                match self.shape(ix) {
                    None => {
                        self.errs.push(KErr {
                            code: "E_KERNEL_RACE",
                            span: *span,
                            msg: format!("{} {what} {name} at an index that two threads may share", self.f.name),
                            hint: Some(format!("index {name} by gid (plus a uniform offset), or by gid * s + j with 'for j in 0..s', so each thread owns its elements")),
                        });
                        return;
                    }
                    Some(sh) => match &first {
                        None => first = Some(sh),
                        Some(f) if *f == sh => {}
                        Some(_) => {
                            self.errs.push(KErr {
                                code: "E_KERNEL_RACE",
                                span: *span,
                                msg: format!("{} {what} {name} at an element that another thread writes", self.f.name),
                                hint: Some(format!("every access to {name} must use the same per-thread index")),
                            });
                            return;
                        }
                    },
                }
            }
        }
    }
}
