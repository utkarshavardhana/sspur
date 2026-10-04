//! Complex numbers; `std_rt.c` section `cx` mirrors each operation in the same order.
use crate::value::Value;
use crate::{trap, R};
use std::rc::Rc;

#[derive(Clone, Copy)]
struct C(f64, f64);

const HALF_PI: f64 = std::f64::consts::FRAC_PI_2;

fn mul(a: C, b: C) -> C {
    let (ac, bd, ad, bc) = (a.0 * b.0, a.1 * b.1, a.0 * b.1, a.1 * b.0);
    C(ac - bd, ad + bc)
}

fn div(a: C, b: C) -> C {
    if b.0.abs() >= b.1.abs() {
        let r = b.1 / b.0;
        let dr = b.1 * r;
        let den = b.0 + dr;
        let (br, ar) = (a.1 * r, a.0 * r);
        C((a.0 + br) / den, (a.1 - ar) / den)
    } else {
        let r = b.0 / b.1;
        let cr = b.0 * r;
        let den = cr + b.1;
        let (ar, br) = (a.0 * r, a.1 * r);
        C((ar + a.1) / den, (br - a.0) / den)
    }
}

fn add(a: C, b: C) -> C {
    C(a.0 + b.0, a.1 + b.1)
}

fn sub(a: C, b: C) -> C {
    C(a.0 - b.0, a.1 - b.1)
}

fn abs(a: C) -> f64 {
    a.0.hypot(a.1)
}

fn exp(a: C) -> C {
    let e = a.0.exp();
    C(e * a.1.cos(), e * a.1.sin())
}

fn ln(a: C) -> C {
    C(abs(a).ln(), a.1.atan2(a.0))
}

fn sqrt(a: C) -> C {
    if a.0 == 0.0 && a.1 == 0.0 {
        return C(0.0, a.1);
    }
    let s = abs(a) + a.0.abs();
    let t = (s / 2.0).sqrt();
    let t2 = 2.0 * t;
    if a.0 >= 0.0 { C(t, a.1 / t2) } else { C(a.1.abs() / t2, t.copysign(a.1)) }
}

fn sin(a: C) -> C {
    C(a.0.sin() * a.1.cosh(), a.0.cos() * a.1.sinh())
}

fn cos(a: C) -> C {
    C(a.0.cos() * a.1.cosh(), -(a.0.sin() * a.1.sinh()))
}

fn sinh(a: C) -> C {
    C(a.0.sinh() * a.1.cos(), a.0.cosh() * a.1.sin())
}

fn cosh(a: C) -> C {
    C(a.0.cosh() * a.1.cos(), a.0.sinh() * a.1.sin())
}

fn times_i(a: C) -> C {
    C(-a.1, a.0)
}

fn asin(z: C) -> C {
    let one = C(1.0, 0.0);
    let w = ln(add(times_i(z), sqrt(sub(one, mul(z, z)))));
    C(w.1, -w.0)
}

fn atan(z: C) -> C {
    let one = C(1.0, 0.0);
    let iz = times_i(z);
    let d = sub(ln(sub(one, iz)), ln(add(one, iz)));
    C(-d.1 / 2.0, d.0 / 2.0)
}

fn unary(name: &str, z: C) -> Option<C> {
    let one = C(1.0, 0.0);
    Some(match name {
        "neg" => C(-z.0, -z.1),
        "conj" => C(z.0, -z.1),
        "exp" => exp(z),
        "ln" => ln(z),
        "sqrt" => sqrt(z),
        "sin" => sin(z),
        "cos" => cos(z),
        "tan" => div(sin(z), cos(z)),
        "sinh" => sinh(z),
        "cosh" => cosh(z),
        "tanh" => div(sinh(z), cosh(z)),
        "asin" => asin(z),
        "acos" => {
            let a = asin(z);
            C(HALF_PI - a.0, -a.1)
        }
        "atan" => atan(z),
        "asinh" => ln(add(z, sqrt(add(mul(z, z), one)))),
        "acosh" => ln(add(z, mul(sqrt(add(z, one)), sqrt(sub(z, one))))),
        "atanh" => {
            let d = sub(ln(add(one, z)), ln(sub(one, z)));
            C(d.0 / 2.0, d.1 / 2.0)
        }
        _ => return None,
    })
}

fn pow(z: C, w: C) -> C {
    if z.0 == 0.0 && z.1 == 0.0 {
        return if w.0 == 0.0 && w.1 == 0.0 { C(1.0, 0.0) } else { C(0.0, 0.0) };
    }
    exp(mul(w, ln(z)))
}

fn value(c: C) -> Value {
    Value::Record("#Complex".into(), Rc::new(vec![("re".into(), Value::Float(c.0)), ("im".into(), Value::Float(c.1))]))
}

fn f(v: &Value) -> R<f64> {
    match v {
        Value::Float(x) => Ok(*x),
        v => trap(format!("expected F64, got {v}")),
    }
}

fn of(v: &Value) -> R<C> {
    match v {
        Value::Record(_, fs) => Ok(C(f(&fs[0].1)?, f(&fs[1].1)?)),
        v => trap(format!("expected Complex, got {v}")),
    }
}

pub fn global(n: &str, a: &[Value]) -> R<Option<Value>> {
    Ok(Some(match n {
        "complex" => value(C(f(&a[0])?, f(&a[1])?)),
        "polar" => {
            let (r, t) = (f(&a[0])?, f(&a[1])?);
            value(C(r * t.cos(), r * t.sin()))
        }
        _ => return Ok(None),
    }))
}

pub fn method(name: &str, fs: &[(Rc<str>, Value)], a: &[Value]) -> R {
    let z = C(f(&fs[0].1)?, f(&fs[1].1)?);
    if let Some(c) = unary(name, z) {
        return Ok(value(c));
    }
    Ok(match name {
        "abs" => Value::Float(abs(z)),
        "arg" => Value::Float(z.1.atan2(z.0)),
        "norm" => {
            let (x, y) = (z.0 * z.0, z.1 * z.1);
            Value::Float(x + y)
        }
        "scale" => {
            let k = f(&a[0])?;
            value(C(z.0 * k, z.1 * k))
        }
        "add" | "sub" | "mul" | "div" | "pow" => {
            let w = of(&a[0])?;
            value(match name {
                "add" => add(z, w),
                "sub" => sub(z, w),
                "mul" => mul(z, w),
                "div" => div(z, w),
                _ => pow(z, w),
            })
        }
        _ => return trap(format!("no method '{name}' on Complex")),
    })
}
