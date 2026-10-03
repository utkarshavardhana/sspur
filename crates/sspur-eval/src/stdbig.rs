use crate::value::Value;
use crate::{trap, R};
use sspur_native::bigint::{dec_cmp, dec_div, dec_parse, dec_str, rescale, Big, BigErr, MAX_SCALE};
use std::rc::Rc;

fn fail<T>(e: BigErr) -> R<T> {
    trap(match e {
        BigErr::DivZero => "division by zero",
        BigErr::Oom => "out of memory",
        BigErr::NegExp => "negative exponent",
    })
}

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

fn big(v: &Value) -> R<Rc<Big>> {
    match v {
        Value::Big(b) => Ok(b.clone()),
        v => trap(format!("expected BigInt, got {v}")),
    }
}

fn dec(v: &Value) -> R<(Rc<Big>, i64)> {
    match v {
        Value::Dec(m, s) => Ok((m.clone(), *s)),
        v => trap(format!("expected Dec, got {v}")),
    }
}

fn scale_ok(s: i64) -> R<()> {
    if (0..=MAX_SCALE).contains(&s) { Ok(()) } else { trap("decimal scale must be in 0..=10000") }
}

fn b(x: Big) -> Value {
    Value::Big(Rc::new(x))
}

fn d(m: Big, s: i64) -> Value {
    Value::Dec(Rc::new(m), s)
}

pub fn to_f64(s: &str) -> f64 {
    s.parse().unwrap_or(f64::NAN)
}

pub fn global(n: &str, a: &[Value]) -> R<Value> {
    let text = || match &a[0] {
        Value::Str(s) => Ok(s.clone()),
        v => trap(format!("expected Str, got {v}")),
    };
    Ok(match n {
        "big" => b(Big::from_i64(int(&a[0])?)),
        "parse_big" => Value::Opt(Big::parse(&text()?).map(|x| Rc::new(b(x)))),
        _ => Value::Opt(dec_parse(&text()?).map(|(m, s)| Rc::new(d(m, s)))),
    })
}

pub fn big_method(name: &str, x: &Big, a: &[Value]) -> R<Value> {
    Ok(match name {
        "add" => b(x.add(&*big(&a[0])?)),
        "sub" => b(x.sub(&*big(&a[0])?)),
        "mul" => b(x.mul(&*big(&a[0])?).or_else(fail)?),
        "div" | "rem" | "divmod" => {
            let (q, r) = x.divmod(&*big(&a[0])?).or_else(fail)?;
            match name {
                "div" => b(q),
                "rem" => b(r),
                _ => Value::Tuple(Rc::new(vec![b(q), b(r)])),
            }
        }
        "pow" => b(x.pow(int(&a[0])?).or_else(fail)?),
        "neg" => b(x.neg()),
        "abs" => b(x.abs()),
        "sign" => Value::Int(x.sign()),
        "to_int" => Value::Opt(x.to_i64().map(|n| Rc::new(Value::Int(n)))),
        "to_f64" => Value::Float(to_f64(&x.to_string())),
        "to_dec" => d(x.clone(), 0),
        _ => return trap(format!("no method '{name}' on BigInt")),
    })
}

pub fn dec_method(name: &str, m: &Big, s: i64, a: &[Value]) -> R<Value> {
    Ok(match name {
        "add" | "sub" => {
            let (om, os) = dec(&a[0])?;
            let t = s.max(os);
            let (x, y) = (rescale(m, s, t), rescale(&om, os, t));
            d(if name == "add" { x.add(&y) } else { x.sub(&y) }, t)
        }
        "mul" => {
            let (om, os) = dec(&a[0])?;
            scale_ok(s + os)?;
            d(m.mul(&om).or_else(fail)?, s + os)
        }
        "div" => {
            let (om, os) = dec(&a[0])?;
            let k = int(&a[1])?;
            scale_ok(k)?;
            d(dec_div(m, s, &om, os, k).or_else(fail)?, k)
        }
        "round" => {
            let k = int(&a[0])?;
            scale_ok(k)?;
            d(rescale(m, s, k), k)
        }
        "scale" => Value::Int(s),
        "neg" => d(m.neg(), s),
        "abs" => d(m.abs(), s),
        "sign" => Value::Int(m.sign()),
        "to_f64" => Value::Float(to_f64(&dec_str(m, s))),
        _ => return trap(format!("no method '{name}' on Dec")),
    })
}

pub fn dec_order(a: &Big, sa: i64, b: &Big, sb: i64) -> std::cmp::Ordering {
    dec_cmp(a, sa, b, sb)
}
