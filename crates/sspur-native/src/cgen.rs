use crate::nval::Layouts;
use crate::{assemble_rich, Compiled, RichFn, T_DEPTH, T_DIV_ZERO, T_INDEX, T_NOMATCH, T_OVERFLOW, T_POST, T_PRE, T_REFINE, T_UNWRAP};
use sspur_check::{expr_key, CheckOutput, Type};
use sspur_syntax::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;

type G<T = String> = Result<T, String>;

const PRELUDE: &str = r#"#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
typedef struct { int64_t code, func, clause, value, limit; int64_t* rbuf; int64_t rlen; } Status;
#define UNLIKELY(x) __builtin_expect(!!(x), 0)
#define TRAPV(c, cl, val) do { st->code = (c); st->func = FIDX; st->clause = (cl); st->value = (int64_t)(val); return (RRT){.code = (c)}; } while (0)
typedef struct Blk { struct Blk* next; size_t used, cap; } Blk;
static Blk* arena_;
typedef struct { Blk* b; size_t used; } Mark;
static Mark arena_mark(void) { return (Mark){arena_, arena_ ? arena_->used : 0}; }
static void arena_release(Mark m) {
    while (arena_ && arena_ != m.b) { Blk* n = arena_->next; free(arena_); arena_ = n; }
    if (arena_) arena_->used = m.used;
}
static void* sspur_alloc(size_t n) {
    n = (n + 7) & ~(size_t)7;
    if (!arena_ || arena_->used + n > arena_->cap) {
        size_t cap = n > ((size_t)1 << 20) ? n : ((size_t)1 << 20);
        Blk* b = (Blk*)malloc(sizeof(Blk) + 16 + cap);
        b->next = arena_; b->used = 0; b->cap = cap; arena_ = b;
    }
    char* base = (char*)(((uintptr_t)(arena_ + 1) + 15) & ~(uintptr_t)15);
    void* p = base + arena_->used;
    arena_->used += n;
    return p;
}
typedef struct { int64_t len; void* data; int64_t* hdr; } RawL;
static RawL raw_alloc(int64_t cap, size_t es) {
    if (cap < 4) cap = 4;
    int64_t* h = (int64_t*)sspur_alloc(16 + (size_t)cap * es);
    h[0] = cap; h[1] = 0;
    return (RawL){0, (void*)(h + 2), h};
}
static int64_t raw_offset(RawL l, size_t es) { return ((char*)l.data - (char*)(l.hdr + 2)) / (int64_t)es; }
static RawL raw_reserve(RawL l, int64_t extra, size_t es) {
    if (l.hdr && raw_offset(l, es) + l.len == l.hdr[1] && l.hdr[1] + extra <= l.hdr[0]) return l;
    RawL n = raw_alloc((l.len + extra) * 2, es);
    if (l.len) memcpy(n.data, l.data, (size_t)l.len * es);
    n.len = l.len; n.hdr[1] = l.len;
    return n;
}
static RawL raw_push(RawL l, const void* v, size_t es) {
    RawL r = raw_reserve(l, 1, es);
    memcpy((char*)r.data + (size_t)r.len * es, v, es);
    r.len += 1; r.hdr[1] = raw_offset(r, es) + r.len;
    return r;
}
static RawL raw_concat(RawL a, RawL b, size_t es) {
    if (b.len == 0) return a;
    RawL r = raw_reserve(a, b.len, es);
    memcpy((char*)r.data + (size_t)r.len * es, b.data, (size_t)b.len * es);
    r.len += b.len; r.hdr[1] = raw_offset(r, es) + r.len;
    return r;
}
static RawL raw_copy(RawL l, size_t es) {
    RawL n = raw_alloc(l.len, es);
    if (l.len) memcpy(n.data, l.data, (size_t)l.len * es);
    n.len = l.len; n.hdr[1] = l.len;
    return n;
}
static void raw_msort(char* a, int64_t n, size_t es, int (*cmp)(const void*, const void*), char* tmp) {
    if (n < 2) return;
    int64_t h = n / 2;
    raw_msort(a, h, es, cmp, tmp);
    raw_msort(a + h * es, n - h, es, cmp, tmp);
    int64_t i = 0, j = h, k = 0;
    while (i < h && j < n) {
        if (cmp(a + j * es, a + i * es) < 0) { memcpy(tmp + k * es, a + j * es, es); j++; }
        else { memcpy(tmp + k * es, a + i * es, es); i++; }
        k++;
    }
    while (i < h) { memcpy(tmp + k * es, a + i * es, es); i++; k++; }
    while (j < n) { memcpy(tmp + k * es, a + j * es, es); j++; k++; }
    memcpy(a, tmp, (size_t)n * es);
}
static RawL raw_sorted(RawL l, size_t es, int (*cmp)(const void*, const void*)) {
    RawL c = raw_copy(l, es);
    if (c.len > 1) raw_msort((char*)c.data, c.len, es, cmp, (char*)sspur_alloc((size_t)c.len * es));
    return c;
}
#define TO_RAW(l) ((RawL){(l).len, (void*)(l).data, (l).hdr})
static inline int64_t dbits(double x) { int64_t b; memcpy(&b, &x, 8); return b; }
static inline double bitsd(int64_t b) { double x; memcpy(&x, &b, 8); return x; }
static inline int64_t dkey(double x) { int64_t b = dbits(x); b ^= (int64_t)(((uint64_t)(b >> 63)) >> 1); return b; }
static inline int cmp_I(int64_t a, int64_t b) { return (a > b) - (a < b); }
static inline int cmp_D(double a, double b) { int64_t x = dkey(a), y = dkey(b); return (x > y) - (x < y); }
static inline int64_t f2i(double x) {
    if (x != x) return 0;
    if (x >= 9223372036854775807.0) return INT64_MAX;
    if (x <= -9223372036854775808.0) return INT64_MIN;
    return (int64_t)x;
}
typedef struct { int64_t v; int64_t code; } PowR;
static inline PowR sspur_pow(int64_t b, int64_t e) {
    if (e < 0) return (PowR){0, 6};
    int64_t r = 1;
    while (e > 0) {
        if (e & 1) { if (__builtin_mul_overflow(r, b, &r)) return (PowR){0, 1}; }
        e >>= 1;
        if (e > 0 && __builtin_mul_overflow(b, b, &b)) return (PowR){0, 1};
    }
    return (PowR){r, 0};
}
typedef struct { int64_t* data; int64_t len, cap; } Buf;
static void buf_push(Buf* b, int64_t w) {
    if (b->len == b->cap) { b->cap = b->cap ? b->cap * 2 : 64; b->data = (int64_t*)realloc(b->data, (size_t)b->cap * 8); }
    b->data[b->len++] = w;
}
void sspur_buf_free(int64_t* p) { free(p); }
"#;

pub fn compile_release(m: &Module, check: &CheckOutput, opt: &str) -> Result<Compiled, String> {
    let (src, plan) = generate(m, check)?;
    let lib = build(&src, opt)?;
    let library = unsafe { libloading::Library::new(&lib) }.map_err(|e| format!("cannot load {}: {e}", lib.display()))?;
    let mut scalar = HashMap::new();
    let mut rich = HashMap::new();
    for (name, (params, ret, is_scalar)) in &plan.fns {
        if *is_scalar {
            let sym = format!("sspur_entry_{name}");
            let p: libloading::Symbol<unsafe extern "C" fn()> = unsafe { library.get(sym.as_bytes()) }.map_err(|e| format!("missing {sym}: {e}"))?;
            scalar.insert(name.clone(), (*p as *const u8, params.len()));
        }
        let sym = format!("sspur_wentry_{name}");
        let p: libloading::Symbol<unsafe extern "C" fn()> = unsafe { library.get(sym.as_bytes()) }.map_err(|e| format!("missing {sym}: {e}"))?;
        rich.insert(name.clone(), RichFn { ptr: *p as *const u8, params: params.clone(), ret: ret.clone() });
    }
    let free: libloading::Symbol<unsafe extern "C" fn(*mut i64)> = unsafe { library.get(b"sspur_buf_free") }.map_err(|e| e.to_string())?;
    let free = *free;
    let defs: Vec<FnDef> = m.defs.iter().filter_map(|d| if let Def::Fn(f) = d { Some(f.clone()) } else { None }).collect();
    Ok(assemble_rich(defs, scalar, rich, plan.skipped, Box::new(library), Layouts::from_check(check), plan.refines, free))
}

pub fn c_source(m: &Module, check: &CheckOutput) -> String {
    match generate(m, check) {
        Ok((src, _)) => src,
        Err(e) => format!("/* {e} */\n"),
    }
}

struct Plan {
    fns: BTreeMap<String, (Vec<Type>, Type, bool)>,
    skipped: BTreeMap<String, String>,
    refines: Vec<(String, String, Type)>,
}

fn generate(m: &Module, check: &CheckOutput) -> Result<(String, Plan), String> {
    let defs: Vec<&FnDef> = m.defs.iter().filter_map(|d| if let Def::Fn(f) = d { Some(f) } else { None }).collect();
    let index: HashMap<String, usize> = defs.iter().enumerate().map(|(i, f)| (f.name.clone(), i)).collect();
    let mut skipped = BTreeMap::new();
    let mut ok: HashSet<String> = HashSet::new();
    for f in &defs {
        match precheck(f) {
            Ok(()) => {
                ok.insert(f.name.clone());
            }
            Err(e) => {
                skipped.insert(f.name.clone(), e);
            }
        }
    }
    let alias_refines: HashMap<String, Expr> = m
        .defs
        .iter()
        .filter_map(|d| match d {
            Def::Type(TypeDef { name, body: TypeBody::Alias(_, Some(r)), .. }) => Some((name.clone(), r.clone())),
            _ => None,
        })
        .collect();
    let field_refines: HashMap<String, Vec<(String, Option<Expr>, Option<String>)>> = m
        .defs
        .iter()
        .flat_map(|d| match d {
            Def::Type(TypeDef { name, body: TypeBody::Record(fs), .. }) => vec![(name.clone(), fs.clone())],
            Def::Type(TypeDef { body: TypeBody::Sum(vs), .. }) => vs.iter().filter_map(|v| v.fields.clone().map(|fs| (v.name.clone(), fs))).collect(),
            _ => vec![],
        })
        .map(|(owner, fs)| (owner, fs.into_iter().map(|f| (f.name.clone(), f.refine.clone(), match &f.ty { Ty::Named { name, .. } => Some(name.clone()), _ => None })).collect()))
        .collect();
    loop {
        let mut cx = Cx::new(check, &ok, &alias_refines, &field_refines);
        let mut failed = Vec::new();
        let mut bodies = String::new();
        let mut plan_fns = BTreeMap::new();
        for f in defs.iter().filter(|f| ok.contains(&f.name)) {
            let Some((params, ret)) = check.fn_types.get(&f.name).cloned() else {
                failed.push((f.name.clone(), "has no type information".to_string()));
                continue;
            };
            let snapshot = cx.snapshot();
            match cx.function(f, &params, &ret, index[&f.name]) {
                Ok(code) => {
                    bodies.push_str(&code);
                    let scalar = params.iter().chain([&ret]).all(|t| matches!(t, Type::Con(n, a) if a.is_empty() && matches!(n.as_str(), "Int" | "Bool" | "Unit")));
                    plan_fns.insert(f.name.clone(), (params, ret, scalar));
                }
                Err(e) => {
                    cx.restore(snapshot);
                    failed.push((f.name.clone(), e));
                }
            }
        }
        if failed.is_empty() {
            let mut entries = String::new();
            for f in defs.iter().filter(|f| ok.contains(&f.name)) {
                let (params, ret, scalar) = plan_fns[&f.name].clone();
                entries.push_str(&cx.entries(&f.name, &params, &ret, scalar)?);
            }
            let mut src = String::from(PRELUDE);
            src.push_str(&cx.fwd);
            src.push_str(&cx.defs);
            src.push_str(&cx.protos);
            for f in defs.iter().filter(|f| ok.contains(&f.name)) {
                let (params, ret, _) = &plan_fns[&f.name];
                let ps: Vec<String> = params.iter().map(|t| cx.cty(t)).collect::<G<_>>()?;
                let rr = cx.rr(ret)?;
                let mut sig: Vec<String> = ps.iter().enumerate().map(|(i, t)| format!("{t} a{i}")).collect();
                sig.push("Status* st".into());
                sig.push("int64_t depth".into());
                writeln!(src, "static {rr} f_{}({});", f.name, sig.join(", ")).unwrap();
            }
            src.push_str(&cx.helpers);
            src.push_str(&bodies);
            src.push_str(&entries);
            src.push_str(&cx.helpers_late);
            let plan = Plan { fns: plan_fns, skipped, refines: cx.refines.clone() };
            return Ok((src, plan));
        }
        for (n, e) in failed {
            ok.remove(&n);
            skipped.insert(n, e);
        }
    }
}

fn precheck(f: &FnDef) -> G<()> {
    if !f.tparams.is_empty() {
        return Err("is generic".into());
    }
    if let Some(e) = f.effects.iter().find(|e| e.name != "div") {
        return Err(format!("performs '{}'", printer::effect(e)));
    }
    if matches!(f.body.kind, ExprKind::Table(_)) {
        return Err("is a rule table".into());
    }
    Ok(())
}

fn cache_dir() -> PathBuf {
    let base = std::env::var_os("SSPUR_CACHE").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/sspur"))).unwrap_or_else(std::env::temp_dir);
    base.join("native")
}

fn build(src: &str, opt: &str) -> Result<PathBuf, String> {
    let key = blake3::hash(format!("{opt}\n{src}").as_bytes()).to_hex().to_string();
    let dir = cache_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let lib = dir.join(format!("{}.{}", &key[..32], std::env::consts::DLL_EXTENSION));
    if lib.exists() {
        return Ok(lib);
    }
    let c = dir.join(format!("{}.c", &key[..32]));
    std::fs::write(&c, src).map_err(|e| e.to_string())?;
    let tmp = lib.with_extension("tmp");
    let cc = std::env::var("CC").unwrap_or_else(|_| "clang".into());
    let out = Command::new(&cc).args([opt, "-shared", "-fPIC", "-w", "-o"]).arg(&tmp).arg(&c).output().map_err(|e| format!("cannot run {cc}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cc} failed: {}", String::from_utf8_lossy(&out.stderr).lines().take(6).collect::<Vec<_>>().join(" | ")));
    }
    std::fs::rename(&tmp, &lib).map_err(|e| e.to_string())?;
    Ok(lib)
}

fn lit(n: i64) -> String {
    if n == i64::MIN { "INT64_MIN".into() } else { format!("{n}LL") }
}

fn is(t: &Type, name: &str) -> bool {
    matches!(t, Type::Con(n, a) if n == name && a.is_empty())
}

fn elem(t: &Type, con: &str) -> Option<Type> {
    match t {
        Type::Con(n, a) if n == con && a.len() == 1 => Some(a[0].clone()),
        _ => None,
    }
}

fn scalar(t: &Type) -> bool {
    is(t, "Int") || is(t, "Bool") || is(t, "Unit") || is(t, "F64")
}

#[derive(Clone)]
struct Snapshot {
    fwd: String,
    defs: String,
    protos: String,
    helpers: String,
    helpers_late: String,
    declared: HashSet<String>,
    complete: HashSet<String>,
    helpers_done: HashSet<String>,
    refines: Vec<(String, String, Type)>,
}

struct Cx<'a> {
    check: &'a CheckOutput,
    eligible: &'a HashSet<String>,
    alias_refines: &'a HashMap<String, Expr>,
    field_refines: &'a HashMap<String, Vec<(String, Option<Expr>, Option<String>)>>,
    layouts: Layouts,
    fwd: String,
    defs: String,
    protos: String,
    helpers: String,
    helpers_late: String,
    declared: HashSet<String>,
    complete: HashSet<String>,
    in_progress: HashSet<String>,
    helpers_done: HashSet<String>,
    refines: Vec<(String, String, Type)>,
    scopes: Vec<HashMap<String, (String, Type)>>,
    counter: usize,
    ret: Type,
}

impl<'a> Cx<'a> {
    fn new(check: &'a CheckOutput, eligible: &'a HashSet<String>, alias_refines: &'a HashMap<String, Expr>, field_refines: &'a HashMap<String, Vec<(String, Option<Expr>, Option<String>)>>) -> Self {
        Cx {
            check,
            eligible,
            alias_refines,
            field_refines,
            layouts: Layouts::from_check(check),
            fwd: String::new(),
            defs: String::new(),
            protos: String::new(),
            helpers: String::new(),
            helpers_late: String::new(),
            declared: HashSet::new(),
            complete: HashSet::new(),
            in_progress: HashSet::new(),
            helpers_done: HashSet::new(),
            refines: Vec::new(),
            scopes: Vec::new(),
            counter: 0,
            ret: Type::unit(),
        }
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            fwd: self.fwd.clone(),
            defs: self.defs.clone(),
            protos: self.protos.clone(),
            helpers: self.helpers.clone(),
            helpers_late: self.helpers_late.clone(),
            declared: self.declared.clone(),
            complete: self.complete.clone(),
            helpers_done: self.helpers_done.clone(),
            refines: self.refines.clone(),
        }
    }

    fn restore(&mut self, s: Snapshot) {
        self.fwd = s.fwd;
        self.defs = s.defs;
        self.protos = s.protos;
        self.helpers = s.helpers;
        self.helpers_late = s.helpers_late;
        self.declared = s.declared;
        self.complete = s.complete;
        self.helpers_done = s.helpers_done;
        self.refines = s.refines;
        self.in_progress.clear();
    }

    fn fresh(&mut self, base: &str) -> String {
        self.counter += 1;
        format!("{base}{}", self.counter)
    }

    fn mangle(&self, t: &Type) -> G {
        Ok(match t {
            Type::Con(n, a) if a.is_empty() && n == "Int" => "I".into(),
            Type::Con(n, a) if a.is_empty() && n == "Bool" => "B".into(),
            Type::Con(n, a) if a.is_empty() && n == "Unit" => "U".into(),
            Type::Con(n, a) if a.is_empty() && n == "F64" => "D".into(),
            Type::Con(n, a) if n == "List" => format!("L_{}", self.mangle(&a[0])?),
            Type::Con(n, a) if n == "Opt" => format!("O_{}", self.mangle(&a[0])?),
            Type::Con(n, a) if self.layouts.records.contains_key(n) || self.layouts.sums.contains_key(n) => {
                let kind = if self.layouts.records.contains_key(n) { "R" } else { "S" };
                let mut s = format!("{kind}_{n}");
                for x in a {
                    s.push('_');
                    s.push_str(&self.mangle(x)?);
                }
                s
            }
            Type::Tuple(xs) => {
                let parts: Vec<String> = xs.iter().map(|x| self.mangle(x)).collect::<G<_>>()?;
                format!("T{}_{}", xs.len(), parts.join("_"))
            }
            other => return Err(format!("uses type {other}, which is not native yet")),
        })
    }

    fn niche(&self, t: &Type) -> Option<(usize, usize)> {
        let Type::Con(n, a) = t else { return None };
        let vs = self.layouts.sum_variants(n, a)?;
        if vs.len() != 2 {
            return None;
        }
        match (&vs[0].1, &vs[1].1) {
            (None, Some(_)) => Some((0, 1)),
            (Some(_), None) => Some((1, 0)),
            _ => None,
        }
    }

    fn tag(&self, t: &Type, v: &str) -> String {
        match self.niche(t) {
            Some((empty, full)) => format!("(({v}) ? {full} : {empty})"),
            None => format!("(({v})->tag)"),
        }
    }

    fn fwd_decl(&mut self, m: &str) {
        if self.declared.insert(m.to_string()) {
            writeln!(self.fwd, "typedef struct {m} {m};").unwrap();
        }
    }

    fn decl(&mut self, t: &Type) -> G {
        match t {
            Type::Con(n, _) if self.layouts.records.contains_key(n) => {
                let m = self.mangle(t)?;
                self.fwd_decl(&m);
                if !self.in_progress.contains(&m) {
                    self.cty(t)?;
                }
                Ok(m)
            }
            _ => self.cty(t),
        }
    }

    fn cty(&mut self, t: &Type) -> G {
        let m = self.mangle(t)?;
        match t {
            Type::Con(n, _) if matches!(n.as_str(), "Int" | "Bool" | "Unit") => Ok("int64_t".into()),
            Type::Con(n, _) if n == "F64" => Ok("double".into()),
            Type::Con(n, a) if n == "List" => {
                self.fwd_decl(&m);
                if self.complete.insert(m.clone()) {
                    let e = self.decl(&a[0])?;
                    writeln!(self.defs, "struct {m} {{ int64_t len; {e}* data; int64_t* hdr; }};").unwrap();
                }
                Ok(m)
            }
            Type::Con(n, a) if n == "Opt" => {
                self.fwd_decl(&m);
                if !self.complete.contains(&m) {
                    let e = self.cty(&a[0])?;
                    if self.complete.insert(m.clone()) {
                        writeln!(self.defs, "struct {m} {{ int64_t some; {e} v; }};").unwrap();
                    }
                }
                Ok(m)
            }
            Type::Tuple(xs) => {
                self.fwd_decl(&m);
                if !self.complete.contains(&m) {
                    let es: Vec<String> = xs.iter().map(|x| self.cty(x)).collect::<G<_>>()?;
                    if self.complete.insert(m.clone()) {
                        let fields: String = es.iter().enumerate().map(|(i, e)| format!("{e} f{i}; ")).collect();
                        writeln!(self.defs, "struct {m} {{ {fields}}};").unwrap();
                    }
                }
                Ok(m)
            }
            Type::Con(n, a) if self.layouts.records.contains_key(n) => {
                self.fwd_decl(&m);
                if self.complete.contains(&m) {
                    return Ok(m);
                }
                if !self.in_progress.insert(m.clone()) {
                    return Err(format!("record {n} contains itself by value"));
                }
                let fs = self.layouts.record_fields(n, a).unwrap();
                let mut body = String::new();
                for (f, ft) in &fs {
                    let c = self.cty(ft)?;
                    write!(body, "{c} {f}; ").unwrap();
                }
                self.in_progress.remove(&m);
                if self.complete.insert(m.clone()) {
                    writeln!(self.defs, "struct {m} {{ {body}}};").unwrap();
                }
                Ok(m)
            }
            Type::Con(n, a) if self.layouts.sums.contains_key(n) => {
                if self.complete.insert(m.clone()) {
                    self.fwd_decl(&m);
                    let vs = self.layouts.sum_variants(n, a).unwrap();
                    let mut union = String::new();
                    for (k, (_, fs)) in vs.iter().enumerate() {
                        if let Some(fs) = fs {
                            let mut body = String::new();
                            for (f, ft) in fs {
                                let c = self.cty(ft)?;
                                write!(body, "{c} {f}; ").unwrap();
                            }
                            write!(union, "struct {{ {body}}} v{k}; ").unwrap();
                        }
                    }
                    let tag_field = if self.niche(t).is_some() { "" } else { "int64_t tag; " };
                    writeln!(self.defs, "struct {m} {{ {tag_field}union {{ char none_; {union}}} u; }};").unwrap();
                }
                Ok(format!("{m}*"))
            }
            other => Err(format!("uses type {other}, which is not native yet")),
        }
    }

    fn rr(&mut self, t: &Type) -> G {
        let m = self.mangle(t)?;
        let name = format!("RR_{m}");
        if !self.complete.contains(&name) {
            let c = self.cty(t)?;
            if self.complete.insert(name.clone()) {
                self.fwd_decl(&name);
                writeln!(self.defs, "struct {name} {{ {c} v; int64_t code; }};").unwrap();
            }
        }
        Ok(name)
    }

    fn helper_cmp(&mut self, t: &Type) -> G {
        if is(t, "Int") || is(t, "Bool") || is(t, "Unit") {
            return Ok("cmp_I".into());
        }
        if is(t, "F64") {
            return Ok("cmp_D".into());
        }
        let m = self.mangle(t)?;
        let name = format!("cmp_{m}");
        if !self.helpers_done.insert(name.clone()) {
            return Ok(name);
        }
        let c = self.cty(t)?;
        writeln!(self.protos, "static int {name}({c} a, {c} b);").unwrap();
        writeln!(self.protos, "static int {name}_p(const void* a, const void* b);").unwrap();
        let mut body = String::new();
        match t {
            Type::Con(n, a) if n == "List" => {
                let ec = self.helper_cmp(&a[0])?;
                write!(body, "int64_t n = a.len < b.len ? a.len : b.len; for (int64_t i = 0; i < n; i++) {{ int c = {ec}(a.data[i], b.data[i]); if (c) return c; }} return cmp_I(a.len, b.len);").unwrap();
            }
            Type::Con(n, a) if n == "Opt" => {
                let ec = self.helper_cmp(&a[0])?;
                write!(body, "if (a.some != b.some) return a.some ? 1 : -1; return a.some ? {ec}(a.v, b.v) : 0;").unwrap();
            }
            Type::Tuple(xs) => {
                for (i, x) in xs.iter().enumerate() {
                    let ec = self.helper_cmp(x)?;
                    write!(body, "{{ int c = {ec}(a.f{i}, b.f{i}); if (c) return c; }} ").unwrap();
                }
                body.push_str("return 0;");
            }
            Type::Con(n, a) if self.layouts.records.contains_key(n) => {
                for (f, ft) in self.layouts.record_fields(n, a).unwrap() {
                    let ec = self.helper_cmp(&ft)?;
                    write!(body, "{{ int c = {ec}(a.{f}, b.{f}); if (c) return c; }} ").unwrap();
                }
                body.push_str("return 0;");
            }
            Type::Con(n, a) if self.layouts.sums.contains_key(n) => {
                let vs = self.layouts.sum_variants(n, a).unwrap();
                let mut names: Vec<&String> = vs.iter().map(|(v, _)| v).collect();
                names.sort();
                let ranks: Vec<String> = vs.iter().map(|(v, _)| names.iter().position(|x| *x == v).unwrap().to_string()).collect();
                let (ta, tb) = (self.tag(t, "a"), self.tag(t, "b"));
                write!(body, "static const int rank[] = {{{}}}; if ({ta} != {tb}) return cmp_I(rank[{ta}], rank[{tb}]); switch ({ta}) {{ ", ranks.join(", ")).unwrap();
                for (k, (_, fs)) in vs.iter().enumerate() {
                    if let Some(fs) = fs {
                        write!(body, "case {k}: ").unwrap();
                        for (f, ft) in fs {
                            let ec = self.helper_cmp(ft)?;
                            write!(body, "{{ int c = {ec}(a->u.v{k}.{f}, b->u.v{k}.{f}); if (c) return c; }} ").unwrap();
                        }
                        body.push_str("return 0; ");
                    }
                }
                body.push_str("default: return 0; }");
            }
            _ => return Err(format!("cannot compare {t} natively")),
        }
        writeln!(self.helpers, "static int {name}({c} a, {c} b) {{ {body} }}").unwrap();
        writeln!(self.helpers, "static int {name}_p(const void* a, const void* b) {{ return {name}(*(const {c}*)a, *(const {c}*)b); }}").unwrap();
        Ok(name)
    }

    fn helper_enc(&mut self, t: &Type) -> G {
        let m = self.mangle(t)?;
        let name = format!("enc_{m}");
        if !self.helpers_done.insert(name.clone()) {
            return Ok(name);
        }
        let c = self.cty(t)?;
        writeln!(self.protos, "static void {name}(Buf* b, {c} v);").unwrap();
        let body = match t {
            _ if is(t, "F64") => "buf_push(b, dbits(v));".to_string(),
            _ if scalar(t) => "buf_push(b, v);".to_string(),
            Type::Con(n, a) if n == "List" => {
                let e = self.helper_enc(&a[0])?;
                format!("buf_push(b, v.len); for (int64_t i = 0; i < v.len; i++) {e}(b, v.data[i]);")
            }
            Type::Con(n, a) if n == "Opt" => {
                let e = self.helper_enc(&a[0])?;
                format!("buf_push(b, v.some); if (v.some) {e}(b, v.v);")
            }
            Type::Tuple(xs) => {
                let mut s = String::new();
                for (i, x) in xs.iter().enumerate() {
                    let e = self.helper_enc(x)?;
                    write!(s, "{e}(b, v.f{i}); ").unwrap();
                }
                s
            }
            Type::Con(n, a) if self.layouts.records.contains_key(n) => {
                let mut s = String::new();
                for (f, ft) in self.layouts.record_fields(n, a).unwrap() {
                    let e = self.helper_enc(&ft)?;
                    write!(s, "{e}(b, v.{f}); ").unwrap();
                }
                s
            }
            Type::Con(n, a) if self.layouts.sums.contains_key(n) => {
                let tv = self.tag(t, "v");
                let mut s = format!("buf_push(b, {tv}); switch ({tv}) {{ ");
                for (k, (_, fs)) in self.layouts.sum_variants(n, a).unwrap().iter().enumerate() {
                    if let Some(fs) = fs {
                        write!(s, "case {k}: ").unwrap();
                        for (f, ft) in fs {
                            let e = self.helper_enc(ft)?;
                            write!(s, "{e}(b, v->u.v{k}.{f}); ").unwrap();
                        }
                        s.push_str("break; ");
                    }
                }
                s.push_str("default: break; }");
                s
            }
            _ => return Err(format!("cannot encode {t}")),
        };
        writeln!(self.helpers_late, "static void {name}(Buf* b, {c} v) {{ {body} }}").unwrap();
        Ok(name)
    }

    fn helper_dec(&mut self, t: &Type) -> G {
        let m = self.mangle(t)?;
        let name = format!("dec_{m}");
        if !self.helpers_done.insert(name.clone()) {
            return Ok(name);
        }
        let c = self.cty(t)?;
        writeln!(self.protos, "static {c} {name}(const int64_t** p);").unwrap();
        let body = match t {
            _ if is(t, "F64") => "return bitsd(*(*p)++);".to_string(),
            _ if scalar(t) => "return *(*p)++;".to_string(),
            Type::Con(n, a) if n == "List" => {
                let d = self.helper_dec(&a[0])?;
                let ec = self.decl(&a[0])?;
                format!("int64_t n = *(*p)++; RawL r = raw_alloc(n, sizeof({ec})); {c} l = {{0, ({ec}*)r.data, r.hdr}}; for (int64_t i = 0; i < n; i++) l.data[i] = {d}(p); l.len = n; l.hdr[1] = n; return l;")
            }
            Type::Con(n, a) if n == "Opt" => {
                let d = self.helper_dec(&a[0])?;
                format!("{c} o = {{0}}; o.some = *(*p)++; if (o.some) o.v = {d}(p); return o;")
            }
            Type::Tuple(xs) => {
                let mut s = format!("{c} v; ");
                for (i, x) in xs.iter().enumerate() {
                    let d = self.helper_dec(x)?;
                    write!(s, "v.f{i} = {d}(p); ").unwrap();
                }
                s.push_str("return v;");
                s
            }
            Type::Con(n, a) if self.layouts.records.contains_key(n) => {
                let mut s = format!("{c} v; ");
                for (f, ft) in self.layouts.record_fields(n, a).unwrap() {
                    let d = self.helper_dec(&ft)?;
                    write!(s, "v.{f} = {d}(p); ").unwrap();
                }
                s.push_str("return v;");
                s
            }
            Type::Con(n, a) if self.layouts.sums.contains_key(n) => {
                let base = c.trim_end_matches('*').to_string();
                let mut s = match self.niche(t) {
                    Some((empty, _)) => format!("int64_t tg = *(*p)++; if (tg == {empty}) return ({c})0; {c} v = ({c})sspur_alloc(sizeof({base})); switch (tg) {{ "),
                    None => format!("{c} v = ({c})sspur_alloc(sizeof({base})); v->tag = *(*p)++; int64_t tg = v->tag; switch (tg) {{ "),
                };
                for (k, (_, fs)) in self.layouts.sum_variants(n, a).unwrap().iter().enumerate() {
                    if let Some(fs) = fs {
                        write!(s, "case {k}: ").unwrap();
                        for (f, ft) in fs {
                            let d = self.helper_dec(ft)?;
                            write!(s, "v->u.v{k}.{f} = {d}(p); ").unwrap();
                        }
                        s.push_str("break; ");
                    }
                }
                s.push_str("default: break; } return v;");
                s
            }
            _ => return Err(format!("cannot decode {t}")),
        };
        writeln!(self.helpers_late, "static {c} {name}(const int64_t** p) {{ {body} }}").unwrap();
        Ok(name)
    }

    fn entries(&mut self, name: &str, params: &[Type], ret: &Type, scalar_abi: bool) -> G {
        let mut s = String::new();
        let args: String = (0..params.len()).map(|i| format!("a{i}, ")).collect();
        if scalar_abi {
            let sig: String = (0..params.len()).map(|i| format!("int64_t a{i}, ")).collect();
            let rr = self.rr(ret)?;
            writeln!(s, "int64_t sspur_entry_{name}({sig}Status* st) {{ Mark m = arena_mark(); {rr} r = f_{name}({args}st, -st->limit); arena_release(m); return r.v; }}").unwrap();
        }
        let mut decs = String::new();
        for (i, t) in params.iter().enumerate() {
            let d = self.helper_dec(t)?;
            let c = self.cty(t)?;
            write!(decs, "{c} a{i} = {d}(&p); ").unwrap();
        }
        let enc = self.helper_enc(ret)?;
        let rr = self.rr(ret)?;
        writeln!(s, "int64_t sspur_wentry_{name}(const int64_t* in, Status* st, int64_t** out, int64_t* out_len) {{ Mark m = arena_mark(); const int64_t* p = in; {decs}{rr} r = f_{name}({args}st, -st->limit); if (r.code) {{ arena_release(m); return r.code; }} Buf b = {{0}}; {enc}(&b, r.v); *out = b.data; *out_len = b.len; arena_release(m); return 0; }}").unwrap();
        Ok(s)
    }

    fn ty(&self, e: &Expr) -> G<Type> {
        let t = self.check.expr_types.get(&expr_key(e)).cloned().ok_or_else(|| "has an expression without type information".to_string())?;
        if has_vars(&t) {
            return Err(format!("has an expression of unresolved type {t}"));
        }
        Ok(t)
    }

    fn bind(&mut self, name: &str, t: Type) -> String {
        let c = self.fresh(&format!("v_{name}_"));
        self.scopes.last_mut().unwrap().insert(name.to_string(), (c.clone(), t));
        c
    }

    fn lookup(&self, name: &str) -> Option<(String, Type)> {
        self.scopes.iter().rev().find_map(|s| s.get(name).cloned())
    }

    fn value_bits(&mut self, v: &str, t: &Type) -> G<(String, String)> {
        if is(t, "F64") {
            return Ok((format!("dbits({v})"), String::new()));
        }
        if scalar(t) {
            return Ok((v.to_string(), String::new()));
        }
        let enc = self.helper_enc(t)?;
        Ok(("0".into(), format!("{{ Buf rb_ = {{0}}; {enc}(&rb_, {v}); st->rbuf = rb_.data; st->rlen = rb_.len; }} ")))
    }

    fn refine_check(&mut self, ctx: String, refine: &Expr, value_var: &str, t: &Type) -> G {
        let idx = self.refines.len();
        self.refines.push((ctx, printer::expr(refine, 0), t.clone()));
        self.scopes.push(HashMap::from([("_".to_string(), (value_var.to_string(), t.clone()))]));
        let cond = self.expr(refine);
        self.scopes.pop();
        let cond = cond?;
        let (bits, pre) = self.value_bits(value_var, t)?;
        Ok(format!("if (UNLIKELY(!({cond}))) {{ {pre}TRAPV({T_REFINE}, {idx}, {bits}); }} "))
    }

    fn alias_check(&mut self, ty: &Ty, ctx: &str, value_var: &str, t: &Type) -> G {
        if let Ty::Named { name, .. } = ty {
            if let Some(r) = self.alias_refines.get(name).cloned() {
                return self.refine_check(format!("type {name} in {ctx}"), &r, value_var, t);
            }
        }
        Ok(String::new())
    }

    fn record_checks(&mut self, owner: &str, var: &str, access: &str, fields: &[(String, Type)]) -> G {
        let Some(decl) = self.field_refines.get(owner).cloned() else { return Ok(String::new()) };
        let mut s = String::new();
        for (fname, refine, _) in &decl {
            if let Some(r) = refine {
                let ft = fields.iter().find(|(n, _)| n == fname).map(|(_, t)| t.clone()).unwrap();
                s.push_str(&self.refine_check(format!("field '{fname}' of {owner}"), r, &format!("{var}{access}{fname}"), &ft)?);
            }
        }
        for (fname, _, tyname) in &decl {
            if let Some(r) = tyname.as_ref().and_then(|n| self.alias_refines.get(n)).cloned() {
                let ft = fields.iter().find(|(n, _)| n == fname).map(|(_, t)| t.clone()).unwrap();
                s.push_str(&self.refine_check(format!("field '{fname}' of {owner}"), &r, &format!("{var}{access}{fname}"), &ft)?);
            }
        }
        Ok(s)
    }

    fn function(&mut self, f: &FnDef, params: &[Type], ret: &Type, fidx: usize) -> G {
        self.scopes = vec![HashMap::new()];
        self.ret = ret.clone();
        let rr = self.rr(ret)?;
        let rc = self.cty(ret)?;
        let mut sig = Vec::new();
        for (i, t) in params.iter().enumerate() {
            sig.push(format!("{} a{i}", self.cty(t)?));
        }
        sig.push("Status* st".into());
        sig.push("int64_t depth".into());
        let mut s = format!("#define RRT {rr}\n#define FIDX {fidx}\nstatic {rr} f_{}({}) {{\n", f.name, sig.join(", "));
        writeln!(s, "  depth += 1; if (UNLIKELY(depth > 0)) TRAPV({T_DEPTH}, 0, 0);").unwrap();
        for (i, (p, t)) in f.params.iter().zip(params).enumerate() {
            self.scopes[0].insert(p.name.clone(), (format!("a{i}"), t.clone()));
        }
        for (i, (p, t)) in f.params.iter().zip(params).enumerate() {
            let ctx = format!("parameter '{}' of {}", p.name, f.name);
            s.push_str("  ");
            s.push_str(&self.alias_check(&p.ty, &ctx, &format!("a{i}"), t)?);
            if let Some(r) = &p.refine {
                s.push_str(&self.refine_check(ctx, r, &format!("a{i}"), t)?);
            }
            s.push('\n');
        }
        for (i, pre) in f.pres.iter().enumerate() {
            let c = self.expr(pre)?;
            writeln!(s, "  if (UNLIKELY(!({c}))) TRAPV({T_PRE}, {i}, 0);").unwrap();
        }
        let body = self.expr(&f.body)?;
        writeln!(s, "  {rc} ret_;\n  ret_ = {body};\n  goto done_;\ndone_: ;").unwrap();
        if let Some(rt) = &f.ret {
            let c = self.alias_check(rt, &format!("result of {}", f.name), "ret_", ret)?;
            writeln!(s, "  {c}").unwrap();
        }
        if !f.posts.is_empty() {
            self.scopes.push(HashMap::from([("r".to_string(), ("ret_".to_string(), ret.clone()))]));
            for (i, post) in f.posts.iter().enumerate() {
                let c = self.expr(post)?;
                let (bits, pre) = self.value_bits("ret_", ret)?;
                writeln!(s, "  if (UNLIKELY(!({c}))) {{ {pre}TRAPV({T_POST}, {i}, {bits}); }}").unwrap();
            }
            self.scopes.pop();
        }
        writeln!(s, "  return ({rr}){{ret_, 0}};\n}}\n#undef RRT\n#undef FIDX").unwrap();
        Ok(s)
    }

    fn call_user(&mut self, name: &str, args: Vec<String>) -> G {
        if !self.eligible.contains(name) {
            return Err(format!("calls '{name}', which is not native"));
        }
        let (_, ret) = self.check.fn_types.get(name).cloned().ok_or("unknown callee")?;
        let rr = self.rr(&ret)?;
        let mut s = String::from("({ ");
        let mut names = Vec::new();
        for a in args {
            let t = self.fresh("ca");
            write!(s, "__auto_type {t} = {a}; ").unwrap();
            names.push(t);
        }
        let call_args: String = names.iter().map(|n| format!("{n}, ")).collect();
        write!(s, "{rr} c_ = f_{name}({call_args}st, depth); if (UNLIKELY(c_.code)) return (RRT){{.code = c_.code}}; c_.v; }})").unwrap();
        Ok(s)
    }

    fn apply(&mut self, f: &Expr, args: Vec<(String, Type)>) -> G {
        match &f.kind {
            ExprKind::Lambda { params, body, .. } => {
                if params.len() != args.len() {
                    return Err("lambda arity mismatch".into());
                }
                let mut scope = HashMap::new();
                for (p, (v, t)) in params.iter().zip(args) {
                    scope.insert(p.clone(), (v, t));
                }
                self.scopes.push(scope);
                let r = self.expr(body);
                self.scopes.pop();
                r
            }
            ExprKind::Name(n) if self.lookup(n).is_none() => self.call_user(n, args.into_iter().map(|(v, _)| v).collect()),
            _ => Err("passes a function value that is not a lambda or a function name".into()),
        }
    }

    fn checked(&mut self, op: BinOp, x: &str, y: &str) -> String {
        match op {
            BinOp::Add => format!("({{ int64_t a_ = {x}; int64_t b_ = {y}; int64_t r_; if (UNLIKELY(__builtin_add_overflow(a_, b_, &r_))) TRAPV({T_OVERFLOW}, 0, 0); r_; }})"),
            BinOp::Sub => format!("({{ int64_t a_ = {x}; int64_t b_ = {y}; int64_t r_; if (UNLIKELY(__builtin_sub_overflow(a_, b_, &r_))) TRAPV({T_OVERFLOW}, 0, 0); r_; }})"),
            BinOp::Mul => format!("({{ int64_t a_ = {x}; int64_t b_ = {y}; int64_t r_; if (UNLIKELY(__builtin_mul_overflow(a_, b_, &r_))) TRAPV({T_OVERFLOW}, 0, 0); r_; }})"),
            BinOp::Div | BinOp::Rem => {
                let sym = if op == BinOp::Div { "/" } else { "%" };
                format!("({{ int64_t a_ = {x}; int64_t b_ = {y}; if (UNLIKELY(b_ == 0)) TRAPV({T_DIV_ZERO}, 0, 0); if (UNLIKELY(a_ == INT64_MIN && b_ == -1)) TRAPV({T_OVERFLOW}, 0, 0); a_ {sym} b_; }})")
            }
            BinOp::Pow => format!("({{ PowR p_ = sspur_pow({x}, {y}); if (UNLIKELY(p_.code)) TRAPV(p_.code, 0, 0); p_.v; }})"),
            _ => unreachable!(),
        }
    }

    fn expr(&mut self, e: &Expr) -> G {
        let t = self.ty(e)?;
        match &e.kind {
            ExprKind::Int(n) => Ok(lit(*n)),
            ExprKind::Float(x) => Ok(format!("bitsd({}LL)", x.to_bits() as i64)),
            ExprKind::Bool(b) => Ok(if *b { "1LL" } else { "0LL" }.into()),
            ExprKind::Unit => Ok("0LL".into()),
            ExprKind::Name(n) if self.lookup(n).is_some() => Ok(self.lookup(n).unwrap().0),
            ExprKind::Placeholder => self.lookup("_").map(|(v, _)| v).ok_or_else(|| "unbound placeholder".into()),
            ExprKind::Name(n) if n == "none" => Ok(format!("({}){{0}}", self.cty(&t)?)),
            ExprKind::Name(n) => self.variant(n, &[], &t),
            ExprKind::Field(x, f) => {
                let xt = self.ty(x)?;
                if let (Type::Tuple(_), Ok(i)) = (&xt, f.parse::<usize>()) {
                    return Ok(format!("({}).f{i}", self.expr(x)?));
                }
                if let Type::Con(n, a) = &xt {
                    if self.layouts.record_fields(n, a).is_some_and(|fs| fs.iter().any(|(fname, _)| fname == f)) {
                        return Ok(format!("({}).{f}", self.expr(x)?));
                    }
                }
                self.method(e, x, f, &[], &t)
            }
            ExprKind::Method { recv, name, args, .. } => self.method(e, recv, name, args, &t),
            ExprKind::Call(f, args) => {
                let ExprKind::Name(n) = &f.kind else { return Err("calls a function value".into()) };
                if self.lookup(n).is_some() {
                    return Err("calls a function-typed variable".into());
                }
                match n.as_str() {
                    "some" => {
                        let v = self.expr(&args[0])?;
                        Ok(format!("({}){{1, {v}}}", self.cty(&t)?))
                    }
                    "min" | "max" => {
                        let (a, b) = (self.expr(&args[0])?, self.expr(&args[1])?);
                        let at = self.ty(&args[0])?;
                        let c = self.helper_cmp(&at)?;
                        let op = if n == "min" { "<" } else { ">" };
                        Ok(format!("({{ __auto_type x_ = {a}; __auto_type y_ = {b}; {c}(y_, x_) {op} 0 ? y_ : x_; }})"))
                    }
                    _ => {
                        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
                        self.call_user(n, vals)
                    }
                }
            }
            ExprKind::Index(a, i) => {
                let (av, iv) = (self.expr(a)?, self.expr(i)?);
                Ok(format!("({{ __auto_type l_ = {av}; int64_t i_ = {iv}; if (UNLIKELY(i_ < 0 || i_ >= l_.len)) TRAPV({T_INDEX}, l_.len, i_); l_.data[i_]; }})"))
            }
            ExprKind::Binary(BinOp::And, a, b) => Ok(format!("((int64_t)(({}) && ({})))", self.expr(a)?, self.expr(b)?)),
            ExprKind::Binary(BinOp::Or, a, b) => Ok(format!("((int64_t)(({}) || ({})))", self.expr(a)?, self.expr(b)?)),
            ExprKind::Binary(op, a, b) => {
                let at = self.ty(a)?;
                let (x, y) = (self.expr(a)?, self.expr(b)?);
                if matches!(op, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) {
                    if is(&at, "Int") || is(&at, "Bool") || is(&at, "Unit") {
                        return Ok(format!("({{ int64_t a_ = {x}; int64_t b_ = {y}; (int64_t)(a_ {} b_); }})", op.symbol()));
                    }
                    let c = self.helper_cmp(&at)?;
                    return Ok(format!("({{ __auto_type a_ = {x}; __auto_type b_ = {y}; (int64_t)({c}(a_, b_) {} 0); }})", op.symbol()));
                }
                if is(&at, "Int") {
                    return Ok(self.checked(*op, &x, &y));
                }
                if is(&at, "F64") {
                    return Ok(match op {
                        BinOp::Rem => format!("({{ double a_ = {x}; double b_ = {y}; fmod(a_, b_); }})"),
                        BinOp::Pow => format!("({{ double a_ = {x}; double b_ = {y}; pow(a_, b_); }})"),
                        _ => format!("({{ double a_ = {x}; double b_ = {y}; a_ {} b_; }})", op.symbol()),
                    });
                }
                if let (BinOp::Add, Some(et)) = (op, elem(&at, "List")) {
                    let ec = self.decl(&et)?;
                    let lc = self.cty(&at)?;
                    return Ok(format!("({{ __auto_type a_ = {x}; __auto_type b_ = {y}; RawL r_ = raw_concat(TO_RAW(a_), TO_RAW(b_), sizeof({ec})); ({lc}){{r_.len, ({ec}*)r_.data, r_.hdr}}; }})"));
                }
                Err(format!("uses '{}' on {at}", op.symbol()))
            }
            ExprKind::Unary(UnOp::Neg, x) => {
                let v = self.expr(x)?;
                if is(&t, "F64") {
                    Ok(format!("(-({v}))"))
                } else {
                    Ok(format!("({{ int64_t t_ = {v}; if (UNLIKELY(t_ == INT64_MIN)) TRAPV({T_OVERFLOW}, 0, 0); -t_; }})"))
                }
            }
            ExprKind::Unary(UnOp::Not, x) => Ok(format!("((int64_t)!({}))", self.expr(x)?)),
            ExprKind::Range(a, b) => {
                let (av, bv) = (self.expr(a)?, self.expr(b)?);
                let lc = self.cty(&t)?;
                Ok(format!("({{ int64_t s_ = {av}; int64_t e_ = {bv}; int64_t n_ = e_ > s_ ? e_ - s_ : 0; RawL r_ = raw_alloc(n_, 8); int64_t* d_ = (int64_t*)r_.data; for (int64_t i_ = 0; i_ < n_; i_++) d_[i_] = s_ + i_; r_.len = n_; r_.hdr[1] = n_; ({lc}){{n_, d_, r_.hdr}}; }})"))
            }
            ExprKind::If(c, a, b) => {
                let cv = self.expr(c)?;
                let av = self.expr(a)?;
                let bv = match b {
                    Some(b) => self.expr(b)?,
                    None => "0LL".into(),
                };
                Ok(format!("(({cv}) ? ({av}) : ({bv}))"))
            }
            ExprKind::Match(s, arms) => self.match_expr(s, arms, &t),
            ExprKind::Block(stmts) => self.block(stmts),
            ExprKind::Record { ctor, fields } => {
                let owner = match ctor {
                    Some(c) => c.clone(),
                    None => self.check.record_types.get(&(e.span.start, e.span.end)).cloned().ok_or("untyped record literal")?,
                };
                if self.layouts.records.contains_key(&owner) {
                    let Type::Con(_, args) = &t else { return Err("bad record type".into()) };
                    let decl = self.layouts.record_fields(&owner, args).unwrap();
                    let rc = self.cty(&t)?;
                    let mut s = String::from("({ ");
                    let mut temps = HashMap::new();
                    for (fname, fe) in fields {
                        let v = self.expr(fe)?;
                        let tv = self.fresh("rf");
                        write!(s, "__auto_type {tv} = {v}; ").unwrap();
                        temps.insert(fname.clone(), tv);
                    }
                    let rv = self.fresh("rv");
                    let inits: Vec<String> = decl.iter().map(|(f, _)| format!(".{f} = {}", temps[f])).collect();
                    write!(s, "{rc} {rv} = ({rc}){{{}}}; ", inits.join(", ")).unwrap();
                    s.push_str(&self.record_checks(&owner, &rv, ".", &decl)?);
                    write!(s, "{rv}; }})").unwrap();
                    Ok(s)
                } else {
                    let vals: Vec<(String, &Expr)> = fields.iter().map(|(n, x)| (n.clone(), x)).collect();
                    self.variant(&owner, &vals, &t)
                }
            }
            ExprKind::List(xs) => {
                let et = elem(&t, "List").ok_or("bad list type")?;
                let ec = self.cty(&et)?;
                let lc = self.cty(&t)?;
                let mut s = format!("({{ RawL r_ = raw_alloc({}, sizeof({ec})); {ec}* d_ = ({ec}*)r_.data; ", xs.len());
                for (i, x) in xs.iter().enumerate() {
                    let v = self.expr(x)?;
                    write!(s, "d_[{i}] = {v}; ").unwrap();
                }
                write!(s, "r_.hdr[1] = {n}; ({lc}){{{n}, d_, r_.hdr}}; }})", n = xs.len()).unwrap();
                Ok(s)
            }
            ExprKind::Tuple(xs) | ExprKind::Par(xs) => {
                let tc = self.cty(&t)?;
                let mut s = String::from("({ ");
                let mut names = Vec::new();
                for x in xs {
                    let v = self.expr(x)?;
                    let n = self.fresh("tu");
                    write!(s, "__auto_type {n} = {v}; ").unwrap();
                    names.push(n);
                }
                write!(s, "({tc}){{{}}}; }})", names.join(", ")).unwrap();
                Ok(s)
            }
            ExprKind::Return(x) => {
                let v = self.expr(x)?;
                let z = self.zero(&t)?;
                Ok(format!("({{ ret_ = {v}; goto done_; {z}; }})"))
            }
            ExprKind::With(base, ups) => self.with_expr(base, ups, &t),
            other => Err(format!("uses {} expressions", kind_name(other))),
        }
    }

    fn zero(&mut self, t: &Type) -> G {
        let c = self.cty(t)?;
        Ok(if c == "int64_t" {
            "0LL".into()
        } else if c == "double" {
            "0.0".into()
        } else if c.ends_with('*') {
            format!("(({c})0)")
        } else {
            format!("(({c}){{0}})")
        })
    }

    fn variant(&mut self, ctor: &str, fields: &[(String, &Expr)], t: &Type) -> G {
        let Type::Con(sum, args) = t else { return Err(format!("'{ctor}' is not a native value")) };
        let vs = self.layouts.sum_variants(sum, args).ok_or_else(|| format!("'{ctor}' is not a native value"))?;
        let k = vs.iter().position(|(v, _)| v == ctor).ok_or("unknown constructor")?;
        let sc = self.cty(t)?;
        let base = sc.trim_end_matches('*').to_string();
        let mut s = String::from("({ ");
        let mut temps = HashMap::new();
        for (fname, fe) in fields {
            let v = self.expr(fe)?;
            let tv = self.fresh("vf");
            write!(s, "__auto_type {tv} = {v}; ").unwrap();
            temps.insert(fname.clone(), tv);
        }
        let niche = self.niche(t);
        if vs[k].1.is_none() && niche.is_some() {
            return Ok(format!("(({sc})0)"));
        }
        if vs[k].1.is_none() {
            let sing = format!("sing_{base}_{k}");
            if self.helpers_done.insert(sing.clone()) {
                writeln!(self.protos, "static {base} {sing} = {{ .tag = {k} }};").unwrap();
            }
            return Ok(format!("(&{sing})"));
        }
        let pv = self.fresh("vp");
        if niche.is_some() {
            write!(s, "{sc} {pv} = ({sc})sspur_alloc(sizeof({base})); ").unwrap();
        } else {
            write!(s, "{sc} {pv} = ({sc})sspur_alloc(sizeof({base})); {pv}->tag = {k}; ").unwrap();
        }
        if let Some(decl) = &vs[k].1 {
            for (f, _) in decl {
                write!(s, "{pv}->u.v{k}.{f} = {}; ", temps[f]).unwrap();
            }
            s.push_str(&self.record_checks(ctor, &format!("{pv}->u.v{k}"), ".", decl)?);
        }
        write!(s, "{pv}; }})").unwrap();
        Ok(s)
    }

    fn method(&mut self, e: &Expr, recv: &Expr, name: &str, args: &[Expr], t: &Type) -> G {
        if self.check.user_methods.contains(&(e.span.start, e.span.end)) {
            let mut vals = vec![self.expr(recv)?];
            for a in args {
                vals.push(self.expr(a)?);
            }
            return self.call_user(name, vals);
        }
        let rt = self.ty(recv)?;
        let r = self.expr(recv)?;
        if let Some(et) = elem(&rt, "List") {
            return self.list_method(&r, &rt, &et, name, args, t);
        }
        if let Some(et) = elem(&rt, "Opt") {
            return Ok(match name {
                "or" => format!("({{ __auto_type o_ = {r}; __auto_type d_ = {}; o_.some ? o_.v : d_; }})", self.expr(&args[0])?),
                "is_some" => format!("(({r}).some)"),
                "is_none" => format!("((int64_t)!({r}).some)"),
                "get" => format!("({{ __auto_type o_ = {r}; if (UNLIKELY(!o_.some)) TRAPV({T_UNWRAP}, 0, 0); o_.v; }})"),
                "map" => {
                    let x = self.fresh("om");
                    let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                    let oc = self.cty(t)?;
                    format!("({{ __auto_type o_ = {r}; {oc} res_ = {{0}}; if (o_.some) {{ __auto_type {x} = o_.v; res_.some = 1; res_.v = {body}; }} res_; }})")
                }
                _ => return Err(format!("uses Opt.{name}")),
            });
        }
        if is(&rt, "Int") {
            return Ok(match name {
                "abs" => format!("({{ int64_t t_ = {r}; if (UNLIKELY(t_ == INT64_MIN)) TRAPV({T_OVERFLOW}, 0, 0); t_ < 0 ? -t_ : t_; }})"),
                "to_f64" => format!("((double)({r}))"),
                _ => return Err(format!("uses Int.{name}")),
            });
        }
        if is(&rt, "F64") {
            return Ok(match name {
                "abs" => format!("fabs({r})"),
                "round" => format!("f2i(round({r}))"),
                "floor" => format!("f2i(floor({r}))"),
                "sqrt" => format!("sqrt({r})"),
                _ => return Err(format!("uses F64.{name}")),
            });
        }
        Err(format!("uses method '{name}' on {rt}"))
    }

    fn list_method(&mut self, r: &str, lt: &Type, et: &Type, name: &str, args: &[Expr], t: &Type) -> G {
        let ec = self.decl(et)?;
        let lc = self.cty(lt)?;
        let l = self.fresh("l");
        let i = self.fresh("i");
        let x = self.fresh("x");
        let head = format!("__auto_type {l} = {r}; ");
        let wrap = |body: String| format!("({{ {head}{body} }})");
        Ok(match name {
            "len" => format!("(({r}).len)"),
            "is_empty" => format!("((int64_t)(({r}).len == 0))"),
            "get" | "first" | "last" => {
                let oc = self.cty(t)?;
                let idx = match name {
                    "get" => self.expr(&args[0])?,
                    "first" => "0".into(),
                    _ => format!("{l}.len - 1"),
                };
                wrap(format!("int64_t {i} = {idx}; {oc} o_ = {{0}}; if ({i} >= 0 && {i} < {l}.len) {{ o_.some = 1; o_.v = {l}.data[{i}]; }} o_;"))
            }
            "push" => {
                let v = self.expr(&args[0])?;
                wrap(format!("__auto_type {x} = {v}; RawL r_ = raw_push(TO_RAW({l}), &{x}, sizeof({ec})); ({lc}){{r_.len, ({ec}*)r_.data, r_.hdr}};"))
            }
            "concat" => {
                let v = self.expr(&args[0])?;
                wrap(format!("__auto_type b_ = {v}; RawL r_ = raw_concat(TO_RAW({l}), TO_RAW(b_), sizeof({ec})); ({lc}){{r_.len, ({ec}*)r_.data, r_.hdr}};"))
            }
            "take" | "drop" => {
                let n = self.expr(&args[0])?;
                let clamp = format!("int64_t n_ = {n}; if (n_ < 0) n_ = 0; if (n_ > {l}.len) n_ = {l}.len; ");
                if name == "take" {
                    wrap(format!("{clamp}({lc}){{n_, {l}.data, {l}.hdr}};"))
                } else {
                    wrap(format!("{clamp}({lc}){{{l}.len - n_, {l}.data + n_, {l}.hdr}};"))
                }
            }
            "reverse" => wrap(format!("RawL r_ = raw_alloc({l}.len, sizeof({ec})); {ec}* d_ = ({ec}*)r_.data; for (int64_t {i} = 0; {i} < {l}.len; {i}++) d_[{i}] = {l}.data[{l}.len - 1 - {i}]; r_.hdr[1] = {l}.len; ({lc}){{{l}.len, d_, r_.hdr}};")),
            "sum" => {
                if is(et, "F64") {
                    wrap(format!("double s_ = 0.0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) s_ += {l}.data[{i}]; s_;"))
                } else {
                    wrap(format!("int64_t s_ = 0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) if (UNLIKELY(__builtin_add_overflow(s_, {l}.data[{i}], &s_))) TRAPV({T_OVERFLOW}, 0, 0); s_;"))
                }
            }
            "contains" => {
                let v = self.expr(&args[0])?;
                let c = self.helper_cmp(et)?;
                wrap(format!("__auto_type {x} = {v}; int64_t f_ = 0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) if ({c}({l}.data[{i}], {x}) == 0) {{ f_ = 1; break; }} f_;"))
            }
            "map" => {
                let ot = elem(t, "List").ok_or("bad map type")?;
                let oc = self.cty(&ot)?;
                let out = self.cty(t)?;
                let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                wrap(format!("RawL r_ = raw_alloc({l}.len, sizeof({oc})); {oc}* d_ = ({oc}*)r_.data; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; d_[{i}] = {body}; }} r_.hdr[1] = {l}.len; ({out}){{{l}.len, d_, r_.hdr}};"))
            }
            "flat_map" => {
                let ot = elem(t, "List").ok_or("bad flat_map type")?;
                let oc = self.decl(&ot)?;
                let out = self.cty(t)?;
                let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                wrap(format!("RawL r_ = raw_alloc({l}.len, sizeof({oc})); for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; __auto_type p_ = {body}; r_ = raw_concat(r_, TO_RAW(p_), sizeof({oc})); }} ({out}){{r_.len, ({oc}*)r_.data, r_.hdr}};"))
            }
            "filter" => {
                let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                wrap(format!("RawL r_ = raw_alloc({l}.len, sizeof({ec})); {ec}* d_ = ({ec}*)r_.data; int64_t n_ = 0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; if ({body}) d_[n_++] = {x}; }} r_.hdr[1] = n_; ({lc}){{n_, d_, r_.hdr}};"))
            }
            "fold" => {
                let init = self.expr(&args[0])?;
                let acc_t = self.ty(&args[0])?;
                let at = self.cty(t)?;
                let acc = self.fresh("acc");
                let body = self.apply(&args[1], vec![(acc.clone(), acc_t), (x.clone(), et.clone())])?;
                wrap(format!("{at} {acc} = {init}; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; {acc} = {body}; }} {acc};"))
            }
            "any" | "all" => {
                let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                let (init, hit) = if name == "any" { ("0", "1") } else { ("1", "0") };
                let test = if name == "any" { format!("({body})") } else { format!("!({body})") };
                wrap(format!("int64_t f_ = {init}; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; if ({test}) {{ f_ = {hit}; break; }} }} f_;"))
            }
            "find" => {
                let oc = self.cty(t)?;
                let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                wrap(format!("{oc} o_ = {{0}}; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; if ({body}) {{ o_.some = 1; o_.v = {x}; break; }} }} o_;"))
            }
            "min" | "max" => {
                let oc = self.cty(t)?;
                let c = self.helper_cmp(et)?;
                let better = if name == "min" { "< 0" } else { ">= 0" };
                wrap(format!("{oc} o_ = {{0}}; for (int64_t {i} = 0; {i} < {l}.len; {i}++) if (!o_.some || {c}({l}.data[{i}], o_.v) {better}) {{ o_.some = 1; o_.v = {l}.data[{i}]; }} o_;"))
            }
            "sort" => {
                let c = self.helper_cmp(et)?;
                wrap(format!("RawL r_ = raw_sorted(TO_RAW({l}), sizeof({ec}), {c}_p); ({lc}){{r_.len, ({ec}*)r_.data, r_.hdr}};"))
            }
            "sort_by" => {
                let kt = match &args[0].kind {
                    ExprKind::Lambda { body, .. } => self.ty(body)?,
                    _ => return Err("sort_by needs a lambda".into()),
                };
                let pair = Type::Tuple(vec![kt.clone(), et.clone()]);
                let pc = self.cty(&pair)?;
                let kc = self.helper_cmp(&kt)?;
                let pcmp = format!("pcmp_{}", self.mangle(&pair)?);
                if self.helpers_done.insert(pcmp.clone()) {
                    writeln!(self.protos, "static int {pcmp}(const void* a, const void* b);").unwrap();
                    writeln!(self.helpers, "static int {pcmp}(const void* a, const void* b) {{ return {kc}(((const {pc}*)a)->f0, ((const {pc}*)b)->f0); }}").unwrap();
                }
                let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                wrap(format!("RawL p_ = raw_alloc({l}.len, sizeof({pc})); {pc}* pd_ = ({pc}*)p_.data; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; pd_[{i}].f0 = {body}; pd_[{i}].f1 = {x}; }} p_.len = {l}.len; p_.hdr[1] = {l}.len; if ({l}.len > 1) raw_msort((char*)pd_, {l}.len, sizeof({pc}), {pcmp}, (char*)sspur_alloc((size_t){l}.len * sizeof({pc}))); RawL r_ = raw_alloc({l}.len, sizeof({ec})); {ec}* d_ = ({ec}*)r_.data; for (int64_t {i} = 0; {i} < {l}.len; {i}++) d_[{i}] = pd_[{i}].f1; r_.hdr[1] = {l}.len; ({lc}){{{l}.len, d_, r_.hdr}};"))
            }
            "unique" => {
                let c = self.helper_cmp(et)?;
                wrap(format!("RawL r_ = raw_alloc({l}.len, sizeof({ec})); {ec}* d_ = ({ec}*)r_.data; int64_t n_ = 0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ int dup_ = 0; for (int64_t j_ = 0; j_ < n_; j_++) if ({c}(d_[j_], {l}.data[{i}]) == 0) {{ dup_ = 1; break; }} if (!dup_) d_[n_++] = {l}.data[{i}]; }} r_.hdr[1] = n_; ({lc}){{n_, d_, r_.hdr}};"))
            }
            "enumerate" | "zip" => {
                let pt = elem(t, "List").ok_or("bad pair list")?;
                let pc = self.cty(&pt)?;
                let out = self.cty(t)?;
                if name == "enumerate" {
                    wrap(format!("RawL r_ = raw_alloc({l}.len, sizeof({pc})); {pc}* d_ = ({pc}*)r_.data; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ d_[{i}].f0 = {i}; d_[{i}].f1 = {l}.data[{i}]; }} r_.hdr[1] = {l}.len; ({out}){{{l}.len, d_, r_.hdr}};"))
                } else {
                    let other = self.expr(&args[0])?;
                    wrap(format!("__auto_type b_ = {other}; int64_t n_ = {l}.len < b_.len ? {l}.len : b_.len; RawL r_ = raw_alloc(n_, sizeof({pc})); {pc}* d_ = ({pc}*)r_.data; for (int64_t {i} = 0; {i} < n_; {i}++) {{ d_[{i}].f0 = {l}.data[{i}]; d_[{i}].f1 = b_.data[{i}]; }} r_.hdr[1] = n_; ({out}){{n_, d_, r_.hdr}};"))
                }
            }
            _ => return Err(format!("uses List.{name}")),
        })
    }

    fn pattern(&mut self, p: &Pat, v: &str, t: &Type, conds: &mut Vec<String>, binds: &mut Vec<(String, String, Type)>) -> G<()> {
        match p {
            Pat::Wild => {}
            Pat::Bind(n) => binds.push((n.clone(), v.to_string(), t.clone())),
            Pat::Int(k) => conds.push(format!("({v} == {})", lit(*k))),
            Pat::Bool(b) => conds.push(format!("({v} == {})", i64::from(*b))),
            Pat::Str(_) => return Err("matches on strings".into()),
            Pat::Tuple(ps) => {
                let Type::Tuple(ts) = t else { return Err("bad tuple pattern".into()) };
                for (i, (p, t)) in ps.iter().zip(ts).enumerate() {
                    self.pattern(p, &format!("{v}.f{i}"), t, conds, binds)?;
                }
            }
            Pat::Ctor { name, args } => {
                if let Some(et) = elem(t, "Opt") {
                    match (name.as_str(), args) {
                        ("some", CtorArgs::Positional(ps)) => {
                            conds.push(format!("{v}.some"));
                            self.pattern(&ps[0], &format!("{v}.v"), &et, conds, binds)?;
                        }
                        ("none", _) => conds.push(format!("!{v}.some")),
                        _ => return Err("unsupported option pattern".into()),
                    }
                    return Ok(());
                }
                let Type::Con(tn, targs) = t else { return Err("bad constructor pattern".into()) };
                if let Some(fs) = self.layouts.record_fields(tn, targs) {
                    if let CtorArgs::Record(pfs) = args {
                        for (f, sp) in pfs {
                            let ft = fs.iter().find(|(n, _)| n == f).map(|(_, t)| t.clone()).ok_or("unknown field")?;
                            self.pattern(sp, &format!("{v}.{f}"), &ft, conds, binds)?;
                        }
                    }
                    return Ok(());
                }
                let vs = self.layouts.sum_variants(tn, targs).ok_or("bad sum pattern")?;
                let k = vs.iter().position(|(n, _)| n == name).ok_or("unknown constructor")?;
                let tag = self.tag(t, v);
                conds.push(format!("{tag} == {k}"));
                if let (CtorArgs::Record(pfs), Some(fs)) = (args, &vs[k].1) {
                    for (f, sp) in pfs {
                        let ft = fs.iter().find(|(n, _)| n == f).map(|(_, t)| t.clone()).ok_or("unknown field")?;
                        self.pattern(sp, &format!("{v}->u.v{k}.{f}"), &ft, conds, binds)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn match_expr(&mut self, scrut: &Expr, arms: &[Arm], t: &Type) -> G {
        let st = self.ty(scrut)?;
        let sv = self.expr(scrut)?;
        let rc = self.cty(t)?;
        let s_ = self.fresh("ms");
        let res = self.fresh("mr");
        let end = self.fresh("mend");
        let mut s = format!("({{ __auto_type {s_} = {sv}; {rc} {res}; ");
        for a in arms {
            let mut conds = Vec::new();
            let mut binds = Vec::new();
            self.pattern(&a.pat, &s_, &st, &mut conds, &mut binds)?;
            self.scopes.push(HashMap::new());
            let mut decl = String::new();
            for (n, expr, bt) in binds {
                let c = self.bind(&n, bt);
                write!(decl, "__auto_type {c} = {expr}; ").unwrap();
            }
            let guard = match &a.guard {
                Some(g) => self.expr(g)?,
                None => "1".into(),
            };
            let body = self.expr(&a.body)?;
            self.scopes.pop();
            let cond = if conds.is_empty() { "1".to_string() } else { conds.join(" && ") };
            write!(s, "if ({cond}) {{ {decl}if ({guard}) {{ {res} = {body}; goto {end}; }} }} ").unwrap();
        }
        write!(s, "TRAPV({T_NOMATCH}, 0, 0); {end}: ; {res}; }})").unwrap();
        Ok(s)
    }

    fn block(&mut self, stmts: &[Stmt]) -> G {
        self.scopes.push(HashMap::new());
        let mut s = String::from("({ ");
        let n = stmts.len();
        for (i, st) in stmts.iter().enumerate() {
            let last = i + 1 == n;
            let r = match st {
                Stmt::Expr(x) => self.expr(x).map(|v| if last { format!("{v}; ") } else { format!("(void)({v}); ") }),
                other => self.stmt(other).map(|c| if last { format!("{c}0LL; ") } else { c }),
            };
            match r {
                Ok(c) => s.push_str(&c),
                Err(e) => {
                    self.scopes.pop();
                    return Err(e);
                }
            }
        }
        self.scopes.pop();
        s.push_str("})");
        Ok(s)
    }

    fn stmt(&mut self, s: &Stmt) -> G {
        match s {
            Stmt::Let(p, e) => {
                let t = self.ty(e)?;
                let v = self.expr(e)?;
                let tmp = self.fresh("lt");
                let mut conds = Vec::new();
                let mut binds = Vec::new();
                self.pattern(p, &tmp, &t, &mut conds, &mut binds)?;
                let mut out = format!("__auto_type {tmp} = {v}; ");
                for (n, expr, bt) in binds {
                    let c = self.bind(&n, bt);
                    write!(out, "__auto_type {c} = {expr}; ").unwrap();
                }
                Ok(out)
            }
            Stmt::Var(n, e) => {
                let t = self.ty(e)?;
                let c = self.cty(&t)?;
                let v = self.expr(e)?;
                let var = self.bind(n, t);
                Ok(format!("{c} {var} = {v}; "))
            }
            Stmt::Assign(n, e, _) => {
                let v = self.expr(e)?;
                let (var, _) = self.lookup(n).ok_or("assigns an unknown variable")?;
                Ok(format!("{var} = {v}; "))
            }
            Stmt::While(c, body) => {
                let cv = self.expr(c)?;
                self.scopes.push(HashMap::new());
                let b = self.expr(body);
                self.scopes.pop();
                Ok(format!("while ({cv}) {{ (void)({}); }} ", b?))
            }
            Stmt::For(p, it, body) => {
                if let (Pat::Bind(i), ExprKind::Range(a, b)) = (p, &it.kind) {
                    let (av, bv) = (self.expr(a)?, self.expr(b)?);
                    let (s_, e_) = (self.fresh("fs"), self.fresh("fe"));
                    self.scopes.push(HashMap::new());
                    let iv = self.bind(i, Type::int());
                    let body = self.expr(body);
                    self.scopes.pop();
                    return Ok(format!("{{ int64_t {s_} = {av}; int64_t {e_} = {bv}; for (int64_t {iv} = {s_}; {iv} < {e_}; {iv}++) {{ (void)({}); }} }} ", body?));
                }
                let lt = self.ty(it)?;
                let et = elem(&lt, "List").ok_or("for loop over a non-list")?;
                let lv = self.expr(it)?;
                let (l, i) = (self.fresh("fl"), self.fresh("fi"));
                self.scopes.push(HashMap::new());
                let mut conds = Vec::new();
                let mut binds = Vec::new();
                let item = self.fresh("fx");
                let r = self.pattern(p, &item, &et, &mut conds, &mut binds);
                if let Err(e) = r {
                    self.scopes.pop();
                    return Err(e);
                }
                let mut decl = String::new();
                for (n, expr, bt) in binds {
                    let c = self.bind(&n, bt);
                    write!(decl, "__auto_type {c} = {expr}; ").unwrap();
                }
                let body = self.expr(body);
                self.scopes.pop();
                let cond = if conds.is_empty() { "1".to_string() } else { conds.join(" && ") };
                Ok(format!("{{ __auto_type {l} = {lv}; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {item} = {l}.data[{i}]; if ({cond}) {{ {decl}(void)({}); }} }} }} ", body?))
            }
            Stmt::Expr(e) => Ok(format!("(void)({}); ", self.expr(e)?)),
            Stmt::Fn(_) => Err("defines a local function".into()),
        }
    }

    fn with_expr(&mut self, base: &Expr, ups: &[(Vec<PathSeg>, Expr)], t: &Type) -> G {
        let bv = self.expr(base)?;
        let w = self.fresh("w");
        let mut s = format!("({{ __auto_type {w} = {bv}; ");
        for (path, value) in ups {
            let mut keys = Vec::new();
            for seg in path {
                if let PathSeg::Index(ix) = seg {
                    let v = self.expr(ix)?;
                    let k = self.fresh("wk");
                    write!(s, "int64_t {k} = {v}; ").unwrap();
                    keys.push(Some(k));
                } else {
                    keys.push(None);
                }
            }
            let nv = self.expr(value)?;
            let vv = self.fresh("wv");
            write!(s, "__auto_type {vv} = {nv}; ").unwrap();
            let mut lval = w.clone();
            let mut cur = t.clone();
            let mut records: Vec<(String, String, Vec<(String, Type)>)> = Vec::new();
            for (seg, key) in path.iter().zip(&keys) {
                match (seg, &cur) {
                    (PathSeg::Field(f), Type::Con(n, a)) => {
                        let fs = self.layouts.record_fields(n, a).ok_or("with on a non-record")?;
                        records.push((n.clone(), lval.clone(), fs.clone()));
                        cur = fs.iter().find(|(x, _)| x == f).map(|(_, t)| t.clone()).ok_or("unknown field")?;
                        lval = format!("{lval}.{f}");
                    }
                    (PathSeg::Index(_), _) => {
                        let et = elem(&cur, "List").ok_or("index into a non-list")?;
                        let ec = self.decl(&et)?;
                        let k = key.clone().unwrap();
                        write!(s, "if (UNLIKELY({k} < 0 || {k} >= {lval}.len)) TRAPV({T_INDEX}, {lval}.len, {k}); {{ RawL c_ = raw_copy(TO_RAW({lval}), sizeof({ec})); {lval}.data = ({ec}*)c_.data; {lval}.hdr = c_.hdr; }} ").unwrap();
                        lval = format!("{lval}.data[{k}]");
                        cur = et;
                    }
                    _ => return Err("unsupported with path".into()),
                }
            }
            write!(s, "{lval} = {vv}; ").unwrap();
            for (owner, lv, fs) in records.iter().rev() {
                s.push_str(&self.record_checks(owner, lv, ".", fs)?);
            }
        }
        write!(s, "{w}; }})").unwrap();
        Ok(s)
    }
}

fn has_vars(t: &Type) -> bool {
    match t {
        Type::Var(_) | Type::Param(_) => true,
        Type::Con(_, a) => a.iter().any(has_vars),
        Type::Tuple(xs) => xs.iter().any(has_vars),
        Type::Fn(ps, r, _) => ps.iter().any(has_vars) || has_vars(r),
    }
}

fn kind_name(k: &ExprKind) -> &'static str {
    match k {
        ExprKind::Str(_) => "string",
        ExprKind::Catch(..) => "catch",
        ExprKind::Raise(_) => "raise",
        ExprKind::Lambda { .. } => "standalone lambda",
        ExprKind::Hole(_) => "hole",
        ExprKind::Table(_) => "rule table",
        _ => "unsupported",
    }
}
