use crate::value::Value;
use crate::{trap, R};
use sspur_syntax::ffi::{signature, CScalar, FfiTy};
use sspur_syntax::FnDef;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

const MAX_INT_ARGS: usize = 6;
const MAX_FLOAT_ARGS: usize = 8;

type IntRet = unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, f64, f64, f64, f64, f64, f64, f64, f64) -> i64;
type FloatRet = unsafe extern "C" fn(i64, i64, i64, i64, i64, i64, f64, f64, f64, f64, f64, f64, f64, f64) -> f64;

type SymCache = HashMap<(Option<String>, String), Result<usize, String>>;

thread_local! {
    static SYMS: RefCell<SymCache> = RefCell::new(HashMap::new());
}

#[cfg(unix)]
fn resolve(lib: Option<&str>, sym: &str) -> Result<usize, String> {
    use libloading::os::unix::{Library, RTLD_GLOBAL, RTLD_NOW};
    let lib = match lib.filter(|l| *l != "c") {
        None => Library::this(),
        Some(l) => sspur_syntax::ffi::lib_candidates(l)
            .iter()
            .find_map(|c| unsafe { Library::open(Some(c), RTLD_NOW | RTLD_GLOBAL) }.ok())
            .ok_or_else(|| format!("ffi: cannot load library {l}"))?,
    };
    let p = unsafe { lib.get::<*const ()>(sym.as_bytes()) }.map_err(|_| format!("ffi: symbol {sym} not found"))?.into_raw() as usize;
    std::mem::forget(lib);
    Ok(p)
}

#[cfg(not(unix))]
fn resolve(_: Option<&str>, _: &str) -> Result<usize, String> {
    Err("ffi: the interpreter supports extern calls on unix only".into())
}

fn int_arg(f: &str, p: &str, s: CScalar, v: i64) -> R<i64> {
    match s.range() {
        Some((lo, hi)) if (v as i128) < lo || (v as i128) > hi => trap(format!("ffi: argument {p} of {f} is out of range for {} (value = {v})", s.name())),
        _ => Ok(v),
    }
}

fn pack(f: &str, p: &str, s: CScalar, xs: &[Value]) -> R<Vec<u8>> {
    let mut out = Vec::with_capacity(xs.len() * 8 + 8);
    for x in xs {
        match (s, x) {
            (CScalar::F64, Value::Float(v)) => out.extend(v.to_ne_bytes()),
            (CScalar::F32, Value::Float(v)) => out.extend((*v as f32).to_ne_bytes()),
            (_, Value::Int(v)) => {
                if s.range().is_some_and(|(lo, hi)| (*v as i128) < lo || (*v as i128) > hi) {
                    return trap(format!("ffi: argument {p} of {f} has an element out of range for {} (value = {v})", s.name()));
                }
                let b = v.to_ne_bytes();
                let n = match s {
                    CScalar::I8 | CScalar::U8 => 1,
                    CScalar::I16 | CScalar::U16 => 2,
                    CScalar::I32 | CScalar::U32 => 4,
                    _ => 8,
                };
                if cfg!(target_endian = "little") { out.extend(&b[..n]) } else { out.extend(&b[8 - n..]) }
            }
            _ => return trap(format!("ffi: argument {p} of {f} has an unexpected element")),
        }
    }
    out.extend([0u8; 8]);
    Ok(out)
}

pub fn call(f: &FnDef, args: &[Value]) -> R {
    let Some(ext) = &f.ext else { return trap("not an extern function") };
    let (ptys, rty) = match signature(f) {
        Ok(s) => s,
        Err(e) => return trap(format!("ffi: {e}")),
    };
    let key = (ext.lib.clone(), ext.symbol.clone());
    let addr = SYMS.with(|m| m.borrow_mut().entry(key).or_insert_with(|| resolve(ext.lib.as_deref(), &ext.symbol)).clone());
    let addr = match addr {
        Ok(a) => a,
        Err(e) => return trap(e),
    };
    let name = f.name.as_str();
    let mut ints: Vec<i64> = Vec::new();
    let mut floats: Vec<f64> = Vec::new();
    let mut keep: Vec<Vec<u8>> = Vec::new();
    for ((p, t), v) in f.params.iter().zip(&ptys).zip(args) {
        let p = p.name.as_str();
        match (t, v) {
            (FfiTy::Scalar(CScalar::F64), Value::Float(x)) => floats.push(*x),
            (FfiTy::Scalar(CScalar::F32), Value::Float(x)) => floats.push(f64::from_bits(u64::from((*x as f32).to_bits()))),
            (FfiTy::Scalar(_), Value::Bool(b)) => ints.push(i64::from(*b)),
            (FfiTy::Scalar(s), Value::Int(x)) => ints.push(int_arg(name, p, *s, *x)?),
            (FfiTy::Str, Value::Str(s)) => {
                let b = cstr(name, p, s)?;
                ints.push(b.as_ptr() as i64);
                keep.push(b);
            }
            (FfiTy::OptStr, Value::Opt(Some(x))) if matches!(&**x, Value::Str(_)) => {
                let Value::Str(s) = &**x else { unreachable!() };
                let b = cstr(name, p, s)?;
                ints.push(b.as_ptr() as i64);
                keep.push(b);
            }
            (FfiTy::OptStr, Value::Opt(None)) => ints.push(0),
            (FfiTy::Buf(s), Value::List(xs)) => {
                let b = pack(name, p, *s, xs)?;
                ints.push(b.as_ptr() as i64);
                keep.push(b);
            }
            _ => return trap(format!("ffi: argument {p} of {name} has an unexpected value")),
        }
    }
    if ints.len() > MAX_INT_ARGS || floats.len() > MAX_FLOAT_ARGS {
        return trap(format!("ffi: {name} has too many arguments for the interpreter ({MAX_INT_ARGS} integer and {MAX_FLOAT_ARGS} float); call it from native code"));
    }
    ints.resize(MAX_INT_ARGS, 0);
    floats.resize(MAX_FLOAT_ARGS, 0.0);
    let (i, x) = (&ints, &floats);
    let float_ret = matches!(rty, FfiTy::Scalar(s) if s.is_float());
    let (ri, rf) = unsafe {
        if float_ret {
            let g = std::mem::transmute::<usize, FloatRet>(addr);
            (0, g(i[0], i[1], i[2], i[3], i[4], i[5], x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]))
        } else {
            let g = std::mem::transmute::<usize, IntRet>(addr);
            (g(i[0], i[1], i[2], i[3], i[4], i[5], x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]), 0.0)
        }
    };
    drop(keep);
    Ok(match rty {
        FfiTy::Unit => Value::Unit,
        FfiTy::Scalar(CScalar::F64) => Value::Float(rf),
        FfiTy::Scalar(CScalar::F32) => Value::Float(f64::from(f32::from_bits(rf.to_bits() as u32))),
        FfiTy::Scalar(CScalar::Bool) => Value::Bool(ri as u8 != 0),
        FfiTy::Scalar(s) => Value::Int(match s {
            CScalar::I8 => ri as i8 as i64,
            CScalar::I16 => ri as i16 as i64,
            CScalar::I32 => ri as i32 as i64,
            CScalar::U8 => ri as u8 as i64,
            CScalar::U16 => ri as u16 as i64,
            CScalar::U32 => ri as u32 as i64,
            CScalar::U64 if ri < 0 => return trap(format!("ffi: result of {name} is out of range for Int (value = {})", ri as u64)),
            _ => ri,
        }),
        FfiTy::Str | FfiTy::OptStr => {
            if ri == 0 {
                return if rty == FfiTy::OptStr { Ok(Value::Opt(None)) } else { trap(format!("ffi: {name} returned a null string")) };
            }
            let c = unsafe { std::ffi::CStr::from_ptr(ri as *const std::ffi::c_char) };
            let Ok(s) = c.to_str() else { return trap(format!("ffi: {name} returned a string that is not valid UTF-8")) };
            let v = Value::Str(Rc::from(s));
            if rty == FfiTy::OptStr { Value::Opt(Some(Rc::new(v))) } else { v }
        }
        FfiTy::Buf(_) => Value::Unit,
    })
}

fn cstr(f: &str, p: &str, s: &str) -> R<Vec<u8>> {
    if s.as_bytes().contains(&0) {
        return trap(format!("ffi: argument {p} of {f} contains a NUL byte"));
    }
    let mut b = s.as_bytes().to_vec();
    b.push(0);
    Ok(b)
}
