use super::*;
use sspur_syntax::ffi::{signature, CScalar, FfiTy};

pub(super) const FFI_PRELUDE: &str = r#"#define FFI_STR2(x) #x
#define FFI_STR(x) FFI_STR2(x)
#define FFI_SYM(s) FFI_STR(__USER_LABEL_PREFIX__) s
static void __attribute__((noinline, cold, noreturn)) ffi_fail(Status* st, int64_t f, const char* m, const char* tail) {
    size_t a = strlen(m), b = tail ? strlen(tail) : 0;
    char* p = (char*)malloc(a + b + 1); memcpy(p, m, a); if (b) memcpy(p + a, tail, b); p[a + b] = 0;
    st->rbuf = (int64_t*)p; st->rlen = (int64_t)(a + b);
    sspur_trap(st, 12, f, 0, 0);
}
static void __attribute__((noinline, cold, noreturn)) ffi_fail_i(Status* st, int64_t f, const char* m, int64_t v) { char t[32]; snprintf(t, sizeof t, "%lld)", (long long)v); ffi_fail(st, f, m, t); }
static void __attribute__((noinline, cold, noreturn)) ffi_fail_u(Status* st, int64_t f, const char* m, uint64_t v) { char t[32]; snprintf(t, sizeof t, "%llu)", (unsigned long long)v); ffi_fail(st, f, m, t); }
static const char* ffi_cstr(Status* st, int64_t f, Str s, const char* m) {
    if (s.len && memchr(s.p, 0, (size_t)s.len)) ffi_fail(st, f, m, 0);
    char* p = (char*)sspur_alloc_atomic((size_t)s.len + 1);
    if (s.len) memcpy(p, s.p, (size_t)s.len);
    p[s.len] = 0;
    return p;
}
static int ffi_utf8_ok(const unsigned char* s, size_t n) {
    size_t i = 0;
    while (i < n) {
        unsigned c = s[i], cp, min; size_t k;
        if (c < 0x80) { i++; continue; }
        if (c >= 0xC2 && c <= 0xDF) { k = 1; cp = c & 0x1F; min = 0x80; }
        else if (c >= 0xE0 && c <= 0xEF) { k = 2; cp = c & 0x0F; min = 0x800; }
        else if (c >= 0xF0 && c <= 0xF4) { k = 3; cp = c & 0x07; min = 0x10000; }
        else return 0;
        if (n - i <= k) return 0;
        for (size_t j = 1; j <= k; j++) { unsigned d = s[i + j]; if ((d & 0xC0) != 0x80) return 0; cp = (cp << 6) | (d & 0x3F); }
        if (cp < min || cp > 0x10FFFF || (cp >= 0xD800 && cp <= 0xDFFF)) return 0;
        i += k + 1;
    }
    return 1;
}
static Str ffi_str(Status* st, int64_t f, const char* p, const char* null_m, const char* utf_m) {
    if (!p) ffi_fail(st, f, null_m, 0);
    size_t n = strlen(p);
    if (!ffi_utf8_ok((const unsigned char*)p, n)) ffi_fail(st, f, utf_m, 0);
    char* o = (char*)sspur_alloc_atomic(n + 1);
    memcpy(o, p, n + 1);
    return (Str){(int64_t)n, o};
}
"#;

fn range_cond(s: CScalar, v: &str) -> Option<String> {
    let (lo, hi) = s.range()?;
    Some(if lo == 0 { format!("{v} < 0 || {v} > {hi}LL") } else { format!("{v} < {lo}LL || {v} > {hi}LL") })
}

impl Cx<'_> {
    pub(super) fn extern_fn(&mut self, f: &FnDef, params: &[Type], ret: &Type, fidx: usize) -> G {
        let ext = f.ext.as_ref().ok_or("not extern")?;
        let (ptys, rty) = signature(f)?;
        let name = &f.name;
        let rr = self.rr(ret)?;
        let mut sig = Vec::new();
        for (i, t) in params.iter().enumerate() {
            sig.push(format!("{} a{i}", self.cty(t)?));
        }
        sig.push("Status* st".into());
        sig.push("int64_t depth".into());
        let cret = match rty {
            FfiTy::Unit => "void".to_string(),
            FfiTy::Scalar(s) => s.c_type().to_string(),
            FfiTy::Str | FfiTy::OptStr => "const char*".to_string(),
            FfiTy::Buf(_) => return Err("returns a buffer".into()),
        };
        let mut cparams = Vec::new();
        let mut body = String::from("  (void)depth;\n");
        let mut args = Vec::new();
        for (i, (p, t)) in f.params.iter().zip(&ptys).enumerate() {
            let a = format!("a{i}");
            let prefix = format!("ffi: argument {} of {name} ", p.name);
            match t {
                FfiTy::Scalar(s) => {
                    cparams.push(s.c_type().to_string());
                    if let Some(c) = range_cond(*s, &a) {
                        let m = c_lit(&format!("{prefix}is out of range for {} (value = ", s.name()));
                        writeln!(body, "  if (UNLIKELY({c})) ffi_fail_i(st, {fidx}, {m}, {a});").unwrap();
                    }
                    args.push(match s {
                        CScalar::Bool => format!("(_Bool)({a} != 0)"),
                        _ => format!("({}){a}", s.c_type()),
                    });
                }
                FfiTy::Str => {
                    cparams.push("const char*".into());
                    args.push(format!("ffi_cstr(st, {fidx}, {a}, {})", c_lit(&format!("{prefix}contains a NUL byte"))));
                }
                FfiTy::OptStr => {
                    cparams.push("const char*".into());
                    args.push(format!("({a}.some ? ffi_cstr(st, {fidx}, {a}.v, {}) : (const char*)0)", c_lit(&format!("{prefix}contains a NUL byte"))));
                }
                FfiTy::Buf(s) => {
                    let ct = s.c_type();
                    cparams.push(format!("const {ct}*"));
                    let b = format!("b{i}");
                    writeln!(body, "  {ct}* {b} = ({ct}*)sspur_alloc_atomic((size_t){a}.len * sizeof({ct}) + 8);").unwrap();
                    write!(body, "  for (int64_t k = 0; k < {a}.len; k++) {{ ").unwrap();
                    if let Some(c) = range_cond(*s, &format!("{a}.data[k]")) {
                        let m = c_lit(&format!("{prefix}has an element out of range for {} (value = ", s.name()));
                        write!(body, "if (UNLIKELY({c})) ffi_fail_i(st, {fidx}, {m}, {a}.data[k]); ").unwrap();
                    }
                    writeln!(body, "{b}[k] = ({ct}){a}.data[k]; }}").unwrap();
                    args.push(b);
                }
                FfiTy::Unit => return Err("has a Unit parameter".into()),
            }
        }
        let cname = format!("sspur_c_{name}");
        let proto = if cparams.is_empty() { "void".to_string() } else { cparams.join(", ") };
        let mut s = format!("extern {cret} {cname}({proto}) __asm__(FFI_SYM({}));\n", c_lit(&ext.symbol));
        writeln!(s, "#define RRT {rr}\n#define FIDX {fidx}\nstatic {rr} f_{name}({}) {{\n{body}", sig.join(", ")).unwrap();
        let call = format!("{cname}({})", args.join(", "));
        let null_m = c_lit(&format!("ffi: {name} returned a null string"));
        let utf_m = c_lit(&format!("ffi: {name} returned a string that is not valid UTF-8"));
        let value = match rty {
            FfiTy::Unit => {
                writeln!(s, "  {call};").unwrap();
                "0".to_string()
            }
            FfiTy::Scalar(sc) => {
                writeln!(s, "  {} r_ = {call};", sc.c_type()).unwrap();
                match sc {
                    CScalar::U64 => {
                        let m = c_lit(&format!("ffi: result of {name} is out of range for Int (value = "));
                        writeln!(s, "  if (UNLIKELY(r_ > (uint64_t)INT64_MAX)) ffi_fail_u(st, {fidx}, {m}, r_);").unwrap();
                        "(int64_t)r_".into()
                    }
                    CScalar::Bool => "(int64_t)(r_ != 0)".into(),
                    CScalar::F32 | CScalar::F64 => "(double)r_".into(),
                    _ => "(int64_t)r_".into(),
                }
            }
            FfiTy::Str => {
                writeln!(s, "  const char* r_ = {call};").unwrap();
                format!("ffi_str(st, {fidx}, r_, {null_m}, {utf_m})")
            }
            FfiTy::OptStr => {
                let oc = self.cty(ret)?;
                writeln!(s, "  const char* r_ = {call};").unwrap();
                format!("(r_ ? ({oc}){{1, ffi_str(st, {fidx}, r_, {null_m}, {utf_m})}} : ({oc}){{0, {{0, 0}}}})")
            }
            FfiTy::Buf(_) => unreachable!(),
        };
        writeln!(s, "  return (RRT){{{value}, 0}};\n}}\n#undef RRT\n#undef FIDX").unwrap();
        Ok(s)
    }
}
