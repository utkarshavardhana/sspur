//! Exact rationals over `BigInt`; `std_rt.c` section `ratio` mirrors each operation.
use crate::value::Value;
use crate::{trap, R};
use sspur_native::bigint::{dec_div, ratio_f64, ratio_norm, Big, BigErr, MAX_SCALE};
use std::rc::Rc;

fn fail<T>(e: BigErr) -> R<T> {
    trap(match e {
        BigErr::DivZero => "division by zero",
        BigErr::Oom => "out of memory",
        BigErr::NegExp => "negative exponent",
    })
}

fn big(v: &Value) -> R<Big> {
    match v {
        Value::Big(b) => Ok((**b).clone()),
        v => trap(format!("expected BigInt, got {v}")),
    }
}

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

pub fn value(n: Big, d: Big) -> Value {
    Value::Record("#Ratio".into(), Rc::new(vec![("num".into(), Value::Big(Rc::new(n))), ("den".into(), Value::Big(Rc::new(d)))]))
}

fn make(n: &Big, d: &Big) -> R {
    let (n, d) = ratio_norm(n, d).or_else(fail)?;
    Ok(value(n, d))
}

pub fn parts(v: &Value) -> R<(Big, Big)> {
    match v {
        Value::Record(n, fs) if &**n == "#Ratio" => Ok((big(&fs[0].1)?, big(&fs[1].1)?)),
        v => trap(format!("expected Ratio, got {v}")),
    }
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

fn parse(s: &str) -> Option<(Big, Big)> {
    let t = s.trim();
    let (a, b) = match t.split_once('/') {
        Some((a, b)) => (a, b),
        None => (t, "1"),
    };
    let body = a.strip_prefix(['+', '-']).unwrap_or(a);
    if !digits(body) || !digits(b) {
        return None;
    }
    let (n, d) = (Big::parse(a)?, Big::parse(b)?);
    ratio_norm(&n, &d).ok()
}

pub fn global(n: &str, a: &[Value]) -> R<Option<Value>> {
    Ok(Some(match n {
        "ratio" => make(&Big::from_i64(int(&a[0])?), &Big::from_i64(int(&a[1])?))?,
        "ratio_big" => make(&big(&a[0])?, &big(&a[1])?)?,
        "parse_ratio" => match &a[0] {
            Value::Str(s) => Value::Opt(parse(s).map(|(n, d)| Rc::new(value(n, d)))),
            v => return trap(format!("expected Str, got {v}")),
        },
        _ => return Ok(None),
    }))
}

fn pow(n: &Big, d: &Big, e: i64) -> R<(Big, Big)> {
    if e == i64::MIN {
        let (h, k) = pow(n, d, e / 2)?;
        return Ok((h.mul(&h).or_else(fail)?, k.mul(&k).or_else(fail)?));
    }
    let k = e.abs();
    let (pn, pd) = (n.pow(k).or_else(fail)?, d.pow(k).or_else(fail)?);
    if e >= 0 { Ok((pn, pd)) } else { ratio_norm(&pd, &pn).or_else(fail) }
}

pub fn method(name: &str, fs: &[(Rc<str>, Value)], a: &[Value]) -> R {
    let (n, d) = (big(&fs[0].1)?, big(&fs[1].1)?);
    let rounded = |mode: u8| -> R {
        let (q, r) = n.divmod(&d).or_else(fail)?;
        let one = Big::from_i64(n.sign());
        let up = match mode {
            0 => r.sign() < 0,
            1 => r.sign() > 0,
            2 => false,
            _ => {
                let twice = r.abs().add(&r.abs());
                match twice.compare(&d) {
                    std::cmp::Ordering::Greater => true,
                    std::cmp::Ordering::Equal => q.divmod(&Big::from_i64(2)).or_else(fail)?.1.sign() != 0,
                    std::cmp::Ordering::Less => false,
                }
            }
        };
        let q = match (mode, up) {
            (_, false) => q,
            (0, true) => q.sub(&Big::from_i64(1)),
            (1, true) => q.add(&Big::from_i64(1)),
            _ => q.add(&one),
        };
        Ok(Value::Big(Rc::new(q)))
    };
    Ok(match name {
        "add" | "sub" | "mul" | "div" => {
            let (bn, bd) = parts(&a[0])?;
            let (x, y) = match name {
                "add" | "sub" => {
                    let l = n.mul(&bd).or_else(fail)?;
                    let r = bn.mul(&d).or_else(fail)?;
                    (if name == "add" { l.add(&r) } else { l.sub(&r) }, d.mul(&bd).or_else(fail)?)
                }
                "mul" => (n.mul(&bn).or_else(fail)?, d.mul(&bd).or_else(fail)?),
                _ => (n.mul(&bd).or_else(fail)?, d.mul(&bn).or_else(fail)?),
            };
            make(&x, &y)?
        }
        "neg" => value(n.neg(), d),
        "abs" => value(n.abs(), d),
        "inv" => make(&d, &n)?,
        "pow" => {
            let (x, y) = pow(&n, &d, int(&a[0])?)?;
            value(x, y)
        }
        "sign" => Value::Int(n.sign()),
        "is_int" => Value::Bool(d.mag == [1]),
        "floor" => rounded(0)?,
        "ceil" => rounded(1)?,
        "trunc" => rounded(2)?,
        "round" => rounded(3)?,
        "to_f64" => Value::Float(ratio_f64(&n, &d).or_else(fail)?),
        "to_dec" => {
            let k = int(&a[0])?;
            if !(0..=MAX_SCALE).contains(&k) {
                return trap("decimal scale must be in 0..=10000");
            }
            Value::Dec(Rc::new(dec_div(&n, 0, &d, 0, k).or_else(fail)?), k)
        }
        _ => return trap(format!("no method '{name}' on Ratio")),
    })
}
