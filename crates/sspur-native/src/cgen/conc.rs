use super::*;
use sspur_check::Row;

pub(super) fn opaque(l: &Layouts, t: &Type) -> bool {
    let mut seen = HashSet::new();
    let mut stack = vec![t.clone()];
    while let Some(t) = stack.pop() {
        match t {
            Type::Con(n, _) if n == "Atomic" || n == "Chan" || n == "#DevBuf" || n == "#View" => return true,
            Type::Con(n, a) => {
                if seen.insert(format!("{n}{a:?}")) {
                    if let Some(fs) = l.record_fields(&n, &a) {
                        stack.extend(fs.into_iter().map(|(_, t)| t));
                    }
                    if let Some(vs) = l.sum_variants(&n, &a) {
                        stack.extend(vs.into_iter().flat_map(|(_, fs)| fs.unwrap_or_default()).map(|(_, t)| t));
                    }
                    if let Some(inner) = l.newtypes.get(&n) {
                        stack.push(inner.clone());
                    }
                }
                stack.extend(a);
            }
            Type::Tuple(xs) => stack.extend(xs),
            Type::Fn(ps, r, _) => {
                stack.extend(ps);
                stack.push(*r);
            }
            _ => {}
        }
    }
    false
}

impl Cx<'_> {
    fn tramp(&mut self, ft: &Type, kind: &str) -> G {
        let Type::Fn(ps, r, _) = ft else { return Err("task without a function type".into()) };
        let m = self.mangle(ft)?;
        let name = format!("{kind}_{m}");
        if !self.helpers_done.insert(name.clone()) {
            return Ok(name);
        }
        let rr = self.rr(r)?;
        let call = match (kind, ps.first()) {
            ("tk", None) => {
                let rc = self.cty(r)?;
                format!("{rr} c_ = k->fn(k->env, &t->st, t->depth); if (c_.code) {{ t->fail = 2; return; }} *({rc}*)t->out = c_.v;")
            }
            ("tkr", Some(_)) => format!("{rr} c_ = k->fn(k->env, t->idx, &t->st, t->depth); if (c_.code) t->fail = 2;"),
            ("tkl", Some(p)) => {
                let pc = self.cty(p)?;
                format!("{rr} c_ = k->fn(k->env, (({pc}*)t->out)[t->idx], &t->st, t->depth); if (c_.code) t->fail = 2;")
            }
            _ => return Err("bad task shape".into()),
        };
        writeln!(self.protos, "static void {name}(SsTask* t);").unwrap();
        writeln!(self.lambdas, "static void {name}(SsTask* t) {{ struct {m}* k = (struct {m}*)t->clo; jmp_buf jb; sspur_jb = &jb; if (setjmp(jb)) {{ t->fail = 1; return; }} {call} }}").unwrap();
        Ok(name)
    }

    pub(super) fn par_tasks(&mut self, xs: &[Expr], t: &Type) -> G {
        let Type::Tuple(ts) = t else { return Err("par without a tuple type".into()) };
        let tc = self.cty(t)?;
        let tv = self.fresh("pt");
        let mut s = format!("({{ SsTask {tv}[{}]; ", xs.len());
        let mut outs = Vec::new();
        for (i, (x, ti)) in xs.iter().zip(ts).enumerate() {
            let ft = Type::Fn(vec![], Box::new(ti.clone()), Row::default());
            let clo = self.closure(&[], x, &ft)?;
            let fc = self.cty(&ft)?;
            let ci = self.cty(ti)?;
            let tr = self.tramp(&ft, "tk")?;
            let (k, r) = (self.fresh("pk"), self.fresh("pr"));
            write!(s, "{fc} {k} = {clo}; {ci} {r}; {tv}[{i}].run = {tr}; {tv}[{i}].clo = &{k}; {tv}[{i}].out = &{r}; ").unwrap();
            outs.push(r);
        }
        let handlers = self.handlers("pc_")?;
        write!(s, "int64_t pc_ = ss_par({tv}, {}, st, depth); if (UNLIKELY(pc_)) {{ {handlers}return (RRT){{.code = pc_}}; }} ({tc}){{{}}}; }})", xs.len(), outs.join(", ")).unwrap();
        Ok(s)
    }

    pub(super) fn par_for(&mut self, p: &Pat, src: &Expr, body: &Expr) -> G {
        let name = match p {
            Pat::Bind(n) => n.clone(),
            Pat::Wild => "_pw".into(),
            _ => return Err("destructures in a par loop".into()),
        };
        let (n, i, k, ts) = (self.fresh("pn"), self.fresh("pi"), self.fresh("pk"), self.fresh("pt"));
        let (setup, et, data, idx) = match &src.kind {
            ExprKind::Range(a, b) => {
                let (av, bv) = (self.expr(a)?, self.expr(b)?);
                let (lo, hi) = (self.fresh("plo"), self.fresh("phi"));
                (format!("int64_t {lo} = {av}; int64_t {hi} = {bv}; int64_t {n} = {hi} > {lo} ? {hi} - {lo} : 0; "), Type::int(), "0".to_string(), format!("{lo} + {i}"))
            }
            _ => {
                let et = elem(&self.ty(src)?, "List").ok_or("par loop over a non-list")?;
                let lv = self.expr(src)?;
                let l = self.fresh("pl");
                (format!("__auto_type {l} = {lv}; int64_t {n} = {l}.len; "), et, format!("(void*){l}.data"), i.clone())
            }
        };
        let kind = if matches!(src.kind, ExprKind::Range(..)) { "tkr" } else { "tkl" };
        let ft = Type::Fn(vec![et], Box::new(Type::unit()), Row::default());
        let clo = self.closure(&[name], body, &ft)?;
        let fc = self.cty(&ft)?;
        let tr = self.tramp(&ft, kind)?;
        let handlers = self.handlers("pc_")?;
        Ok(format!("{{ {setup}{fc} {k} = {clo}; SsTask* {ts} = (SsTask*)malloc(sizeof(SsTask) * (size_t)({n} > 0 ? {n} : 1)); for (int64_t {i} = 0; {i} < {n}; {i}++) {{ {ts}[{i}].run = {tr}; {ts}[{i}].clo = &{k}; {ts}[{i}].out = {data}; {ts}[{i}].idx = {idx}; }} int64_t pc_ = ss_par({ts}, {n}, st, depth); free({ts}); if (UNLIKELY(pc_)) {{ {handlers}return (RRT){{.code = pc_}}; }} }} "))
    }

    pub(super) fn chan_for(&mut self, p: &Pat, it: &Expr, body: &Expr) -> G {
        let et = elem(&self.ty(it)?, "Chan").ok_or("for loop over a non-channel")?;
        let ec = self.cty(&et)?;
        let cv = self.expr(it)?;
        let (c, item) = (self.fresh("fc"), self.fresh("fx"));
        self.scopes.push(HashMap::new());
        let mut conds = Vec::new();
        let mut binds = Vec::new();
        if let Err(e) = self.pattern(p, &item, &et, &mut conds, &mut binds) {
            self.scopes.pop();
            return Err(e);
        }
        let mut decl = String::new();
        for (n, expr, bt) in binds {
            if self.drop_fn(&bt).is_some() {
                match self.owned_decl(&n, &bt, &expr) {
                    Ok(d) => decl.push_str(&d),
                    Err(e) => {
                        self.scopes.pop();
                        return Err(e);
                    }
                }
                continue;
            }
            let v = self.bind(&n, bt);
            write!(decl, "__auto_type {v} = {expr}; ").unwrap();
        }
        let body = self.expr(body);
        self.scopes.pop();
        let cond = if conds.is_empty() { "1".to_string() } else { conds.join(" && ") };
        Ok(format!("{{ SsChan* {c} = {cv}; {ec} {item}; while (ss_recv({c}, &{item}, st)) {{ if ({cond}) {{ {decl}(void)({}); }} }} }} ", body?))
    }

    pub(super) fn conc_global(&mut self, name: &str, args: &[Expr], t: &Type) -> G<Option<String>> {
        Ok(Some(match name {
            "atomic" => format!("ss_atomic({})", self.expr(&args[0])?),
            "chan" => {
                let et = elem(t, "Chan").ok_or("chan without an element type")?;
                format!("ss_chan(sizeof({}))", self.cty(&et)?)
            }
            _ => return Ok(None),
        }))
    }

    pub(super) fn conc_method(&mut self, r: &str, rt: &Type, name: &str, args: &[Expr], t: &Type) -> G {
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        if elem(rt, "Atomic").is_some() {
            return Ok(match (name, vals.as_slice()) {
                ("load", []) => format!("__atomic_load_n(&({r})->v, __ATOMIC_SEQ_CST)"),
                ("store", [v]) => format!("({{ SsAtom* a_ = {r}; int64_t v_ = {v}; __atomic_store_n(&a_->v, v_, __ATOMIC_SEQ_CST); 0LL; }})"),
                ("add", [v]) => format!("({{ SsAtom* a_ = {r}; int64_t v_ = {v}; ss_add(a_, v_, st); }})"),
                ("cas", [e, v]) => format!("({{ SsAtom* a_ = {r}; int64_t ce_ = {e}; int64_t v_ = {v}; ss_cas(a_, ce_, v_); }})"),
                _ => return Err(format!("uses Atomic.{name}")),
            });
        }
        let et = elem(rt, "Chan").ok_or("channel method on a non-channel")?;
        let ec = self.cty(&et)?;
        Ok(match (name, vals.as_slice()) {
            ("send", [v]) => format!("({{ SsChan* c_ = {r}; {ec} v_ = {v}; ss_send(c_, &v_, st); 0LL; }})"),
            ("recv", []) => {
                let oc = self.cty(t)?;
                format!("({{ SsChan* c_ = {r}; {oc} o_ = {{0}}; o_.some = ss_recv(c_, &o_.v, st); o_; }})")
            }
            ("close", []) => format!("({{ ss_close({r}); 0LL; }})"),
            _ => return Err(format!("uses Chan.{name}")),
        })
    }
}
