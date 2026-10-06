//! Locale values over the bundled rules in `sspur_native::locale`; `std_rt.c` section `locale`
//! implements the same rules over tables generated from the same data.
use crate::value::Value;
use crate::{trap, R};
use sspur_native::locale;
use std::rc::Rc;

fn text(v: &Value) -> R<Rc<str>> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        v => trap(format!("expected Str, got {v}")),
    }
}

pub fn global(n: &str, a: &[Value]) -> R<Option<Value>> {
    if n != "locale" {
        return Ok(None);
    }
    let tag = text(&a[0])?;
    Ok(Some(Value::Opt(locale::index(&tag).map(|i| Rc::new(Value::Record("#Locale".into(), Rc::new(vec![("tag".into(), Value::Str(locale::LOCALES[i].tag.into()))])))))))
}

pub fn method(name: &str, fs: &[(Rc<str>, Value)], a: &[Value]) -> R {
    let i = locale::index(&text(&fs[0].1)?).unwrap_or(0);
    let time = |v: &Value| match v {
        Value::Time(t) => Ok(*t),
        v => trap(format!("expected Time, got {v}")),
    };
    Ok(match name {
        "compare" => Value::Int(locale::compare(&text(&a[0])?, &text(&a[1])?) as i64),
        "sort" => match &a[0] {
            Value::List(xs) => {
                let mut v: Vec<Value> = xs.to_vec();
                v.sort_by(|x, y| match (x, y) {
                    (Value::Str(p), Value::Str(q)) => locale::compare(p, q),
                    _ => x.cmp(y),
                });
                Value::list(v)
            }
            v => return trap(format!("expected List, got {v}")),
        },
        "format_int" => match &a[0] {
            Value::Int(n) => Value::str(&locale::localize(i, &n.to_string())),
            v => return trap(format!("expected Int, got {v}")),
        },
        "format_f64" => {
            let (Value::Float(x), Value::Int(d)) = (&a[0], &a[1]) else { return trap("expected F64 and Int") };
            if !(0..=20).contains(d) {
                return trap("fmt digits must be in 0..=20");
            }
            Value::str(&locale::localize(i, &crate::stdlib::fmt_fixed(*x, *d as usize)))
        }
        "format_dec" => match &a[0] {
            Value::Dec(m, s) => Value::str(&locale::localize(i, &sspur_native::bigint::dec_str(m, *s))),
            v => return trap(format!("expected Dec, got {v}")),
        },
        "format_date" => Value::str(&locale::format_time(i, 0, time(&a[0])?)),
        "format_date_long" => Value::str(&locale::format_time(i, 1, time(&a[0])?)),
        "format_time" => Value::str(&locale::format_time(i, 2, time(&a[0])?)),
        _ => return trap(format!("no method '{name}' on Locale")),
    })
}
