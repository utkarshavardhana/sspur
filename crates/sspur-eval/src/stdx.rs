use crate::value::Value;
use crate::{trap, R};

#[derive(Default)]
struct Spec {
    fill: Option<char>,
    align: u8,
    sign: u8,
    alt: bool,
    zero: bool,
    comma: bool,
    width: usize,
    prec: Option<usize>,
    ty: u8,
}

fn parse_spec(s: &str) -> Option<Spec> {
    let c: Vec<char> = s.chars().collect();
    let is_align = |ch: char| matches!(ch, '<' | '>' | '^' | '=');
    let mut f = Spec::default();
    let mut i = 0;
    if c.len() >= 2 && is_align(c[1]) {
        f.fill = Some(c[0]);
        f.align = c[1] as u8;
        i = 2;
    } else if !c.is_empty() && is_align(c[0]) {
        f.align = c[0] as u8;
        i = 1;
    }
    if i < c.len() && matches!(c[i], '+' | '-' | ' ') {
        f.sign = c[i] as u8;
        i += 1;
    }
    if i < c.len() && c[i] == '#' {
        f.alt = true;
        i += 1;
    }
    if i < c.len() && c[i] == '0' {
        f.zero = true;
        i += 1;
    }
    while i < c.len() && c[i].is_ascii_digit() {
        f.width = f.width * 10 + (c[i] as usize - '0' as usize);
        if f.width > 100_000 {
            return None;
        }
        i += 1;
    }
    if i < c.len() && c[i] == ',' {
        f.comma = true;
        i += 1;
    }
    if i < c.len() && c[i] == '.' {
        i += 1;
        let st = i;
        let mut p = 0usize;
        while i < c.len() && c[i].is_ascii_digit() {
            p = p * 10 + (c[i] as usize - '0' as usize);
            if p > 1000 {
                return None;
            }
            i += 1;
        }
        if i == st {
            return None;
        }
        f.prec = Some(p);
    }
    if i < c.len() && "dxXobfeE%s".contains(c[i]) {
        f.ty = c[i] as u8;
        i += 1;
    }
    (i == c.len()).then_some(f)
}

fn group(digits: &str) -> String {
    let lead = digits.bytes().take_while(u8::is_ascii_digit).count();
    let (int, rest) = digits.split_at(lead);
    let mut out = String::new();
    for (k, ch) in int.chars().enumerate() {
        if k > 0 && (lead - k) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out + rest
}

fn pad(f: &Spec, sign: &str, prefix: &str, body: &str, numeric: bool) -> String {
    let len = sign.chars().count() + prefix.chars().count() + body.chars().count();
    let fill = f.fill.unwrap_or(if f.zero { '0' } else { ' ' });
    let align = match f.align {
        0 if f.zero => b'=',
        0 if numeric => b'>',
        0 => b'<',
        a => a,
    };
    let n = f.width.saturating_sub(len);
    let rep = |k: usize| std::iter::repeat_n(fill, k).collect::<String>();
    match align {
        b'<' => format!("{sign}{prefix}{body}{}", rep(n)),
        b'^' => format!("{}{sign}{prefix}{body}{}", rep(n / 2), rep(n - n / 2)),
        b'=' => format!("{sign}{prefix}{}{body}", rep(n)),
        _ => format!("{}{sign}{prefix}{body}", rep(n)),
    }
}

fn sign_str(neg: bool, f: &Spec) -> &'static str {
    if neg {
        "-"
    } else if f.sign == b'+' {
        "+"
    } else if f.sign == b' ' {
        " "
    } else {
        ""
    }
}

fn bad(spec: &str) -> R<String> {
    trap(format!("bad format spec '{spec}'"))
}

pub fn format_int(n: i64, spec: &str) -> R<String> {
    let Some(f) = parse_spec(spec) else { return bad(spec) };
    if f.prec.is_some() || !matches!(f.ty, 0 | b'd' | b'x' | b'X' | b'o' | b'b') || (f.comma && !matches!(f.ty, 0 | b'd')) {
        return bad(spec);
    }
    let u = n.unsigned_abs();
    let (digits, prefix) = match f.ty {
        b'x' => (format!("{u:x}"), "0x"),
        b'X' => (format!("{u:X}"), "0X"),
        b'o' => (format!("{u:o}"), "0o"),
        b'b' => (format!("{u:b}"), "0b"),
        _ => (u.to_string(), ""),
    };
    let digits = if f.comma { group(&digits) } else { digits };
    Ok(pad(&f, sign_str(n < 0, &f), if f.alt { prefix } else { "" }, &digits, true))
}

fn exp_form(x: f64, p: usize, upper: bool) -> String {
    let s = format!("{x:.p$e}");
    let (m, e) = s.split_once('e').unwrap_or((&s, "0"));
    let e: i64 = e.parse().unwrap_or(0);
    format!("{m}{}{}{:02}", if upper { 'E' } else { 'e' }, if e < 0 { '-' } else { '+' }, e.unsigned_abs())
}

pub fn format_f64(x: f64, spec: &str) -> R<String> {
    let Some(f) = parse_spec(spec) else { return bad(spec) };
    if f.alt || !matches!(f.ty, 0 | b'f' | b'e' | b'E' | b'%') || (f.comma && matches!(f.ty, b'e' | b'E')) {
        return bad(spec);
    }
    let neg = x.is_sign_negative() && !x.is_nan();
    let a = x.abs();
    let mut body = if x.is_nan() {
        "NaN".to_string()
    } else if a.is_infinite() {
        "inf".to_string()
    } else {
        match (f.ty, f.prec) {
            (b'e' | b'E', p) => exp_form(a, p.unwrap_or(6), f.ty == b'E'),
            (b'%', p) => format!("{:.*}%", p.unwrap_or(6), a * 100.0),
            (b'f', p) | (_, p @ Some(_)) => format!("{a:.*}", p.unwrap_or(6)),
            _ => Value::Float(a).to_string(),
        }
    };
    if f.comma && x.is_finite() {
        body = group(&body);
    }
    Ok(pad(&f, sign_str(neg, &f), "", &body, true))
}

pub fn format_str(s: &str, spec: &str) -> R<String> {
    let Some(f) = parse_spec(spec) else { return bad(spec) };
    if f.sign != 0 || f.alt || f.zero || f.comma || f.align == b'=' || !matches!(f.ty, 0 | b's') {
        return bad(spec);
    }
    let body: String = match f.prec {
        Some(p) => s.chars().take(p).collect(),
        None => s.to_string(),
    };
    Ok(pad(&f, "", "", &body, false))
}
