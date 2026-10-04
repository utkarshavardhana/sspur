//! Lazy views. A pipeline whose source and stages are visible at the consumer (or `for` loop)
//! becomes one fused C loop with the lambdas inlined; any other view is a pull iterator built
//! from per-stage C functions over closures. Both follow the interpreter's pull order exactly.
use super::*;

const STAGES: &[&str] = &["map", "filter", "take", "drop", "take_while", "drop_while", "enumerate"];
const CONSUMERS: &[&str] = &["to_list", "fold", "len", "sum", "first", "find", "any", "all"];

enum FnRef {
    Inline(Expr),
    Var(String, Type),
}

enum Src {
    List(String),
    Iota(String, String),
    Iterate(String, FnRef, String),
    Pull(String),
}

struct Stage {
    kind: String,
    f: Option<FnRef>,
    n: Option<String>,
    out: Type,
}

fn velem(t: &Type) -> Type {
    match t {
        Type::Con(_, a) if !a.is_empty() => a[0].clone(),
        _ => Type::unit(),
    }
}

fn is_view(t: &Type) -> bool {
    matches!(t, Type::Con(n, _) if n == "#View")
}

impl Cx<'_> {
    pub(super) fn view_cty(&mut self, t: &Type, m: &str) -> G {
        let rr = self.rr(&Type::app("Opt", vec![velem(t)]))?;
        self.fwd_decl(m);
        if self.complete.insert(m.to_string()) {
            writeln!(self.defs, "struct {m} {{ void* (*start)(void*); {rr} (*next)(void*, Status*, int64_t); void* env; }};").unwrap();
        }
        Ok(m.to_string())
    }

    fn is_builtin_call(&self, e: &Expr, name: &str, arity: usize) -> bool {
        matches!(&e.kind, ExprKind::Call(f, args) if args.len() == arity && matches!(&f.kind, ExprKind::Name(n) if n == name && self.lookup(n).is_none() && !self.check.fn_types.contains_key(n)))
    }

    fn fn_arg(&mut self, f: &Expr, pre: &mut String) -> G<FnRef> {
        let inline = match &f.kind {
            ExprKind::Lambda { .. } => true,
            ExprKind::Name(n) => self.lookup(n).is_none() && self.check.fn_types.contains_key(n),
            _ => false,
        };
        if inline {
            return Ok(FnRef::Inline(f.clone()));
        }
        let ft = self.ty(f)?;
        let v = self.fresh("vf");
        write!(pre, "__auto_type {v} = {}; ", self.expr(f)?).unwrap();
        Ok(FnRef::Var(v, ft))
    }

    fn call_ref(&mut self, f: &FnRef, args: Vec<(String, Type)>) -> G {
        match f {
            FnRef::Inline(e) => self.apply(e, args),
            FnRef::Var(v, ft) => self.call_closure(v, ft, args.into_iter().map(|(a, _)| a).collect()),
        }
    }

    fn hoist(&mut self, e: &Expr, pre: &mut String) -> G {
        let v = self.fresh("vh");
        write!(pre, "__auto_type {v} = {}; ", self.expr(e)?).unwrap();
        Ok(v)
    }

    fn view_parts(&mut self, e: &Expr, pre: &mut String) -> G<(Src, Type, Vec<Stage>)> {
        let call = match &e.kind {
            ExprKind::Method { recv, name, args, .. } => Some((recv, name, args.as_slice())),
            ExprKind::Field(recv, name) => Some((recv, name, &[][..])),
            _ => None,
        };
        if let Some((recv, name, args)) = call
            && !self.check.user_methods.contains(&(e.span.start, e.span.end)) {
                let rt = self.ty(recv)?;
                if is_view(&rt) && STAGES.contains(&name.as_str()) {
                    let (src, st, mut stages) = self.view_parts(recv, pre)?;
                    let out = velem(&self.ty(e)?);
                    let (f, n) = match name.as_str() {
                        "take" | "drop" => (None, Some(self.hoist(&args[0], pre)?)),
                        "enumerate" => (None, None),
                        _ => (Some(self.fn_arg(&args[0], pre)?), None),
                    };
                    stages.push(Stage { kind: name.clone(), f, n, out });
                    return Ok((src, st, stages));
                }
                if name == "view" && is(&rt, "List") {
                    let l = self.hoist(recv, pre)?;
                    return Ok((Src::List(l), velem(&rt), vec![]));
                }
            }
        if self.is_builtin_call(e, "iota", 2)
            && let ExprKind::Call(_, args) = &e.kind {
                let a = self.hoist(&args[0], pre)?;
                let b = self.hoist(&args[1], pre)?;
                return Ok((Src::Iota(a, b), Type::int(), vec![]));
            }
        if self.is_builtin_call(e, "iterate", 3)
            && let ExprKind::Call(_, args) = &e.kind {
                let x = self.hoist(&args[0], pre)?;
                let f = self.fn_arg(&args[1], pre)?;
                let n = self.hoist(&args[2], pre)?;
                return Ok((Src::Iterate(x, f, n), velem(&self.ty(e)?), vec![]));
            }
        let v = self.hoist(e, pre)?;
        Ok((Src::Pull(v), velem(&self.ty(e)?), vec![]))
    }

    /// The fused loop: `consume` gets the final element variable and returns the per-element code.
    fn view_loop(&mut self, src: Src, st: Type, stages: Vec<Stage>, consume: &mut dyn FnMut(&mut Self, &str, &Type) -> G) -> G {
        let stop = self.fresh("vstop");
        let mut decl = format!("int {stop} = 0; ");
        let mut guard = vec!["1".to_string()];
        let x0 = self.fresh("vx");
        let head = match src {
            Src::List(l) => {
                let i = self.fresh("vi");
                format!("for (int64_t {i} = 0; {i} < {l}.len; {i}++) {{ if ({stop}) break; __auto_type {x0} = {l}.data[{i}]; ")
            }
            Src::Iota(a, b) => {
                let c = self.fresh("vc");
                format!("for (int64_t {c} = {a}; {c} < {b}; {c}++) {{ if ({stop}) break; int64_t {x0} = {c}; ")
            }
            Src::Iterate(x, f, n) => {
                let (cur, k) = (self.fresh("vcur"), self.fresh("vk"));
                let xc = self.cty(&st)?;
                write!(decl, "{xc} {cur} = {x}; ").unwrap();
                let next = self.call_ref(&f, vec![(cur.clone(), st.clone())])?;
                format!("for (int64_t {k} = 0; {k} < {n}; {k}++) {{ if ({stop}) break; if ({k} > 0) {cur} = {next}; __auto_type {x0} = {cur}; ")
            }
            Src::Pull(v) => {
                let (it, r) = (self.fresh("vit"), self.fresh("vr"));
                let rr = self.rr(&Type::app("Opt", vec![st.clone()]))?;
                let handlers = self.handlers(&format!("{r}.code"))?;
                write!(decl, "void* {it} = {v}.start({v}.env); ").unwrap();
                format!("for (;;) {{ if ({stop}) break; {rr} {r} = {v}.next({it}, st, depth); if (UNLIKELY({r}.code)) {{ {handlers}return (RRT){{.code = {r}.code}}; }} if (!{r}.v.some) break; __auto_type {x0} = {r}.v.v; ")
            }
        };
        let mut body = String::new();
        let (mut cur, mut ct) = (x0, st);
        for s in stages {
            match s.kind.as_str() {
                "map" => {
                    let y = self.fresh("vy");
                    let c = self.call_ref(s.f.as_ref().unwrap(), vec![(cur.clone(), ct.clone())])?;
                    let yc = self.cty(&s.out)?;
                    write!(body, "{yc} {y} = {c}; ").unwrap();
                    (cur, ct) = (y, s.out);
                }
                "filter" => {
                    let c = self.call_ref(s.f.as_ref().unwrap(), vec![(cur.clone(), ct.clone())])?;
                    write!(body, "if (!({c})) continue; ").unwrap();
                }
                "take_while" => {
                    let c = self.call_ref(s.f.as_ref().unwrap(), vec![(cur.clone(), ct.clone())])?;
                    write!(body, "if (!({c})) break; ").unwrap();
                }
                "drop_while" => {
                    let dw = self.fresh("vdw");
                    write!(decl, "int {dw} = 1; ").unwrap();
                    let c = self.call_ref(s.f.as_ref().unwrap(), vec![(cur.clone(), ct.clone())])?;
                    write!(body, "if ({dw}) {{ if ({c}) continue; {dw} = 0; }} ").unwrap();
                }
                "take" => {
                    let left = self.fresh("vleft");
                    write!(decl, "int64_t {left} = {}; ", s.n.unwrap()).unwrap();
                    guard.push(format!("{left} > 0"));
                    write!(body, "{left}--; if ({left} == 0) {stop} = 1; ").unwrap();
                }
                "drop" => {
                    let d = self.fresh("vdrop");
                    write!(decl, "int64_t {d} = {}; ", s.n.unwrap()).unwrap();
                    write!(body, "if ({d} > 0) {{ {d}--; continue; }} ").unwrap();
                }
                _ => {
                    let (k, y) = (self.fresh("vk"), self.fresh("vy"));
                    write!(decl, "int64_t {k} = 0; ").unwrap();
                    let tc = self.cty(&s.out)?;
                    write!(body, "{tc} {y} = ({tc}){{{k}++, {cur}}}; ").unwrap();
                    (cur, ct) = (y, s.out);
                }
            }
        }
        let tail = consume(self, &cur, &ct)?;
        Ok(format!("{decl}if ({}) {{ {head}{body}{tail} }} }} ", guard.join(" && ")))
    }

    pub(super) fn view_for(&mut self, p: &Pat, it: &Expr, body: &Expr) -> G {
        let mut pre = String::new();
        let (src, st, stages) = self.view_parts(it, &mut pre)?;
        let lp = self.view_loop(src, st, stages, &mut |me: &mut Self, x: &str, xt: &Type| {
            me.scopes.push(HashMap::new());
            let mut conds = Vec::new();
            let mut binds = Vec::new();
            if let Err(e) = me.pattern(p, x, xt, &mut conds, &mut binds) {
                me.scopes.pop();
                return Err(e);
            }
            let mut decl = String::new();
            for (n, expr, bt) in binds {
                let c = me.bind(&n, bt);
                write!(decl, "__auto_type {c} = {expr}; ").unwrap();
            }
            let b = me.expr(body);
            me.scopes.pop();
            let cond = if conds.is_empty() { "1".to_string() } else { conds.join(" && ") };
            Ok(format!("if ({cond}) {{ {decl}(void)({}); }} ", b?))
        })?;
        Ok(format!("{{ {pre}{lp}}} "))
    }

    pub(super) fn view_method(&mut self, e: &Expr, recv: &Expr, name: &str, args: &[Expr], t: &Type) -> G {
        if CONSUMERS.contains(&name) {
            return self.view_consume(recv, name, args, t);
        }
        let rt = self.ty(recv)?;
        let a = velem(&rt);
        let b = velem(t);
        let r = self.expr(recv)?;
        let _ = e;
        match name {
            "zip" | "chain" => {
                let w = self.expr(&args[0])?;
                self.view_node2(name, &rt, &self.ty(&args[0])?, t, &r, &w)
            }
            "take" | "drop" => {
                let n = self.expr(&args[0])?;
                self.view_node(name, &a, &b, &r, None, Some(&n), t)
            }
            "enumerate" => self.view_node(name, &a, &b, &r, None, None, t),
            _ => {
                let ft = self.ty(&args[0])?;
                let f = self.expr(&args[0])?;
                self.view_node(name, &a, &b, &r, Some((&f, &ft)), None, t)
            }
        }
    }

    fn view_consume(&mut self, recv: &Expr, name: &str, args: &[Expr], t: &Type) -> G {
        let mut pre = String::new();
        let (src, st, stages) = self.view_parts(recv, &mut pre)?;
        let out = self.fresh("vo");
        let tc = self.cty(t)?;
        let (init, fref) = match name {
            "fold" => {
                let i = self.hoist(&args[0], &mut pre)?;
                let f = self.fn_arg(&args[1], &mut pre)?;
                (format!("{tc} {out} = {i}; "), Some(f))
            }
            "find" | "any" | "all" => {
                let f = self.fn_arg(&args[0], &mut pre)?;
                let v = match name {
                    "all" => "1",
                    _ => "0",
                };
                (if name == "find" { format!("{tc} {out}; memset(&{out}, 0, sizeof {out}); ") } else { format!("int64_t {out} = {v}; ") }, Some(f))
            }
            "to_list" => {
                let ec = self.decl(&velem(t))?;
                (format!("RawL {out} = raw_alloc(8, sizeof({ec})); "), None)
            }
            "first" => (format!("{tc} {out}; memset(&{out}, 0, sizeof {out}); "), None),
            _ => (format!("int64_t {out} = 0; "), None),
        };
        let name_s = name.to_string();
        let tt = t.clone();
        let o = out.clone();
        let lp = self.view_loop(src, st, stages, &mut |me: &mut Self, x: &str, xt: &Type| {
            Ok(match name_s.as_str() {
                "to_list" => {
                    let ec = me.decl(xt)?;
                    me.std("fail");
                    format!("if (UNLIKELY({o}.len == ((int64_t)1 << 26))) ss_fail(st, \"out of memory\"); {o} = raw_push({o}, &{x}, sizeof({ec})); ")
                }
                "fold" => {
                    let c = me.call_ref(fref.as_ref().unwrap(), vec![(o.clone(), tt.clone()), (x.to_string(), xt.clone())])?;
                    format!("{o} = {c}; ")
                }
                "len" => format!("{o}++; "),
                "sum" => format!("if (UNLIKELY(__builtin_add_overflow({o}, {x}, &{o}))) TRAPV({T_OVERFLOW}, 0, 0); "),
                "first" => format!("{o}.some = 1; {o}.v = {x}; break; "),
                "find" => {
                    let c = me.call_ref(fref.as_ref().unwrap(), vec![(x.to_string(), xt.clone())])?;
                    format!("if ({c}) {{ {o}.some = 1; {o}.v = {x}; break; }} ")
                }
                "any" => {
                    let c = me.call_ref(fref.as_ref().unwrap(), vec![(x.to_string(), xt.clone())])?;
                    format!("if ({c}) {{ {o} = 1; break; }} ")
                }
                _ => {
                    let c = me.call_ref(fref.as_ref().unwrap(), vec![(x.to_string(), xt.clone())])?;
                    format!("if (!({c})) {{ {o} = 0; break; }} ")
                }
            })
        })?;
        let result = if name == "to_list" {
            let ec = self.decl(&velem(t))?;
            format!("({tc}){{{out}.len, ({ec}*){out}.data, {out}.hdr}}")
        } else {
            out
        };
        Ok(format!("({{ {pre}{init}{lp}{result}; }})"))
    }

    pub(super) fn view_of_list(&mut self, l: &str, lt: &Type, t: &Type) -> G {
        let a = velem(lt);
        let lc = self.cty(lt)?;
        let id = self.view_stage("list", &a, &a, &format!("{lc} l;"), &format!("{lc} l; int64_t i;"), "s->l = e->l; s->i = 0;", "if (s->i < s->l.len) VW_SOME(s->l.data[s->i++]); VW_NONE;")?;
        let vc = self.cty(t)?;
        Ok(format!("({{ struct VE_{id}* e_ = (struct VE_{id}*)sspur_alloc(sizeof(struct VE_{id})); e_->l = {l}; ({vc}){{vs_{id}, vn_{id}, e_}}; }})"))
    }

    pub(super) fn view_global(&mut self, n: &str, args: &[Expr], vals: &[String], t: &Type) -> G<Option<String>> {
        let vc = match n {
            "iota" | "iterate" => self.cty(t)?,
            _ => return Ok(None),
        };
        if n == "iota" {
            let id = self.view_stage("iota", &Type::int(), &Type::int(), "int64_t a, b;", "int64_t c, b;", "s->c = e->a; s->b = e->b;", "if (s->c < s->b) VW_SOME(s->c++); VW_NONE;")?;
            return Ok(Some(format!("({{ struct VE_{id}* e_ = (struct VE_{id}*)sspur_alloc(sizeof(struct VE_{id})); e_->a = {}; e_->b = {}; ({vc}){{vs_{id}, vn_{id}, e_}}; }})", vals[0], vals[1])));
        }
        let a = velem(t);
        let ac = self.cty(&a)?;
        let ft = self.ty(&args[1])?;
        let fc = self.cty(&ft)?;
        let rf = self.rr(&a)?;
        let id = self.view_stage("iterate", &a, &a, &format!("{ac} x; {fc} f; int64_t n;"), &format!("{ac} cur; {fc} f; int64_t k, n;"), "s->cur = e->x; s->f = e->f; s->k = 0; s->n = e->n;", &format!("if (s->k >= s->n) VW_NONE; if (s->k > 0) {{ {rf} c = s->f.fn(s->f.env, s->cur, st, depth); if (UNLIKELY(c.code)) VW_FAIL(c.code); s->cur = c.v; }} s->k++; VW_SOME(s->cur);"))?;
        Ok(Some(format!("({{ struct VE_{id}* e_ = (struct VE_{id}*)sspur_alloc(sizeof(struct VE_{id})); e_->x = {}; e_->f = {}; e_->n = {}; ({vc}){{vs_{id}, vn_{id}, e_}}; }})", vals[0], vals[1], vals[2])))
    }

    /// Emits one stage's env and state structs and its start and next functions, once per type.
    #[allow(clippy::too_many_arguments)]
    fn view_stage(&mut self, kind: &str, a: &Type, b: &Type, env: &str, state: &str, start: &str, next: &str) -> G {
        let id = format!("{kind}_{}_{}", self.mangle(a)?, self.mangle(b)?);
        if !self.helpers_done.insert(format!("vstage:{id}")) {
            return Ok(id);
        }
        let ob = Type::app("Opt", vec![b.clone()]);
        let rr = self.rr(&ob)?;
        writeln!(self.defs, "struct VE_{id} {{ {env} }};\nstruct VS_{id} {{ {state} }};").unwrap();
        writeln!(self.protos, "static void* vs_{id}(void* env);\nstatic {rr} vn_{id}(void* it, Status* st, int64_t depth);").unwrap();
        let macros = format!("#define VW_NONE do {{ {rr} o_; memset(&o_, 0, sizeof o_); return o_; }} while (0)\n#define VW_SOME(x) do {{ {rr} o_; memset(&o_, 0, sizeof o_); o_.v.some = 1; o_.v.v = (x); return o_; }} while (0)\n#define VW_FAIL(c) do {{ {rr} o_; memset(&o_, 0, sizeof o_); o_.code = (c); return o_; }} while (0)");
        writeln!(self.lambdas, "{macros}\nstatic void* vs_{id}(void* env) {{ struct VE_{id}* e = (struct VE_{id}*)env; struct VS_{id}* s = (struct VS_{id}*)sspur_alloc(sizeof(struct VS_{id})); {start} return s; }}\nstatic {rr} vn_{id}(void* it, Status* st, int64_t depth) {{ struct VS_{id}* s = (struct VS_{id}*)it; (void)st; (void)depth; {next} }}\n#undef VW_NONE\n#undef VW_SOME\n#undef VW_FAIL").unwrap();
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    fn view_node(&mut self, kind: &str, a: &Type, b: &Type, src: &str, f: Option<(&str, &Type)>, n: Option<&str>, t: &Type) -> G {
        let va = Type::app("#View", vec![a.clone()]);
        let vac = self.cty(&va)?;
        let ra = self.rr(&Type::app("Opt", vec![a.clone()]))?;
        let vc = self.cty(t)?;
        let pull = format!("{ra} r = s->src.next(s->it, st, depth); if (UNLIKELY(r.code)) VW_FAIL(r.code);");
        let (fc, rf) = match f {
            Some((_, ft)) => {
                let Type::Fn(_, ret, _) = ft else { return Err("view stage needs a function".into()) };
                (self.cty(ft)?, self.rr(ret)?)
            }
            None => (String::new(), String::new()),
        };
        let call = |x: &str| format!("{rf} c = s->f.fn(s->f.env, {x}, st, depth); if (UNLIKELY(c.code)) VW_FAIL(c.code);");
        let (env, state, start, next) = match kind {
            "map" => (format!("{vac} src; {fc} f;"), format!("{vac} src; void* it; {fc} f;"), "s->f = e->f;".to_string(), format!("{pull} if (!r.v.some) VW_NONE; {} VW_SOME(c.v);", call("r.v.v"))),
            "filter" => (format!("{vac} src; {fc} f;"), format!("{vac} src; void* it; {fc} f;"), "s->f = e->f;".into(), format!("for (;;) {{ {pull} if (!r.v.some) VW_NONE; {} if (c.v) VW_SOME(r.v.v); }}", call("r.v.v"))),
            "take_while" => (format!("{vac} src; {fc} f;"), format!("{vac} src; void* it; {fc} f; int done;"), "s->f = e->f; s->done = 0;".into(), format!("if (s->done) VW_NONE; {pull} if (!r.v.some) VW_NONE; {} if (!c.v) {{ s->done = 1; VW_NONE; }} VW_SOME(r.v.v);", call("r.v.v"))),
            "drop_while" => (format!("{vac} src; {fc} f;"), format!("{vac} src; void* it; {fc} f; int dropping;"), "s->f = e->f; s->dropping = 1;".into(), format!("for (;;) {{ {pull} if (!r.v.some) VW_NONE; if (s->dropping) {{ {} if (c.v) continue; }} s->dropping = 0; VW_SOME(r.v.v); }}", call("r.v.v"))),
            "take" => (format!("{vac} src; int64_t n;"), format!("{vac} src; void* it; int64_t left;"), "s->left = e->n;".into(), format!("if (s->left <= 0) VW_NONE; s->left--; {pull} if (!r.v.some) VW_NONE; VW_SOME(r.v.v);")),
            "drop" => (format!("{vac} src; int64_t n;"), format!("{vac} src; void* it; int64_t n;"), "s->n = e->n;".into(), format!("while (s->n > 0) {{ s->n--; {pull} if (!r.v.some) VW_NONE; }} {{ {pull} if (!r.v.some) VW_NONE; VW_SOME(r.v.v); }}")),
            _ => {
                let tc = self.cty(b)?;
                (format!("{vac} src;"), format!("{vac} src; void* it; int64_t k;"), "s->k = 0;".into(), format!("{pull} if (!r.v.some) VW_NONE; {tc} y = ({tc}){{s->k++, r.v.v}}; VW_SOME(y);"))
            }
        };
        let start = format!("s->src = e->src; s->it = e->src.start(e->src.env); {start}");
        let id = self.view_stage(kind, a, b, &env, &state, &start, &next)?;
        let mut set = format!("e_->src = {src}; ");
        if let Some((fv, _)) = f {
            write!(set, "e_->f = {fv}; ").unwrap();
        }
        if let Some(nv) = n {
            write!(set, "e_->n = {nv}; ").unwrap();
        }
        Ok(format!("({{ struct VE_{id}* e_ = (struct VE_{id}*)sspur_alloc(sizeof(struct VE_{id})); {set}({vc}){{vs_{id}, vn_{id}, e_}}; }})"))
    }

    fn view_node2(&mut self, kind: &str, vt: &Type, wt: &Type, t: &Type, v: &str, w: &str) -> G {
        let (a, b) = (velem(vt), velem(wt));
        let (vc, wc, oc) = (self.cty(vt)?, self.cty(wt)?, self.cty(t)?);
        let (ra, rb) = (self.rr(&Type::app("Opt", vec![a.clone()]))?, self.rr(&Type::app("Opt", vec![b.clone()]))?);
        let out = velem(t);
        let (state, start, next) = if kind == "zip" {
            let tc = self.cty(&out)?;
            (
                format!("{vc} a; void* ia; {wc} b; void* ib;"),
                "s->a = e->a; s->ia = e->a.start(e->a.env); s->b = e->b; s->ib = e->b.start(e->b.env);".to_string(),
                format!("{ra} x = s->a.next(s->ia, st, depth); if (UNLIKELY(x.code)) VW_FAIL(x.code); if (!x.v.some) VW_NONE; {rb} y = s->b.next(s->ib, st, depth); if (UNLIKELY(y.code)) VW_FAIL(y.code); if (!y.v.some) VW_NONE; {tc} p = ({tc}){{x.v.v, y.v.v}}; VW_SOME(p);"),
            )
        } else {
            (
                format!("{vc} a; void* ia; {wc} b; void* ib; int second;"),
                "s->a = e->a; s->ia = e->a.start(e->a.env); s->b = e->b; s->ib = 0; s->second = 0;".to_string(),
                format!("if (!s->second) {{ {ra} x = s->a.next(s->ia, st, depth); if (UNLIKELY(x.code)) VW_FAIL(x.code); if (x.v.some) VW_SOME(x.v.v); s->second = 1; s->ib = s->b.start(s->b.env); }} {{ {rb} y = s->b.next(s->ib, st, depth); if (UNLIKELY(y.code)) VW_FAIL(y.code); if (!y.v.some) VW_NONE; VW_SOME(y.v.v); }}"),
            )
        };
        let key = Type::Tuple(vec![a, b]);
        let id = self.view_stage(kind, &key, &out, &format!("{vc} a; {wc} b;"), &state, &start, &next)?;
        Ok(format!("({{ struct VE_{id}* e_ = (struct VE_{id}*)sspur_alloc(sizeof(struct VE_{id})); e_->a = {v}; e_->b = {w}; ({oc}){{vs_{id}, vn_{id}, e_}}; }})"))
    }
}
