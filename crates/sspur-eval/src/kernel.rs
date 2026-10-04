use crate::{trap, value::Value, R};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use sspur_check::kernel::{has_barrier, AOp, KExpr, KFn, KIntr, KStmt, KTy, Kernel};
use sspur_syntax::{printer, BinOp, FnDef};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KV {
    I(i64),
    F(f32),
    D(f64),
    B(bool),
}

impl KV {
    fn i(self) -> i64 {
        match self {
            KV::I(v) => v,
            KV::B(b) => b as i64,
            _ => 0,
        }
    }

    fn b(self) -> bool {
        matches!(self, KV::B(true))
    }
}

pub fn lit(t: KTy, bits: u64) -> KV {
    match t {
        KTy::F32 => KV::F(f32::from_bits(bits as u32)),
        KTy::F64 => KV::D(f64::from_bits(bits)),
        KTy::Bool => KV::B(bits != 0),
        _ => KV::I(bits as i64),
    }
}

fn overflow<T>() -> R<T> {
    trap("integer overflow")
}

fn fit(t: KTy, v: Option<i64>) -> R<KV> {
    let (lo, hi) = t.range();
    match v {
        Some(v) if (lo..=hi).contains(&(v as i128)) => Ok(KV::I(v)),
        _ => overflow(),
    }
}

pub fn to_kv(t: KTy, v: &Value, what: &dyn Fn() -> String) -> R<KV> {
    Ok(match (t, v) {
        (KTy::F32, Value::Float(x)) => KV::F(*x as f32),
        (KTy::F64, Value::Float(x)) => KV::D(*x),
        (KTy::Bool, Value::Bool(b)) => KV::B(*b),
        (t, Value::Int(n)) if t.is_int() => {
            let (lo, hi) = t.range();
            if !(lo..=hi).contains(&(*n as i128)) {
                return trap(format!("dev: {} is out of range for {} (value = {n})", what(), t.name()));
            }
            KV::I(*n)
        }
        (t, v) => return trap(format!("dev: {} expects {}, got {v}", what(), t.name())),
    })
}

pub fn from_kv(v: KV) -> Value {
    match v {
        KV::I(n) => Value::Int(n),
        KV::B(b) => Value::Bool(b),
        KV::F(x) if x.is_nan() => Value::Float(f64::NAN),
        KV::F(x) => Value::Float(x as f64),
        KV::D(x) if x.is_nan() => Value::Float(f64::NAN),
        KV::D(x) => Value::Float(x),
    }
}

pub enum Arg {
    Scalar(KV),
    Slice(Vec<KV>),
}

struct Run<'a> {
    k: &'a Kernel,
    args: &'a mut [Arg],
    locals: Vec<KV>,
    gid: i64,
    n: i64,
    shared: Vec<Vec<KV>>,
}

fn zero(t: KTy) -> KV {
    match t {
        KTy::F32 => KV::F(0.0),
        KTy::F64 => KV::D(0.0),
        KTy::Bool => KV::B(false),
        _ => KV::I(0),
    }
}

pub fn prepare(f: &FnDef, k: &Kernel, args: &mut [Arg]) -> R<i64> {
    let mut r = Run { k, args, locals: vec![KV::I(0); k.locals.len()], gid: 0, n: 0, shared: vec![] };
    for (i, p) in k.pres.iter().enumerate() {
        if !r.expr(p)?.b() {
            return trap(format!("contract violated: pre {} in {}", printer::expr(&f.pres[i], 0), f.name));
        }
    }
    let n = r.expr(&k.grid)?.i();
    let g = k.group as i64;
    if k.grouped && n > 0 && n % g != 0 {
        return trap(format!("dev: the thread count of {} is not a multiple of its group size {g} (count = {n})", k.name));
    }
    Ok(n)
}

pub fn run(k: &Kernel, args: &mut [Arg], n: i64) -> R<()> {
    let mut r = Run { k, args, locals: vec![KV::I(0); k.locals.len()], gid: 0, n: n.max(0), shared: vec![] };
    if k.grouped {
        let g = k.group as i64;
        let mut lanes = vec![vec![KV::I(0); k.locals.len()]; g as usize];
        for grp in 0..n.max(0) / g {
            r.shared = k.shared.iter().map(|(_, t, len)| vec![zero(*t); *len as usize]).collect();
            r.gblock(&k.body, &mut lanes, grp * g)?;
        }
        return Ok(());
    }
    for gid in 0..n {
        r.gid = gid;
        r.block(&k.body)?;
    }
    Ok(())
}

impl Run<'_> {
    fn lane0<T>(&mut self, lanes: &mut [Vec<KV>], base: i64, f: impl FnOnce(&mut Self) -> R<T>) -> R<T> {
        std::mem::swap(&mut self.locals, &mut lanes[0]);
        self.gid = base;
        let r = f(self);
        std::mem::swap(&mut self.locals, &mut lanes[0]);
        r
    }

    fn gblock(&mut self, b: &[KStmt], lanes: &mut [Vec<KV>], base: i64) -> R<()> {
        for s in b {
            if !has_barrier(s) {
                for (t, lane) in lanes.iter_mut().enumerate() {
                    std::mem::swap(&mut self.locals, lane);
                    self.gid = base + t as i64;
                    let r = self.stmt(s);
                    std::mem::swap(&mut self.locals, lane);
                    r?;
                }
                continue;
            }
            match s {
                KStmt::For(slot, a, b, body) => {
                    let (lo, hi) = self.lane0(lanes, base, |r| Ok((r.expr(a)?.i(), r.expr(b)?.i())))?;
                    let mut i = lo;
                    while i < hi {
                        for lane in lanes.iter_mut() {
                            lane[*slot] = KV::I(i);
                        }
                        self.gblock(body, lanes, base)?;
                        i += 1;
                    }
                }
                KStmt::If(c, t, f) => {
                    let c = self.lane0(lanes, base, |r| Ok(r.expr(c)?.b()))?;
                    self.gblock(if c { t } else { f }, lanes, base)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn block(&mut self, b: &[KStmt]) -> R<()> {
        for s in b {
            self.stmt(s)?;
        }
        Ok(())
    }

    fn slice(&self, p: usize) -> &[KV] {
        match &self.args[p] {
            Arg::Slice(v) => v,
            Arg::Scalar(_) => &[],
        }
    }

    fn stmt(&mut self, s: &KStmt) -> R<()> {
        match s {
            KStmt::Let(slot, e) | KStmt::Set(slot, e) => {
                let v = self.expr(e)?;
                self.locals[*slot] = v;
            }
            KStmt::Store(p, i, v) => {
                let i = self.expr(i)?.i();
                let v = self.expr(v)?;
                let Arg::Slice(xs) = &mut self.args[*p] else { unreachable!() };
                if i < 0 || i as usize >= xs.len() {
                    return trap(format!("index {i} out of bounds for list of length {}", xs.len()));
                }
                xs[i as usize] = v;
            }
            KStmt::For(slot, a, b, body) => {
                let lo = self.expr(a)?.i();
                let hi = self.expr(b)?.i();
                let mut i = lo;
                while i < hi {
                    self.locals[*slot] = KV::I(i);
                    self.block(body)?;
                    i += 1;
                }
            }
            KStmt::If(c, t, f) => {
                if self.expr(c)?.b() {
                    self.block(t)?;
                } else {
                    self.block(f)?;
                }
            }
            KStmt::Eval(e) => {
                self.expr(e)?;
            }
            KStmt::SStore(a, i, v) => {
                let i = self.expr(i)?.i();
                let v = self.expr(v)?;
                let xs = &mut self.shared[*a];
                if i < 0 || i as usize >= xs.len() {
                    return trap(format!("index {i} out of bounds for list of length {}", xs.len()));
                }
                xs[i as usize] = v;
            }
            KStmt::Barrier => {}
            KStmt::Atomic(op, shared, w, i, v, v2) => {
                let i = self.expr(i)?.i();
                let v = self.expr(v)?;
                let v2 = match v2 {
                    Some(x) => Some(self.expr(x)?),
                    None => None,
                };
                let (xs, t) = if *shared {
                    (&mut self.shared[*w], self.k.shared[*w].1)
                } else {
                    let Arg::Slice(xs) = &mut self.args[*w] else { unreachable!() };
                    (xs, self.k.params[*w].ty)
                };
                if i < 0 || i as usize >= xs.len() {
                    return trap(format!("index {i} out of bounds for list of length {}", xs.len()));
                }
                let old = xs[i as usize];
                xs[i as usize] = atomic(*op, t, old, v, v2);
            }
        }
        Ok(())
    }

    fn expr(&mut self, e: &KExpr) -> R<KV> {
        Ok(match e {
            KExpr::Lit(t, b) => lit(*t, *b),
            KExpr::Local(s) => self.locals[*s],
            KExpr::Param(p) => match &self.args[*p] {
                Arg::Scalar(v) => *v,
                Arg::Slice(_) => unreachable!(),
            },
            KExpr::Len(p) => KV::I(self.slice(*p).len() as i64),
            KExpr::Intr(i) => KV::I(match i {
                KIntr::Gid => self.gid,
                KIntr::Lid => self.gid % self.k.group as i64,
                KIntr::GroupId => self.gid / self.k.group as i64,
                KIntr::GroupSize => self.k.group as i64,
                KIntr::GridSize => self.n,
            }),
            KExpr::Load(p, i) => {
                let i = self.expr(i)?.i();
                let xs = self.slice(*p);
                if i < 0 || i as usize >= xs.len() {
                    return trap(format!("index {i} out of bounds for list of length {}", xs.len()));
                }
                xs[i as usize]
            }
            KExpr::Bin(BinOp::And, _, a, b) => KV::B(self.expr(a)?.b() && self.expr(b)?.b()),
            KExpr::Bin(BinOp::Or, _, a, b) => KV::B(self.expr(a)?.b() || self.expr(b)?.b()),
            KExpr::Bin(op, t, a, b) => {
                let x = self.expr(a)?;
                let y = self.expr(b)?;
                bin(*op, *t, x, y)?
            }
            KExpr::Neg(t, a) => match self.expr(a)? {
                KV::F(x) => KV::F(-x),
                KV::D(x) => KV::D(-x),
                KV::I(x) => fit(*t, x.checked_neg())?,
                v => v,
            },
            KExpr::Not(a) => KV::B(!self.expr(a)?.b()),
            KExpr::Cast(to, _, a) => {
                let v = self.expr(a)?;
                cast(*to, v)?
            }
            KExpr::Call(f, t, xs) => {
                let a = self.expr(&xs[0])?;
                let b = match xs.get(1) {
                    Some(x) => Some(self.expr(x)?),
                    None => None,
                };
                call(*f, *t, a, b)?
            }
            KExpr::If(c, a, b) => {
                if self.expr(c)?.b() {
                    self.expr(a)?
                } else {
                    self.expr(b)?
                }
            }
            KExpr::SLoad(a, i) => {
                let i = self.expr(i)?.i();
                let xs = &self.shared[*a];
                if i < 0 || i as usize >= xs.len() {
                    return trap(format!("index {i} out of bounds for list of length {}", xs.len()));
                }
                xs[i as usize]
            }
        })
    }
}

fn cmp<T: PartialOrd>(op: BinOp, x: T, y: T) -> bool {
    match op {
        BinOp::Eq => x == y,
        BinOp::Ne => x != y,
        BinOp::Lt => x < y,
        BinOp::Le => x <= y,
        BinOp::Gt => x > y,
        _ => x >= y,
    }
}

pub fn bin(op: BinOp, t: KTy, x: KV, y: KV) -> R<KV> {
    use BinOp::*;
    if matches!(op, Eq | Ne | Lt | Le | Gt | Ge) {
        return Ok(KV::B(match (x, y) {
            (KV::F(a), KV::F(b)) => cmp(op, a, b),
            (KV::D(a), KV::D(b)) => cmp(op, a, b),
            (KV::B(a), KV::B(b)) => cmp(op, a, b),
            (a, b) => cmp(op, a.i(), b.i()),
        }));
    }
    Ok(match (x, y) {
        (KV::F(a), KV::F(b)) => KV::F(match op {
            Add => a + b,
            Sub => a - b,
            Mul => a * b,
            _ => a / b,
        }),
        (KV::D(a), KV::D(b)) => KV::D(match op {
            Add => a + b,
            Sub => a - b,
            Mul => a * b,
            _ => a / b,
        }),
        (a, b) => {
            let (a, b) = (a.i(), b.i());
            if matches!(op, Div | Rem) && b == 0 {
                return trap("division by zero");
            }
            let r = match op {
                Add => a.checked_add(b),
                Sub => a.checked_sub(b),
                Mul => a.checked_mul(b),
                Div => a.checked_div(b),
                _ => a.checked_rem(b),
            };
            fit(t, r)?
        }
    })
}

pub fn atomic(op: AOp, t: KTy, old: KV, v: KV, v2: Option<KV>) -> KV {
    match (op, old, v) {
        (AOp::Add, KV::F(a), KV::F(b)) => KV::F(a + b),
        (AOp::Add, KV::I(a), KV::I(b)) if t == KTy::U32 => KV::I((a as u32).wrapping_add(b as u32) as i64),
        (AOp::Add, KV::I(a), KV::I(b)) => KV::I((a as i32).wrapping_add(b as i32) as i64),
        (AOp::Min, KV::I(a), KV::I(b)) => KV::I(a.min(b)),
        (AOp::Max, KV::I(a), KV::I(b)) => KV::I(a.max(b)),
        (AOp::Cas, a, e) if a == e => v2.unwrap_or(a),
        _ => old,
    }
}

pub fn cast(to: KTy, v: KV) -> R<KV> {

    Ok(match (to, v) {
        (KTy::F32, KV::I(n)) => KV::F(n as f32),
        (KTy::F32, KV::D(x)) => KV::F(x as f32),
        (KTy::F64, KV::I(n)) => KV::D(n as f64),
        (KTy::F64, KV::F(x)) => KV::D(x as f64),
        (t, KV::I(n)) => fit(t, Some(n))?,
        (_, v) => v,
    })
}

pub fn call(f: KFn, t: KTy, a: KV, b: Option<KV>) -> R<KV> {
    Ok(match (f, a) {
        (KFn::Min, a) => {
            let b = b.unwrap();
            if bin(BinOp::Lt, t, a, b)?.b() { a } else { b }
        }
        (KFn::Max, a) => {
            let b = b.unwrap();
            if bin(BinOp::Gt, t, a, b)?.b() { a } else { b }
        }
        (KFn::Abs, KV::I(n)) => fit(t, n.checked_abs())?,
        (KFn::Abs, KV::F(x)) => KV::F(x.abs()),
        (KFn::Abs, KV::D(x)) => KV::D(x.abs()),
        (KFn::Sqrt, KV::F(x)) => KV::F(x.sqrt()),
        (KFn::Sqrt, KV::D(x)) => KV::D(x.sqrt()),
        (KFn::Floor, KV::F(x)) => KV::F(x.floor()),
        (KFn::Floor, KV::D(x)) => KV::D(x.floor()),
        (KFn::Ceil, KV::F(x)) => KV::F(x.ceil()),
        (KFn::Ceil, KV::D(x)) => KV::D(x.ceil()),
        (KFn::Round, KV::F(x)) => KV::F(x.round()),
        (KFn::Round, KV::D(x)) => KV::D(x.round()),
        (KFn::Shl | KFn::Shr | KFn::Band | KFn::Bor | KFn::Bxor, KV::I(a)) => {
            let b = b.unwrap().i();
            let sh = (0..64).contains(&b);
            fit(
                t,
                Some(match f {
                    KFn::Shl if sh => ((a as u64) << b) as i64,
                    KFn::Shr if sh => ((a as u64) >> b) as i64,
                    KFn::Shl | KFn::Shr => 0,
                    KFn::Band => a & b,
                    KFn::Bor => a | b,
                    _ => a ^ b,
                }),
            )?
        }

        (_, v) => v,
    })
}

pub struct DevCell {
    pub id: u64,
    pub ty: KTy,
    pub data: RefCell<Vec<KV>>,
}

thread_local! {
    static DEV_IDS: Cell<u64> = const { Cell::new(0) };
    static PENDING: RefCell<Option<crate::Ctrl>> = const { RefCell::new(None) };
}

pub fn pending() -> bool {
    PENDING.with(|p| p.borrow().is_some())
}

pub fn defer(c: crate::Ctrl) {
    PENDING.with(|p| {
        let mut p = p.borrow_mut();
        if p.is_none() {
            *p = Some(c);
        }
    });
}

pub fn sync() -> R<()> {
    match PENDING.with(|p| p.borrow_mut().take()) {
        Some(c) => Err(c),
        None => Ok(()),
    }
}

pub fn dev_new(name: &str, xs: &Value) -> R<Value> {
    let ty = match name {
        "dev_f32" => KTy::F32,
        "dev_f64" => KTy::F64,
        "dev_i32" => KTy::I32,
        "dev_u32" => KTy::U32,
        _ => KTy::Int,
    };
    let Value::List(xs) = xs else { return trap(format!("{name} expects a list")) };
    let mut data = Vec::with_capacity(xs.len());
    for (j, x) in xs.iter().enumerate() {
        data.push(to_kv(ty, x, &|| format!("element {j} of {name}"))?);
    }
    let id = DEV_IDS.with(|c| {
        c.set(c.get() + 1);
        c.get()
    });
    Ok(Value::Dev(Rc::new(DevCell { id, ty, data: RefCell::new(data) })))
}

pub fn dev_method(name: &str, d: &DevCell, a: &[Value]) -> R<Value> {
    match (name, a) {
        ("len", []) => Ok(Value::Int(d.data.borrow().len() as i64)),
        ("to_list", []) => sync().map(|_| Value::list(d.data.borrow().iter().map(|x| from_kv(*x)).collect())),
        _ => trap(format!("no method '{name}' on DevBuf")),
    }
}
