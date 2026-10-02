use crate::{assemble, eligible, Compiled};
use sspur_syntax::*;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Command;

const PRELUDE: &str = r#"#include <stdint.h>
typedef struct { int64_t v; int64_t code; } R;
typedef struct { int64_t code, func, clause, value, limit; } Status;
#define UNLIKELY(x) __builtin_expect(!!(x), 0)
#define TRAP(c, fi, cl, val) do { st->code = (c); st->func = (fi); st->clause = (cl); st->value = (val); return (R){0, (c)}; } while (0)
static inline R sspur_pow(int64_t b, int64_t e) {
    if (e < 0) return (R){0, 6};
    int64_t r = 1;
    while (e > 0) {
        if (e & 1) { if (__builtin_mul_overflow(r, b, &r)) return (R){0, 1}; }
        e >>= 1;
        if (e > 0 && __builtin_mul_overflow(b, b, &b)) return (R){0, 1};
    }
    return (R){r, 0};
}
"#;

pub fn compile_release(m: &Module, opt: &str) -> Result<Compiled, String> {
    let (defs, ok, skipped) = eligible(m);
    let index: HashMap<String, usize> = defs.iter().enumerate().map(|(i, f)| (f.name.clone(), i)).collect();
    let native: Vec<&FnDef> = defs.iter().filter(|f| ok.contains(&f.name)).collect();
    let mut src = String::from(PRELUDE);
    for f in &native {
        writeln!(src, "static R f_{}({});", f.name, params_decl(f, true)).unwrap();
    }
    for f in &native {
        let mut g = CGen { fidx: index[&f.name] as i64, scopes: vec![HashMap::new()], counter: 0 };
        src.push_str(&g.function(f));
        writeln!(src, "int64_t sspur_entry_{}({}) {{ return f_{}({}st, -st->limit).v; }}", f.name, params_decl(f, false), f.name, f.params.iter().map(|p| format!("p_{}, ", p.name)).collect::<String>()).unwrap();
    }
    let lib = build(&src, opt)?;
    let library = unsafe { libloading::Library::new(&lib) }.map_err(|e| format!("cannot load {}: {e}", lib.display()))?;
    let mut ptrs = HashMap::new();
    for f in &native {
        let sym = format!("sspur_entry_{}", f.name);
        let p: libloading::Symbol<unsafe extern "C" fn()> = unsafe { library.get(sym.as_bytes()) }.map_err(|e| format!("missing symbol {sym}: {e}"))?;
        ptrs.insert(f.name.clone(), (*p as *const u8, f.params.len()));
    }
    Ok(assemble(defs, ptrs, skipped, Box::new(library)))
}

pub fn c_source(m: &Module) -> String {
    let (defs, ok, _) = eligible(m);
    let index: HashMap<String, usize> = defs.iter().enumerate().map(|(i, f)| (f.name.clone(), i)).collect();
    let mut src = String::from(PRELUDE);
    for f in defs.iter().filter(|f| ok.contains(&f.name)) {
        let mut g = CGen { fidx: index[&f.name] as i64, scopes: vec![HashMap::new()], counter: 0 };
        src.push_str(&g.function(f));
    }
    src
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
    let out = Command::new(&cc)
        .args([opt, "-shared", "-fPIC", "-w", "-o"])
        .arg(&tmp)
        .arg(&c)
        .output()
        .map_err(|e| format!("cannot run {cc}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cc} failed: {}", String::from_utf8_lossy(&out.stderr).lines().take(5).collect::<Vec<_>>().join(" | ")));
    }
    std::fs::rename(&tmp, &lib).map_err(|e| e.to_string())?;
    Ok(lib)
}

fn params_decl(f: &FnDef, internal: bool) -> String {
    let mut ps: Vec<String> = f.params.iter().map(|p| format!("int64_t p_{}", p.name)).collect();
    ps.push("Status* st".into());
    if internal {
        ps.push("int64_t depth".into());
    }
    ps.join(", ")
}

fn lit(n: i64) -> String {
    if n == i64::MIN { "INT64_MIN".into() } else { format!("{n}LL") }
}

struct CGen {
    fidx: i64,
    scopes: Vec<HashMap<String, String>>,
    counter: usize,
}

impl CGen {
    fn fresh(&mut self, base: &str) -> String {
        self.counter += 1;
        format!("v_{base}_{}", self.counter)
    }

    fn bind(&mut self, name: &str) -> String {
        let c = self.fresh(name);
        self.scopes.last_mut().unwrap().insert(name.to_string(), c.clone());
        c
    }

    fn lookup(&self, name: &str) -> String {
        self.scopes.iter().rev().find_map(|s| s.get(name)).cloned().expect("eligibility guarantees bound names")
    }

    fn trap(&self, code: i64, clause: usize, value: &str) -> String {
        format!("TRAP({code}, {}, {clause}, {value});", self.fidx)
    }

    fn function(&mut self, f: &FnDef) -> String {
        let mut s = format!("static R f_{}({}) {{\n", f.name, params_decl(f, true));
        writeln!(s, "  depth += 1;\n  if (UNLIKELY(depth > 0)) {}", self.trap(7, 0, "0")).unwrap();
        for p in &f.params {
            self.scopes.last_mut().unwrap().insert(p.name.clone(), format!("p_{}", p.name));
        }
        for (i, p) in f.params.iter().enumerate() {
            if let Some(r) = &p.refine {
                self.scopes.push(HashMap::from([("_".to_string(), format!("p_{}", p.name))]));
                let c = self.expr(r);
                self.scopes.pop();
                writeln!(s, "  if (UNLIKELY(!({c}))) {}", self.trap(5, i, &format!("p_{}", p.name))).unwrap();
            }
        }
        for (i, pre) in f.pres.iter().enumerate() {
            let c = self.expr(pre);
            writeln!(s, "  if (UNLIKELY(!({c}))) {}", self.trap(3, i, "0")).unwrap();
        }
        let body = self.expr(&f.body);
        writeln!(s, "  int64_t ret_;\n  ret_ = {body};\n  goto done_;\ndone_: ;").unwrap();
        if !f.posts.is_empty() {
            self.scopes.push(HashMap::from([("r".to_string(), "ret_".to_string())]));
            for (i, post) in f.posts.iter().enumerate() {
                let c = self.expr(post);
                writeln!(s, "  if (UNLIKELY(!({c}))) {}", self.trap(4, i, "ret_")).unwrap();
            }
            self.scopes.pop();
        }
        s.push_str("  return (R){ret_, 0};\n}\n");
        s
    }

    fn expr(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Int(n) => lit(*n),
            ExprKind::Bool(b) => (if *b { "1LL" } else { "0LL" }).into(),
            ExprKind::Unit => "0LL".into(),
            ExprKind::Name(n) => self.lookup(n),
            ExprKind::Placeholder => self.lookup("_"),
            ExprKind::Unary(UnOp::Neg, x) => {
                let v = self.expr(x);
                format!("({{ int64_t t_ = {v}; if (UNLIKELY(t_ == INT64_MIN)) {} -t_; }})", self.trap(1, 0, "0"))
            }
            ExprKind::Unary(UnOp::Not, x) => format!("((int64_t)!({}))", self.expr(x)),
            ExprKind::Binary(BinOp::And, a, b) => format!("((int64_t)(({}) && ({})))", self.expr(a), self.expr(b)),
            ExprKind::Binary(BinOp::Or, a, b) => format!("((int64_t)(({}) || ({})))", self.expr(a), self.expr(b)),
            ExprKind::Binary(op, a, b) => {
                let (x, y) = (self.expr(a), self.expr(b));
                let ov = self.trap(1, 0, "0");
                let body = match op {
                    BinOp::Add => format!("int64_t r_; if (UNLIKELY(__builtin_add_overflow(a_, b_, &r_))) {ov} r_;"),
                    BinOp::Sub => format!("int64_t r_; if (UNLIKELY(__builtin_sub_overflow(a_, b_, &r_))) {ov} r_;"),
                    BinOp::Mul => format!("int64_t r_; if (UNLIKELY(__builtin_mul_overflow(a_, b_, &r_))) {ov} r_;"),
                    BinOp::Div | BinOp::Rem => {
                        let sym = if *op == BinOp::Div { "/" } else { "%" };
                        format!("if (UNLIKELY(b_ == 0)) {} if (UNLIKELY(a_ == INT64_MIN && b_ == -1)) {ov} a_ {sym} b_;", self.trap(2, 0, "0"))
                    }
                    BinOp::Pow => format!("R p_ = sspur_pow(a_, b_); if (UNLIKELY(p_.code)) TRAP(p_.code, {}, 0, 0); p_.v;", self.fidx),
                    cmp => format!("(int64_t)(a_ {} b_);", cmp.symbol()),
                };
                format!("({{ int64_t a_ = {x}; int64_t b_ = {y}; {body} }})")
            }
            ExprKind::If(c, t, f) => {
                let cv = self.expr(c);
                let tv = self.expr(t);
                let fv = f.as_ref().map_or("0LL".to_string(), |f| self.expr(f));
                format!("(({cv}) ? ({tv}) : ({fv}))")
            }
            ExprKind::Call(f, args) => {
                let ExprKind::Name(n) = &f.kind else { unreachable!() };
                let mut s = String::from("({ ");
                let mut names = Vec::new();
                for a in args {
                    let v = self.expr(a);
                    let t = self.fresh("arg");
                    write!(s, "int64_t {t} = {v}; ").unwrap();
                    names.push(t);
                }
                let call_args: String = names.iter().map(|n| format!("{n}, ")).collect();
                write!(s, "R c_ = f_{n}({call_args}st, depth); if (UNLIKELY(c_.code)) return (R){{0, c_.code}}; c_.v; }})").unwrap();
                s
            }
            ExprKind::Return(x) => format!("({{ ret_ = {}; goto done_; 0LL; }})", self.expr(x)),
            ExprKind::Block(stmts) => {
                self.scopes.push(HashMap::new());
                let mut s = String::from("({ ");
                let n = stmts.len();
                for (i, st) in stmts.iter().enumerate() {
                    let last = i + 1 == n;
                    match st {
                        Stmt::Expr(x) => {
                            let v = self.expr(x);
                            if last {
                                write!(s, "{v}; ").unwrap();
                            } else {
                                write!(s, "(void)({v}); ").unwrap();
                            }
                        }
                        other => {
                            s.push_str(&self.stmt(other));
                            if last {
                                s.push_str("0LL; ");
                            }
                        }
                    }
                }
                self.scopes.pop();
                s.push_str("})");
                s
            }
            _ => unreachable!("eligibility check rejects this expression"),
        }
    }

    fn stmt(&mut self, s: &Stmt) -> String {
        match s {
            Stmt::Let(Pat::Bind(n), e) | Stmt::Var(n, e) => {
                let v = self.expr(e);
                let c = self.bind(n);
                format!("int64_t {c} = {v}; ")
            }
            Stmt::Let(_, e) => format!("(void)({}); ", self.expr(e)),
            Stmt::Assign(n, e, _) => {
                let v = self.expr(e);
                format!("{} = {v}; ", self.lookup(n))
            }
            Stmt::While(c, body) => {
                let cv = self.expr(c);
                self.scopes.push(HashMap::new());
                let b = self.expr(body);
                self.scopes.pop();
                format!("while ({cv}) {{ (void)({b}); }} ")
            }
            Stmt::For(Pat::Bind(i), it, body) => {
                let ExprKind::Range(a, b) = &it.kind else { unreachable!() };
                let (av, bv) = (self.expr(a), self.expr(b));
                let (s_, e_) = (self.fresh("start"), self.fresh("end"));
                self.scopes.push(HashMap::new());
                let iv = self.bind(i);
                let body = self.expr(body);
                self.scopes.pop();
                format!("{{ int64_t {s_} = {av}; int64_t {e_} = {bv}; for (int64_t {iv} = {s_}; {iv} < {e_}; {iv}++) {{ (void)({body}); }} }} ")
            }
            Stmt::Expr(e) => format!("(void)({}); ", self.expr(e)),
            _ => unreachable!("eligibility check rejects this statement"),
        }
    }
}
