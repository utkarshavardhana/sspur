use crate::value::Value;
use crate::{trap, R};
use sspur_native::chrono::{civil_ms, days_from_civil, iso, mdays, parse, parts, strftime, DAY};

fn ovf<T>() -> R<T> {
    trap("integer overflow")
}

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

fn dur(v: &Value) -> R<i64> {
    match v {
        Value::Dur(n) | Value::Time(n) => Ok(*n),
        v => trap(format!("expected Duration, got {v}")),
    }
}

fn opt_time(t: Option<i64>) -> Value {
    Value::Opt(t.map(|t| std::rc::Rc::new(Value::Time(t))))
}

pub fn add_months(t: i64, n: i64) -> R<i64> {
    let p = parts(t);
    let Some(total) = (p.y * 12 + p.mo - 1).checked_add(n) else { return ovf() };
    let (ny, nm) = (total.div_euclid(12), total.rem_euclid(12) + 1);
    if !(-300_000_000..=300_000_000).contains(&ny) {
        return ovf();
    }
    let nd = p.d.min(mdays(ny, nm));
    match (days_from_civil(ny, nm, nd).checked_mul(DAY)).and_then(|x| x.checked_add(t.rem_euclid(DAY))) {
        Some(v) => Ok(v),
        None => ovf(),
    }
}

pub fn global(n: &str, a: &[Value]) -> R<Value> {
    let scaled = |k: i64| match int(&a[0])?.checked_mul(k) {
        Some(v) => Ok(Value::Dur(v)),
        None => ovf(),
    };
    Ok(match n {
        "time_ms" => Value::Time(int(&a[0])?),
        "date" => opt_time(civil_ms(int(&a[0])?, int(&a[1])?, int(&a[2])?, 0, 0, 0)),
        "datetime" => opt_time(civil_ms(int(&a[0])?, int(&a[1])?, int(&a[2])?, int(&a[3])?, int(&a[4])?, int(&a[5])?)),
        "parse_time" => match &a[0] {
            Value::Str(s) => opt_time(parse(s)),
            _ => return trap("expected Str"),
        },
        "now" => Value::Time(crate::sys::wall_ms()),
        "millis" => scaled(1)?,
        "secs" => scaled(1000)?,
        "mins" => scaled(60_000)?,
        "hours" => scaled(3_600_000)?,
        "days" => scaled(DAY)?,
        _ => return trap(format!("unknown builtin '{n}'")),
    })
}

pub fn time_method(name: &str, t: i64, a: &[Value]) -> R<Value> {
    let p = || parts(t);
    let i = Value::Int;
    Ok(match name {
        "unix_ms" => i(t),
        "year" => i(p().y),
        "month" => i(p().mo),
        "day" => i(p().d),
        "hour" => i(p().h),
        "minute" => i(p().mi),
        "second" => i(p().s),
        "milli" => i(p().ms),
        "weekday" => i(p().wd),
        "yday" => i(p().yd),
        "date" => Value::Time(t - t.rem_euclid(DAY)),
        "iso" => Value::str(&iso(t)),
        "format" => match &a[0] {
            Value::Str(s) => match strftime(t, s) {
                Some(out) => Value::str(&out),
                None => return trap(format!("bad time format '{s}'")),
            },
            _ => return trap("expected Str"),
        },
        "add" | "sub" => {
            let d = dur(&a[0])?;
            match if name == "add" { t.checked_add(d) } else { t.checked_sub(d) } {
                Some(v) => Value::Time(v),
                None => return ovf(),
            }
        }
        "since" => match t.checked_sub(dur(&a[0])?) {
            Some(v) => Value::Dur(v),
            None => return ovf(),
        },
        "add_months" => Value::Time(add_months(t, int(&a[0])?)?),
        "add_years" => match int(&a[0])?.checked_mul(12) {
            Some(m) => Value::Time(add_months(t, m)?),
            None => return ovf(),
        },
        _ => return trap(format!("no method '{name}' on Time")),
    })
}

pub fn dur_method(name: &str, d: i64, a: &[Value]) -> R<Value> {
    let k = || int(&a[0]);
    let wrap = |v: Option<i64>| v.map_or_else(ovf, |v| Ok(Value::Dur(v)));
    Ok(match name {
        "ms" => Value::Int(d),
        "secs" => Value::Int(d / 1000),
        "mins" => Value::Int(d / 60_000),
        "hours" => Value::Int(d / 3_600_000),
        "days" => Value::Int(d / DAY),
        "add" => wrap(d.checked_add(dur(&a[0])?))?,
        "sub" => wrap(d.checked_sub(dur(&a[0])?))?,
        "mul" => wrap(d.checked_mul(k()?))?,
        "div" => {
            let k = k()?;
            if k == 0 {
                return trap("division by zero");
            }
            wrap(d.checked_div(k))?
        }
        "neg" => wrap(d.checked_neg())?,
        "abs" => wrap(d.checked_abs())?,
        _ => return trap(format!("no method '{name}' on Duration")),
    })
}
