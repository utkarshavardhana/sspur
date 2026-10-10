use crate::ast::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CScalar {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FfiTy {
    Unit,
    Scalar(CScalar),
    Str,
    OptStr,
    Buf(CScalar),
}

impl CScalar {
    pub fn from_name(n: &str) -> Option<CScalar> {
        Some(match n {
            "I8" => CScalar::I8,
            "I16" => CScalar::I16,
            "I32" => CScalar::I32,
            "I64" | "Int" => CScalar::I64,
            "U8" => CScalar::U8,
            "U16" => CScalar::U16,
            "U32" => CScalar::U32,
            "U64" => CScalar::U64,
            "F32" => CScalar::F32,
            "F64" => CScalar::F64,
            "Bool" => CScalar::Bool,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            CScalar::I8 => "I8",
            CScalar::I16 => "I16",
            CScalar::I32 => "I32",
            CScalar::I64 => "Int",
            CScalar::U8 => "U8",
            CScalar::U16 => "U16",
            CScalar::U32 => "U32",
            CScalar::U64 => "U64",
            CScalar::F32 => "F32",
            CScalar::F64 => "F64",
            CScalar::Bool => "Bool",
        }
    }

    pub fn c_type(self) -> &'static str {
        match self {
            CScalar::I8 => "int8_t",
            CScalar::I16 => "int16_t",
            CScalar::I32 => "int32_t",
            CScalar::I64 => "int64_t",
            CScalar::U8 => "uint8_t",
            CScalar::U16 => "uint16_t",
            CScalar::U32 => "uint32_t",
            CScalar::U64 => "uint64_t",
            CScalar::F32 => "float",
            CScalar::F64 => "double",
            CScalar::Bool => "_Bool",
        }
    }

    pub fn sspur(self) -> &'static str {
        match self {
            CScalar::F32 | CScalar::F64 => "F64",
            CScalar::Bool => "Bool",
            _ => "Int",
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, CScalar::F32 | CScalar::F64)
    }

    pub fn range(self) -> Option<(i128, i128)> {
        Some(match self {
            CScalar::I8 => (i8::MIN as i128, i8::MAX as i128),
            CScalar::I16 => (i16::MIN as i128, i16::MAX as i128),
            CScalar::I32 => (i32::MIN as i128, i32::MAX as i128),
            CScalar::U8 => (0, u8::MAX as i128),
            CScalar::U16 => (0, u16::MAX as i128),
            CScalar::U32 => (0, u32::MAX as i128),
            CScalar::U64 => (0, i64::MAX as i128),
            _ => return None,
        })
    }
}

fn named(t: &Ty) -> Option<(&str, &[Ty])> {
    match t {
        Ty::Named { name, args, .. } => Some((name, args)),
        _ => None,
    }
}

pub fn ffi_ty(t: &Ty, ret: bool) -> Result<FfiTy, String> {
    let bad = || Err(format!("type {} cannot cross the C boundary", crate::printer::ty(t)));
    let Some((n, args)) = named(t) else { return bad() };
    match (n, args) {
        ("Unit", []) if ret => Ok(FfiTy::Unit),
        ("Str", []) => Ok(FfiTy::Str),
        ("Opt", [a]) if named(a).is_some_and(|(n, a)| n == "Str" && a.is_empty()) => Ok(FfiTy::OptStr),
        ("List", [a]) if !ret => match named(a).filter(|(_, a)| a.is_empty()).and_then(|(n, _)| CScalar::from_name(n)) {
            Some(s) if s != CScalar::Bool => Ok(FfiTy::Buf(s)),
            _ => bad(),
        },
        (n, []) => CScalar::from_name(n).map(FfiTy::Scalar).map_or_else(bad, Ok),
        _ => bad(),
    }
}

pub fn signature(f: &FnDef) -> Result<(Vec<FfiTy>, FfiTy), String> {
    let params = f.params.iter().map(|p| ffi_ty(&p.ty, false).map_err(|e| format!("parameter '{}': {e}", p.name))).collect::<Result<_, _>>()?;
    let ret = match &f.ret {
        Some(t) => ffi_ty(t, true).map_err(|e| format!("result: {e}"))?,
        None => FfiTy::Unit,
    };
    Ok((params, ret))
}

fn view_ty(t: &Ty) -> Ty {
    match t {
        Ty::Named { name, args, span } => {
            let name = CScalar::from_name(name).map_or_else(|| name.clone(), |s| s.sspur().to_string());
            Ty::Named { name, args: args.iter().map(view_ty).collect(), span: *span }
        }
        other => other.clone(),
    }
}

pub fn sspur_view(f: &FnDef) -> FnDef {
    let mut g = f.clone();
    for p in &mut g.params {
        p.ty = view_ty(&p.ty);
    }
    g.ret = g.ret.as_ref().map(view_ty);
    g
}

pub fn lib_candidates(lib: &str) -> Vec<String> {
    if lib.contains('/') || lib.contains(".so") || lib.ends_with(".dylib") || (cfg!(windows) && (lib.contains('\\') || lib.ends_with(".dll") || lib.ends_with(".lib"))) {
        return vec![lib.to_string()];
    }
    if cfg!(windows) {
        match lib {
            "c" | "m" => vec!["ucrtbase.dll".into(), "msvcrt.dll".into()],
            _ => vec![format!("{lib}.dll"), format!("lib{lib}.dll")],
        }
    } else if cfg!(target_os = "macos") {
        vec![format!("lib{lib}.dylib"), format!("/usr/lib/lib{lib}.dylib")]
    } else {
        vec![format!("lib{lib}.so.6"), format!("lib{lib}.so")]
    }
}

pub fn link_args(lib: &str) -> Vec<String> {
    if lib == "c" || (cfg!(windows) && lib == "m") {
        vec![]
    } else if lib_candidates(lib).len() == 1 {
        vec![lib.to_string()]
    } else {
        vec![format!("-l{lib}")]
    }
}
