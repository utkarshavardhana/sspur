use crate::{trap, value::Value, R};
use sspur_check::kernel::{KExpr, KFn, KIntr, KStmt, KTy, Kernel};
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
}

pub fn run(f: &FnDef, k: &Kernel, args: &mut [Arg]) -> R<()> {
    let mut r = Run { k, args, locals: vec![KV::I(0); k.locals.len()], gid: 0, n: 0 };
    for (i, p) in k.pres.iter().enumerate() {
        if !r.expr(p)?.b() {
            return trap(format!("contract violated: pre {} in {}", printer::expr(&f.pres[i], 0), f.name));
        }
    }
    let n = r.expr(&k.grid)?.i();
    r.n = n.max(0);
    for gid in 0..n {
        r.gid = gid;
        r.block(&k.body)?;
    }
    Ok(())
}

impl Run<'_> {
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
        (_, v) => v,
    })
}
