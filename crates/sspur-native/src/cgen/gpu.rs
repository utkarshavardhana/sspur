use super::*;
use sspur_check::kernel::{KExpr, KFn, KIntr, KStmt, KTy, Kernel};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tgt {
    C,
    Msl,
    Ocl,
}

pub(super) struct Em<'a> {
    k: &'a Kernel,
    t: Tgt,
    out: String,
    n: usize,
    ind: usize,
}

const FLT_TINY: &str = "2.3509887e-38f";

fn lit(t: KTy, bits: u64, tgt: Tgt) -> String {
    match t {
        KTy::F32 => {
            let x = f32::from_bits(bits as u32);
            if x.is_nan() {
                return "(0.0f/0.0f)".into();
            }
            if x.is_infinite() {
                return if x > 0.0 { "(1.0f/0.0f)".into() } else { "(-1.0f/0.0f)".into() };
            }
            format!("{x:e}f")
        }
        KTy::F64 => {
            let x = f64::from_bits(bits);
            if x.is_nan() {
                return "(0.0/0.0)".into();
            }
            if x.is_infinite() {
                return if x > 0.0 { "(1.0/0.0)".into() } else { "(-1.0/0.0)".into() };
            }
            format!("{x:e}")
        }
        KTy::Bool => if bits != 0 { "1" } else { "0" }.into(),
        _ => {
            let v = bits as i64;
            match (v, tgt) {
                (i64::MIN, Tgt::C) => "INT64_MIN".into(),
                (i64::MIN, _) => "(-9223372036854775807L - 1L)".into(),
                (v, Tgt::C) => format!("{v}LL"),
                (v, _) => format!("{v}L"),
            }
        }
    }
}

pub fn elem_c(t: KTy, tgt: Tgt) -> &'static str {
    match (t, tgt) {
        (KTy::F32, _) => "float",
        (KTy::F64, _) => "double",
        (KTy::I32, Tgt::C) => "int32_t",
        (KTy::U32, Tgt::C) => "uint32_t",
        (KTy::I32, _) => "int",
        (KTy::U32, _) => "uint",
        (KTy::Int | KTy::Bool, Tgt::C) => "int64_t",
        (KTy::Int | KTy::Bool, _) => "long",
    }
}

impl<'a> Em<'a> {
    pub(super) fn new(k: &'a Kernel, t: Tgt, ind: usize) -> Self {
        Em { k, t, out: String::new(), n: 0, ind }
    }

    pub(super) fn take(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    fn vty(&self, t: KTy) -> &'static str {
        match (t, self.t) {
            (KTy::F32, _) => "float",
            (KTy::F64, _) => "double",
            (KTy::Bool, Tgt::C) => "int",
            (KTy::Bool, _) => "bool",
            (_, Tgt::C) => "int64_t",
            _ => "long",
        }
    }

    fn line(&mut self, s: &str) {
        for _ in 0..self.ind {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn fresh(&mut self) -> String {
        self.n += 1;
        format!("t{}", self.n)
    }

    fn fail(&mut self, cond: &str, code: i64, clause: &str, value: &str) {
        match self.t {
            Tgt::C => self.line(&format!("if (UNLIKELY({cond})) TRAPV({code}, {clause}, {value});")),
            Tgt::Msl => self.line(&format!("if ({cond}) {{ atomic_store_explicit(ss_flag, 1u, memory_order_relaxed); return; }}")),
            Tgt::Ocl => self.line(&format!("if ({cond}) {{ *ss_flag = 1u; return; }}")),
        }
    }

    fn range(&mut self, t: KTy, v: &str) {
        match t {
            KTy::I32 => self.fail(&format!("{v} < -2147483648LL || {v} > 2147483647LL").replace("LL", if self.t == Tgt::C { "LL" } else { "L" }), T_OVERFLOW, "0", "0"),
            KTy::U32 => self.fail(&format!("{v} < 0 || {v} > 4294967295LL").replace("LL", if self.t == Tgt::C { "LL" } else { "L" }), T_OVERFLOW, "0", "0"),
            _ => {}
        }
    }

    fn tiny(&mut self, r: &str, cond: &str) {
        if self.t != Tgt::C {
            self.fail(&format!("fabs({r}) < {FLT_TINY} && ({cond})"), 0, "0", "0");
        }
    }

    fn min_i(&self) -> &'static str {
        if self.t == Tgt::C { "INT64_MIN" } else { "(-9223372036854775807L - 1L)" }
    }

    pub(super) fn expr(&mut self, e: &KExpr) -> String {
        match e {
            KExpr::Lit(t, b) => lit(*t, *b, self.t),
            KExpr::Local(s) => format!("v{s}"),
            KExpr::Param(p) => match self.k.params[*p].ty {
                KTy::Bool if self.t != Tgt::C => format!("(p{p} != 0)"),
                _ => format!("p{p}"),
            },
            KExpr::Len(p) => format!("l{p}"),
            KExpr::Intr(i) => match i {
                KIntr::Gid => "gid".into(),
                KIntr::Lid => format!("(gid % {})", self.k.group),
                KIntr::GroupId => format!("(gid / {})", self.k.group),
                KIntr::GroupSize => format!("{}", self.k.group),
                KIntr::GridSize => "ss_n".into(),
            },
            KExpr::Load(p, i) => {
                let i = self.expr(i);
                let t = self.fresh();
                let et = self.k.params[*p].ty;
                let cast = if self.t == Tgt::C { "(uint64_t)" } else { "(ulong)" };
                self.fail(&format!("{cast}{i} >= {cast}l{p}"), T_INDEX, &format!("l{p}"), &i);
                let vt = self.vty(et);
                self.line(&format!("{vt} {t} = ({vt})p{p}[{i}];"));
                if et == KTy::F32 {
                    match self.t {
                        Tgt::Msl => self.fail(&format!("ss_sub(as_type<uint>({t}))"), 0, "0", "0"),
                        Tgt::Ocl => self.fail(&format!("ss_sub(as_uint({t}))"), 0, "0", "0"),
                        Tgt::C => {}
                    }
                }
                t
            }
            KExpr::Bin(op @ (BinOp::And | BinOp::Or), _, a, b) => {
                let a = self.expr(a);
                let t = self.fresh();
                let bt = self.vty(KTy::Bool);
                self.line(&format!("{bt} {t} = {a};"));
                self.line(&format!("if ({}{t}) {{", if *op == BinOp::And { "" } else { "!" }));
                self.ind += 1;
                let b = self.expr(b);
                self.line(&format!("{t} = {b};"));
                self.ind -= 1;
                self.line("}");
                t
            }
            KExpr::Bin(op, ty, a, b) => {
                let a = self.expr(a);
                let b = self.expr(b);
                self.bin(*op, *ty, &a, &b)
            }
            KExpr::Neg(ty, a) => {
                let a = self.expr(a);
                let t = self.fresh();
                let vt = self.vty(*ty);
                if ty.is_int() {
                    let min = self.min_i();
                    self.fail(&format!("{a} == {min}"), T_OVERFLOW, "0", "0");
                }
                self.line(&format!("{vt} {t} = -{a};"));
                self.range(*ty, &t);
                t
            }
            KExpr::Not(a) => {
                let a = self.expr(a);
                format!("(!{a})")
            }
            KExpr::Cast(to, from, a) => {
                let a = self.expr(a);
                let t = self.fresh();
                let vt = self.vty(*to);
                match (to, from) {
                    (KTy::F32 | KTy::F64, _) => self.line(&format!("{vt} {t} = ({vt}){a};")),
                    _ => {
                        self.line(&format!("{vt} {t} = {a};"));
                        self.range(*to, &t);
                    }
                }
                t
            }
            KExpr::Call(f, ty, xs) => {
                let a = self.expr(&xs[0]);
                let b = xs.get(1).map(|x| self.expr(x));
                let t = self.fresh();
                let vt = self.vty(*ty);
                let f32 = *ty == KTy::F32 && self.t == Tgt::C;
                let fun = |n: &str| if f32 { format!("{n}f") } else { n.to_string() };
                match f {
                    KFn::Min => self.line(&format!("{vt} {t} = {a} < {} ? {a} : {};", b.clone().unwrap(), b.unwrap())),
                    KFn::Max => self.line(&format!("{vt} {t} = {a} > {} ? {a} : {};", b.clone().unwrap(), b.unwrap())),
                    KFn::Abs if ty.is_int() => {
                        let min = self.min_i();
                        self.fail(&format!("{a} == {min}"), T_OVERFLOW, "0", "0");
                        self.line(&format!("{vt} {t} = {a} < 0 ? -{a} : {a};"));
                        self.range(*ty, &t);
                    }
                    KFn::Abs => self.line(&format!("{vt} {t} = {}({a});", fun("fabs"))),
                    KFn::Sqrt => self.line(&format!("{vt} {t} = {}({a});", fun("sqrt"))),
                    KFn::Floor => self.line(&format!("{vt} {t} = {}({a});", fun("floor"))),
                    KFn::Ceil => self.line(&format!("{vt} {t} = {}({a});", fun("ceil"))),
                    KFn::Round => self.line(&format!("{vt} {t} = {}({a});", fun("round"))),
                }
                t
            }
            KExpr::If(c, a, b) => {
                let c = self.expr(c);
                let t = self.fresh();
                let ty = self.expr_ty(e);
                let vt = self.vty(ty);
                self.line(&format!("{vt} {t};"));
                self.line(&format!("if ({c}) {{"));
                self.ind += 1;
                let a = self.expr(a);
                self.line(&format!("{t} = {a};"));
                self.ind -= 1;
                self.line("} else {");
                self.ind += 1;
                let b = self.expr(b);
                self.line(&format!("{t} = {b};"));
                self.ind -= 1;
                self.line("}");
                t
            }
        }
    }

    fn expr_ty(&self, e: &KExpr) -> KTy {
        match e {
            KExpr::Lit(t, _) => *t,
            KExpr::Local(s) => self.k.locals[*s],
            KExpr::Param(p) | KExpr::Load(p, _) => self.k.params[*p].ty,
            KExpr::Len(_) | KExpr::Intr(_) => KTy::Int,
            KExpr::Bin(op, t, ..) => {
                if matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::And | BinOp::Or) {
                    KTy::Bool
                } else {
                    *t
                }
            }
            KExpr::Neg(t, _) | KExpr::Cast(t, _, _) | KExpr::Call(_, t, _) => *t,
            KExpr::Not(_) => KTy::Bool,
            KExpr::If(_, a, _) => self.expr_ty(a),
        }
    }

    fn bin(&mut self, op: BinOp, ty: KTy, a: &str, b: &str) -> String {
        use BinOp::*;
        let t = self.fresh();
        if matches!(op, Eq | Ne | Lt | Le | Gt | Ge) {
            let bt = self.vty(KTy::Bool);
            self.line(&format!("{bt} {t} = {a} {} {b};", op.symbol()));
            return t;
        }
        let vt = self.vty(ty);
        if ty.is_float() {
            self.line(&format!("{vt} {t} = {a} {} {b};", op.symbol()));
            if ty == KTy::F32 {
                match op {
                    Add => self.tiny(&t, &format!("{a} != -{b}")),
                    Sub => self.tiny(&t, &format!("{a} != {b}")),
                    Mul => self.tiny(&t, &format!("{a} != 0.0f && {b} != 0.0f")),
                    _ => self.tiny(&t, &format!("{a} != 0.0f && !isinf({b})")),
                }
            }
            return t;
        }
        let min = self.min_i();
        let wide = ty == KTy::Int;
        match op {
            Div | Rem => {
                self.fail(&format!("{b} == 0"), T_DIV_ZERO, "0", "0");
                self.fail(&format!("{a} == {min} && {b} == -1"), T_OVERFLOW, "0", "0");
                self.line(&format!("{vt} {t} = {a} {} {b};", op.symbol()));
            }
            _ if !wide => self.line(&format!("{vt} {t} = {a} {} {b};", op.symbol())),
            _ if self.t == Tgt::C => {
                let f = match op {
                    Add => "add",
                    Sub => "sub",
                    _ => "mul",
                };
                self.line(&format!("{vt} {t};"));
                self.fail(&format!("__builtin_{f}_overflow({a}, {b}, &{t})"), T_OVERFLOW, "0", "0");
            }
            Add => {
                self.line(&format!("{vt} {t} = (long)((ulong){a} + (ulong){b});"));
                self.fail(&format!("(({a} ^ {t}) & ({b} ^ {t})) < 0"), T_OVERFLOW, "0", "0");
            }
            Sub => {
                self.line(&format!("{vt} {t} = (long)((ulong){a} - (ulong){b});"));
                self.fail(&format!("(({a} ^ {b}) & ({a} ^ {t})) < 0"), T_OVERFLOW, "0", "0");
            }
            _ => {
                self.line(&format!("{vt} {t} = (long)((ulong){a} * (ulong){b});"));
                self.fail(&format!("SS_MULHI({a}, {b}) != ({t} >> 63)"), T_OVERFLOW, "0", "0");
            }
        }
        self.range(ty, &t);
        t
    }

    pub(super) fn block(&mut self, b: &[KStmt]) {
        for s in b {
            self.stmt(s);
        }
    }

    fn stmt(&mut self, s: &KStmt) {
        match s {
            KStmt::Let(slot, e) => {
                let v = self.expr(e);
                let vt = self.vty(self.k.locals[*slot]);
                self.line(&format!("{vt} v{slot} = {v};"));
            }
            KStmt::Set(slot, e) => {
                let v = self.expr(e);
                self.line(&format!("v{slot} = {v};"));
            }
            KStmt::Store(p, i, v) => {
                let i = self.expr(i);
                let v = self.expr(v);
                let cast = if self.t == Tgt::C { "(uint64_t)" } else { "(ulong)" };
                self.fail(&format!("{cast}{i} >= {cast}l{p}"), T_INDEX, &format!("l{p}"), &i);
                let et = elem_c(self.k.params[*p].ty, self.t);
                self.line(&format!("p{p}[{i}] = ({et}){v};"));
            }
            KStmt::For(slot, a, b, body) => {
                let lo = self.expr(a);
                let hi = self.expr(b);
                let it = if self.t == Tgt::C { "int64_t" } else { "long" };
                let h = self.fresh();
                self.line(&format!("{it} {h} = {hi};"));
                self.line(&format!("for ({it} v{slot} = {lo}; v{slot} < {h}; v{slot}++) {{"));
                self.ind += 1;
                self.block(body);
                self.ind -= 1;
                self.line("}");
            }
            KStmt::If(c, t, f) => {
                let c = self.expr(c);
                self.line(&format!("if ({c}) {{"));
                self.ind += 1;
                self.block(t);
                self.ind -= 1;
                if f.is_empty() {
                    self.line("}");
                } else {
                    self.line("} else {");
                    self.ind += 1;
                    self.block(f);
                    self.ind -= 1;
                    self.line("}");
                }
            }
            KStmt::Eval(e) => {
                let v = self.expr(e);
                self.line(&format!("(void){v};"));
            }
        }
    }
}

pub fn scalar_c(t: KTy) -> &'static str {
    match t {
        KTy::F32 => "float",
        KTy::F64 => "double",
        _ => "int64_t",
    }
}

pub fn cpu_kernel(k: &Kernel, fidx: usize) -> String {
    let mut sig = Vec::new();
    for (i, p) in k.params.iter().enumerate() {
        match p.slice {
            None => sig.push(format!("{} p{i}", scalar_c(p.ty))),
            Some(m) => {
                sig.push(format!("{}{}* p{i}", if m { "" } else { "const " }, elem_c(p.ty, Tgt::C)));
                sig.push(format!("int64_t l{i}"));
            }
        }
    }
    let mut em = Em::new(k, Tgt::C, 2);
    em.block(&k.body);
    let body = em.take();
    format!(
        "#define FIDX {fidx}\nstatic void kc_{}({}, int64_t ss_n, Status* st) {{\n#pragma clang fp contract(off)\n  for (int64_t gid = 0; gid < ss_n; gid++) {{\n{body}  }}\n}}\n#undef FIDX\n",
        k.name,
        sig.join(", ")
    )
}

pub const MSL_PRELUDE: &str = "#include <metal_stdlib>\nusing namespace metal;\n#pragma METAL fp math_mode(safe)\n#pragma METAL fp contract(off)\n#define SS_MULHI(a, b) mulhi((long)(a), (long)(b))\nstatic inline bool ss_sub(uint b) { return (b & 0x7f800000u) == 0u && (b & 0x007fffffu) != 0u; }\n";

pub const OCL_PRELUDE: &str = "#ifdef __NVPTX__\n#define get_global_id(d) ((size_t)__nvvm_read_ptx_sreg_ctaid_x() * __nvvm_read_ptx_sreg_ntid_x() + __nvvm_read_ptx_sreg_tid_x())\n#define SS_MULHI(a, b) __nvvm_mulhi_ll((a), (b))\n#define fabs(x) __builtin_fabs(x)\n#define sqrt(x) __builtin_sqrt(x)\n#define floor(x) __builtin_floor(x)\n#define ceil(x) __builtin_ceil(x)\n#define round(x) __builtin_round(x)\n#define isinf(x) __builtin_isinf(x)\n#else\n#define SS_MULHI(a, b) mul_hi((long)(a), (long)(b))\n#endif\n#pragma OPENCL EXTENSION cl_khr_fp64 : enable\n#pragma OPENCL FP_CONTRACT OFF\nstatic inline bool ss_sub(uint b) { return (b & 0x7f800000u) == 0u && (b & 0x007fffffu) != 0u; }\n";

pub fn device_kernel(k: &Kernel, tgt: Tgt) -> String {
    let mut sig = Vec::new();
    let mut slot = 0;
    let mut buf = |s: String, sig: &mut Vec<String>| {
        if tgt == Tgt::Msl {
            sig.push(format!("{s} [[buffer({slot})]]"));
        } else {
            sig.push(s);
        }
        slot += 1;
    };
    let (dev, con) = if tgt == Tgt::Msl { ("device ", "constant ") } else { ("__global ", "") };
    for (i, p) in k.params.iter().enumerate() {
        let r = if tgt == Tgt::Msl { "&" } else { "" };
        match p.slice {
            None => {
                let t = match p.ty {
                    KTy::F32 => "float",
                    KTy::F64 => "double",
                    _ => "long",
                };
                buf(format!("{con}{t}{r} p{i}"), &mut sig);
            }
            Some(m) => {
                buf(format!("{dev}{}{}* p{i}", if m { "" } else { "const " }, elem_c(p.ty, tgt)), &mut sig);
                buf(format!("{con}long{r} l{i}"), &mut sig);
            }
        }
    }
    let r = if tgt == Tgt::Msl { "&" } else { "" };
    buf(format!("{con}long{r} ss_n"), &mut sig);
    let flag = if tgt == Tgt::Msl { "device atomic_uint* ss_flag" } else { "__global uint* ss_flag" };
    buf(flag.to_string(), &mut sig);
    let mut em = Em::new(k, tgt, 1);
    em.block(&k.body);
    let body = em.take();
    match tgt {
        Tgt::Msl => format!("kernel void k_{}({}, uint ss_t [[thread_position_in_grid]]) {{\n  long gid = (long)ss_t;\n  if (gid >= ss_n) return;\n{body}}}\n", k.name, sig.join(", ")),
        _ => format!("__kernel void k_{}({}) {{\n  long gid = (long)get_global_id(0);\n  if (gid >= ss_n) return;\n{body}}}\n", k.name, sig.join(", ")),
    }
}

pub fn msl_source(ks: &[&Kernel]) -> String {
    let mut s = String::from(MSL_PRELUDE);
    for k in ks {
        s.push_str(&device_kernel(k, Tgt::Msl));
    }
    s
}

pub fn opencl_source(ks: &[&Kernel]) -> String {
    let mut s = String::from(OCL_PRELUDE);
    for k in ks {
        s.push_str(&device_kernel(k, Tgt::Ocl));
    }
    s
}

pub(super) const SHIM_LINK: &str = "@sspur-gpu";
const SHIM_SRC: &str = include_str!("gpu_rt.m");

pub fn shim() -> Result<PathBuf, String> {
    let key = blake3::hash(SHIM_SRC.as_bytes()).to_hex().to_string();
    let dir = cache_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lib = dir.join(format!("libsspur_gpu_{}.dylib", &key[..16]));
    if lib.exists() {
        return Ok(lib);
    }
    let src = dir.join(format!("sspur_gpu_{}.m", &key[..16]));
    std::fs::write(&src, SHIM_SRC).map_err(|e| e.to_string())?;
    let tmp = lib.with_extension("tmp");
    let cc = std::env::var("CC").unwrap_or_else(|_| "clang".into());
    let install = format!("-Wl,-install_name,{}", lib.display());
    let metal = Command::new(&cc).args(["-O2", "-fobjc-arc", "-shared", "-fPIC", "-w", &install, "-framework", "Metal", "-framework", "Foundation", "-o"]).arg(&tmp).arg(&src).output();
    if !metal.as_ref().is_ok_and(|o| o.status.success()) {
        let stub = Command::new(&cc).args(["-O2", "-x", "c", "-shared", "-fPIC", "-w", &install, "-o"]).arg(&tmp).arg(&src).output().map_err(|e| format!("cannot run {cc}: {e}"))?;
        if !stub.status.success() {
            return Err(format!("{cc} failed to build the GPU runtime: {}", String::from_utf8_lossy(&stub.stderr).lines().take(4).collect::<Vec<_>>().join(" | ")));
        }
    }
    std::fs::rename(&tmp, &lib).map_err(|e| e.to_string())?;
    Ok(lib)
}

pub(super) const HOST_PRELUDE: &str = r#"typedef struct { int32_t kind, pad; const void* ptr; int64_t bytes; void* dev; } SsGpuArg;
#ifdef __APPLE__
int ss_gpu_ready(void);
int ss_gpu_run(const char* src, const char* entry, int32_t nargs, const SsGpuArg* args, int64_t n, int64_t group);
void* ss_gpu_alloc(int64_t bytes, void** host);
void ss_gpu_free(void* dev);
#else
static int ss_gpu_ready(void) { return 0; }
static int ss_gpu_run(const char* src, const char* entry, int32_t nargs, const SsGpuArg* args, int64_t n, int64_t group) { (void)src; (void)entry; (void)nargs; (void)args; (void)n; (void)group; return 2; }
static void* ss_gpu_alloc(int64_t bytes, void** host) { (void)bytes; (void)host; return 0; }
static void ss_gpu_free(void* dev) { (void)dev; }
#endif
static int ss_trace_on = -1;
static void ss_trace(const char* k, const char* how, int64_t n) {
    if (ss_trace_on < 0) { const char* t = getenv("SSPUR_GPU_TRACE"); ss_trace_on = t && *t && *t != '0'; }
    if (ss_trace_on) fprintf(stderr, "gpu %s: %s, %lld threads\n", k, how, (long long)n);
}
static void __attribute__((noinline, cold, noreturn)) dev_fail(Status* st, int64_t f, const char* a, int64_t j, const char* b, int64_t v) {
    char* p = (char*)malloc(strlen(a) + (b ? strlen(b) : 0) + 64);
    if (j >= 0) sprintf(p, "%s%lld%s%lld)", a, (long long)j, b, (long long)v); else if (b) sprintf(p, "%s%s%lld)", a, b, (long long)v); else strcpy(p, a);
    st->rbuf = (int64_t*)p; st->rlen = (int64_t)strlen(p);
    sspur_trap(st, 12, f, 0, 0);
}
static inline int ss_sub32(float x) { return x != 0.0f && fabsf(x) < 1.17549435e-38f; }
static inline double ss_canon(double x) { return x != x ? NAN : x; }
struct SsDev { int64_t len, es; void* host; void* dev; struct SsDev* next; };
static struct SsDev* ss_devs;
static void ss_dev_reset(void) {
    while (ss_devs) { struct SsDev* d = ss_devs; ss_devs = d->next; if (d->dev) ss_gpu_free(d->dev); else free(d->host); free(d); }
}
static struct SsDev* ss_dev_alloc(int64_t n, int64_t es) {
    struct SsDev* d = (struct SsDev*)calloc(1, sizeof(struct SsDev));
    d->len = n; d->es = es;
    if (ss_gpu_ready()) d->dev = ss_gpu_alloc(n * es, &d->host);
    if (!d->dev) d->host = calloc((size_t)(n ? n : 1), (size_t)es);
    d->next = ss_devs; ss_devs = d;
    ss_reset_hook = ss_dev_reset;
    return d;
}
"#;

fn es(t: KTy) -> usize {
    if matches!(t, KTy::F32 | KTy::I32 | KTy::U32) { 4 } else { 8 }
}

fn dev_elem(t: &Type) -> KTy {
    match t {
        Type::Con(_, a) => match a.first() {
            Some(Type::Con(e, _)) => KTy::from_name(e).unwrap_or(KTy::Int),
            _ => KTy::Int,
        },
        _ => KTy::Int,
    }
}

impl Cx<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn kernel_fn(&mut self, f: &FnDef, params: &[Type], ret: &Type, fidx: usize, k: &Kernel, devs: &[bool], cname: &str) -> G {
        let rr = self.rr(ret)?;
        let dev = |i: usize| devs.get(i).copied().unwrap_or(false);
        let mut sig = Vec::new();
        for (i, (t, p)) in params.iter().zip(&f.params).enumerate() {
            if dev(i) {
                sig.push(format!("SsDev* a{i}"));
                continue;
            }
            let star = if is_mut_borrow(&p.ty) { "*" } else { "" };
            sig.push(format!("{}{star} a{i}", self.cty(t)?));
        }
        sig.push("Status* st".into());
        sig.push("int64_t depth".into());
        let name = &f.name;
        let mut s = format!("#define FIDX {fidx}\nstatic {rr} f_{cname}({}) {{\n  (void)depth;\n", sig.join(", "));
        for (j, q) in k.params.iter().enumerate() {
            for (i, p) in k.params.iter().enumerate().take(j) {
                if dev(i) && dev(j) && (p.slice == Some(true) || q.slice == Some(true)) {
                    let m = c_lit(&format!("dev: arguments {} and {} of {name} are the same buffer", p.name, q.name));
                    writeln!(s, "  if (UNLIKELY(a{i} == a{j})) dev_fail(st, FIDX, {m}, -1, 0, 0);").unwrap();
                }
            }
        }
        let mut gargs = Vec::new();
        let mut cargs = Vec::new();
        let mut back = String::new();
        let mut save = String::new();
        let mut restore = String::new();
        for (i, (p, t)) in k.params.iter().zip(params).enumerate() {
            let src = if is_mut_borrow(&f.params[i].ty) { format!("(*a{i})") } else { format!("a{i}") };
            let Some(m) = p.slice else {
                let c = scalar_c(p.ty);
                if let Some((lo, hi)) = matches!(p.ty, KTy::I32 | KTy::U32).then(|| p.ty.range()) {
                    writeln!(s, "  if (UNLIKELY({src} < {lo}LL || {src} > {hi}LL)) dev_fail(st, FIDX, \"dev: argument {} of {name} is out of range for {} (value = \", -1, \"\", {src});", p.name, p.ty.name()).unwrap();
                }
                writeln!(s, "  {c} p{i} = ({c}){src};").unwrap();
                let bytes = if p.ty == KTy::F32 { 4 } else { 8 };
                gargs.push(format!("{{0, 0, &p{i}, {bytes}, 0}}"));
                cargs.push(format!("p{i}"));
                continue;
            };
            let e = elem_c(p.ty, Tgt::C);
            let es = es(p.ty);
            if dev(i) {
                writeln!(s, "  int64_t l{i} = a{i}->len; {e}* p{i} = ({e}*)a{i}->host;").unwrap();
                gargs.push(format!("a{i}->dev ? (SsGpuArg){{3, 0, p{i}, l{i} * {es}, a{i}->dev}} : (SsGpuArg){{{}, 0, p{i}, l{i} * {es}, 0}}", if m { 2 } else { 1 }));
                if m {
                    writeln!(save, "      void* bk{i} = 0; if (a{i}->dev) {{ bk{i} = sspur_alloc_atomic((size_t)(l{i} ? l{i} : 1) * {es}); memcpy(bk{i}, p{i}, (size_t)l{i} * {es}); }}").unwrap();
                    writeln!(restore, "      if (r_ && bk{i}) memcpy(p{i}, bk{i}, (size_t)l{i} * {es});").unwrap();
                }
            } else {
                writeln!(s, "  int64_t l{i} = {src}.len;").unwrap();
                match p.ty {
                    KTy::F64 | KTy::Int if !m => writeln!(s, "  const {e}* p{i} = (const {e}*){src}.data;").unwrap(),
                    KTy::F64 | KTy::Int => writeln!(s, "  {e}* p{i} = ({e}*)sspur_alloc_atomic((size_t)(l{i} ? l{i} : 1) * 8); if (l{i}) memcpy(p{i}, {src}.data, (size_t)l{i} * 8);").unwrap(),
                    KTy::F32 => writeln!(s, "  float* p{i} = (float*)sspur_alloc_atomic((size_t)(l{i} ? l{i} : 1) * 4); for (int64_t j_ = 0; j_ < l{i}; j_++) p{i}[j_] = (float){src}.data[j_];").unwrap(),
                    _ => {
                        let (lo, hi) = p.ty.range();
                        writeln!(s, "  {e}* p{i} = ({e}*)sspur_alloc_atomic((size_t)(l{i} ? l{i} : 1) * 4); for (int64_t j_ = 0; j_ < l{i}; j_++) {{ int64_t v_ = {src}.data[j_]; if (UNLIKELY(v_ < {lo}LL || v_ > {hi}LL)) dev_fail(st, FIDX, \"dev: element \", j_, \" of {} is out of range for {} (value = \", v_); p{i}[j_] = ({e})v_; }}", p.name, p.ty.name()).unwrap();
                    }
                }
                gargs.push(format!("{{{}, 0, p{i}, l{i} * {es}, 0}}", if m { 2 } else { 1 }));
                if m {
                    let lc = self.cty(t)?;
                    let ec = if p.ty.is_float() { "double" } else { "int64_t" };
                    let conv = if p.ty.is_float() { format!("ss_canon((double)p{i}[j_])") } else { format!("(int64_t)p{i}[j_]") };
                    writeln!(back, "  {{ RawL r_ = raw_alloc_a(l{i}, 8, 1); {ec}* d_ = ({ec}*)r_.data; for (int64_t j_ = 0; j_ < l{i}; j_++) d_[j_] = {conv}; r_.hdr[1] = l{i}; *a{i} = ({lc}){{l{i}, d_, r_.hdr}}; }}").unwrap();
                }
            }
            gargs.push(format!("{{0, 0, &l{i}, 8, 0}}"));
            cargs.push(format!("p{i}"));
            cargs.push(format!("l{i}"));
        }
        let mut em = Em::new(k, Tgt::C, 1);
        for (i, pre) in k.pres.iter().enumerate() {
            let c = em.expr(pre);
            em.line(&format!("if (UNLIKELY(!({c}))) TRAPV({T_PRE}, {i}, 0);"));
        }
        let n = em.expr(&k.grid);
        s.push_str(&em.take());
        writeln!(s, "  int64_t ss_n = {n};").unwrap();
        let sub: Vec<String> = k.params.iter().enumerate().filter(|(_, p)| p.slice.is_none() && p.ty == KTy::F32).map(|(i, _)| format!(" && !ss_sub32(p{i})")).collect();
        gargs.push("{0, 0, &ss_n, 8, 0}".into());
        writeln!(s, "  if (ss_n > 0) {{").unwrap();
        if k.uses_f64() {
            writeln!(s, "    ss_trace(\"{name}\", \"cpu (F64 is not available on the GPU)\", ss_n);").unwrap();
            writeln!(s, "    kc_{name}({}, ss_n, st);", cargs.join(", ")).unwrap();
        } else {
            writeln!(s, "    int r_ = 2;").unwrap();
            writeln!(s, "    if (ss_gpu_ready(){}) {{", sub.concat()).unwrap();
            s.push_str(&save);
            writeln!(s, "      SsGpuArg g_[] = {{{}}};", gargs.join(", ")).unwrap();
            writeln!(s, "      r_ = ss_gpu_run(ss_msl, \"k_{name}\", {}, g_, ss_n, {});", gargs.len(), k.group).unwrap();
            s.push_str(&restore);
            writeln!(s, "    }}").unwrap();
            writeln!(s, "    ss_trace(\"{name}\", r_ == 0 ? \"metal\" : r_ == 1 ? \"cpu rerun (trap or subnormal)\" : \"cpu\", ss_n);").unwrap();
            writeln!(s, "    if (r_) kc_{name}({}, ss_n, st);", cargs.join(", ")).unwrap();
        }
        writeln!(s, "  }}").unwrap();
        s.push_str(&back);
        writeln!(s, "  return ({rr}){{0, 0}};\n}}\n#undef FIDX").unwrap();
        Ok(s)
    }

    pub(super) fn kernel_call(&mut self, name: &str, args: &[Expr]) -> G {
        let k = self.check.kernels.get(name).cloned().ok_or("unknown kernel")?;
        let f = self.fn_def(name).cloned().ok_or("unknown kernel")?;
        let mut vals = Vec::new();
        let mut devs = Vec::new();
        for (a, p) in args.iter().zip(&k.params) {
            let place = match &a.kind {
                ExprKind::Unary(UnOp::Ref | UnOp::RefMut, x) if p.slice.is_some() => &**x,
                _ => a,
            };
            let dev = p.slice.is_some() && matches!(self.ty(place)?, Type::Con(n, _) if n == "#DevBuf");
            devs.push(dev);
            let v = self.expr(place)?;
            vals.push(if p.slice == Some(true) && !dev { format!("&({v})") } else { v });
        }
        let cname = if devs.contains(&true) { format!("{name}__{}", devs.iter().map(|d| if *d { 'D' } else { 'L' }).collect::<String>()) } else { name.to_string() };
        if cname != name && self.helpers_done.insert(format!("kernel {cname}")) {
            let (params, ret) = self.check.fn_types.get(name).cloned().ok_or("kernel has no type")?;
            let fidx = *self.fn_index.get(name).ok_or("kernel has no index")?;
            let code = self.kernel_fn(&f, &params, &ret, fidx, &k, &devs, &cname)?;
            let proto = code.lines().nth(1).ok_or("empty kernel wrapper")?.trim_end_matches(" {").to_string();
            writeln!(self.protos, "{proto};").unwrap();
            self.helpers_late.push_str(&code);
        }
        let mut s = String::from("({ ");
        let mut names = Vec::new();
        for v in vals {
            let t = self.fresh("ka");
            write!(s, "__auto_type {t} = {v}; ").unwrap();
            names.push(t);
        }
        let call: String = names.iter().map(|n| format!("{n}, ")).collect();
        write!(s, "f_{cname}({call}st, depth).v; }})").unwrap();
        Ok(s)
    }

    pub(super) fn dev_new(&mut self, n: &str, list: &str, t: &Type) -> G {
        let kt = dev_elem(t);
        let e = elem_c(kt, Tgt::C);
        let es = es(kt);
        let fill = match kt {
            KTy::F32 => "h_[j_] = (float)l_.data[j_];".to_string(),
            KTy::F64 | KTy::Int => format!("h_[j_] = ({e})l_.data[j_];"),
            _ => {
                let (lo, hi) = kt.range();
                format!("int64_t v_ = l_.data[j_]; if (UNLIKELY(v_ < {lo}LL || v_ > {hi}LL)) dev_fail(st, FIDX, \"dev: element \", j_, \" of {n} is out of range for {} (value = \", v_); h_[j_] = ({e})v_;", kt.name())
            }
        };
        Ok(format!("({{ __auto_type l_ = {list}; SsDev* d_ = ss_dev_alloc(l_.len, {es}); {e}* h_ = ({e}*)d_->host; for (int64_t j_ = 0; j_ < l_.len; j_++) {{ {fill} }} d_; }})"))
    }

    pub(super) fn dev_method(&mut self, r: &str, rt: &Type, name: &str, t: &Type) -> G {
        match name {
            "len" => Ok(format!("(({r})->len)")),
            "to_list" => {
                let kt = dev_elem(rt);
                let lc = self.cty(t)?;
                let e = elem_c(kt, Tgt::C);
                let (oc, conv) = if kt.is_float() { ("double", "ss_canon((double)h_[j_])") } else { ("int64_t", "(int64_t)h_[j_]") };
                Ok(format!("({{ SsDev* d_ = {r}; {e}* h_ = ({e}*)d_->host; RawL r_ = raw_alloc_a(d_->len, 8, 1); {oc}* o_ = ({oc}*)r_.data; for (int64_t j_ = 0; j_ < d_->len; j_++) o_[j_] = {conv}; r_.hdr[1] = d_->len; ({lc}){{d_->len, o_, r_.hdr}}; }})"))
            }
            _ => Err(format!("uses DevBuf.{name}")),
        }
    }
}
