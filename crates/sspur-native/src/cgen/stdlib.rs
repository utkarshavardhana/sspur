use super::*;

const STD_RT: &str = include_str!("std_rt.c");

pub(super) const STD_CONS: &[&str] = &["#Set", "#Heap", "#StrBuf", "Res"];

fn section(key: &str) -> (Vec<&'static str>, &'static str) {
    for part in STD_RT.split("//@ ").skip(1) {
        let (head, body) = part.split_once('\n').unwrap_or((part, ""));
        let mut words = head.split_whitespace();
        if words.next() == Some(key) {
            return (words.collect(), body);
        }
    }
    panic!("missing std runtime section {key}")
}

pub(super) fn is_std(t: &Type) -> bool {
    matches!(t, Type::Con(n, _) if STD_CONS.contains(&n.as_str()))
}

fn set_map(t: &Type) -> Type {
    Type::app("Map", vec![t.clone(), Type::unit()])
}

fn arg0(t: &Type) -> Type {
    match t {
        Type::Con(_, a) if !a.is_empty() => a[0].clone(),
        _ => Type::unit(),
    }
}

fn arg1(t: &Type) -> Type {
    match t {
        Type::Con(_, a) if a.len() > 1 => a[1].clone(),
        _ => Type::unit(),
    }
}

impl Cx<'_> {
    pub(super) fn std(&mut self, key: &str) {
        if !self.helpers_done.insert(format!("std:{key}")) {
            return;
        }
        let (deps, body) = section(key);
        for d in deps {
            self.std(d);
        }
        self.protos.push_str(body);
    }

    pub(super) fn std_mangle(&self, n: &str, a: &[Type]) -> G {
        Ok(match n {
            "#Set" => format!("ST_{}", self.mangle(&a[0])?),
            "#Heap" => format!("HP_{}", self.mangle(&a[0])?),
            "#StrBuf" => "BU".into(),
            _ => format!("RS_{}_{}", self.mangle(&a[0])?, self.mangle(&a[1])?),
        })
    }

    pub(super) fn std_cty(&mut self, t: &Type, m: &str) -> G {
        let Type::Con(n, a) = t else { return Err("bad std type".into()) };
        match n.as_str() {
            "#Set" => self.cty(&set_map(&a[0])),
            "#StrBuf" => {
                if self.helpers_done.insert("sbuf_type".into()) {
                    writeln!(self.defs, "typedef struct {{ int64_t len; char* data; int64_t* hdr; }} SBuf;").unwrap();
                }
                Ok("SBuf".into())
            }
            "#Heap" => {
                self.fwd_decl(m);
                if self.complete.insert(m.to_string()) {
                    self.cty(&a[0])?;
                    let node = format!("HN_{m}");
                    self.fwd_decl(&node);
                    writeln!(self.defs, "struct {m} {{ {node}* root; }};").unwrap();
                }
                Ok(m.to_string())
            }
            _ => {
                self.fwd_decl(m);
                if !self.complete.contains(m) {
                    let (ac, ec) = (self.cty(&a[0])?, self.cty(&a[1])?);
                    if self.complete.insert(m.to_string()) {
                        writeln!(self.defs, "struct {m} {{ int64_t ok; {ac} v; {ec} e; }};").unwrap();
                    }
                }
                Ok(m.to_string())
            }
        }
    }

    pub(super) fn heap_helpers(&mut self, t: &Type) -> G<(String, String)> {
        let et = arg0(t);
        let hm = self.mangle(t)?;
        let node = format!("HN_{hm}");
        self.cty(t)?;
        if self.helpers_done.insert(format!("heap_{hm}")) {
            let ec = self.cty(&et)?;
            let c = self.helper_cmp(&et)?;
            writeln!(self.defs, "struct {node} {{ {ec} v; int64_t rk, sz; {node}* l; {node}* r; }};").unwrap();
            writeln!(self.protos, "static {node}* hmerge_{hm}({node}* a, {node}* b);").unwrap();
            writeln!(self.protos, "static {node}* hpush_{hm}({node}* t, {ec} v);").unwrap();
            writeln!(self.protos, "static RawL hsorted_{hm}({node}* t);").unwrap();
            let h = &mut self.helpers;
            writeln!(h, "static inline int64_t hrk_{hm}({node}* t) {{ return t ? t->rk : 0; }}").unwrap();
            writeln!(h, "static inline int64_t hsz_{hm}({node}* t) {{ return t ? t->sz : 0; }}").unwrap();
            writeln!(h, "static {node}* hmerge_{hm}({node}* a, {node}* b) {{ if (!a) return b; if (!b) return a; if ({c}(b->v, a->v) < 0) {{ {node}* t = a; a = b; b = t; }} {node}* n = ({node}*)sspur_alloc(sizeof({node})); *n = *a; n->r = hmerge_{hm}(a->r, b); if (hrk_{hm}(n->l) < hrk_{hm}(n->r)) {{ {node}* t = n->l; n->l = n->r; n->r = t; }} n->rk = hrk_{hm}(n->r) + 1; n->sz = hsz_{hm}(n->l) + hsz_{hm}(n->r) + 1; return n; }}").unwrap();
            writeln!(h, "static {node}* hpush_{hm}({node}* t, {ec} v) {{ {node}* n = ({node}*)sspur_alloc(sizeof({node})); n->v = v; n->rk = 1; n->sz = 1; n->l = n->r = 0; return hmerge_{hm}(t, n); }}").unwrap();
            writeln!(h, "static RawL hsorted_{hm}({node}* t) {{ int64_t n = hsz_{hm}(t); RawL r = raw_alloc(n, sizeof({ec})); {ec}* d = ({ec}*)r.data; int64_t k = 0, sp = 0; {node}** stk = ({node}**)malloc((size_t)(n + 1) * sizeof({node}*)); if (t) stk[sp++] = t; while (sp) {{ {node}* x = stk[--sp]; d[k++] = x->v; if (x->l) stk[sp++] = x->l; if (x->r) stk[sp++] = x->r; }} free(stk); r.len = n; r.hdr[1] = n; if (n > 1) raw_msort((char*)d, n, sizeof({ec}), {c}_p, (char*)sspur_alloc((size_t)n * sizeof({ec}))); return r; }}").unwrap();
        }
        Ok((hm, node))
    }

    fn set_parts(&mut self, t: &Type) -> G<(String, String, String)> {
        let mt = set_map(&arg0(t));
        let (mm, node) = self.map_helpers(&mt)?;
        let sc = self.cty(t)?;
        Ok((mm, node, sc))
    }

    fn fill_keys(&mut self, mm: &str, node: &str, root: &str) -> String {
        format!("int64_t n = sz_{mm}({root}); {node}** xs = ({node}**)sspur_alloc((size_t)(n + 1) * sizeof({node}*)); int64_t c = 0; fill_{mm}({root}, xs, &c); ")
    }

    pub(super) fn std_cmp_body(&mut self, t: &Type) -> G {
        let Type::Con(n, a) = t else { return Err("bad std type".into()) };
        Ok(match n.as_str() {
            "#Set" => {
                let (mm, node, _) = self.set_parts(t)?;
                let kc = self.helper_cmp(&a[0])?;
                format!("int64_t na = sz_{mm}(a.root), nb = sz_{mm}(b.root); {node}** xa = ({node}**)sspur_alloc((size_t)(na + 1) * sizeof({node}*)); {node}** xb = ({node}**)sspur_alloc((size_t)(nb + 1) * sizeof({node}*)); int64_t ca = 0, cb = 0; fill_{mm}(a.root, xa, &ca); fill_{mm}(b.root, xb, &cb); int64_t n = na < nb ? na : nb; for (int64_t i = 0; i < n; i++) {{ int c = {kc}(xa[i]->k, xb[i]->k); if (c) return c; }} return cmp_I(na, nb);")
            }
            "#Heap" => {
                let (hm, _) = self.heap_helpers(t)?;
                let ec = self.cty(&a[0])?;
                let c = self.helper_cmp(&a[0])?;
                format!("RawL xa = hsorted_{hm}(a.root), xb = hsorted_{hm}(b.root); int64_t n = xa.len < xb.len ? xa.len : xb.len; for (int64_t i = 0; i < n; i++) {{ int c = {c}((({ec}*)xa.data)[i], (({ec}*)xb.data)[i]); if (c) return c; }} return cmp_I(xa.len, xb.len);")
            }
            "#StrBuf" => "return cmp_S((Str){a.len, a.data}, (Str){b.len, b.data});".into(),
            _ => {
                let (ca, ce) = (self.helper_cmp(&a[0])?, self.helper_cmp(&a[1])?);
                format!("if (a.ok != b.ok) return a.ok ? -1 : 1; return a.ok ? {ca}(a.v, b.v) : {ce}(a.e, b.e);")
            }
        })
    }

    pub(super) fn std_enc_body(&mut self, t: &Type) -> G {
        let Type::Con(n, a) = t else { return Err("bad std type".into()) };
        Ok(match n.as_str() {
            "#Set" => {
                let (mm, node, _) = self.set_parts(t)?;
                let e = self.helper_enc(&a[0])?;
                format!("{}buf_push(b, n); for (int64_t i = 0; i < n; i++) {e}(b, xs[i]->k);", self.fill_keys(&mm, &node, "v.root"))
            }
            "#Heap" => {
                let (hm, _) = self.heap_helpers(t)?;
                let ec = self.cty(&a[0])?;
                let e = self.helper_enc(&a[0])?;
                format!("RawL x = hsorted_{hm}(v.root); buf_push(b, x.len); for (int64_t i = 0; i < x.len; i++) {e}(b, (({ec}*)x.data)[i]);")
            }
            "#StrBuf" => "buf_push(b, v.len); for (int64_t i = 0; i < v.len; i += 8) { int64_t w = 0; memcpy(&w, v.data + i, (size_t)(v.len - i < 8 ? v.len - i : 8)); buf_push(b, w); }".into(),
            _ => {
                let (ea, ee) = (self.helper_enc(&a[0])?, self.helper_enc(&a[1])?);
                format!("buf_push(b, v.ok); if (v.ok) {ea}(b, v.v); else {ee}(b, v.e);")
            }
        })
    }

    pub(super) fn std_dec_body(&mut self, t: &Type, c: &str) -> G {
        let Type::Con(n, a) = t else { return Err("bad std type".into()) };
        Ok(match n.as_str() {
            "#Set" => {
                let (mm, _, _) = self.set_parts(t)?;
                let d = self.helper_dec(&a[0])?;
                format!("int64_t n = *(*p)++; {c} s = {{0}}; for (int64_t i = 0; i < n; i++) {{ __auto_type k = {d}(p); s.root = mput_{mm}(s.root, k, 0, sspur_prio()); }} return s;")
            }
            "#Heap" => {
                let (hm, _) = self.heap_helpers(t)?;
                let d = self.helper_dec(&a[0])?;
                format!("int64_t n = *(*p)++; {c} h = {{0}}; for (int64_t i = 0; i < n; i++) {{ __auto_type x = {d}(p); h.root = hpush_{hm}(h.root, x); }} return h;")
            }
            "#StrBuf" => "int64_t n = *(*p)++; char* s = (char*)sspur_alloc_atomic((size_t)n + 1); for (int64_t i = 0; i < n; i += 8) { int64_t w = *(*p)++; memcpy(s + i, &w, (size_t)(n - i < 8 ? n - i : 8)); } return (SBuf){n, s, 0};".into(),
            _ => {
                let (da, de) = (self.helper_dec(&a[0])?, self.helper_dec(&a[1])?);
                format!("{c} r; memset(&r, 0, sizeof r); r.ok = *(*p)++; if (r.ok) r.v = {da}(p); else r.e = {de}(p); return r;")
            }
        })
    }

    pub(super) fn std_show_body(&mut self, t: &Type) -> G {
        let Type::Con(n, a) = t else { return Err("bad std type".into()) };
        Ok(match n.as_str() {
            "#Set" => {
                let (mm, node, _) = self.set_parts(t)?;
                let sk = self.helper_show(&a[0])?;
                format!("{}sb_put(b, \"{{\", 1); for (int64_t i = 0; i < n; i++) {{ if (i) sb_put(b, \", \", 2); {sk}(b, xs[i]->k, 1); }} sb_put(b, \"}}\", 1);", self.fill_keys(&mm, &node, "v.root"))
            }
            "#Heap" => {
                let (hm, _) = self.heap_helpers(t)?;
                let ec = self.cty(&a[0])?;
                let se = self.helper_show(&a[0])?;
                format!("RawL x = hsorted_{hm}(v.root); sb_put(b, \"heap[\", 5); for (int64_t i = 0; i < x.len; i++) {{ if (i) sb_put(b, \", \", 2); {se}(b, (({ec}*)x.data)[i], 1); }} sb_put(b, \"]\", 1);")
            }
            "#StrBuf" => "if (q) sb_strq(b, (Str){v.len, v.data}); else sb_put(b, v.data, v.len);".into(),
            _ => {
                let (sa, se) = (self.helper_show(&a[0])?, self.helper_show(&a[1])?);
                format!("if (v.ok) {{ sb_put(b, \"ok(\", 3); {sa}(b, v.v, 1); }} else {{ sb_put(b, \"err(\", 4); {se}(b, v.e, 1); }} sb_put(b, \")\", 1);")
            }
        })
    }

    pub(super) fn std_hash_body(&mut self, t: &Type) -> G {
        let Type::Con(n, a) = t else { return Err("bad std type".into()) };
        Ok(match n.as_str() {
            "#Set" => {
                let (mm, node, _) = self.set_parts(t)?;
                let hk = self.helper_hash(&a[0])?;
                format!("{}for (int64_t i = 0; i < n; i++) h = hmix(h * 31 + {hk}(xs[i]->k)); h = hmix(h ^ (uint64_t)n);", self.fill_keys(&mm, &node, "v.root"))
            }
            "#Heap" => {
                let (hm, _) = self.heap_helpers(t)?;
                let ec = self.cty(&a[0])?;
                let he = self.helper_hash(&a[0])?;
                format!("RawL x = hsorted_{hm}(v.root); for (int64_t i = 0; i < x.len; i++) h = hmix(h * 31 + {he}((({ec}*)x.data)[i])); h = hmix(h ^ (uint64_t)x.len);")
            }
            "#StrBuf" => "h = hash_S((Str){v.len, v.data});".into(),
            _ => {
                let (ha, he) = (self.helper_hash(&a[0])?, self.helper_hash(&a[1])?);
                format!("h = v.ok ? hmix(5 + {ha}(v.v)) : hmix(9 + {he}(v.e));")
            }
        })
    }

    pub(super) fn res_pattern(&mut self, name: &str, args: &CtorArgs, v: &str, t: &Type, conds: &mut Vec<String>, binds: &mut Vec<(String, String, Type)>) -> G<()> {
        match (name, args) {
            ("ok", CtorArgs::Positional(ps)) => {
                conds.push(format!("{v}.ok"));
                self.pattern(&ps[0], &format!("{v}.v"), &arg0(t), conds, binds)
            }
            ("err", CtorArgs::Positional(ps)) => {
                conds.push(format!("!{v}.ok"));
                self.pattern(&ps[0], &format!("{v}.e"), &arg1(t), conds, binds)
            }
            _ => Err("unsupported result pattern".into()),
        }
    }

    pub(super) fn std_global(&mut self, n: &str, args: &[Expr], t: &Type) -> G<Option<String>> {
        if self.lookup(n).is_some() || self.check.fn_types.contains_key(n) {
            return Ok(None);
        }
        let mut vals = Vec::new();
        let known = matches!(n, "ok" | "err") || sspur_check::STD_GLOBAL_NAMES.contains(&n);
        if !known {
            return Ok(None);
        }
        for a in args {
            vals.push(self.expr(a)?);
        }
        let v = |i: usize| vals[i].clone();
        let c = self.cty(t)?;
        let res = |ok: &str, err: &str| format!("{c} r_; memset(&r_, 0, sizeof r_); if ({ok}) r_.ok = 1; else r_.e = {err}; r_; ");
        Ok(Some(match n {
            "ok" => format!("({{ __auto_type v_ = {}; {c} r_; memset(&r_, 0, sizeof r_); r_.ok = 1; r_.v = v_; r_; }})", v(0)),
            "err" => format!("({{ __auto_type v_ = {}; {c} r_; memset(&r_, 0, sizeof r_); r_.e = v_; r_; }})", v(0)),
            "empty_set" | "empty_heap" => format!("(({c}){{0}})"),
            "str_buf" => "((SBuf){0, 0, 0})".into(),
            "range" => {
                self.std("fail");
                format!("({{ int64_t s_ = {}; int64_t e_ = {}; int64_t k_ = {}; if (UNLIKELY(k_ == 0)) ss_fail(st, \"range step must not be 0\"); __int128 n_ = 0; if (k_ > 0 && e_ > s_) n_ = ((__int128)e_ - s_ - 1) / k_ + 1; else if (k_ < 0 && e_ < s_) n_ = ((__int128)s_ - e_ - 1) / -(__int128)k_ + 1; if (UNLIKELY(n_ > ((__int128)1 << 26))) TRAPV({T_OOM}, 0, 0); RawL r_ = raw_alloc((int64_t)n_, 8); int64_t* d_ = (int64_t*)r_.data; for (int64_t i_ = 0; i_ < (int64_t)n_; i_++) d_[i_] = (int64_t)(s_ + (__int128)i_ * k_); r_.len = (int64_t)n_; r_.hdr[1] = r_.len; ({c}){{r_.len, d_, r_.hdr}}; }})", v(0), v(1), v(2))
            }
            "clamp" => {
                let cmp = self.helper_cmp(t)?;
                format!("({{ __auto_type x_ = {}; __auto_type lo_ = {}; __auto_type hi_ = {}; {cmp}(x_, lo_) < 0 ? lo_ : {cmp}(x_, hi_) > 0 ? hi_ : x_; }})", v(0), v(1), v(2))
            }
            "rand" | "rand_int" | "rand_f64" => {
                let mix = "uint64_t m_ = z_; m_ = (m_ ^ (m_ >> 30)) * 0xBF58476D1CE4E5B9ULL; m_ = (m_ ^ (m_ >> 27)) * 0x94D049BB133111EBULL; m_ ^= m_ >> 31; ";
                let head = format!("uint64_t z_ = (uint64_t)({}) + 0x9E3779B97F4A7C15ULL; ", v(0));
                match n {
                    "rand" => format!("({{ {head}{mix}({c}){{(int64_t)(m_ >> 1), (int64_t)z_}}; }})"),
                    "rand_f64" => format!("({{ {head}{mix}({c}){{(double)(m_ >> 11) * (1.0 / 9007199254740992.0), (int64_t)z_}}; }})"),
                    _ => {
                        self.std("fail");
                        format!("({{ {head}int64_t lo_ = {}; int64_t hi_ = {}; if (UNLIKELY(lo_ >= hi_)) ss_fail(st, \"rand_int needs lo < hi\"); {mix}({c}){{(int64_t)((__int128)lo_ + (__int128)(((unsigned __int128)m_ * (unsigned __int128)((__int128)hi_ - lo_)) >> 64)), (int64_t)z_}}; }})", v(1), v(2))
                    }
                }
            }
            "from_bytes" | "from_codes" => {
                self.std("strx");
                format!("({{ __auto_type b_ = {}; Str o_; {c} r_ = {{0}}; if (ss_{n}(b_.data, b_.len, &o_)) {{ r_.some = 1; r_.v = o_; }} r_; }})", v(0))
            }
            "read_file" => {
                self.std("fs");
                format!("({{ Str p_ = {}; Str o_ = {{0, 0}}, e_ = {{0, 0}}; int k_ = ss_read_file(p_, &o_, &e_); {}}})", v(0), res("k_", "e_").replace("r_.ok = 1;", "{ r_.ok = 1; r_.v = o_; }"))
            }
            "write_file" | "append_file" => {
                self.std("fs");
                format!("({{ Str p_ = {}; Str s_ = {}; Str e_ = {{0, 0}}; int k_ = ss_write_file(p_, s_, {}, &e_); {}}})", v(0), v(1), i32::from(n == "append_file"), res("k_", "e_"))
            }
            "remove_file" => {
                self.std("fs");
                format!("({{ Str p_ = {}; Str e_ = {{0, 0}}; int k_ = ss_remove_file(p_, &e_); {}}})", v(0), res("k_", "e_"))
            }
            "list_dir" => {
                self.std("fs");
                let lt = arg0(t);
                let lc = self.cty(&lt)?;
                format!("({{ Str p_ = {}; RawL o_; Str e_ = {{0, 0}}; int k_ = ss_list_dir(p_, &o_, &e_); {}}})", v(0), res("k_", "e_").replace("r_.ok = 1;", &format!("{{ r_.ok = 1; r_.v = ({lc}){{o_.len, (Str*)o_.data, o_.hdr}}; }}")))
            }
            "read_line" => {
                self.std("io");
                format!("({{ Str o_; {c} r_ = {{0}}; if (ss_read_line(&o_, st)) {{ r_.some = 1; r_.v = o_; }} r_; }})")
            }
            "read_lines" => {
                self.std("io");
                format!("({{ RawL r_ = raw_alloc(8, sizeof(Str)); Str o_; while (ss_read_line(&o_, st)) r_ = raw_push(r_, &o_, sizeof(Str)); ({c}){{r_.len, (Str*)r_.data, r_.hdr}}; }})")
            }
            "now_ms" | "mono_ns" => {
                self.std("time");
                format!("ss_{n}()")
            }
            "sleep_ms" => {
                self.std("time");
                format!("({{ ss_sleep_ms({}); 0LL; }})", v(0))
            }
            "env_var" => {
                self.std("env");
                format!("({{ Str k_ = {}; Str o_; {c} r_ = {{0}}; if (ss_env(k_, &o_)) {{ r_.some = 1; r_.v = o_; }} r_; }})", v(0))
            }
            "args" => {
                self.std("env");
                format!("({{ RawL r_ = raw_alloc(ss_argc, sizeof(Str)); for (int64_t i_ = 0; i_ < ss_argc; i_++) ((Str*)r_.data)[i_] = ss_argv[i_]; r_.len = ss_argc; r_.hdr[1] = ss_argc; ({c}){{r_.len, (Str*)r_.data, r_.hdr}}; }})")
            }
            "pi" => "3.141592653589793".into(),
            "euler" => "2.718281828459045".into(),
            "inf" => "__builtin_inf()".into(),
            "nan" => "bitsd(0x7ff8000000000000LL)".into(),
            _ => return Err(format!("uses {n}")),
        }))
    }

    pub(super) fn std_method(&mut self, r: &str, rt: &Type, name: &str, args: &[Expr], t: &Type) -> G {
        let Type::Con(n, _) = rt else { return Err("bad receiver".into()) };
        let vals: Vec<String> = if name == "map" { vec![] } else { args.iter().map(|a| self.expr(a)).collect::<G<_>>()? };
        let et = arg0(rt);
        match n.as_str() {
            "#Set" => {
                let (mm, node, sc) = self.set_parts(rt)?;
                let s = self.fresh("sv");
                let head = format!("__auto_type {s} = {r}; ");
                Ok(match name {
                    "add" => format!("({{ {head}__auto_type k_ = {}; ({sc}){{find_{mm}({s}.root, k_) ? {s}.root : put_{mm}({s}.root, k_, 0, sspur_prio())}}; }})", vals[0]),
                    "remove" => format!("({{ {head}__auto_type k_ = {}; ({sc}){{find_{mm}({s}.root, k_) ? del_{mm}({s}.root, k_) : {s}.root}}; }})", vals[0]),
                    "has" => format!("({{ {head}(int64_t)(find_{mm}({s}.root, {}) != 0); }})", vals[0]),
                    "len" => format!("sz_{mm}(({r}).root)"),
                    "is_empty" => format!("((int64_t)(({r}).root == 0))"),
                    "items" => {
                        let lc = self.cty(t)?;
                        let ec = self.cty(&et)?;
                        format!("({{ {head}{}RawL r_ = raw_alloc(n, sizeof({ec})); {ec}* d_ = ({ec}*)r_.data; for (int64_t i_ = 0; i_ < n; i_++) d_[i_] = xs[i_]->k; r_.hdr[1] = n; ({lc}){{n, d_, r_.hdr}}; }})", self.fill_keys(&mm, &node, &format!("{s}.root")))
                    }
                    "union" => format!("({{ {head}__auto_type o_ = {}; {node}* big_ = {s}.root; {node}* small_ = o_.root; if (sz_{mm}(small_) > sz_{mm}(big_)) {{ big_ = o_.root; small_ = {s}.root; }} {}for (int64_t i_ = 0; i_ < n; i_++) if (!find_{mm}(big_, xs[i_]->k)) big_ = put_{mm}(big_, xs[i_]->k, 0, sspur_prio()); ({sc}){{big_}}; }})", vals[0], self.fill_keys(&mm, &node, "small_")),
                    "inter" | "diff" => {
                        let keep = if name == "inter" { "" } else { "!" };
                        format!("({{ {head}__auto_type o_ = {}; {}{node}* t_ = 0; for (int64_t i_ = 0; i_ < n; i_++) if ({keep}find_{mm}(o_.root, xs[i_]->k)) t_ = mput_{mm}(t_, xs[i_]->k, 0, sspur_prio()); ({sc}){{t_}}; }})", vals[0], self.fill_keys(&mm, &node, &format!("{s}.root")))
                    }
                    "min" | "max" => {
                        let oc = self.cty(t)?;
                        let side = if name == "min" { "l" } else { "r" };
                        format!("({{ {head}{node}* t_ = {s}.root; {oc} o_ = {{0}}; if (t_) {{ while (t_->{side}) t_ = t_->{side}; o_.some = 1; o_.v = t_->k; }} o_; }})")
                    }
                    _ => return Err(format!("uses Set.{name}")),
                })
            }
            "#Heap" => {
                let (hm, _) = self.heap_helpers(rt)?;
                let hc = self.cty(rt)?;
                let h = self.fresh("hv");
                let head = format!("__auto_type {h} = {r}; ");
                Ok(match name {
                    "push" => format!("({{ {head}__auto_type x_ = {}; ({hc}){{hpush_{hm}({h}.root, x_)}}; }})", vals[0]),
                    "pop" => {
                        let oc = self.cty(t)?;
                        format!("({{ {head}{oc} o_ = {{0}}; if ({h}.root) {{ o_.some = 1; o_.v.f0 = {h}.root->v; o_.v.f1 = ({hc}){{hmerge_{hm}({h}.root->l, {h}.root->r)}}; }} o_; }})")
                    }
                    "peek" => {
                        let oc = self.cty(t)?;
                        format!("({{ {head}{oc} o_ = {{0}}; if ({h}.root) {{ o_.some = 1; o_.v = {h}.root->v; }} o_; }})")
                    }
                    "len" => format!("hsz_{hm}(({r}).root)"),
                    "is_empty" => format!("((int64_t)(({r}).root == 0))"),
                    "items" => {
                        let lc = self.cty(t)?;
                        let ec = self.cty(&et)?;
                        format!("({{ RawL r_ = hsorted_{hm}(({r}).root); ({lc}){{r_.len, ({ec}*)r_.data, r_.hdr}}; }})")
                    }
                    _ => return Err(format!("uses Heap.{name}")),
                })
            }
            "#StrBuf" => Ok(match name {
                "add" => {
                    self.std("sbuf");
                    format!("({{ SBuf b_ = {r}; Str s_ = {}; sbuf_add(b_, s_); }})", vals[0])
                }
                "byte_len" => format!("(({r}).len)"),
                _ => return Err(format!("uses StrBuf.{name}")),
            }),
            _ => Ok(match name {
                "is_ok" => format!("(({r}).ok)"),
                "is_err" => format!("((int64_t)!({r}).ok)"),
                "or" => format!("({{ __auto_type r_ = {r}; __auto_type d_ = {}; r_.ok ? r_.v : d_; }})", vals[0]),
                "map" => {
                    let x = self.fresh("rm");
                    let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                    let oc = self.cty(t)?;
                    format!("({{ __auto_type r_ = {r}; {oc} o_; memset(&o_, 0, sizeof o_); if (r_.ok) {{ __auto_type {x} = r_.v; o_.ok = 1; o_.v = {body}; }} else o_.e = r_.e; o_; }})")
                }
                "get" => {
                    let errt = arg1(rt);
                    let rv = self.fresh("rv");
                    let ev = self.fresh("re");
                    let z = self.zero(&et)?;
                    let raise = self.raise_code(&ev, &errt, &z)?;
                    format!("({{ __auto_type {rv} = {r}; if (!{rv}.ok) {{ __auto_type {ev} = {rv}.e; (void)({raise}); }} {rv}.v; }})")
                }
                _ => return Err(format!("uses Res.{name}")),
            }),
        }
    }

    pub(super) fn std_str(&mut self, r: &str, name: &str, args: &[Expr], t: &Type) -> G {
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let c = self.cty(t)?;
        Ok(match name {
            "split_once" | "index_of" => {
                let fill = if name == "split_once" { "o_.v.f0 = (Str){k_, s_.p}; o_.v.f1 = (Str){s_.len - k_ - p_.len, s_.p + k_ + p_.len};" } else { "o_.v = utf8_len((Str){k_, s_.p});" };
                format!("({{ Str s_ = {r}; Str p_ = {}; int64_t k_ = str_find(s_, p_, 0); {c} o_ = {{0}}; if (k_ >= 0) {{ o_.some = 1; {fill} }} o_; }})", vals[0])
            }
            "pad_left" | "pad_right" => {
                self.std("strx");
                format!("({{ Str s_ = {r}; int64_t n_ = {}; Str f_ = {}; ss_pad(s_, n_, f_, {}, st); }})", vals[0], vals[1], i32::from(name == "pad_left"))
            }
            "to_f64" => {
                self.std("fmt");
                format!("({{ double d_; {c} o_ = {{0}}; if (ss_to_f64({r}, &d_)) {{ o_.some = 1; o_.v = d_; }} o_; }})")
            }
            "bytes" | "codes" => {
                self.std("strx");
                format!("({{ RawL r_ = ss_{name}({r}); ({c}){{r_.len, (int64_t*)r_.data, r_.hdr}}; }})")
            }
            _ => return Err(format!("uses Str.{name}")),
        })
    }

    pub(super) fn std_float(&mut self, r: &str, name: &str, args: &[Expr]) -> G {
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let one = |f: &str| format!("{f}({r})");
        Ok(match name {
            "fmt" => {
                self.std("fmt");
                format!("({{ double x_ = {r}; int64_t d_ = {}; ss_fmt_fixed(x_, d_, st); }})", vals[0])
            }
            "pow" | "atan2" | "hypot" => format!("({{ double x_ = {r}; double y_ = {}; {name}(x_, y_); }})", vals[0]),
            "exp" | "log2" | "log10" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" => one(name),
            "ln" => one("log"),
            "ceil" | "trunc" => format!("f2i({name}({r}))"),
            "is_nan" => format!("({{ double x_ = {r}; (int64_t)(x_ != x_); }})"),
            "is_finite" => format!("((int64_t)isfinite({r}))"),
            "is_inf" => format!("((int64_t)isinf({r}))"),
            "sinh" | "cosh" | "tanh" | "asinh" | "acosh" | "atanh" | "cbrt" | "exp2" | "expm1" | "log1p" | "erf" | "erfc" | "lgamma" => one(name),
            "gamma" => one("tgamma"),
            "fmod" | "remainder" | "copysign" | "nextafter" | "fdim" => format!("({{ double x_ = {r}; double y_ = {}; {name}(x_, y_); }})", vals[0]),
            "fma" => format!("({{ double x_ = {r}; double y_ = {}; double z_ = {}; fma(x_, y_, z_); }})", vals[0], vals[1]),
            _ => return Err(format!("uses F64.{name}")),
        })
    }

    pub(super) fn std_int(&mut self, r: &str, name: &str, args: &[Expr], t: &Type) -> G {
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let two = |body: &str| format!("({{ int64_t a_ = {r}; int64_t b_ = {}; {body} }})", vals[0]);
        let gcd = "uint64_t x_ = a_ < 0 ? (uint64_t)0 - (uint64_t)a_ : (uint64_t)a_; uint64_t y_ = b_ < 0 ? (uint64_t)0 - (uint64_t)b_ : (uint64_t)b_; uint64_t p_ = x_, q_ = y_; while (q_) { uint64_t t_ = p_ % q_; p_ = q_; q_ = t_; } ";
        Ok(match name {
            "gcd" => two(&format!("{gcd}if (UNLIKELY(p_ > (uint64_t)INT64_MAX)) TRAPV({T_OVERFLOW}, 0, 0); (int64_t)p_;")),
            "lcm" => two(&format!("int64_t l_ = 0; if (a_ && b_) {{ {gcd}unsigned __int128 w_ = (unsigned __int128)(x_ / p_) * y_; if (UNLIKELY(w_ > (unsigned __int128)INT64_MAX)) TRAPV({T_OVERFLOW}, 0, 0); l_ = (int64_t)w_; }} l_;")),
            "wrapping_add" => two("(int64_t)((uint64_t)a_ + (uint64_t)b_);"),
            "wrapping_sub" => two("(int64_t)((uint64_t)a_ - (uint64_t)b_);"),
            "wrapping_mul" => two("(int64_t)((uint64_t)a_ * (uint64_t)b_);"),
            "checked_add" | "checked_sub" | "checked_mul" | "checked_div" => {
                let oc = self.cty(t)?;
                let test = match name {
                    "checked_add" => "!__builtin_add_overflow(a_, b_, &r_)",
                    "checked_sub" => "!__builtin_sub_overflow(a_, b_, &r_)",
                    "checked_mul" => "!__builtin_mul_overflow(a_, b_, &r_)",
                    _ => "b_ != 0 && !(a_ == INT64_MIN && b_ == -1) && ((r_ = a_ / b_), 1)",
                };
                two(&format!("int64_t r_ = 0; {oc} o_ = {{0}}; if ({test}) {{ o_.some = 1; o_.v = r_; }} o_;"))
            }
            "bnot" => format!("(~({r}))"),
            "popcount" => format!("((int64_t)__builtin_popcountll((uint64_t)({r})))"),
            "clz" | "ctz" => format!("({{ uint64_t u_ = (uint64_t)({r}); u_ ? (int64_t)__builtin_{name}ll(u_) : 64LL; }})"),
            "rotl" => two("uint64_t u_ = (uint64_t)a_; int k_ = (int)(b_ & 63); (int64_t)(k_ ? (u_ << k_) | (u_ >> (64 - k_)) : u_);"),
            "rotr" => two("uint64_t u_ = (uint64_t)a_; int k_ = (int)(b_ & 63); (int64_t)(k_ ? (u_ >> k_) | (u_ << (64 - k_)) : u_);"),
            "byteswap" => format!("((int64_t)__builtin_bswap64((uint64_t)({r})))"),
            "saturating_add" => two("int64_t r_; __builtin_add_overflow(a_, b_, &r_) ? (a_ < 0 ? INT64_MIN : INT64_MAX) : r_;"),
            "saturating_sub" => two("int64_t r_; __builtin_sub_overflow(a_, b_, &r_) ? (a_ < 0 ? INT64_MIN : INT64_MAX) : r_;"),
            "saturating_mul" => two("int64_t r_; __builtin_mul_overflow(a_, b_, &r_) ? ((a_ < 0) != (b_ < 0) ? INT64_MIN : INT64_MAX) : r_;"),
            _ => return Err(format!("uses Int.{name}")),
        })
    }

    pub(super) fn std_list(&mut self, r: &str, lt: &Type, et: &Type, name: &str, args: &[Expr], t: &Type) -> G {
        let ec = self.decl(et)?;
        let lc = self.cty(lt)?;
        let l = self.fresh("l");
        let i = self.fresh("i");
        let x = self.fresh("x");
        let head = format!("__auto_type {l} = {r}; ");
        let wrap = |body: String| format!("({{ {head}{body} }})");
        Ok(match name {
            "binary_search" | "lower_bound" => {
                let v = self.expr(&args[0])?;
                let c = self.helper_cmp(et)?;
                let core = format!("__auto_type {x} = {v}; int64_t lo_ = 0, hi_ = {l}.len; while (lo_ < hi_) {{ int64_t m_ = lo_ + (hi_ - lo_) / 2; if ({c}({l}.data[m_], {x}) < 0) lo_ = m_ + 1; else hi_ = m_; }} ");
                if name == "lower_bound" {
                    wrap(format!("{core}lo_;"))
                } else {
                    let oc = self.cty(t)?;
                    wrap(format!("{core}{oc} o_ = {{0}}; if (lo_ < {l}.len && {c}({l}.data[lo_], {x}) == 0) {{ o_.some = 1; o_.v = lo_; }} o_;"))
                }
            }
            "sort_with" => {
                let y = self.fresh("y");
                let body = self.apply(&args[0], vec![(x.clone(), et.clone()), (y.clone(), et.clone())])?;
                wrap(format!("int64_t n_ = {l}.len; RawL r_ = raw_alloc(n_, sizeof({ec})); {ec}* a_ = ({ec}*)r_.data; if (n_) memcpy(a_, {l}.data, (size_t)n_ * sizeof({ec})); r_.hdr[1] = n_; {ec}* t_ = ({ec}*)sspur_alloc((size_t)(n_ ? n_ : 1) * sizeof({ec})); int64_t sk_[400]; int sp_ = 0; if (n_ > 1) {{ sk_[0] = 0; sk_[1] = n_; sk_[2] = 0; sp_ = 3; }} while (sp_) {{ sp_ -= 3; int64_t lo_ = sk_[sp_], hi_ = sk_[sp_ + 1], ph_ = sk_[sp_ + 2]; if (hi_ - lo_ < 2) continue; int64_t mid_ = lo_ + (hi_ - lo_) / 2; if (!ph_) {{ sk_[sp_ + 2] = 1; sk_[sp_ + 3] = mid_; sk_[sp_ + 4] = hi_; sk_[sp_ + 5] = 0; sk_[sp_ + 6] = lo_; sk_[sp_ + 7] = mid_; sk_[sp_ + 8] = 0; sp_ += 9; continue; }} int64_t p_ = lo_, q_ = mid_, k_ = 0; while (p_ < mid_ && q_ < hi_) {{ __auto_type {x} = a_[q_]; __auto_type {y} = a_[p_]; if (({body}) < 0) t_[k_++] = a_[q_++]; else t_[k_++] = a_[p_++]; }} while (p_ < mid_) t_[k_++] = a_[p_++]; while (q_ < hi_) t_[k_++] = a_[q_++]; memcpy(a_ + lo_, t_, (size_t)k_ * sizeof({ec})); }} ({lc}){{n_, a_, r_.hdr}};"))
            }
            "chunks" | "windows" => {
                self.std("fail");
                let n = self.expr(&args[0])?;
                let out = self.cty(t)?;
                let what = if name == "chunks" { "chunk" } else { "window" };
                let (count, item) = if name == "chunks" {
                    ("n_ ? {l}.len / n_ + ({l}.len % n_ != 0) : 0", format!("int64_t s_ = {i} * n_; int64_t e_ = {l}.len - s_ < n_ ? {l}.len : s_ + n_; d_[{i}] = ({lc}){{e_ - s_, {l}.data + s_, {l}.hdr}};"))
                } else {
                    ("{l}.len >= n_ ? {l}.len - n_ + 1 : 0", format!("d_[{i}] = ({lc}){{n_, {l}.data + {i}, {l}.hdr}};"))
                };
                let count = count.replace("{l}", &l);
                wrap(format!("int64_t n_ = {n}; if (UNLIKELY(n_ <= 0)) ss_fail(st, \"{what} size must be > 0\"); int64_t m_ = {count}; RawL r_ = raw_alloc(m_, sizeof({lc})); {lc}* d_ = ({lc}*)r_.data; for (int64_t {i} = 0; {i} < m_; {i}++) {{ {item} }} r_.hdr[1] = m_; ({out}){{m_, d_, r_.hdr}};"))
            }
            "group_by" => {
                let pt = elem(t, "List").ok_or("bad group_by type")?;
                let Type::Tuple(parts) = &pt else { return Err("bad group_by type".into()) };
                let kt = parts[0].clone();
                let pc = self.cty(&pt)?;
                let out = self.cty(t)?;
                let (mm, node) = self.map_helpers(&Type::app("Map", vec![kt, Type::int()]))?;
                let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                wrap(format!("RawL r_ = raw_alloc(4, sizeof({pc})); {node}* ix_ = 0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; __auto_type k_ = {body}; {node}* f_ = find_{mm}(ix_, k_); int64_t g_; if (f_) g_ = f_->v; else {{ g_ = r_.len; ix_ = mput_{mm}(ix_, k_, g_, sspur_prio()); {pc} p_; p_.f0 = k_; p_.f1 = ({lc}){{0, 0, 0}}; r_ = raw_push(r_, &p_, sizeof({pc})); }} {pc}* d_ = ({pc}*)r_.data; RawL q_ = raw_push(TO_RAW(d_[g_].f1), &{x}, sizeof({ec})); d_[g_].f1 = ({lc}){{q_.len, ({ec}*)q_.data, q_.hdr}}; }} ({out}){{r_.len, ({pc}*)r_.data, r_.hdr}};"))
            }
            "partition" => {
                let tc = self.cty(t)?;
                let body = self.apply(&args[0], vec![(x.clone(), et.clone())])?;
                wrap(format!("RawL ya_ = raw_alloc({l}.len, sizeof({ec})); RawL na_ = raw_alloc({l}.len, sizeof({ec})); {ec}* yd_ = ({ec}*)ya_.data; {ec}* nd_ = ({ec}*)na_.data; int64_t yn_ = 0, nn_ = 0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; if ({body}) yd_[yn_++] = {x}; else nd_[nn_++] = {x}; }} ya_.hdr[1] = yn_; na_.hdr[1] = nn_; ({tc}){{({lc}){{yn_, yd_, ya_.hdr}}, ({lc}){{nn_, nd_, na_.hdr}}}};"))
            }
            "scan" => {
                let init = self.expr(&args[0])?;
                let acc_t = self.ty(&args[0])?;
                let bt = elem(t, "List").ok_or("bad scan type")?;
                let bc = self.cty(&bt)?;
                let out = self.cty(t)?;
                let acc = self.fresh("acc");
                let body = self.apply(&args[1], vec![(acc.clone(), acc_t), (x.clone(), et.clone())])?;
                wrap(format!("{bc} {acc} = {init}; RawL r_ = raw_alloc({l}.len, sizeof({bc})); {bc}* d_ = ({bc}*)r_.data; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; {acc} = {body}; d_[{i}] = {acc}; }} r_.hdr[1] = {l}.len; ({out}){{{l}.len, d_, r_.hdr}};"))
            }
            "flatten" => {
                let it = elem(et, "List").ok_or("bad flatten type")?;
                let ic = self.decl(&it)?;
                let out = self.cty(t)?;
                wrap(format!("int64_t n_ = 0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) n_ += {l}.data[{i}].len; RawL r_ = raw_alloc(n_, sizeof({ic})); {ic}* d_ = ({ic}*)r_.data; int64_t k_ = 0; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ if ({l}.data[{i}].len) memcpy(d_ + k_, {l}.data[{i}].data, (size_t){l}.data[{i}].len * sizeof({ic})); k_ += {l}.data[{i}].len; }} r_.hdr[1] = n_; ({out}){{n_, d_, r_.hdr}};"))
            }
            "index_of" | "find_index" => {
                let oc = self.cty(t)?;
                let (pre, test) = if name == "index_of" {
                    let v = self.expr(&args[0])?;
                    let c = self.helper_cmp(et)?;
                    let y = self.fresh("y");
                    (format!("__auto_type {y} = {v}; "), format!("{c}({x}, {y}) == 0"))
                } else {
                    (String::new(), self.apply(&args[0], vec![(x.clone(), et.clone())])?)
                };
                wrap(format!("{pre}{oc} o_ = {{0}}; for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ __auto_type {x} = {l}.data[{i}]; if ({test}) {{ o_.some = 1; o_.v = {i}; break; }} }} o_;"))
            }
            "slice" => {
                let (a, b) = (self.expr(&args[0])?, self.expr(&args[1])?);
                wrap(format!("int64_t a_ = {a}; int64_t b_ = {b}; if (a_ < 0) a_ = 0; if (a_ > {l}.len) a_ = {l}.len; if (b_ < 0) b_ = 0; if (b_ > {l}.len) b_ = {l}.len; if (b_ < a_) b_ = a_; ({lc}){{b_ - a_, {l}.data + a_, {l}.hdr}};"))
            }
            "push_front" => {
                self.std("front");
                let v = self.expr(&args[0])?;
                wrap(format!("__auto_type {x} = {v}; RawL r_ = raw_push_front(TO_RAW({l}), &{x}, sizeof({ec})); ({lc}){{r_.len, ({ec}*)r_.data, r_.hdr}};"))
            }
            "pop_front" | "pop_back" => {
                let oc = self.cty(t)?;
                let (v, rest) = if name == "pop_front" { (format!("{l}.data[0]"), format!("({lc}){{{l}.len - 1, {l}.data + 1, {l}.hdr}}")) } else { (format!("{l}.data[{l}.len - 1]"), format!("({lc}){{{l}.len - 1, {l}.data, {l}.hdr}}")) };
                wrap(format!("{oc} o_ = {{0}}; if ({l}.len) {{ o_.some = 1; o_.v.f0 = {v}; o_.v.f1 = {rest}; }} o_;"))
            }
            "to_set" => {
                let (mm, _, sc) = self.set_parts(t)?;
                wrap(format!("{sc} s_ = {{0}}; for (int64_t {i} = 0; {i} < {l}.len; {i}++) s_.root = mput_{mm}(s_.root, {l}.data[{i}], 0, sspur_prio()); s_;"))
            }
            "to_heap" => {
                let (hm, _) = self.heap_helpers(t)?;
                let hc = self.cty(t)?;
                wrap(format!("{hc} h_ = {{0}}; for (int64_t {i} = 0; {i} < {l}.len; {i}++) h_.root = hpush_{hm}(h_.root, {l}.data[{i}]); h_;"))
            }
            _ => return Err(format!("uses List.{name}")),
        })
    }

    fn json_strip(&self, t: &Type) -> Type {
        match t {
            Type::Con(n, a) if n == "Pii" && a.len() == 1 => self.json_strip(&a[0]),
            Type::Con(n, a) if a.is_empty() && self.layouts.newtypes.contains_key(n) => self.json_strip(&self.layouts.newtypes[n]),
            _ => t.clone(),
        }
    }

    pub(super) fn json_call(&mut self, e: &Expr, name: &str, args: &[Expr], t: &Type) -> G {
        let jt = self.check.json_types.get(&(e.span.start, e.span.end)).cloned().ok_or("uses json without type information")?;
        let jt = self.subst(&jt);
        self.std("json");
        let v = self.expr(&args[0])?;
        if name == "encode" {
            let je = self.json_enc(&jt)?;
            return Ok(format!("({{ __auto_type jv_ = {v}; SB_INIT(jb_); {je}(&jb_, jv_); sb_done(&jb_); }})"));
        }
        let jd = self.json_dec(&jt)?;
        let rc = self.cty(t)?;
        Ok(format!("({{ Str js_ = {v}; SsJ* jp_ = sj_parse(js_); {rc} jr_; memset(&jr_, 0, sizeof jr_); if (!jp_) jr_.e = str_lit(\"invalid JSON\", 12); else {{ SsJE je_; memset(&je_, 0, sizeof je_); if ({jd}(jp_, &jr_.v, &je_)) jr_.ok = 1; else jr_.e = je_.msg; }} jr_; }})"))
    }

    fn json_enc(&mut self, t: &Type) -> G {
        let t = self.json_strip(t);
        let m = self.mangle(&t)?;
        let name = format!("je_{m}");
        if !self.helpers_done.insert(name.clone()) {
            return Ok(name);
        }
        let c = self.cty(&t)?;
        writeln!(self.protos, "static void {name}(SB* b, {c} v);").unwrap();
        let lit = |s: &str| format!("sb_put(b, {}, {}); ", c_lit(s), s.len());
        let seq = |cx: &mut Self, et: &Type, n: &str, at: &str| -> G {
            let e = cx.json_enc(et)?;
            Ok(format!("sb_put(b, \"[\", 1); for (int64_t i = 0; i < {n}; i++) {{ if (i) sb_put(b, \",\", 1); {e}(b, {at}); }} sb_put(b, \"]\", 1);"))
        };
        let body = match &t {
            _ if is(&t, "Int") => "sb_int(b, v);".to_string(),
            _ if is(&t, "Bool") => "if (v) sb_put(b, \"true\", 4); else sb_put(b, \"false\", 5);".to_string(),
            _ if is(&t, "Unit") => "(void)v; sb_put(b, \"null\", 4);".to_string(),
            _ if is(&t, "F64") => "sj_f64_put(b, v);".to_string(),
            _ if is(&t, "Str") => "sj_quote(b, v);".to_string(),
            Type::Con(n, _) if n == "#StrBuf" => "sj_quote(b, (Str){v.len, v.data});".to_string(),
            Type::Con(n, a) if n == "List" => seq(self, &a[0], "v.len", "v.data[i]")?,
            Type::Con(n, a) if n == "#Set" => {
                let (mm, node, _) = self.set_parts(&t)?;
                format!("{}{}", self.fill_keys(&mm, &node, "v.root"), seq(self, &a[0], "n", "xs[i]->k")?)
            }
            Type::Con(n, a) if n == "#Heap" => {
                let (hm, _) = self.heap_helpers(&t)?;
                let ec = self.cty(&a[0])?;
                format!("RawL x = hsorted_{hm}(v.root); {}", seq(self, &a[0], "x.len", &format!("(({ec}*)x.data)[i]"))?)
            }
            Type::Con(n, a) if n == "Opt" => {
                let e = self.json_enc(&a[0])?;
                format!("if (!v.some) sb_put(b, \"null\", 4); else {e}(b, v.v);")
            }
            Type::Con(n, a) if n == "Map" => {
                let (mm, node) = self.map_helpers(&t)?;
                let (ek, ev) = (self.json_enc(&a[0])?, self.json_enc(&a[1])?);
                let fill = self.fill_keys(&mm, &node, "v.root");
                if self.json_strip(&a[0]) == Type::str() {
                    format!("{fill}sb_put(b, \"{{\", 1); for (int64_t i = 0; i < n; i++) {{ if (i) sb_put(b, \",\", 1); {ek}(b, xs[i]->k); sb_put(b, \":\", 1); {ev}(b, xs[i]->v); }} sb_put(b, \"}}\", 1);")
                } else {
                    format!("{fill}sb_put(b, \"[\", 1); for (int64_t i = 0; i < n; i++) {{ if (i) sb_put(b, \",\", 1); sb_put(b, \"[\", 1); {ek}(b, xs[i]->k); sb_put(b, \",\", 1); {ev}(b, xs[i]->v); sb_put(b, \"]\", 1); }} sb_put(b, \"]\", 1);")
                }
            }
            Type::Tuple(xs) => {
                let mut s = String::from("sb_put(b, \"[\", 1); ");
                for (i, x) in xs.iter().enumerate() {
                    let e = self.json_enc(x)?;
                    if i > 0 {
                        s.push_str("sb_put(b, \",\", 1); ");
                    }
                    write!(s, "{e}(b, v.f{i}); ").unwrap();
                }
                s.push_str("sb_put(b, \"]\", 1);");
                s
            }
            Type::Con(n, a) if self.layouts.records.contains_key(n) => {
                let mut s = String::from("sb_put(b, \"{\", 1); ");
                for (i, (f, ft)) in self.layouts.record_fields(n, a).unwrap().iter().enumerate() {
                    let e = self.json_enc(ft)?;
                    s.push_str(&lit(&format!("{}\"{f}\":", if i > 0 { "," } else { "" })));
                    write!(s, "{e}(b, v.{f}); ").unwrap();
                }
                s.push_str("sb_put(b, \"}\", 1);");
                s
            }
            Type::Con(n, a) if self.layouts.sums.contains_key(n) => {
                let tag = self.tag(&t, "v");
                let mut s = format!("switch ({tag}) {{ ");
                for (k, (vn, fs)) in self.layouts.sum_variants(n, a).unwrap().iter().enumerate() {
                    write!(s, "case {k}: ").unwrap();
                    match fs {
                        None => s.push_str(&lit(&format!("\"{vn}\""))),
                        Some(fs) => {
                            s.push_str(&lit(&format!("{{\"tag\":\"{vn}\"")));
                            for (f, ft) in fs {
                                let e = self.json_enc(ft)?;
                                s.push_str(&lit(&format!(",\"{f}\":")));
                                write!(s, "{e}(b, v->u.v{k}.{f}); ").unwrap();
                            }
                            s.push_str("sb_put(b, \"}\", 1); ");
                        }
                    }
                    s.push_str("break; ");
                }
                s.push_str("default: break; }");
                s
            }
            _ => return Err(format!("cannot encode {t} as JSON")),
        };
        writeln!(self.helpers, "static void {name}(SB* b, {c} v) {{ {body} }}").unwrap();
        Ok(name)
    }

    fn json_dec(&mut self, t: &Type) -> G {
        let t = self.json_strip(t);
        let m = self.mangle(&t)?;
        let name = format!("jd_{m}");
        if !self.helpers_done.insert(name.clone()) {
            return Ok(name);
        }
        let c = self.cty(&t)?;
        writeln!(self.protos, "static int {name}(SsJ* v, {c}* out, SsJE* e);").unwrap();
        let want = t.to_string();
        let err = |w: &str, v: &str| format!("return sj_err(e, {}, {}, {v});", c_lit(w), w.len());
        let bad = err(&want, "v");
        let items = |cx: &mut Self, et: &Type, each: &str| -> G {
            let d = cx.json_dec(et)?;
            let ec = cx.cty(et)?;
            Ok(format!("if (!v || v->t != SJ_ARR) {bad} for (int64_t i = 0; i < v->len; i++) {{ {ec} x; memset(&x, 0, sizeof x); sj_pushi(e, i); if (!{d}(&v->items[i], &x, e)) return 0; e->n--; {each} }}"))
        };
        let body = match &t {
            _ if is(&t, "Int") => format!("if (!sj_int(v, out)) {bad} return 1;"),
            _ if is(&t, "F64") => format!("if (!sj_f64(v, out)) {bad} return 1;"),
            _ if is(&t, "Bool") => format!("if (!v || (v->t != SJ_TRUE && v->t != SJ_FALSE)) {bad} *out = v->t == SJ_TRUE; return 1;"),
            _ if is(&t, "Unit") => "(void)v; (void)e; *out = 0; return 1;".to_string(),
            _ if is(&t, "Str") => format!("if (!v || v->t != SJ_STR) {bad} *out = v->s; return 1;"),
            Type::Con(n, _) if n == "#StrBuf" => format!("if (!v || v->t != SJ_STR) {bad} *out = (SBuf){{v->s.len, (char*)v->s.p, 0}}; return 1;"),
            Type::Con(n, a) if n == "List" => {
                let ec = self.decl(&a[0])?;
                let each = "r = raw_push(r, &x, sizeof(EC));".replace("EC", &ec);
                format!("RawL r = raw_alloc(v ? v->len : 0, sizeof({ec})); {} *out = ({c}){{r.len, ({ec}*)r.data, r.hdr}}; return 1;", items(self, &a[0], &each)?)
            }
            Type::Con(n, a) if n == "#Set" => {
                let (mm, _, _) = self.set_parts(&t)?;
                format!("{c} s = {{0}}; {} *out = s; return 1;", items(self, &a[0], &format!("s.root = mput_{mm}(s.root, x, 0, sspur_prio());"))?)
            }
            Type::Con(n, a) if n == "#Heap" => {
                let (hm, _) = self.heap_helpers(&t)?;
                format!("{c} h = {{0}}; {} *out = h; return 1;", items(self, &a[0], &format!("h.root = hpush_{hm}(h.root, x);"))?)
            }
            Type::Con(n, a) if n == "Opt" => {
                let d = self.json_dec(&a[0])?;
                format!("memset(out, 0, sizeof *out); if (!v || v->t == SJ_NULL) return 1; out->some = 1; return {d}(v, &out->v, e);")
            }
            Type::Con(n, a) if n == "Map" => {
                let (mm, _) = self.map_helpers(&t)?;
                let (dk, dv) = (self.json_dec(&a[0])?, self.json_dec(&a[1])?);
                let (kc, vc) = (self.cty(&a[0])?, self.cty(&a[1])?);
                if self.json_strip(&a[0]) == Type::str() {
                    format!("if (!v || v->t != SJ_OBJ) {bad} {c} m = {{0}}; for (int64_t i = 0; i < v->len; i++) {{ SsJ kj; memset(&kj, 0, sizeof kj); kj.t = SJ_STR; kj.s = v->keys[i]; {kc} k; if (!{dk}(&kj, &k, e)) return 0; sj_push(e, v->keys[i]); {vc} x; memset(&x, 0, sizeof x); if (!{dv}(&v->items[i], &x, e)) return 0; e->n--; m.root = mput_{mm}(m.root, k, x, sspur_prio()); }} *out = m; return 1;")
                } else {
                    format!("if (!v || v->t != SJ_ARR) {bad} {c} m = {{0}}; for (int64_t i = 0; i < v->len; i++) {{ SsJ* p = &v->items[i]; sj_pushi(e, i); if (p->t != SJ_ARR || p->len != 2) {} {kc} k; memset(&k, 0, sizeof k); {vc} x; memset(&x, 0, sizeof x); if (!{dk}(&p->items[0], &k, e) || !{dv}(&p->items[1], &x, e)) return 0; e->n--; m.root = mput_{mm}(m.root, k, x, sspur_prio()); }} *out = m; return 1;", err("[key, value]", "p"))
                }
            }
            Type::Tuple(xs) => {
                let mut s = format!("if (!v || v->t != SJ_ARR || v->len != {}) {bad} ", xs.len());
                for (i, x) in xs.iter().enumerate() {
                    let d = self.json_dec(x)?;
                    write!(s, "sj_pushi(e, {i}); if (!{d}(&v->items[{i}], &out->f{i}, e)) return 0; e->n--; ").unwrap();
                }
                s.push_str("return 1;");
                s
            }
            Type::Con(n, a) if self.layouts.records.contains_key(n) => {
                let mut s = format!("if (!v || v->t != SJ_OBJ) {bad} ");
                for (f, ft) in self.layouts.record_fields(n, a).unwrap() {
                    let d = self.json_dec(&ft)?;
                    write!(s, "sj_push(e, str_lit({fl}, {n})); if (!{d}(sj_get(v, {fl}, {n}), &out->{f}, e)) return 0; e->n--; ", fl = c_lit(&f), n = f.len()).unwrap();
                }
                s.push_str("return 1;");
                s
            }
            Type::Con(n, a) if self.layouts.sums.contains_key(n) => {
                let vs = self.layouts.sum_variants(n, a).unwrap();
                let base = c.trim_end_matches('*').to_string();
                let niche = self.niche(&t);
                let mut s = format!("SsJ* tag = v && v->t == SJ_OBJ ? sj_get(v, \"tag\", 3) : v; if (!tag || tag->t != SJ_STR) {bad} ");
                for (k, (vn, fs)) in vs.iter().enumerate() {
                    write!(s, "if (tag->s.len == {} && !memcmp(tag->s.p, {}, {})) {{ ", vn.len(), c_lit(vn), vn.len()).unwrap();
                    if fs.is_none() && niche.is_some() {
                        s.push_str("*out = 0; return 1; } ");
                        continue;
                    }
                    write!(s, "{c} p = ({c})sspur_alloc(sizeof({base})); memset(p, 0, sizeof({base})); ").unwrap();
                    if niche.is_none() {
                        write!(s, "p->tag = {k}; ").unwrap();
                    }
                    for (f, ft) in fs.iter().flatten() {
                        let d = self.json_dec(ft)?;
                        write!(s, "sj_push(e, str_lit({fl}, {n})); if (!{d}(sj_get(v, {fl}, {n}), &p->u.v{k}.{f}, e)) return 0; e->n--; ", fl = c_lit(f), n = f.len()).unwrap();
                    }
                    s.push_str("*out = p; return 1; } ");
                }
                let one = format!("one of {}", vs.iter().map(|v| v.0.as_str()).collect::<Vec<_>>().join(", "));
                s.push_str(&err(&one, "tag"));
                s
            }
            _ => return Err(format!("cannot decode {t} from JSON")),
        };
        writeln!(self.helpers, "static int {name}(SsJ* v, {c}* out, SsJE* e) {{ {body} }}").unwrap();
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_runtime_section_compiles_alone() {
        if Command::new("clang").arg("--version").output().is_err() {
            return;
        }
        let keys: Vec<&str> = STD_RT.split("//@ ").skip(1).filter_map(|p| p.split_whitespace().next()).collect();
        assert!(keys.len() >= 10);
        let m = sspur_syntax::parse("").unwrap();
        for key in keys {
            let check = CheckOutput::default();
            let (eligible, alias, fields) = (HashSet::new(), HashMap::new(), HashMap::new());
            let smt = sspur_smt::Oracle::new(&m, &check);
            let mut cx = Cx::new(&check, &eligible, &alias, &fields, &smt);
            cx.std_cty(&Type::con("#StrBuf"), "BU").unwrap();
            cx.std(key);
            let src = format!("{PRELUDE}{}{}", cx.defs, cx.protos);
            let path = std::env::temp_dir().join(format!("sspur_std_rt_{}_{key}.c", std::process::id()));
            std::fs::write(&path, &src).unwrap();
            let out = Command::new("clang").args(["-fsyntax-only", "-Werror=implicit-function-declaration", "-Werror=implicit-int"]).arg(&path).output().unwrap();
            std::fs::remove_file(&path).ok();
            assert!(out.status.success(), "section {key}: {}", String::from_utf8_lossy(&out.stderr));
        }
    }
}
