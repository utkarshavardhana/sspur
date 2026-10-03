use crate::value::Value;
use crate::{from_nval, to_nval, trap, Interp, R};
use sspur_syntax::{Expr, Span};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

fn s(v: &Value) -> R<&str> {
    match v {
        Value::Str(x) => Ok(x),
        v => trap(format!("expected Str, got {v}")),
    }
}

fn boolean(v: Value) -> R<bool> {
    match v {
        Value::Bool(b) => Ok(b),
        v => trap(format!("expected Bool, got {v}")),
    }
}

fn list(v: &Value) -> R<Rc<Vec<Value>>> {
    match v {
        Value::List(xs) => Ok(xs.clone()),
        v => trap(format!("expected List, got {v}")),
    }
}

fn pair(a: Value, b: Value) -> Value {
    Value::Tuple(Rc::new(vec![a, b]))
}

fn opt(o: Option<Value>) -> Value {
    Value::Opt(o.map(Rc::new))
}

fn ints(xs: impl Iterator<Item = i64>) -> Value {
    Value::list(xs.map(Value::Int).collect())
}

const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn rng(s: &mut u64) -> u64 {
    *s = s.wrapping_add(GOLDEN);
    mix(*s)
}

fn rng_f64(s: &mut u64) -> f64 {
    (rng(s) >> 11) as f64 * (1.0 / 9007199254740992.0)
}

fn rng_below(s: &mut u64, n: usize) -> usize {
    ((rng(s) as u128 * n as u128) >> 64) as usize
}

fn rng_normal(s: &mut u64, mean: f64, sd: f64) -> f64 {
    let u1 = rng_f64(s);
    let u2 = rng_f64(s);
    let r = (-2.0 * (1.0 - u1).ln()).sqrt();
    let c = (std::f64::consts::TAU * u2).cos();
    let z = r * c;
    let t = sd * z;
    mean + t
}

#[cfg(target_os = "macos")]
const ENOTEMPTY: i32 = 66;
#[cfg(not(target_os = "macos"))]
const ENOTEMPTY: i32 = 39;

pub fn os_reason(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(2) => "not found".into(),
        Some(1 | 13) => "permission denied".into(),
        Some(21) => "is a directory".into(),
        Some(20) => "not a directory".into(),
        Some(17) => "already exists".into(),
        Some(ENOTEMPTY) => "directory not empty".into(),
        Some(n) => format!("os error {n}"),
        None => "invalid UTF-8".into(),
    }
}

fn io_res(path: &str, r: Result<Value, std::io::Error>) -> Value {
    if path.contains('\0') {
        return Value::Res(Err(Rc::new(Value::str(&format!("{path}: invalid path")))));
    }
    match r {
        Ok(v) => Value::Res(Ok(Rc::new(v))),
        Err(e) => Value::Res(Err(Rc::new(Value::str(&format!("{path}: {}", os_reason(&e)))))),
    }
}

unsafe extern "C" {
    fn read(fd: i32, buf: *mut std::ffi::c_void, n: usize) -> isize;
    fn clock_gettime(clk: i32, tp: *mut [i64; 2]) -> i32;
}

#[cfg(target_os = "macos")]
const CLOCK_MONOTONIC: i32 = 6;
#[cfg(not(target_os = "macos"))]
const CLOCK_MONOTONIC: i32 = 1;

pub fn mono_ns() -> i64 {
    let mut t = [0i64; 2];
    unsafe { clock_gettime(CLOCK_MONOTONIC, &mut t) };
    t[0].saturating_mul(1_000_000_000).saturating_add(t[1])
}

fn run_cmd(prog: &str, args: &[String], input: String) -> Result<Value, String> {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{Command, Stdio};
    if prog.contains('\0') {
        return Err("invalid path".into());
    }
    if args.iter().any(|a| a.contains('\0')) {
        return Err("invalid argument".into());
    }
    let mut child = Command::new(prog).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|e| os_reason(&e))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let writer = std::thread::spawn(move || {
        use std::io::Write;
        let _ = stdin.write_all(input.as_bytes());
    });
    let out = child.wait_with_output().map_err(|e| os_reason(&e))?;
    let _ = writer.join();
    let code = out.status.code().map_or_else(|| 128 + i64::from(out.status.signal().unwrap_or(0)), i64::from);
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|_| "invalid UTF-8".to_string());
    let (so, se) = (text(out.stdout)?, text(out.stderr)?);
    Ok(Value::Tuple(Rc::new(vec![Value::Int(code), Value::str(&so), Value::str(&se)])))
}

fn mkdir_all(p: &str) -> Result<Value, std::io::Error> {
    if p.is_empty() {
        return Err(std::io::Error::from_raw_os_error(2));
    }
    let b = p.as_bytes();
    for i in 1..=b.len() {
        if i < b.len() && b[i] != b'/' {
            continue;
        }
        if let Err(e) = std::fs::create_dir(&p[..i])
            && !(e.raw_os_error() == Some(17) && std::fs::metadata(&p[..i]).is_ok_and(|m| m.is_dir())) {
                return Err(e);
            }
    }
    Ok(Value::Unit)
}

fn read_line_raw() -> R<Option<String>> {
    let mut line = Vec::new();
    let mut got = false;
    loop {
        let mut b = 0u8;
        let n = unsafe { read(0, (&raw mut b).cast(), 1) };
        if n <= 0 {
            break;
        }
        got = true;
        if b == b'\n' {
            break;
        }
        line.push(b);
    }
    if !got {
        return Ok(None);
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    match String::from_utf8(line) {
        Ok(l) => Ok(Some(l)),
        Err(_) => trap("stdin is not valid UTF-8"),
    }
}

pub fn fmt_fixed(x: f64, d: usize) -> String {
    if x.is_nan() {
        "NaN".into()
    } else if x.is_infinite() {
        if x > 0.0 { "inf".into() } else { "-inf".into() }
    } else {
        format!("{x:.d$}")
    }
}

pub fn float_syntax(t: &str) -> bool {
    let b = t.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        i = 1;
    }
    let d0 = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = i > d0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let f0 = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        digits |= i > f0;
    }
    if !digits {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let e0 = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == e0 {
            return false;
        }
    }
    i == b.len()
}

fn pad(x: &str, n: i64, fill: &str, left: bool) -> R {
    let len = x.chars().count() as i64;
    if fill.is_empty() || len >= n {
        return Ok(Value::str(x));
    }
    let fc = fill.chars().count() as i64;
    let copies = (n - len + fc - 1) / fc;
    if copies > (1i64 << 28) / fill.len() as i64 {
        return trap("out of memory");
    }
    let p: String = fill.repeat(copies as usize).chars().take((n - len) as usize).collect();
    Ok(Value::str(&if left { p + x } else { x.to_string() + &p }))
}

impl Interp {
    pub(crate) fn std_global(&self, n: &str, a: Vec<Value>) -> R {
        Ok(match n {
            "empty_set" => Value::Set(Rc::new(BTreeSet::new())),
            "empty_heap" => Value::Heap(Rc::new(Vec::new())),
            "str_buf" => Value::str(""),
            "range" => {
                let (st, en, step) = (int(&a[0])? as i128, int(&a[1])? as i128, int(&a[2])? as i128);
                if step == 0 {
                    return trap("range step must not be 0");
                }
                let count = if step > 0 && en > st {
                    (en - st - 1) / step + 1
                } else if step < 0 && en < st {
                    (st - en - 1) / -step + 1
                } else {
                    0
                };
                if count > 1 << 26 {
                    return trap("out of memory");
                }
                ints((0..count).map(|k| (st + k * step) as i64))
            }
            "clamp" => {
                let mut it = a.into_iter();
                let (x, lo, hi) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
                if x < lo {
                    lo
                } else if x > hi {
                    hi
                } else {
                    x
                }
            }
            "rand" | "rand_int" | "rand_f64" => {
                let z = (int(&a[0])? as u64).wrapping_add(GOLDEN);
                let m = mix(z);
                let v = match n {
                    "rand" => Value::Int((m >> 1) as i64),
                    "rand_f64" => Value::Float((m >> 11) as f64 * (1.0 / 9007199254740992.0)),
                    _ => {
                        let (lo, hi) = (int(&a[1])?, int(&a[2])?);
                        if lo >= hi {
                            return trap("rand_int needs lo < hi");
                        }
                        let span = (hi as i128 - lo as i128) as u128;
                        Value::Int((lo as i128 + ((m as u128 * span) >> 64) as i128) as i64)
                    }
                };
                pair(v, Value::Int(z as i64))
            }
            "rand_normal" | "rand_uniform" | "rand_exp" | "rand_bool" => {
                let mut st = int(&a[0])? as u64;
                let f = |i: usize| match a.get(i) {
                    Some(Value::Float(x)) => Ok(*x),
                    _ => trap("expected F64"),
                };
                let v = match n {
                    "rand_normal" => Value::Float(rng_normal(&mut st, f(1)?, f(2)?)),
                    "rand_uniform" => {
                        let (lo, hi) = (f(1)?, f(2)?);
                        let u = rng_f64(&mut st);
                        let w = hi - lo;
                        let t = w * u;
                        Value::Float(lo + t)
                    }
                    "rand_exp" => {
                        let rate = f(1)?;
                        Value::Float(-(1.0 - rng_f64(&mut st)).ln() / rate)
                    }
                    _ => {
                        let p = f(1)?;
                        Value::Bool(rng_f64(&mut st) < p)
                    }
                };
                pair(v, Value::Int(st as i64))
            }
            "from_bytes" => {
                let xs = list(&a[0])?;
                let mut bs = Vec::with_capacity(xs.len());
                for x in xs.iter() {
                    match u8::try_from(int(x)?) {
                        Ok(b) => bs.push(b),
                        Err(_) => return Ok(opt(None)),
                    }
                }
                opt(String::from_utf8(bs).ok().map(|s| Value::str(&s)))
            }
            "from_codes" => {
                let xs = list(&a[0])?;
                let mut out = String::new();
                for x in xs.iter() {
                    match u32::try_from(int(x)?).ok().and_then(char::from_u32) {
                        Some(c) => out.push(c),
                        None => return Ok(opt(None)),
                    }
                }
                opt(Some(Value::str(&out)))
            }
            "read_file" => {
                let p = s(&a[0])?;
                let r = std::fs::read(p).and_then(|b| String::from_utf8(b).map_err(|_| std::io::Error::other("utf8"))).map(|t| Value::str(&t));
                io_res(p, r)
            }
            "write_file" | "append_file" => {
                use std::io::Write;
                let (p, text) = (s(&a[0])?, s(&a[1])?);
                let f = if n == "write_file" { std::fs::File::create(p) } else { std::fs::OpenOptions::new().append(true).create(true).open(p) };
                io_res(p, f.and_then(|mut f| f.write_all(text.as_bytes())).map(|_| Value::Unit))
            }
            "list_dir" => {
                let p = s(&a[0])?;
                let r = std::fs::read_dir(p).and_then(|rd| {
                    let mut names = Vec::new();
                    for e in rd {
                        names.push(e?.file_name().to_string_lossy().into_owned());
                    }
                    names.sort();
                    Ok(Value::list(names.iter().map(|x| Value::str(x)).collect()))
                });
                io_res(p, r)
            }
            "remove_file" => {
                let p = s(&a[0])?;
                io_res(p, std::fs::remove_file(p).map(|_| Value::Unit))
            }
            "read_bytes" => {
                let p = s(&a[0])?;
                io_res(p, std::fs::read(p).map(|b| ints(b.into_iter().map(i64::from))))
            }
            "write_bytes" => {
                let p = s(&a[0])?;
                let mut bs = Vec::new();
                for x in list(&a[1])?.iter() {
                    match u8::try_from(int(x)?) {
                        Ok(b) => bs.push(b),
                        Err(_) if !p.contains('\0') => return Ok(Value::Res(Err(Rc::new(Value::str(&format!("{p}: byte out of range")))))),
                        Err(_) => break,
                    }
                }
                io_res(p, std::fs::write(p, bs).map(|_| Value::Unit))
            }
            "mkdir" => {
                let p = s(&a[0])?;
                io_res(p, std::fs::create_dir(p).map(|_| Value::Unit))
            }
            "mkdir_all" => {
                let p = s(&a[0])?;
                io_res(p, mkdir_all(p))
            }
            "remove_dir" => {
                let p = s(&a[0])?;
                io_res(p, std::fs::remove_dir(p).map(|_| Value::Unit))
            }
            "rename" => {
                let (p, q) = (s(&a[0])?, s(&a[1])?);
                if q.contains('\0') {
                    return Ok(Value::Res(Err(Rc::new(Value::str(&format!("{p}: invalid path"))))));
                }
                io_res(p, std::fs::rename(p, q).map(|_| Value::Unit))
            }
            "exists" | "is_dir" => {
                let p = s(&a[0])?;
                let m = if p.contains('\0') { None } else { std::fs::metadata(p).ok() };
                Value::Bool(m.is_some_and(|m| n == "exists" || m.is_dir()))
            }
            "file_size" | "modified_ms" => {
                use std::os::unix::fs::MetadataExt;
                let p = s(&a[0])?;
                io_res(p, std::fs::metadata(p).map(|m| Value::Int(if n == "file_size" { m.size() as i64 } else { m.mtime() * 1000 + m.mtime_nsec() / 1_000_000 })))
            }
            "run_cmd" => {
                let prog = s(&a[0])?;
                let args = list(&a[1])?.iter().map(|x| s(x).map(str::to_string)).collect::<R<Vec<_>>>()?;
                Value::Res(match run_cmd(prog, &args, s(&a[2])?.to_string()) {
                    Ok(v) => Ok(Rc::new(v)),
                    Err(m) => Err(Rc::new(Value::str(&format!("{prog}: {m}")))),
                })
            }
            "eprint" => {
                use std::io::Write;
                let _ = writeln!(std::io::stderr(), "{}", s(&a[0])?);
                Value::Unit
            }
            "read_line" => opt(read_line_raw()?.map(|l| Value::str(&l))),
            "read_lines" => {
                let mut out = Vec::new();
                while let Some(l) = read_line_raw()? {
                    out.push(Value::str(&l));
                }
                Value::list(out)
            }
            "now_ms" => Value::Int(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)),
            "mono_ns" => Value::Int(mono_ns()),
            "sleep_ms" => {
                std::thread::sleep(Duration::from_millis(int(&a[0])?.max(0) as u64));
                Value::Unit
            }
            "env_var" => {
                let k = s(&a[0])?;
                if k.is_empty() || k.contains('=') || k.contains('\0') {
                    opt(None)
                } else {
                    opt(std::env::var(k).ok().map(|v| Value::str(&v)))
                }
            }
            "args" => Value::list(sspur_native::program_args().iter().map(|x| Value::str(x)).collect()),
            "pi" => Value::Float(std::f64::consts::PI),
            "euler" => Value::Float(std::f64::consts::E),
            "inf" => Value::Float(f64::INFINITY),
            "nan" => Value::Float(f64::NAN),
            "bits" => crate::stdx::bits_new(int(&a[0])?)?,
            "time_ms" | "date" | "datetime" | "parse_time" | "now" | "millis" | "secs" | "mins" | "hours" | "days" => crate::stdtime::global(n, &a)?,
            _ => return trap(format!("unknown builtin '{n}'")),
        })
    }

    fn msort(&self, a: &mut [Value], f: &Value) -> R<()> {
        let n = a.len();
        if n < 2 {
            return Ok(());
        }
        let h = n / 2;
        self.msort(&mut a[..h], f)?;
        self.msort(&mut a[h..], f)?;
        let (mut i, mut j) = (0, h);
        let mut out = Vec::with_capacity(n);
        while i < h && j < n {
            if int(&self.apply(f, vec![a[j].clone(), a[i].clone()])?)? < 0 {
                out.push(a[j].clone());
                j += 1;
            } else {
                out.push(a[i].clone());
                i += 1;
            }
        }
        out.extend_from_slice(&a[i..h]);
        out.extend_from_slice(&a[j..n]);
        a.clone_from_slice(&out);
        Ok(())
    }

    pub(crate) fn std_list(&self, name: &str, xs: &Rc<Vec<Value>>, mut a: Vec<Value>) -> R {
        let lower = |x: &Value| {
            let (mut lo, mut hi) = (0, xs.len());
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if xs[mid] < *x {
                    lo = mid + 1
                } else {
                    hi = mid
                }
            }
            lo
        };
        let sizes = |what: &str| -> R<usize> {
            let n = int(&a[0])?;
            if n <= 0 { trap(format!("{what} size must be > 0")) } else { Ok(n as usize) }
        };
        Ok(match name {
            "binary_search" => {
                let i = lower(&a[0]);
                opt((i < xs.len() && xs[i] == a[0]).then_some(Value::Int(i as i64)))
            }
            "lower_bound" => Value::Int(lower(&a[0]) as i64),
            "sort_with" => {
                let mut v = (**xs).clone();
                self.msort(&mut v, &a[0])?;
                Value::list(v)
            }
            "chunks" => {
                let n = sizes("chunk")?;
                Value::list(xs.chunks(n).map(|c| Value::list(c.to_vec())).collect())
            }
            "windows" => {
                let n = sizes("window")?;
                Value::list(xs.windows(n).map(|c| Value::list(c.to_vec())).collect())
            }
            "group_by" => {
                let mut idx: BTreeMap<Value, usize> = BTreeMap::new();
                let mut groups: Vec<(Value, Vec<Value>)> = Vec::new();
                for x in xs.iter() {
                    let k = self.apply(&a[0], vec![x.clone()])?;
                    match idx.get(&k) {
                        Some(&g) => groups[g].1.push(x.clone()),
                        None => {
                            idx.insert(k.clone(), groups.len());
                            groups.push((k, vec![x.clone()]));
                        }
                    }
                }
                Value::list(groups.into_iter().map(|(k, g)| pair(k, Value::list(g))).collect())
            }
            "partition" => {
                let (mut yes, mut no) = (Vec::new(), Vec::new());
                for x in xs.iter() {
                    if boolean(self.apply(&a[0], vec![x.clone()])?)? {
                        yes.push(x.clone())
                    } else {
                        no.push(x.clone())
                    }
                }
                pair(Value::list(yes), Value::list(no))
            }
            "scan" => {
                let g = a.remove(1);
                let mut acc = a.remove(0);
                let mut out = Vec::with_capacity(xs.len());
                for x in xs.iter() {
                    acc = self.apply(&g, vec![acc, x.clone()])?;
                    out.push(acc.clone());
                }
                Value::list(out)
            }
            "flatten" => {
                let mut out = Vec::new();
                for x in xs.iter() {
                    out.extend(list(x)?.iter().cloned());
                }
                Value::list(out)
            }
            "index_of" => opt(xs.iter().position(|x| *x == a[0]).map(|i| Value::Int(i as i64))),
            "find_index" => {
                for (i, x) in xs.iter().enumerate() {
                    if boolean(self.apply(&a[0], vec![x.clone()])?)? {
                        return Ok(opt(Some(Value::Int(i as i64))));
                    }
                }
                opt(None)
            }
            "slice" => {
                let clampi = |v: i64| v.clamp(0, xs.len() as i64) as usize;
                let (from, to) = (clampi(int(&a[0])?), clampi(int(&a[1])?));
                Value::list(if to > from { xs[from..to].to_vec() } else { vec![] })
            }
            "push_front" => {
                let mut v = Vec::with_capacity(xs.len() + 1);
                v.push(a.remove(0));
                v.extend(xs.iter().cloned());
                Value::list(v)
            }
            "pop_front" => opt(xs.first().map(|x| pair(x.clone(), Value::list(xs[1..].to_vec())))),
            "pop_back" => opt(xs.last().map(|x| pair(x.clone(), Value::list(xs[..xs.len() - 1].to_vec())))),
            "to_set" => Value::Set(Rc::new(xs.iter().cloned().collect())),
            "to_bits" => crate::stdx::bits_from(xs, int(&a[0])?)?,
            "shuffle" => {
                let mut v = (**xs).clone();
                let mut st = int(&a[0])? as u64;
                for i in (1..v.len()).rev() {
                    let j = rng_below(&mut st, i + 1);
                    v.swap(i, j);
                }
                Value::list(v)
            }
            "choice" => {
                let mut st = int(&a[0])? as u64;
                opt((!xs.is_empty()).then(|| xs[rng_below(&mut st, xs.len())].clone()))
            }
            "to_heap" => {
                let mut v = (**xs).clone();
                v.sort();
                Value::Heap(Rc::new(v))
            }
            _ => return trap(format!("no method '{name}' on List")),
        })
    }

    pub(crate) fn set_method(&self, name: &str, st: &Rc<BTreeSet<Value>>, a: Vec<Value>) -> R {
        let other = |v: &Value| match v {
            Value::Set(t) => Ok(t.clone()),
            v => trap(format!("expected Set, got {v}")),
        };
        Ok(match name {
            "add" => {
                let mut n = (**st).clone();
                n.insert(a[0].clone());
                Value::Set(Rc::new(n))
            }
            "remove" => {
                let mut n = (**st).clone();
                n.remove(&a[0]);
                Value::Set(Rc::new(n))
            }
            "has" => Value::Bool(st.contains(&a[0])),
            "len" => Value::Int(st.len() as i64),
            "is_empty" => Value::Bool(st.is_empty()),
            "items" => Value::list(st.iter().cloned().collect()),
            "union" => Value::Set(Rc::new(st.union(&*other(&a[0])?).cloned().collect())),
            "inter" => Value::Set(Rc::new(st.intersection(&*other(&a[0])?).cloned().collect())),
            "diff" => Value::Set(Rc::new(st.difference(&*other(&a[0])?).cloned().collect())),
            "min" => opt(st.first().cloned()),
            "max" => opt(st.last().cloned()),
            _ => return trap(format!("no method '{name}' on Set")),
        })
    }

    pub(crate) fn heap_method(&self, name: &str, h: &Rc<Vec<Value>>, a: Vec<Value>) -> R {
        Ok(match name {
            "push" => {
                let mut v = (**h).clone();
                let i = v.partition_point(|x| *x <= a[0]);
                v.insert(i, a[0].clone());
                Value::Heap(Rc::new(v))
            }
            "pop" => opt(h.first().map(|x| pair(x.clone(), Value::Heap(Rc::new(h[1..].to_vec()))))),
            "peek" => opt(h.first().cloned()),
            "len" => Value::Int(h.len() as i64),
            "is_empty" => Value::Bool(h.is_empty()),
            "items" => Value::list((**h).clone()),
            _ => return trap(format!("no method '{name}' on Heap")),
        })
    }

    pub(crate) fn json_op(&self, span: Span, name: &str, args: &[Expr], env: &Rc<crate::value::Env>) -> R {
        let Some(t) = self.json_types.get(&(span.start, span.end)) else { return trap("json needs the checker's type information") };
        let v = self.eval(&args[0], env)?;
        match name {
            "encode" => {
                let mut out = String::new();
                match to_nval(&v).and_then(|nv| sspur_native::json::encode(&self.layouts, &nv, t, &mut out)) {
                    Some(()) => Ok(Value::str(&out)),
                    None => trap(format!("cannot encode {v} as JSON")),
                }
            }
            _ => Ok(Value::Res(match sspur_native::json::decode(&self.layouts, s(&v)?, t) {
                Ok(nv) => Ok(Rc::new(from_nval(nv))),
                Err(m) => Err(Rc::new(Value::str(&m))),
            })),
        }
    }
}

pub(crate) fn std_str(name: &str, x: &str, a: Vec<Value>) -> R {
    Ok(match name {
        "add" => Value::str(&format!("{x}{}", s(&a[0])?)),
        "split_once" => opt(x.split_once(s(&a[0])?).map(|(l, r)| pair(Value::str(l), Value::str(r)))),
        "index_of" => opt(x.find(s(&a[0])?).map(|b| Value::Int(x[..b].chars().count() as i64))),
        "pad_left" | "pad_right" => return pad(x, int(&a[0])?, s(&a[1])?, name == "pad_left"),
        "to_f64" => {
            let t = x.trim();
            opt(if float_syntax(t) { t.parse::<f64>().ok().map(Value::Float) } else { None })
        }
        "format" => Value::str(&crate::stdx::format_str(x, s(&a[0])?)?),
        "bytes" => ints(x.bytes().map(i64::from)),
        "codes" => ints(x.chars().map(|c| c as i64)),
        _ => return trap(format!("no method '{name}' on Str")),
    })
}

pub(crate) fn std_float(name: &str, x: f64, a: Vec<Value>) -> R {
    let y = || match a.first() {
        Some(Value::Float(y)) => Ok(*y),
        v => trap(format!("expected F64, got {v:?}", v = v.map(|v| v.to_string()))),
    };
    let to_int = |v: f64| Value::Int(v as i64);
    Ok(match name {
        "fmt" => {
            let d = int(&a[0])?;
            if !(0..=20).contains(&d) {
                return trap("fmt digits must be in 0..=20");
            }
            Value::str(&fmt_fixed(x, d as usize))
        }
        "pow" => Value::Float(x.powf(y()?)),
        "exp" => Value::Float(x.exp()),
        "ln" => Value::Float(x.ln()),
        "log2" => Value::Float(x.log2()),
        "log10" => Value::Float(x.log10()),
        "sin" => Value::Float(x.sin()),
        "cos" => Value::Float(x.cos()),
        "tan" => Value::Float(x.tan()),
        "asin" => Value::Float(x.asin()),
        "acos" => Value::Float(x.acos()),
        "atan" => Value::Float(x.atan()),
        "atan2" => Value::Float(x.atan2(y()?)),
        "hypot" => Value::Float(x.hypot(y()?)),
        "ceil" => to_int(x.ceil()),
        "trunc" => to_int(x.trunc()),
        "is_nan" => Value::Bool(x.is_nan()),
        "is_finite" => Value::Bool(x.is_finite()),
        "is_inf" => Value::Bool(x.is_infinite()),
        "format" => Value::str(&crate::stdx::format_f64(x, s(&a[0])?)?),
        "fma" => match a.get(1) {
            Some(Value::Float(z)) => Value::Float(unsafe { cm::fma(x, y()?, *z) }),
            _ => return trap("expected F64"),
        },
        _ => match (cm::unary(name), cm::binary(name)) {
            (Some(f), _) => Value::Float(unsafe { f(x) }),
            (_, Some(f)) => Value::Float(unsafe { f(x, y()?) }),
            _ => return trap(format!("no method '{name}' on F64")),
        },
    })
}

mod cm {
    unsafe extern "C" {
        fn sinh(x: f64) -> f64;
        fn cosh(x: f64) -> f64;
        fn tanh(x: f64) -> f64;
        fn asinh(x: f64) -> f64;
        fn acosh(x: f64) -> f64;
        fn atanh(x: f64) -> f64;
        fn cbrt(x: f64) -> f64;
        fn exp2(x: f64) -> f64;
        fn expm1(x: f64) -> f64;
        fn log1p(x: f64) -> f64;
        fn erf(x: f64) -> f64;
        fn erfc(x: f64) -> f64;
        fn tgamma(x: f64) -> f64;
        fn lgamma(x: f64) -> f64;
        fn fmod(x: f64, y: f64) -> f64;
        fn remainder(x: f64, y: f64) -> f64;
        fn copysign(x: f64, y: f64) -> f64;
        fn nextafter(x: f64, y: f64) -> f64;
        fn fdim(x: f64, y: f64) -> f64;
        pub fn fma(x: f64, y: f64, z: f64) -> f64;
    }

    type U = unsafe extern "C" fn(f64) -> f64;
    type B = unsafe extern "C" fn(f64, f64) -> f64;

    pub fn unary(n: &str) -> Option<U> {
        Some(match n {
            "sinh" => sinh,
            "cosh" => cosh,
            "tanh" => tanh,
            "asinh" => asinh,
            "acosh" => acosh,
            "atanh" => atanh,
            "cbrt" => cbrt,
            "exp2" => exp2,
            "expm1" => expm1,
            "log1p" => log1p,
            "erf" => erf,
            "erfc" => erfc,
            "gamma" => tgamma,
            "lgamma" => lgamma,
            _ => return None,
        })
    }

    pub fn binary(n: &str) -> Option<B> {
        Some(match n {
            "fmod" => fmod,
            "remainder" => remainder,
            "copysign" => copysign,
            "nextafter" => nextafter,
            "fdim" => fdim,
            _ => return None,
        })
    }
}

fn gcd_u(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

pub(crate) fn std_int(name: &str, n: i64, a: Vec<Value>) -> R {
    let b = || int(&a[0]);
    let ovf = || trap("integer overflow");
    Ok(match name {
        "gcd" => {
            let g = gcd_u(n.unsigned_abs(), b()?.unsigned_abs());
            match i64::try_from(g) {
                Ok(g) => Value::Int(g),
                Err(_) => return ovf(),
            }
        }
        "lcm" => {
            let m = b()?;
            if n == 0 || m == 0 {
                return Ok(Value::Int(0));
            }
            let g = gcd_u(n.unsigned_abs(), m.unsigned_abs()) as u128;
            match i64::try_from(n.unsigned_abs() as u128 / g * m.unsigned_abs() as u128) {
                Ok(l) => Value::Int(l),
                Err(_) => return ovf(),
            }
        }
        "wrapping_add" => Value::Int(n.wrapping_add(b()?)),
        "wrapping_sub" => Value::Int(n.wrapping_sub(b()?)),
        "wrapping_mul" => Value::Int(n.wrapping_mul(b()?)),
        "checked_add" => opt(n.checked_add(b()?).map(Value::Int)),
        "checked_sub" => opt(n.checked_sub(b()?).map(Value::Int)),
        "checked_mul" => opt(n.checked_mul(b()?).map(Value::Int)),
        "checked_div" => opt(n.checked_div(b()?).map(Value::Int)),
        "bnot" => Value::Int(!n),
        "popcount" => Value::Int(i64::from(n.count_ones())),
        "clz" => Value::Int(i64::from(n.leading_zeros())),
        "ctz" => Value::Int(i64::from(n.trailing_zeros())),
        "rotl" => Value::Int((n as u64).rotate_left((b()? & 63) as u32) as i64),
        "rotr" => Value::Int((n as u64).rotate_right((b()? & 63) as u32) as i64),
        "byteswap" => Value::Int(n.swap_bytes()),
        "format" => Value::str(&crate::stdx::format_int(n, s(&a[0])?)?),
        "saturating_add" => Value::Int(n.saturating_add(b()?)),
        "saturating_sub" => Value::Int(n.saturating_sub(b()?)),
        "saturating_mul" => Value::Int(n.saturating_mul(b()?)),
        _ => return trap(format!("no method '{name}' on Int")),
    })
}
