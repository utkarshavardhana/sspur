use crate::value::Value;
use crate::{trap, R};
use sspur_native::rnd::Rng;
use std::rc::Rc;

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

fn f(v: &Value) -> R<f64> {
    match v {
        Value::Float(x) => Ok(*x),
        v => trap(format!("expected F64, got {v}")),
    }
}

fn floats(v: &Value) -> R<Vec<f64>> {
    match v {
        Value::List(xs) => xs.iter().map(f).collect(),
        v => trap(format!("expected List, got {v}")),
    }
}

fn pair(a: Value, b: Value) -> Value {
    Value::Tuple(Rc::new(vec![a, b]))
}

pub fn rng_value(r: Rng) -> Value {
    Value::Record("#Rng".into(), Rc::new(vec![("state".into(), Value::Int(r.s as i64)), ("gamma".into(), Value::Int(r.g as i64))]))
}

fn rng_of(fs: &[(Rc<str>, Value)]) -> R<Rng> {
    Ok(Rng { s: int(&fs[0].1)? as u64, g: int(&fs[1].1)? as u64 })
}

fn lift<T>(r: Result<T, &'static str>) -> R<T> {
    r.or_else(|e| trap(e))
}

fn draw(r: &mut Rng, name: &str, a: &[Value]) -> R<Option<Value>> {
    Ok(Some(match name {
        "next" => Value::Int((r.next_u64() >> 1) as i64),
        "int" => Value::Int(lift(r.int(int(&a[0])?, int(&a[1])?))?),
        "f64" => Value::Float(r.f64()),
        "normal" => Value::Float(r.normal(f(&a[0])?, f(&a[1])?)),
        "uniform" => {
            let (lo, hi) = (f(&a[0])?, f(&a[1])?);
            let u = r.f64();
            let w = hi - lo;
            let t = w * u;
            Value::Float(lo + t)
        }
        "exp" => {
            let rate = f(&a[0])?;
            Value::Float(-(1.0 - r.f64()).ln() / rate)
        }
        "bool" => {
            let p = f(&a[0])?;
            Value::Bool(r.f64() < p)
        }
        "binomial" => Value::Int(lift(r.binomial(int(&a[0])?, f(&a[1])?))?),
        "poisson" => Value::Int(lift(r.poisson(f(&a[0])?))?),
        "geometric" => Value::Int(lift(r.geometric(f(&a[0])?))?),
        "gamma" => Value::Float(lift(r.gamma(f(&a[0])?, f(&a[1])?))?),
        "beta" => Value::Float(lift(r.beta(f(&a[0])?, f(&a[1])?))?),
        "weighted" => Value::Int(lift(r.weighted(&floats(&a[0])?))?),
        _ => return Ok(None),
    }))
}

pub fn global(n: &str, a: &[Value]) -> R<Option<Value>> {
    if n == "rng" {
        return Ok(Some(rng_value(Rng::seed(int(&a[0])?))));
    }
    let Some(d) = n.strip_prefix("rand_") else { return Ok(None) };
    if !matches!(d, "binomial" | "poisson" | "geometric" | "gamma" | "beta" | "weighted") {
        return Ok(None);
    }
    let mut r = Rng::seed(int(&a[0])?);
    let v = draw(&mut r, d, &a[1..])?.unwrap();
    Ok(Some(pair(v, Value::Int(r.s as i64))))
}

pub fn method(name: &str, fs: &[(Rc<str>, Value)], a: &[Value]) -> R {
    let mut r = rng_of(fs)?;
    match name {
        "split" => {
            let child = r.split();
            return Ok(pair(rng_value(r), rng_value(child)));
        }
        "stream" => return Ok(rng_value(r.stream(int(&a[0])?))),
        _ => {}
    }
    match draw(&mut r, name, a)? {
        Some(v) => Ok(pair(v, rng_value(r))),
        None => trap(format!("no method '{name}' on Rng")),
    }
}
