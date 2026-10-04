use crate::value::Value;
use crate::{trap, R};
use std::collections::BTreeMap;
use std::rc::Rc;

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

fn list(v: &Value) -> R<Rc<Vec<Value>>> {
    match v {
        Value::List(xs) => Ok(xs.clone()),
        v => trap(format!("expected List, got {v}")),
    }
}

fn ints(v: &Value) -> R<Vec<i64>> {
    list(v)?.iter().map(int).collect()
}

fn int_list(xs: &[i64]) -> Value {
    Value::list(xs.iter().map(|x| Value::Int(*x)).collect())
}

fn flat(keys: Vec<Value>, values: Vec<Value>) -> Value {
    Value::Record("#FlatMap".into(), Rc::new(vec![("keys".into(), Value::list(keys)), ("values".into(), Value::list(values))]))
}

pub fn empty_flat() -> Value {
    flat(vec![], vec![])
}

pub fn to_flat(xs: &[Value]) -> R {
    let mut m = BTreeMap::new();
    for x in xs {
        let Value::Tuple(p) = x else { return trap("expected a pair") };
        m.insert(p[0].clone(), p[1].clone());
    }
    let (k, v) = m.into_iter().unzip();
    Ok(flat(k, v))
}

fn lower(keys: &[Value], k: &Value) -> usize {
    let (mut lo, mut hi) = (0, keys.len());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if keys[mid] < *k {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

pub fn flat_method(name: &str, fs: &[(Rc<str>, Value)], a: &[Value]) -> R {
    let (ks, vs) = (list(&fs[0].1)?, list(&fs[1].1)?);
    let n = ks.len().min(vs.len());
    let (keys, values) = (&ks[..n], &vs[..n]);
    let find = |k: &Value| {
        let i = lower(keys, k);
        (i, i < n && keys[i] == *k)
    };
    Ok(match name {
        "len" => Value::Int(n as i64),
        "is_empty" => Value::Bool(n == 0),
        "lower_bound" => Value::Int(lower(keys, &a[0]) as i64),
        "has" => Value::Bool(find(&a[0]).1),
        "get" => match find(&a[0]) {
            (i, true) => Value::some(values[i].clone()),
            _ => Value::Opt(None),
        },
        "items" => Value::list(keys.iter().zip(values).map(|(k, v)| Value::Tuple(Rc::new(vec![k.clone(), v.clone()]))).collect()),
        "put" => {
            let (i, found) = find(&a[0]);
            let (mut k2, mut v2) = (keys.to_vec(), values.to_vec());
            if found {
                v2[i] = a[1].clone();
            } else {
                k2.insert(i, a[0].clone());
                v2.insert(i, a[1].clone());
            }
            flat(k2, v2)
        }
        "remove" => {
            let (i, found) = find(&a[0]);
            let (mut k2, mut v2) = (keys.to_vec(), values.to_vec());
            if found {
                k2.remove(i);
                v2.remove(i);
            }
            flat(k2, v2)
        }
        "between" => {
            let lo = lower(keys, &a[0]);
            let hi = lower(keys, &a[1]).max(lo);
            flat(keys[lo..hi].to_vec(), values[lo..hi].to_vec())
        }
        _ => return trap(format!("no method '{name}' on FlatMap")),
    })
}

struct Md {
    data: Rc<Vec<Value>>,
    off: i64,
    shape: Vec<i64>,
    strides: Vec<i64>,
}

const MALFORMED: &str = "malformed mdspan";

fn md_value(data: Rc<Vec<Value>>, off: i64, shape: &[i64], strides: &[i64]) -> Value {
    Value::Record("#MdSpan".into(), Rc::new(vec![("data".into(), Value::List(data)), ("offset".into(), Value::Int(off)), ("shape".into(), int_list(shape)), ("strides".into(), int_list(strides))]))
}

fn md_of(fs: &[(Rc<str>, Value)]) -> R<Md> {
    let m = Md { data: list(&fs[0].1)?, off: int(&fs[1].1)?, shape: ints(&fs[2].1)?, strides: ints(&fs[3].1)? };
    if m.strides.len() != m.shape.len() || m.shape.iter().any(|e| *e < 0) {
        return trap(MALFORMED);
    }
    Ok(m)
}

fn product(shape: &[i64]) -> R<i64> {
    let mut p: i64 = 1;
    for e in shape {
        p = match p.checked_mul(*e) {
            Some(v) => v,
            None => return trap("integer overflow"),
        };
    }
    Ok(p)
}

fn at(m: &Md, idx: &[i64]) -> R<usize> {
    let mut f = m.off as i128;
    for (d, i) in idx.iter().enumerate() {
        f += *i as i128 * m.strides[d] as i128;
        if f.abs() > 1 << 64 {
            return trap(MALFORMED);
        }
    }
    if f < 0 || f >= m.data.len() as i128 {
        return trap(MALFORMED);
    }
    Ok(f as usize)
}

fn coord(i: i64, e: i64) -> R<()> {
    if i < 0 || i >= e {
        return trap(format!("mdspan index {i} out of range for extent {e}"));
    }
    Ok(())
}

fn index(m: &Md, v: &Value) -> R<usize> {
    let idx = ints(v)?;
    if idx.len() != m.shape.len() {
        return trap(format!("mdspan index needs {} coordinates, got {}", m.shape.len(), idx.len()));
    }
    for (d, i) in idx.iter().enumerate() {
        coord(*i, m.shape[d])?;
    }
    at(m, &idx)
}

fn shift(off: i64, i: i64, s: i64) -> R<i64> {
    let f = off as i128 + i as i128 * s as i128;
    i64::try_from(f).or_else(|_| trap(MALFORMED))
}

pub fn mdspan(data: &Value, shape: &Value) -> R {
    let data = list(data)?;
    let shape = ints(shape)?;
    if shape.iter().any(|e| *e < 0) {
        return trap("mdspan extents must be >= 0");
    }
    let p = product(&shape)?;
    if p != data.len() as i64 {
        return trap(format!("mdspan shape needs {p} elements, got {}", data.len()));
    }
    let mut strides = vec![1i64; shape.len()];
    for d in (0..shape.len().saturating_sub(1)).rev() {
        strides[d] = match strides[d + 1].checked_mul(shape[d + 1]) {
            Some(v) => v,
            None => return trap("integer overflow"),
        };
    }
    Ok(md_value(data, 0, &shape, &strides))
}

pub fn md_method(name: &str, fs: &[(Rc<str>, Value)], a: &[Value]) -> R {
    let m = md_of(fs)?;
    let rank = m.shape.len() as i64;
    Ok(match name {
        "rank" => Value::Int(rank),
        "size" => Value::Int(product(&m.shape)?),
        "get" => m.data[index(&m, &a[0])?].clone(),
        "set" => {
            let f = index(&m, &a[0])?;
            let mut d = (*m.data).clone();
            d[f] = a[1].clone();
            md_value(Rc::new(d), m.off, &m.shape, &m.strides)
        }
        "transpose" => {
            let (mut sh, mut sd) = (m.shape.clone(), m.strides.clone());
            sh.reverse();
            sd.reverse();
            md_value(m.data.clone(), m.off, &sh, &sd)
        }
        "slice" => {
            let (dim, lo, hi) = (int(&a[0])?, int(&a[1])?, int(&a[2])?);
            if dim < 0 || dim >= rank {
                return trap(format!("mdspan dimension {dim} out of range for rank {rank}"));
            }
            let e = m.shape[dim as usize];
            if lo < 0 || hi < lo || hi > e {
                return trap(format!("mdspan slice {lo}..{hi} out of range for extent {e}"));
            }
            let off = shift(m.off, lo, m.strides[dim as usize])?;
            let mut sh = m.shape.clone();
            sh[dim as usize] = hi - lo;
            md_value(m.data.clone(), off, &sh, &m.strides)
        }
        "sub" => {
            if rank == 0 {
                return trap("mdspan dimension 0 out of range for rank 0");
            }
            let i = int(&a[0])?;
            coord(i, m.shape[0])?;
            let off = shift(m.off, i, m.strides[0])?;
            md_value(m.data.clone(), off, &m.shape[1..], &m.strides[1..])
        }
        "to_list" => {
            let n = product(&m.shape)?;
            if n > m.data.len() as i64 {
                return trap(MALFORMED);
            }
            let mut out = Vec::with_capacity(n as usize);
            let mut idx = vec![0i64; m.shape.len()];
            for _ in 0..n {
                out.push(m.data[at(&m, &idx)?].clone());
                for d in (0..idx.len()).rev() {
                    idx[d] += 1;
                    if idx[d] < m.shape[d] {
                        break;
                    }
                    idx[d] = 0;
                }
            }
            Value::list(out)
        }
        _ => return trap(format!("no method '{name}' on MdSpan")),
    })
}
