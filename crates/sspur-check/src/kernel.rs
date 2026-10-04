use sspur_syntax::*;
use std::collections::{HashMap, HashSet};

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
    Shl,
    Shr,
    Band,
    Bor,
    Bxor,
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
    SLoad(usize, Box<KExpr>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum KStmt {
    Let(usize, KExpr),
    Set(usize, KExpr),
    Store(usize, KExpr, KExpr),
    For(usize, KExpr, KExpr, Vec<KStmt>),
    If(KExpr, Vec<KStmt>, Vec<KStmt>),
    Eval(KExpr),
    SStore(usize, KExpr, KExpr),
    Barrier,
    Atomic(AOp, bool, usize, KExpr, KExpr, Option<KExpr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AOp {
    Add,
    Min,
    Max,
    Cas,
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
    pub shared: Vec<(String, KTy, u32)>,
    pub grouped: bool,
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
        KExpr::Load(_, i) | KExpr::SLoad(_, i) | KExpr::Neg(_, i) | KExpr::Not(i) | KExpr::Cast(_, _, i) => expr_has(i, p),
        KExpr::Bin(_, _, a, b) => expr_has(a, p) || expr_has(b, p),
        KExpr::Call(_, _, xs) => xs.iter().any(|x| expr_has(x, p)),
        KExpr::If(c, a, b) => expr_has(c, p) || expr_has(a, p) || expr_has(b, p),
        _ => false,
    }
}

pub fn stmt_has(s: &KStmt, p: &dyn Fn(&KExpr) -> bool) -> bool {
    match s {
        KStmt::Let(_, e) | KStmt::Set(_, e) | KStmt::Eval(e) => expr_has(e, p),
        KStmt::Store(_, i, v) | KStmt::SStore(_, i, v) => expr_has(i, p) || expr_has(v, p),
        KStmt::Atomic(_, _, _, i, v, w) => expr_has(i, p) || expr_has(v, p) || w.as_ref().is_some_and(|w| expr_has(w, p)),
        KStmt::For(_, a, b, body) => expr_has(a, p) || expr_has(b, p) || body.iter().any(|s| stmt_has(s, p)),
        KStmt::If(c, t, f) => expr_has(c, p) || t.iter().chain(f).any(|s| stmt_has(s, p)),
        KStmt::Barrier => false,
    }
}

pub fn has_barrier(s: &KStmt) -> bool {
    match s {
        KStmt::Barrier => true,
        KStmt::For(_, _, _, b) => b.iter().any(has_barrier),
        KStmt::If(_, t, f) => t.iter().chain(f).any(has_barrier),
        _ => false,
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

#[derive(Clone)]
struct SAcc {
    arr: usize,
    segs: Vec<usize>,
    write: bool,
    atomic: Option<String>,
    idx: KExpr,
    guards: Vec<KExpr>,
    ver: HashMap<usize, u32>,
    loops: Vec<usize>,
    span: Span,
}

type Lin = (std::collections::BTreeMap<String, i128>, i128);

struct Lower<'a> {
    f: &'a FnDef,
    params: Vec<KParam>,
    scopes: Vec<HashMap<String, Local>>,
    locals: Vec<KTy>,
    defs: Vec<Def>,
    body: bool,
    accesses: Vec<(usize, KExpr, Span, bool, Vec<KExpr>, Option<String>)>,
    errs: Vec<KErr>,
    float: KTy,
    group: u32,
    shared: Vec<(String, KTy, u32)>,
    cf: Vec<bool>,
    guards: Vec<KExpr>,
    loops: Vec<usize>,
    cur: Vec<usize>,
    nseg: usize,
    sacc: Vec<SAcc>,
    free_loops: HashSet<usize>,
    barriers: usize,
    nest: usize,
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
    let mut lw = Lower {
        f,
        params,
        scopes: vec![HashMap::new()],
        locals: vec![],
        defs: vec![],
        body: false,
        accesses: vec![],
        errs: vec![],
        float,
        group,
        shared: vec![],
        cf: vec![],
        guards: vec![],
        loops: vec![],
        cur: vec![0],
        nseg: 1,
        sacc: vec![],
        free_loops: HashSet::new(),
        barriers: 0,
        nest: 0,
    };
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
    lw.shared_check();
    if !lw.errs.is_empty() {
        return Err(lw.errs);
    }
    let mut writes = vec![false; lw.params.len()];
    for (p, _, _, w, _, _) in &lw.accesses {
        if *w {
            writes[*p] = true;
        }
    }
    let grouped = lw.barriers > 0 || !lw.shared.is_empty();
    Ok(Kernel { name: f.name.clone(), params: lw.params, grid: grid.unwrap(), group, pres, body, locals: lw.locals, writes, shared: lw.shared, grouped })
}

fn atomic_op(n: &str) -> Option<AOp> {
    Some(match n {
        "atomic_add" => AOp::Add,
        "atomic_min" => AOp::Min,
        "atomic_max" => AOp::Max,
        "atomic_cas" => AOp::Cas,
        _ => return None,
    })
}

fn shared_decl(e: &Expr) -> Option<(&str, &Expr)> {

    let ExprKind::Call(f, args) = &e.kind else { return None };
    let ExprKind::Index(s, t) = &f.kind else { return None };
    match (&s.kind, &t.kind, args.as_slice()) {
        (ExprKind::Name(s), ExprKind::Name(t), [n]) if s == "shared" => Some((t.as_str(), n)),
        _ => None,
    }
}

fn es_bytes(t: KTy) -> i64 {
    if matches!(t, KTy::F32 | KTy::I32 | KTy::U32) { 4 } else { 8 }
}

fn lin_add(a: &Lin, b: &Lin, k: i128) -> Lin {
    let mut m = a.0.clone();
    for (x, c) in &b.0 {
        *m.entry(x.clone()).or_insert(0) += k * c;
    }
    m.retain(|_, c| *c != 0);
    (m, a.1 + k * b.1)
}

fn lin_le(a: &Lin, b: &Lin) -> bool {
    let d = lin_add(b, a, -1);
    d.0.is_empty() && d.1 >= 0
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

    fn shared_of(&self, n: &str) -> Option<usize> {
        self.shared.iter().position(|s| s.0 == n)
    }

    fn fresh_seg(&mut self) -> usize {
        self.nseg += 1;
        self.nseg - 1
    }

    fn record(&mut self, arr: usize, write: bool, idx: &KExpr, span: Span) {
        self.sacc.push(SAcc { arr, segs: self.cur.clone(), write, atomic: None, idx: idx.clone(), guards: self.guards.clone(), ver: HashMap::new(), loops: self.loops.clone(), span });
    }

    fn bind(&mut self, n: &str, ty: KTy, mutable: bool, def: Def, span: Span) -> KR<usize> {
        if self.param(n).is_some() || INTRINSICS.iter().any(|(i, _)| *i == n) || self.lookup(n).is_some() || self.shared_of(n).is_some() {
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
            Stmt::Let(Pat::Bind(n), e) if shared_decl(e).is_some() => {
                let (tn, len) = shared_decl(e).unwrap();
                let Some(t) = KTy::from_name(tn).filter(|t| *t != KTy::Bool) else { return err("E_KERNEL_SHARED", e.span, format!("shared arrays hold Int, I32, U32, F32 or F64, not {tn}")) };
                if self.nest > 0 {
                    return err("E_KERNEL_SHARED", e.span, "declare shared arrays at the top of the kernel body, not inside 'if' or 'for'");
                }
                let (lx, _) = self.expr(len, Some(KTy::Int))?;
                let Some(n_) = self.konst(&lx).filter(|v| (1..=1 << 20).contains(v)) else { return err("E_KERNEL_SHARED", len.span, "the length of a shared array must be a positive constant (literals and group_size)") };
                if self.param(n).is_some() || INTRINSICS.iter().any(|(i, _)| *i == n) || self.lookup(n).is_some() || self.shared_of(n).is_some() {
                    return err("E_KERNEL", e.span, format!("'{n}' is already defined; kernel locals can't shadow parameters, intrinsics or other locals"));
                }
                let bytes: i64 = self.shared.iter().map(|s| s.2 as i64 * es_bytes(s.1)).sum::<i64>() + n_ * es_bytes(t);
                if bytes > 32768 {
                    return err("E_KERNEL_SHARED", e.span, format!("{} needs {bytes} bytes of shared memory; the limit is 32768", self.f.name));
                }
                self.shared.push((n.clone(), t, n_ as u32));
                Ok(None)
            }
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
                    && let Some(a) = self.shared_of(n)
                {
                    let [(path, value)] = ups.as_slice() else { return err("E_KERNEL", e.span, "assign one element at a time") };
                    let [PathSeg::Index(i)] = path.as_slice() else { return err("E_KERNEL", e.span, format!("'{n}' is a shared array; assign elements with {n}[i] := v")) };
                    let (ix, it) = self.expr(i, Some(KTy::Int))?;
                    if it != KTy::Int {
                        return err("E_KERNEL_TYPE", i.span, format!("shared array indices are Int, got {}", it.name()));
                    }
                    let et = self.shared[a].1;
                    let (v, vt) = self.expr(value, Some(et))?;
                    if vt != et {
                        return err("E_KERNEL_TYPE", value.span, format!("'{n}' holds {}, got {}", et.name(), vt.name()));
                    }
                    self.record(a, true, &ix, i.span);
                    return Ok(Some(KStmt::SStore(a, ix, v)));
                }
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
                    self.accesses.push((p, ix.clone(), i.span, true, self.guards.clone(), None));
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
                let uni = self.guniform(&lo) && self.guniform(&hi);
                let slot = self.bind(i, KTy::Int, false, Def::Loop(lo.clone(), hi.clone()), it.span);
                let (c0, nb, na) = (self.cur.clone(), self.barriers, self.sacc.len());
                self.cf.push(uni);
                self.nest += 1;
                let pushed = slot.is_ok();
                if let Ok(s) = &slot {
                    self.loops.push(*s);
                }
                let r = slot.and_then(|slot| self.block_inner(body).map(|b| (slot, b)));
                if pushed {
                    self.loops.pop();
                }
                self.nest -= 1;
                self.cf.pop();
                self.scopes.pop();
                let (slot, b) = r?;
                if self.barriers == nb {
                    self.free_loops.insert(slot);
                } else {
                    let once = matches!((self.deref(&lo), self.deref(&hi)), (KExpr::Lit(KTy::Int, a), KExpr::Lit(KTy::Int, b)) if (*b as i64) > (*a as i64));
                    self.back_edge(slot, &c0, na, !once);
                }
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
                    let uni = self.guniform(&cx);
                    let c0 = self.cur.clone();
                    self.cf.push(uni);
                    self.nest += 1;
                    self.guards.push(cx.clone());
                    let tb = self.block(t);
                    self.guards.pop();
                    let c1 = std::mem::replace(&mut self.cur, c0);
                    let fb = match f {
                        Some(f) => self.block(f),
                        None => Ok(vec![]),
                    };
                    self.nest -= 1;
                    self.cf.pop();
                    for x in c1 {
                        if !self.cur.contains(&x) {
                            self.cur.push(x);
                        }
                    }
                    Ok(Some(KStmt::If(cx, tb?, fb?)))
                }
                ExprKind::Block(_) => {
                    self.nest += 1;
                    let b = self.block(e);
                    self.nest -= 1;
                    Ok(Some(KStmt::If(KExpr::Lit(KTy::Bool, 1), b?, vec![])))
                }
                ExprKind::Method { recv, name, args, targs } if targs.is_empty() && atomic_op(name).is_some() && matches!(&recv.kind, ExprKind::Name(_)) => self.atomic(e, recv, name, args),
                ExprKind::Call(c, args) if matches!(&c.kind, ExprKind::Name(n) if n == "barrier") && self.lookup("barrier").is_none() => {
                    if !args.is_empty() {
                        return err("E_KERNEL", e.span, "barrier() takes no arguments");
                    }
                    if self.cf.iter().any(|u| !u) {
                        return Err(KErr {
                            code: "E_KERNEL_BARRIER",
                            span: e.span,
                            msg: format!("barrier() in {} must be reached by every thread of the group, but it sits under an 'if' or 'for' that depends on the thread", self.f.name),
                            hint: Some("move the barrier out of thread-dependent control flow; conditions and loop bounds around it may use parameters, lengths, literals, group_id and group_size".into()),
                        });
                    }
                    self.barriers += 1;
                    self.cur = vec![self.fresh_seg()];
                    Ok(Some(KStmt::Barrier))
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
                if self.shared_of(n).is_some() {
                    return err("E_KERNEL", e.span, format!("shared array '{n}' can only be indexed ({n}[i]) or measured ({n}.len)"));
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
            ExprKind::Field(r, m) | ExprKind::Method { recv: r, name: m, .. } if m == "len" && matches!(&r.kind, ExprKind::Name(n) if self.shared_of(n).is_some()) => {
                let ExprKind::Name(n) = &r.kind else { unreachable!() };
                Ok((KExpr::Lit(KTy::Int, self.shared[self.shared_of(n).unwrap()].2 as u64), KTy::Int))
            }
            ExprKind::Field(r, m) => self.method(e, r, m, &[], hint),
            ExprKind::Method { recv, name, args, targs } if targs.is_empty() => self.method(e, recv, name, args, hint),
            ExprKind::Index(a, i) if matches!(&a.kind, ExprKind::Name(n) if self.shared_of(n).is_some()) => {
                let ExprKind::Name(n) = &a.kind else { unreachable!() };
                let arr = self.shared_of(n).unwrap();
                let (ix, it) = self.expr(i, Some(KTy::Int))?;
                if it != KTy::Int {
                    return err("E_KERNEL_TYPE", i.span, format!("shared array indices are Int, got {}", it.name()));
                }
                self.record(arr, false, &ix, i.span);
                Ok((KExpr::SLoad(arr, Box::new(ix)), self.shared[arr].1))
            }
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
                self.accesses.push((p, ix.clone(), i.span, false, self.guards.clone(), None));
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
                ExprKind::Name(n) if n == "barrier" => err("E_KERNEL", e.span, "barrier() is a statement"),
                ExprKind::Index(..) if shared_decl(e).is_some() => err("E_KERNEL_SHARED", e.span, "shared arrays are declared with 'name = shared[T](n)' at the top of the kernel body"),
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

    fn atomic(&mut self, e: &Expr, recv: &Expr, name: &str, args: &[Expr]) -> KR<Option<KStmt>> {
        let op = atomic_op(name).unwrap();
        let ExprKind::Name(n) = &recv.kind else { unreachable!() };
        let (shared, which, et) = if let Some(a) = self.shared_of(n) {
            (true, a, self.shared[a].1)
        } else if let Some(p) = self.param(n).filter(|p| self.params[*p].slice.is_some()) {
            if self.params[p].slice != Some(true) {
                return err("E_KERNEL_WRITE", e.span, format!("'{n}' is a read-only slice; declare it '&mut [{}]' to update it atomically", self.params[p].ty.name()));
            }
            (false, p, self.params[p].ty)
        } else {
            return err("E_KERNEL", recv.span, format!("'{n}' is not a slice parameter or shared array"));
        };
        let ok = matches!(et, KTy::I32 | KTy::U32) || (et == KTy::F32 && op == AOp::Add);
        if !ok {
            return err("E_KERNEL_TYPE", e.span, format!("{name} needs I32 or U32 elements{}, not {}", if op == AOp::Add { " (or F32)" } else { "" }, et.name()));
        }
        let want = if op == AOp::Cas { 3 } else { 2 };
        if args.len() != want {
            return err("E_KERNEL", e.span, format!("{name} takes {want} arguments"));
        }
        let (ix, it) = self.expr(&args[0], Some(KTy::Int))?;
        if it != KTy::Int {
            return err("E_KERNEL_TYPE", args[0].span, format!("indices are Int, got {}", it.name()));
        }
        let mut vals = Vec::new();
        for a in &args[1..] {
            let (v, vt) = self.expr(a, Some(et))?;
            if vt != et {
                return err("E_KERNEL_TYPE", a.span, format!("'{n}' holds {}, got {}", et.name(), vt.name()));
            }
            vals.push(v);
        }
        let key = if op == AOp::Cas {
            let (Some(x), Some(y)) = (self.uniform(&vals[0]), self.uniform(&vals[1])) else {
                return err("E_KERNEL_RACE", e.span, format!("the expected and new values of {name} must be the same in every thread, or which thread wins would decide the result"));
            };
            format!("cas {x} {y}")
        } else {
            format!("{op:?}")
        };
        if shared {
            self.record(which, true, &ix, args[0].span);
            self.sacc.last_mut().unwrap().atomic = Some(key);
        } else {
            self.accesses.push((which, ix.clone(), args[0].span, true, self.guards.clone(), Some(key)));
        }
        let v2 = vals.get(1).cloned();
        Ok(Some(KStmt::Atomic(op, shared, which, ix, vals.remove(0), v2)))
    }

    fn method(&mut self, e: &Expr, recv: &Expr, name: &str, args: &[Expr], hint: Option<KTy>) -> KR<(KExpr, KTy)> {
        if atomic_op(name).is_some() {
            return err("E_KERNEL", e.span, format!("{name} is a statement: the old value it would return depends on the order threads run"));
        }
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
            "shl" => KFn::Shl,
            "shr" => KFn::Shr,
            "band" => KFn::Band,
            "bor" => KFn::Bor,
            "bxor" => KFn::Bxor,
            _ => {
                self.expr(recv, None)?;
                return err("E_KERNEL", e.span, format!("'.{name}' is not available in kernels"));
            }
        };
        let binary = !matches!(f, KFn::Sqrt | KFn::Abs | KFn::Floor | KFn::Ceil | KFn::Round);
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
            KFn::Shl | KFn::Shr | KFn::Band | KFn::Bor | KFn::Bxor => t.is_int(),
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
                    if let Some(s) = self.stride(m)
                        && let KExpr::Lit(KTy::Int, c) = self.deref(j)
                        && let KExpr::Lit(KTy::Int, w) = self.stride_expr(m)
                        && (0..*w as i64).contains(&(*c as i64))
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

    fn group_shape(&self, e: &KExpr, guards: &[KExpr]) -> Option<(bool, String)> {
        let off = match self.deref(e) {
            KExpr::Intr(KIntr::GroupId) => "0".to_string(),
            KExpr::Bin(BinOp::Add, KTy::Int, a, b) if matches!(self.deref(a), KExpr::Intr(KIntr::GroupId)) => self.uniform(b)?,
            KExpr::Bin(BinOp::Add, KTy::Int, a, b) if matches!(self.deref(b), KExpr::Intr(KIntr::GroupId)) => self.uniform(a)?,
            _ => return None,
        };
        let lead = guards.iter().find_map(|g| self.lead(g))?;
        Some((false, format!("group {off} lid {lead}")))
    }

    fn lead(&self, g: &KExpr) -> Option<String> {
        match self.deref(g) {
            KExpr::Bin(BinOp::Eq, KTy::Int, a, b) if self.is_lid(a) => self.uniform(b),
            KExpr::Bin(BinOp::Eq, KTy::Int, a, b) if self.is_lid(b) => self.uniform(a),
            KExpr::Bin(BinOp::And, _, a, b) => self.lead(a).or_else(|| self.lead(b)),
            _ => None,
        }
    }

    fn stride(&self, e: &KExpr) -> Option<String> {

        match self.deref(e) {
            KExpr::Bin(BinOp::Mul, KTy::Int, a, b) if self.is_gid(a) => self.uniform(b),
            KExpr::Bin(BinOp::Mul, KTy::Int, a, b) if self.is_gid(b) => self.uniform(a),
            _ => None,
        }
    }

    fn stride_expr<'b>(&'b self, e: &'b KExpr) -> &'b KExpr {
        match self.deref(e) {
            KExpr::Bin(BinOp::Mul, KTy::Int, a, b) if self.is_gid(a) => self.deref(b),
            KExpr::Bin(BinOp::Mul, KTy::Int, a, _) => self.deref(a),
            other => other,
        }
    }

    fn konst(&self, e: &KExpr) -> Option<i64> {
        match self.deref(e) {
            KExpr::Lit(KTy::Int, b) => Some(*b as i64),
            KExpr::Intr(KIntr::GroupSize) => Some(self.group as i64),
            KExpr::Bin(op, KTy::Int, a, b) => {
                let (a, b) = (self.konst(a)?, self.konst(b)?);
                match op {
                    BinOp::Add => a.checked_add(b),
                    BinOp::Sub => a.checked_sub(b),
                    BinOp::Mul => a.checked_mul(b),
                    BinOp::Div if b != 0 => a.checked_div(b),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn guniform(&self, e: &KExpr) -> bool {
        match self.deref(e) {
            KExpr::Lit(..) | KExpr::Param(_) | KExpr::Len(_) => true,
            KExpr::Intr(i) => !matches!(i, KIntr::Gid | KIntr::Lid),
            KExpr::Local(s) => match &self.defs[*s] {
                Def::Loop(lo, hi) => self.guniform(lo) && self.guniform(hi),
                Def::Value(x) => self.guniform(x),
                Def::Var => false,
            },
            KExpr::Bin(_, _, a, b) => self.guniform(a) && self.guniform(b),
            KExpr::Neg(_, a) | KExpr::Not(a) | KExpr::Cast(_, _, a) => self.guniform(a),
            KExpr::Call(_, _, xs) => xs.iter().all(|x| self.guniform(x)),
            KExpr::If(c, a, b) => self.guniform(c) && self.guniform(a) && self.guniform(b),
            KExpr::Load(..) | KExpr::SLoad(..) => false,
        }
    }

    fn back_edge(&mut self, slot: usize, c0: &[usize], na: usize, maybe_zero: bool) {
        let c1 = self.cur.clone();
        let b: Vec<usize> = c1.iter().map(|_| self.fresh_seg()).collect();
        let n = self.sacc.len();
        for a in &mut self.sacc[..n] {
            if a.segs.iter().any(|s| c1.contains(s)) {
                a.segs.extend(&b);
            }
        }
        for i in na..n {
            if self.sacc[i].segs.iter().any(|s| c0.contains(s)) {
                let mut c = self.sacc[i].clone();
                c.segs = b.clone();
                let d = c.loops.iter().position(|l| *l == slot).unwrap_or(0);
                for l in c.loops[d..].to_vec() {
                    *c.ver.entry(l).or_insert(0) += 1;
                }
                self.sacc.push(c);
            }
        }
        if maybe_zero {
            for x in c0 {
                if !self.cur.contains(x) {
                    self.cur.push(*x);
                }
            }
        }
    }

    fn ukey(&self, e: &KExpr, side: char, ver: &HashMap<usize, u32>) -> Option<String> {
        Some(match self.deref(e) {
            KExpr::Lit(t, b) => format!("{}:{b}", t.name()),
            KExpr::Param(p) => format!("p{p}"),
            KExpr::Len(p) => format!("len{p}"),
            KExpr::Intr(i) if !matches!(i, KIntr::Gid | KIntr::Lid) => format!("{i:?}"),
            KExpr::Local(s) => match &self.defs[*s] {
                Def::Loop(lo, hi) if self.guniform(lo) && self.guniform(hi) => {
                    let v = ver.get(s).copied().unwrap_or(0);
                    if self.free_loops.contains(s) { format!("L{s}.{v}{side}") } else { format!("L{s}.{v}") }
                }
                _ => return None,
            },
            KExpr::Bin(op, _, a, b) => format!("({} {} {})", self.ukey(a, side, ver)?, op.symbol(), self.ukey(b, side, ver)?),
            KExpr::Neg(_, a) => format!("(-{})", self.ukey(a, side, ver)?),
            KExpr::Cast(t, _, a) => format!("{}({})", t.name(), self.ukey(a, side, ver)?),
            KExpr::Call(f, _, xs) => format!("{f:?}({})", xs.iter().map(|x| self.ukey(x, side, ver)).collect::<Option<Vec<_>>>()?.join(", ")),
            _ => return None,
        })
    }

    fn lin(&self, e: &KExpr, side: char, ver: &HashMap<usize, u32>) -> Option<Lin> {
        let e = self.deref(e);
        match e {
            KExpr::Lit(KTy::Int, b) => Some((Default::default(), *b as i64 as i128)),
            KExpr::Bin(BinOp::Add, KTy::Int, a, b) => Some(lin_add(&self.lin(a, side, ver)?, &self.lin(b, side, ver)?, 1)),
            KExpr::Bin(BinOp::Sub, KTy::Int, a, b) => Some(lin_add(&self.lin(a, side, ver)?, &self.lin(b, side, ver)?, -1)),
            KExpr::Bin(BinOp::Mul, KTy::Int, a, b) => {
                let (x, y) = (self.lin(a, side, ver)?, self.lin(b, side, ver)?);
                let (k, v) = if x.0.is_empty() { (x.1, y) } else if y.0.is_empty() { (y.1, x) } else { return self.ukey(e, side, ver).map(|k| ([(k, 1)].into(), 0)) };
                Some(lin_add(&(Default::default(), 0), &v, k))
            }
            KExpr::Neg(KTy::Int, a) => Some(lin_add(&(Default::default(), 0), &self.lin(a, side, ver)?, -1)),
            _ => self.ukey(e, side, ver).map(|k| ([(k, 1)].into(), 0)),
        }
    }

    fn is_lid(&self, e: &KExpr) -> bool {
        matches!(self.deref(e), KExpr::Intr(KIntr::Lid))
    }

    fn stid(&self, e: &KExpr, side: char, ver: &HashMap<usize, u32>) -> Option<Lin> {
        let e = self.deref(e);
        if self.is_lid(e) {
            return Some((Default::default(), 0));
        }
        match e {
            KExpr::Bin(BinOp::Add, KTy::Int, a, b) if self.is_lid(a) => self.lin(b, side, ver),
            KExpr::Bin(BinOp::Add, KTy::Int, a, b) if self.is_lid(b) => self.lin(a, side, ver),
            KExpr::Bin(BinOp::Sub, KTy::Int, a, b) if self.is_lid(a) => self.lin(b, side, ver).map(|l| lin_add(&(Default::default(), 0), &l, -1)),
            _ => None,
        }
    }

    fn guard_cnt(&self, g: &KExpr, side: char, ver: &HashMap<usize, u32>) -> Option<Lin> {
        match self.deref(g) {
            KExpr::Bin(BinOp::Lt, KTy::Int, x, u) if self.is_lid(x) => self.lin(u, side, ver),
            KExpr::Bin(BinOp::Gt, KTy::Int, u, x) if self.is_lid(x) => self.lin(u, side, ver),
            KExpr::Bin(BinOp::Le, KTy::Int, x, u) if self.is_lid(x) => self.lin(u, side, ver).map(|l| lin_add(&l, &(Default::default(), 1), 1)),
            KExpr::Bin(BinOp::And, _, a, b) => self.guard_cnt(a, side, ver).or_else(|| self.guard_cnt(b, side, ver)),
            _ => None,
        }
    }

    fn span_of(&self, a: &SAcc, side: char) -> Option<(Lin, Lin)> {
        let cnt = a.guards.iter().find_map(|g| self.guard_cnt(g, side, &a.ver)).unwrap_or((Default::default(), self.group as i128));
        if let Some(o) = self.stid(&a.idx, side, &a.ver) {
            return Some((o, cnt));
        }
        self.lin(&a.idx, side, &a.ver).map(|u| (u, (Default::default(), 1)))
    }

    fn shared_check(&mut self) {
        for arr in 0..self.shared.len() {
            let name = self.shared[arr].0.clone();
            let segs: HashSet<usize> = self.sacc.iter().filter(|a| a.arr == arr).flat_map(|a| a.segs.iter().copied()).collect();
            let mut segs: Vec<usize> = segs.into_iter().collect();
            segs.sort();
            for sg in segs {
                let group: Vec<&SAcc> = self.sacc.iter().filter(|a| a.arr == arr && a.segs.contains(&sg)).collect();
                if let Some(k0) = group.iter().find_map(|a| a.atomic.clone()) {
                    if let Some(a) = group.iter().find(|a| a.atomic.as_ref() != Some(&k0)) {
                        self.errs.push(KErr {
                            code: "E_KERNEL_RACE",
                            span: a.span,
                            msg: format!("{} {} shared {name} in a phase that updates it atomically{}", self.f.name, if a.atomic.is_some() { "uses another atomic operation on" } else if a.write { "writes" } else { "reads" }, if a.atomic.is_some() { "" } else { "; separate them with barrier()" }),
                            hint: Some("between two barriers, a shared array is either updated with one kind of atomic operation or accessed directly".into()),
                        });
                        return;
                    }
                    continue;
                }
                for w in group.iter().filter(|a| a.write) {
                    let Some(ow) = self.stid(&w.idx, 'a', &w.ver) else {
                        self.errs.push(KErr {
                            code: "E_KERNEL_RACE",
                            span: w.span,
                            msg: format!("{} writes shared {name} at an index that two threads of a group may share", self.f.name),
                            hint: Some(format!("write {name}[lid] (plus a uniform offset), so each thread owns its element until the next barrier()")),
                        });
                        return;
                    };
                    let cw = self.span_of(w, 'a').map(|s| s.1).unwrap();
                    for a in &group {
                        let ok = match self.span_of(a, 'b') {
                            Some((oa, ca)) => (self.stid(&a.idx, 'b', &a.ver).is_some() && oa == ow) || lin_le(&lin_add(&ow, &cw, 1), &oa) || lin_le(&lin_add(&oa, &ca, 1), &ow),
                            None => false,
                        };
                        if !ok {
                            let what = if a.write { "writes" } else { "reads" };
                            self.errs.push(KErr {
                                code: "E_KERNEL_RACE",
                                span: a.span,
                                msg: format!("{} {what} shared {name} at an element another thread of the group may write before the next barrier()", self.f.name),
                                hint: Some("separate the write and this access with barrier(), or keep them disjoint (for example 'if lid < s' around writes of [lid] and reads of [lid + s])".into()),
                            });
                            return;
                        }
                    }
                }
            }
        }
    }

    fn race_check(&mut self) {

        for p in 0..self.params.len() {
            let acc: Vec<(KExpr, Span, bool, Vec<KExpr>)> = self.accesses.iter().filter(|a| a.0 == p).map(|a| (a.1.clone(), a.2, a.3, a.4.clone())).collect();
            if !acc.iter().any(|a| a.2) {
                continue;
            }
            let name = self.params[p].name.clone();
            let keys: Vec<(Option<String>, Span)> = self.accesses.iter().filter(|a| a.0 == p).map(|a| (a.5.clone(), a.2)).collect();
            if let Some((Some(k0), _)) = keys.iter().find(|k| k.0.is_some()) {
                if let Some((k, span)) = keys.iter().find(|k| k.0.as_ref() != Some(k0)) {
                    self.errs.push(KErr {
                        code: "E_KERNEL_RACE",
                        span: *span,
                        msg: match k {
                            None => format!("{} updates {name} atomically, so it can't also read or write it directly", self.f.name),
                            Some(_) => format!("{} mixes different atomic operations on {name}; their results depend on the order threads run", self.f.name),
                        },
                        hint: Some(format!("use one kind of atomic update on {name} per kernel and read the result after the launch")),
                    });
                    return;
                }
                continue;
            }
            let mut first: Option<(bool, String)> = None;
            for (ix, span, write, guards) in &acc {
                let what = if *write { "writes" } else { "reads" };
                match self.shape(ix).or_else(|| self.group_shape(ix, guards)) {
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
