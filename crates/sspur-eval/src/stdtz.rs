use crate::value::Value;
use crate::{trap, R};
use sspur_native::tz::{self, Zone};
use std::rc::Rc;

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) | Value::Time(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

fn ints(v: &Value) -> R<Vec<i64>> {
    match v {
        Value::List(xs) => xs.iter().map(int).collect(),
        v => trap(format!("expected List, got {v}")),
    }
}

fn s(v: &Value) -> R<String> {
    match v {
        Value::Str(x) => Ok(x.to_string()),
        v => trap(format!("expected Str, got {v}")),
    }
}

pub fn zone_value(z: &Zone) -> Value {
    let il = |xs: &[i64]| Value::list(xs.iter().map(|x| Value::Int(*x)).collect());
    Value::Record(
        "#Zone".into(),
        Rc::new(vec![
            ("name".into(), Value::str(&z.name)),
            ("trans".into(), il(&z.trans)),
            ("idx".into(), il(&z.idx)),
            ("offs".into(), il(&z.offs)),
            ("dst".into(), Value::list(z.dst.iter().map(|b| Value::Bool(*b)).collect())),
            ("abbrs".into(), Value::list(z.abbrs.iter().map(|a| Value::str(a)).collect())),
            ("tail".into(), il(&z.rule)),
        ]),
    )
}

pub fn zone_of(v: &Value) -> R<Zone> {
    let Value::Record(_, fs) = v else { return trap(format!("expected Zone, got {v}")) };
    let Value::List(dst) = &fs[4].1 else { return trap("expected List") };
    let Value::List(abbrs) = &fs[5].1 else { return trap("expected List") };
    Ok(Zone {
        name: s(&fs[0].1)?,
        trans: ints(&fs[1].1)?,
        idx: ints(&fs[2].1)?,
        offs: ints(&fs[3].1)?,
        dst: dst.iter().map(|b| matches!(b, Value::Bool(true))).collect(),
        abbrs: abbrs.iter().map(s).collect::<R<_>>()?,
        rule: ints(&fs[6].1)?,
    })
}

fn res(r: Result<Zone, String>) -> Value {
    match r {
        Ok(z) => Value::Res(Ok(Rc::new(zone_value(&z)))),
        Err(e) => Value::Res(Err(Rc::new(Value::str(&e)))),
    }
}

pub fn global(n: &str, a: &[Value]) -> R<Option<Value>> {
    Ok(Some(match n {
        "time_zone" => res(tz::load(&s(&a[0])?)),
        "local_zone" => res(tz::load_local()),
        "fixed_zone" => {
            let m = int(&a[0])?;
            if !(-1440 < m && m < 1440) {
                return trap("fixed_zone needs -1440 < minutes < 1440");
            }
            zone_value(&tz::fixed(&tz::offset_name(m * 60), m * 60))
        }
        _ => return Ok(None),
    }))
}

fn local(z: &Zone, t: i64) -> R<i64> {
    z.local(t).map_or_else(|| trap("integer overflow"), Ok)
}

pub fn method(name: &str, recv: &Value, a: &[Value]) -> R {
    let z = zone_of(recv)?;
    let t = int(&a[0])?;
    Ok(match name {
        "offset" => Value::Dur(z.offset_ms(t)),
        "abbr" => Value::str(&z.abbrs[z.type_at(t)]),
        "is_dst" => Value::Bool(z.dst[z.type_at(t)]),
        "local" => Value::Time(local(&z, t)?),
        "utc" => match z.utc(t) {
            Some(r) => Value::Opt(r.map(|x| Rc::new(Value::Time(x)))),
            None => return trap("integer overflow"),
        },
        _ => return trap(format!("no method '{name}' on Zone")),
    })
}

pub fn time_method(name: &str, t: i64, a: &[Value]) -> R<Option<Value>> {
    if !matches!(name, "format_in" | "iso_in") {
        return Ok(None);
    }
    let z = zone_of(&a[0])?;
    let off = z.offset_ms(t);
    let l = local(&z, t)?;
    Ok(Some(if name == "iso_in" {
        let iso = sspur_native::chrono::iso(l);
        Value::str(&if off == 0 { iso } else { format!("{}{}", &iso[..iso.len() - 1], tz::offset_name(off / 1000)) })
    } else {
        let pat = s(&a[1])?;
        match sspur_native::chrono::strftime(l, &tz::zone_pattern(&pat, off, &z.abbrs[z.type_at(t)])) {
            Some(x) => Value::str(&x),
            None => return trap(format!("bad time format '{pat}'")),
        }
    }))
}
