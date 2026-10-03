use super::prove::Fact;
use super::*;
use std::collections::BTreeSet;

fn traps(e: &Expr, out: &mut BTreeSet<i64>) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) | ExprKind::Unit | ExprKind::Name(_) | ExprKind::Placeholder => true,
        ExprKind::Field(x, _) => traps(x, out),
        ExprKind::Binary(op, a, b) => {
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul => {
                    out.insert(T_OVERFLOW);
                }
                BinOp::Div | BinOp::Rem => {
                    out.insert(T_OVERFLOW);
                    out.insert(T_DIV_ZERO);
                }
                BinOp::Pow => return false,
                _ => {}
            }
            traps(a, out) && traps(b, out)
        }
        ExprKind::Unary(op, x) => {
            if *op == UnOp::Neg {
                out.insert(T_OVERFLOW);
            }
            traps(x, out)
        }
        ExprKind::If(c, a, b) => traps(c, out) && traps(a, out) && b.as_ref().is_none_or(|b| traps(b, out)),
        ExprKind::Tuple(xs) => xs.iter().all(|x| traps(x, out)),
        ExprKind::Method { recv, name, args, .. } if args.is_empty() && matches!(name.as_str(), "to_f64" | "len") => traps(recv, out),
        _ => false,
    }
}

fn list_of_scalar(t: &Type) -> bool {
    elem(t, "List").is_some_and(|e| scalar(&e))
}

enum End {
    Sum,
    Len,
    Collect,
}

#[derive(Default)]
struct Scan {
    caps: Vec<Expr>,
    calls: bool,
    heavy: bool,
}

struct Plan {
    deferred: bool,
    par: Option<Scan>,
}

impl Cx<'_> {
    pub(super) fn fused(&mut self, e: &Expr, name: &str, recv: &Expr, args: &[Expr], t: &Type) -> Option<G> {
        let end = match name {
            "sum" if args.is_empty() => End::Sum,
            "len" if args.is_empty() => End::Len,
            "map" | "filter" if args.len() == 1 => End::Collect,
            _ => return None,
        };
        let mut stages: Vec<(&str, &Expr, &Expr)> = Vec::new();
        if matches!(end, End::Collect) {
            stages.push((name, &args[0], e));
        }
        let mut cur = recv;
        while let ExprKind::Method { recv: r, name: n, args: a, .. } = &cur.kind {
            if !(n == "map" || n == "filter") || a.len() != 1 || self.check.user_methods.contains(&(cur.span.start, cur.span.end)) {
                break;
            }
            stages.push((n, &a[0], cur));
            cur = r;
        }
        stages.reverse();
        if stages.is_empty() {
            return None;
        }
        let src_t = self.ty(cur).ok()?;
        let et0 = elem(&src_t, "List")?;
        let int_sum = matches!(end, End::Sum) && is(&elem(&self.ty(recv).ok()?, "List")?, "Int");
        let mut risks = Vec::new();
        let mut scans = Vec::new();
        for (_, f, _) in &stages {
            let mut codes = BTreeSet::new();
            let known = matches!(&f.kind, ExprKind::Lambda { body, params, .. } if params.len() == 1 && traps(body, &mut codes));
            let scan = self.par_stage(f);
            if !known && scan.is_none() {
                return None;
            }
            risks.push(known.then_some(codes));
            scans.push(scan);
        }
        let deferred = if risks.iter().any(Option::is_none) {
            if risks.iter().filter(|r| r.as_ref().is_none_or(|c| !c.is_empty())).count() > 1 {
                return None;
            }
            int_sum
        } else {
            let mut codes: BTreeSet<i64> = risks.iter().flatten().flatten().copied().collect();
            if int_sum {
                codes.insert(T_OVERFLOW);
            }
            if codes.len() > 1 {
                return None;
            }
            false
        };
        let par = self.par_plan(cur, &et0, &stages, scans, &end, int_sum, t);
        if stages.len() < 2 && matches!(end, End::Collect) && par.is_none() {
            return None;
        }
        Some(self.fused_code(cur, et0, &stages, end, t, Plan { deferred, par }))
    }

    #[allow(clippy::too_many_arguments)]
    fn par_plan(&self, src: &Expr, et0: &Type, stages: &[(&str, &Expr, &Expr)], scans: Vec<Option<Scan>>, end: &End, int_sum: bool, t: &Type) -> Option<Scan> {
        if self.par_mode || !scalar(et0) {
            return None;
        }
        let ok_end = match end {
            End::Sum => int_sum,
            End::Len => true,
            End::Collect => stages.iter().all(|(k, _, _)| *k == "map") && elem(t, "List").is_some_and(|e| scalar(&e)),
        };
        if !ok_end {
            return None;
        }
        if !matches!(src.kind, ExprKind::Range(..)) && !self.ty(src).is_ok_and(|t| list_of_scalar(&t)) {
            return None;
        }
        let mut all = Scan::default();
        let mut seen = HashSet::new();
        for s in scans {
            let s = s?;
            all.calls |= s.calls;
            all.heavy |= s.heavy;
            for c in s.caps {
                if let ExprKind::Name(n) = &c.kind
                    && seen.insert(n.clone()) {
                        all.caps.push(c);
                    }
            }
        }
        Some(all)
    }

    fn par_stage(&mut self, f: &Expr) -> Option<Scan> {
        let mut sc = Scan::default();
        let mut visiting = HashSet::new();
        let ok = match &f.kind {
            ExprKind::Lambda { params, body, .. } if params.len() == 1 => {
                let mut locals = params.clone();
                self.par_expr(body, &mut locals, true, &mut sc, &mut visiting)
            }
            ExprKind::Name(n) if self.lookup(n).is_none() => {
                sc.calls = true;
                self.check.fn_types.get(n).is_some_and(|(ps, r)| ps.len() == 1 && scalar(&ps[0]) && scalar(r)) && self.par_fn(n, &mut sc, &mut visiting)
            }
            _ => false,
        };
        if !ok {
            return None;
        }
        for n in visiting {
            self.par_memo.insert(n, true);
        }
        Some(sc)
    }

    fn par_fn(&mut self, name: &str, sc: &mut Scan, visiting: &mut HashSet<String>) -> bool {
        if let Some(v) = self.par_memo.get(name) {
            sc.heavy |= self.par_heavy.contains(name);
            return *v;
        }
        if !visiting.insert(name.to_string()) {
            sc.heavy = true;
            return true;
        }
        let mut inner = Scan::default();
        let ok = self.par_fn_in(name, &mut inner, visiting);
        if !ok {
            self.par_memo.insert(name.to_string(), false);
        } else if inner.heavy {
            self.par_heavy.insert(name.to_string());
            sc.heavy = true;
        }
        ok
    }

    fn par_fn_in(&mut self, name: &str, sc: &mut Scan, visiting: &mut HashSet<String>) -> bool {
        let Some(f) = self.all_fns.get(name).cloned() else { return false };
        if !self.eligible.contains(name) || !f.tparams.is_empty() || self.generics.contains_key(name) || f.effects.iter().any(|e| e.name != "div") || matches!(f.body.kind, ExprKind::Table(_)) {
            return false;
        }
        let Some((ps, r)) = self.check.fn_types.get(name).cloned() else { return false };
        if !ps.iter().all(scalar) || !scalar(&r) {
            return false;
        }
        let params: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
        let mut refines: Vec<(&Expr, Vec<String>)> = Vec::new();
        let alias = |ty: &Ty| match ty {
            Ty::Named { name, .. } => self.alias_refines.get(name).cloned(),
            _ => None,
        };
        let mut owned = Vec::new();
        for p in &f.params {
            if let Some(r) = alias(&p.ty) {
                owned.push(r);
            }
        }
        if let Some(r) = f.ret.as_ref().and_then(alias) {
            owned.push(r);
        }
        let mut with_us = params.clone();
        with_us.push("_".into());
        for r in &owned {
            refines.push((r, vec!["_".into()]));
        }
        for p in &f.params {
            if let Some(r) = &p.refine {
                refines.push((r, with_us.clone()));
            }
        }
        for pre in &f.pres {
            refines.push((pre, params.clone()));
        }
        let mut with_r = params.clone();
        with_r.push("r".into());
        for post in &f.posts {
            refines.push((post, with_r.clone()));
        }
        for (e, mut locals) in refines {
            if !self.par_expr(e, &mut locals, false, sc, visiting) {
                return false;
            }
        }
        let mut locals = params;
        self.par_expr(&f.body, &mut locals, false, sc, visiting)
    }

    fn par_call(&mut self, n: &str, args: &[&Expr], locals: &mut Vec<String>, lam: bool, sc: &mut Scan, visiting: &mut HashSet<String>) -> bool {
        if locals.iter().any(|l| l == n) || (lam && self.lookup(n).is_some()) {
            return false;
        }
        for a in args {
            if !self.par_expr(a, locals, lam, sc, visiting) {
                return false;
            }
        }
        if matches!(n, "min" | "max") && args.len() == 2 {
            return true;
        }
        sc.calls = true;
        self.check.fn_types.get(n).is_some_and(|(ps, _)| ps.len() == args.len()) && self.par_fn(n, sc, visiting)
    }

    fn par_expr(&mut self, e: &Expr, locals: &mut Vec<String>, lam: bool, sc: &mut Scan, visiting: &mut HashSet<String>) -> bool {
        let Ok(t) = self.ty(e) else { return false };
        let user = self.check.user_methods.contains(&(e.span.start, e.span.end));
        match &e.kind {
            ExprKind::Index(b, i) => return scalar(&t) && self.list_cap(b, locals, lam, sc) && self.par_expr(i, locals, lam, sc, visiting),
            ExprKind::Method { recv, name, args, .. } if !user && name == "len" && args.is_empty() => return self.list_cap(recv, locals, lam, sc),
            ExprKind::Field(recv, name) if !user && name == "len" => return self.list_cap(recv, locals, lam, sc),
            _ => {}
        }
        if !scalar(&t) {
            return false;
        }
        match &e.kind {
            ExprKind::Int(_) | ExprKind::Float(_) | ExprKind::Bool(_) | ExprKind::Unit => true,
            ExprKind::Placeholder => locals.iter().any(|l| l == "_"),
            ExprKind::Name(n) => {
                if locals.iter().any(|l| l == n) {
                    true
                } else if lam && self.lookup(n).is_some() {
                    sc.caps.push(e.clone());
                    true
                } else {
                    false
                }
            }
            ExprKind::Binary(_, a, b) => self.par_expr(a, locals, lam, sc, visiting) && self.par_expr(b, locals, lam, sc, visiting),
            ExprKind::Unary(_, x) => self.par_expr(x, locals, lam, sc, visiting),
            ExprKind::If(c, a, b) => {
                self.par_expr(c, locals, lam, sc, visiting) && self.par_expr(a, locals, lam, sc, visiting) && b.as_ref().is_none_or(|b| self.par_expr(b, locals, lam, sc, visiting))
            }
            ExprKind::Block(stmts) => {
                let mark = locals.len();
                let ok = stmts.iter().all(|s| self.par_stmt(s, locals, lam, sc, visiting));
                locals.truncate(mark);
                ok
            }
            ExprKind::Match(x, arms) => {
                if !self.par_expr(x, locals, lam, sc, visiting) {
                    return false;
                }
                arms.iter().all(|a| {
                    let mark = locals.len();
                    let ok = match &a.pat {
                        Pat::Wild | Pat::Int(_) | Pat::Bool(_) => true,
                        Pat::Bind(n) => {
                            locals.push(n.clone());
                            true
                        }
                        _ => false,
                    } && a.guard.as_ref().is_none_or(|g| self.par_expr(g, locals, lam, sc, visiting))
                        && self.par_expr(&a.body, locals, lam, sc, visiting);
                    locals.truncate(mark);
                    ok
                })
            }
            ExprKind::Call(f, args) => match &f.kind {
                ExprKind::Name(n) => {
                    let args: Vec<&Expr> = args.iter().collect();
                    self.par_call(n, &args, locals, lam, sc, visiting)
                }
                _ => false,
            },
            ExprKind::Method { recv, name, args, .. } if user => {
                let all: Vec<&Expr> = std::iter::once(&**recv).chain(args.iter()).collect();
                self.par_call(name, &all, locals, lam, sc, visiting)
            }
            ExprKind::Field(recv, name) if user => self.par_call(name, &[&**recv], locals, lam, sc, visiting),
            ExprKind::Method { recv, name, args, .. } if args.is_empty() => self.par_builtin(recv, name, locals, lam, sc, visiting),
            ExprKind::Field(recv, name) => self.par_builtin(recv, name, locals, lam, sc, visiting),
            ExprKind::Return(x) if !lam => self.par_expr(x, locals, lam, sc, visiting),
            _ => false,
        }
    }

    fn par_builtin(&mut self, recv: &Expr, name: &str, locals: &mut Vec<String>, lam: bool, sc: &mut Scan, visiting: &mut HashSet<String>) -> bool {
        let Ok(rt) = self.ty(recv) else { return false };
        let ok = (is(&rt, "Int") && matches!(name, "abs" | "to_f64")) || (is(&rt, "F64") && matches!(name, "abs" | "round" | "floor" | "sqrt"));
        ok && self.par_expr(recv, locals, lam, sc, visiting)
    }

    fn list_cap(&self, b: &Expr, locals: &[String], lam: bool, sc: &mut Scan) -> bool {
        match &b.kind {
            ExprKind::Name(n) if lam && !locals.iter().any(|l| l == n) && self.lookup(n).is_some_and(|(_, t)| list_of_scalar(&t)) => {
                sc.caps.push(b.clone());
                true
            }
            _ => false,
        }
    }

    fn par_stmt(&mut self, s: &Stmt, locals: &mut Vec<String>, lam: bool, sc: &mut Scan, visiting: &mut HashSet<String>) -> bool {
        match s {
            Stmt::Let(Pat::Bind(n), x) | Stmt::Var(n, x) => {
                let ok = self.par_expr(x, locals, lam, sc, visiting);
                locals.push(n.clone());
                ok
            }
            Stmt::Let(Pat::Wild, x) | Stmt::Expr(x) => self.par_expr(x, locals, lam, sc, visiting),
            Stmt::Assign(n, x, _) => locals.iter().any(|l| l == n) && self.par_expr(x, locals, lam, sc, visiting),
            Stmt::While(c, b) => {
                sc.heavy = true;
                self.par_expr(c, locals, lam, sc, visiting) && self.par_expr(b, locals, lam, sc, visiting)
            }
            Stmt::For(Pat::Bind(i), it, b) => {
                let ExprKind::Range(lo, hi) = &it.kind else { return false };
                sc.heavy = true;
                if !self.par_expr(lo, locals, lam, sc, visiting) || !self.par_expr(hi, locals, lam, sc, visiting) {
                    return false;
                }
                let mark = locals.len();
                locals.push(i.clone());
                let ok = self.par_expr(b, locals, lam, sc, visiting);
                locals.truncate(mark);
                ok
            }
            _ => false,
        }
    }

    fn stage_body(&mut self, stages: &[(&str, &Expr, &Expr)], mut x: String, mut et: Type) -> G<(String, String)> {
        let mut body = String::new();
        for (kind, f, stage) in stages {
            let v = self.apply(f, vec![(x.clone(), et.clone())])?;
            if *kind == "map" {
                let nx = self.fresh("x");
                write!(body, "__auto_type {nx} = {v}; ").unwrap();
                x = nx;
                et = self.ty(stage).ok().and_then(|st| elem(&st, "List")).ok_or("bad fused map type")?;
            } else {
                write!(body, "if (!({v})) continue; ").unwrap();
            }
        }
        Ok((body, x))
    }

    fn fused_code(&mut self, src: &Expr, et0: Type, stages: &[(&str, &Expr, &Expr)], end: End, t: &Type, plan: Plan) -> G {
        let (i, n) = (self.fresh("i"), self.fresh("n"));
        let x = self.fresh("x");
        let l = self.fresh("l");
        let mark = self.know.facts.len();
        let mut riv = None;
        let (setup, head) = match &src.kind {
            ExprKind::Range(a, b) => {
                let (av, bv) = (self.expr(a)?, self.expr(b)?);
                riv = Some((self.range(a).0, self.range(b).1 - 1));
                self.for_range_facts(&x, a, b);
                (format!("int64_t s_ = {av}; int64_t re_ = {bv}; int64_t {n} = re_ > s_ ? re_ - s_ : 0; "), format!("int64_t {x} = s_ + {i}; "))
            }
            _ => {
                let lv = self.expr(src)?;
                (format!("__auto_type {l} = {lv}; int64_t {n} = {l}.len; "), format!("__auto_type {x} = {l}.data[{i}]; "))
            }
        };
        let r = self.stage_body(stages, x, et0.clone());
        self.know.facts.truncate(mark);
        let (body, xf) = r?;
        let float = is(t, "F64");
        let (decl, step, fin) = match end {
            End::Sum if float => ("double acc_ = 0.0; ".to_string(), format!("acc_ += {xf}; "), "acc_; ".to_string()),
            End::Sum if plan.deferred => (
                "int64_t acc_ = 0; int ov_ = 0; ".to_string(),
                format!("if (UNLIKELY(__builtin_add_overflow(acc_, {xf}, &acc_))) ov_ = 1; "),
                format!("if (UNLIKELY(ov_)) TRAPV({T_OVERFLOW}, 0, 0); acc_; "),
            ),
            End::Sum => ("int64_t acc_ = 0; ".to_string(), format!("if (UNLIKELY(__builtin_add_overflow(acc_, {xf}, &acc_))) TRAPV({T_OVERFLOW}, 0, 0); "), "acc_; ".to_string()),
            End::Len => ("int64_t acc_ = 0; ".to_string(), "acc_++; ".to_string(), "acc_; ".to_string()),
            End::Collect => {
                let et = elem(t, "List").ok_or("bad fused collect type")?;
                let oc = self.decl(&et)?;
                let lc = self.cty(t)?;
                (format!("RawL r_ = raw_alloc({n}, sizeof({oc})); {oc}* d_ = ({oc}*)r_.data; int64_t k_ = 0; "), format!("d_[k_++] = {xf}; "), format!("r_.hdr[1] = k_; ({lc}){{k_, d_, r_.hdr}}; "))
            }
        };
        let Some(scan) = plan.par else {
            return Ok(format!("({{ {setup}{decl}for (int64_t {i} = 0; {i} < {n}; {i}++) {{ {head}{body}{step}}} {fin}}})"));
        };
        let snap = self.snapshot();
        let worker = match self.par_worker(src, riv, stages, &et0, &end, &scan, &l, t) {
            Ok(w) => w,
            Err(_) => {
                let keep = (std::mem::take(&mut self.know), std::mem::take(&mut self.catch_stack), std::mem::take(&mut self.in_progress));
                self.restore(snap);
                (self.know, self.catch_stack, self.in_progress) = keep;
                return Ok(format!("({{ {setup}{decl}for (int64_t {i} = 0; {i} < {n}; {i}++) {{ {head}{body}{step}}} {fin}}})"));
            }
        };
        let (id, init) = worker;
        let (minn, b0) = if scan.heavy { (16, 1) } else if scan.calls { (1024, 64) } else { (4096, 256) };
        let guard = if plan.deferred { "!ov_ && " } else { "" };
        let combine = match end {
            End::Sum => "int64_t p_ = acc_; for (pc_ = 0; pc_ < nc_; pc_++) { ParRes* r_ = &par_res[pc_]; if (par_stat[pc_] != 1 || (__int128)p_ + r_->mn < INT64_MIN || (__int128)p_ + r_->mx > INT64_MAX) break; p_ += r_->acc; } acc_ = p_; i0_ = par_lo(pc_); ",
            End::Len => "for (pc_ = 0; pc_ < nc_; pc_++) { if (par_stat[pc_] != 1) break; acc_ += par_res[pc_].acc; } i0_ = par_lo(pc_); ",
            End::Collect => "for (pc_ = 0; pc_ < nc_; pc_++) if (par_stat[pc_] != 1) break; i0_ = par_lo(pc_); k_ = i0_; ",
        };
        Ok(format!(
            "({{ {setup}{decl}int64_t i0_ = 0; int64_t m_ = {n}; uint64_t t0_ = 0; \
if (UNLIKELY({n} >= {minn})) {{ if (UNLIKELY(!par_nt)) par_init(); if (par_nt > 1) {{ m_ = {b0}; t0_ = par_now(); }} }} \
for (;;) {{ int64_t h_ = {n} - i0_ > m_ ? i0_ + m_ : {n}; \
for (int64_t {i} = i0_; {i} < h_; {i}++) {{ {head}{body}{step}}} \
i0_ = h_; if (LIKELY_(i0_ >= {n})) break; m_ *= 2; uint64_t el_ = par_now() - t0_; \
if (el_ >= 20000) {{ m_ = {n}; if ({guard}(double)el_ * (double)({n} - i0_) >= 1e5 * (double)i0_) {{ \
struct {id}_c pcx_ = {{ {init} }}; int64_t nc_ = par_run({id}, &pcx_, i0_, {n}); \
if (nc_) {{ int64_t pc_; {combine}par_release(); }} }} }} }} {fin}}})"
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn par_worker(&mut self, src: &Expr, riv: Option<Iv>, stages: &[(&str, &Expr, &Expr)], et0: &Type, end: &End, scan: &Scan, l: &str, t: &Type) -> G<(String, String)> {
        let id = self.fresh("pw");
        let mut fields = String::new();
        let mut init = Vec::new();
        let mut scope = HashMap::new();
        for (k, c) in scan.caps.iter().enumerate() {
            let ExprKind::Name(name) = &c.kind else { return Err("bad capture".into()) };
            let ct = self.ty(c)?;
            let v = self.expr(c)?;
            write!(fields, "{} f{k}; ", self.cty(&ct)?).unwrap();
            init.push(format!(".f{k} = {v}"));
            scope.insert(name.clone(), (format!("pc_->f{k}"), ct));
        }
        let x = self.fresh("x");
        let head = if riv.is_some() {
            fields.push_str("int64_t s; ");
            init.push(".s = s_".into());
            format!("int64_t {x} = pc_->s + i_; ")
        } else {
            write!(fields, "{} l; ", self.cty(&self.ty(src)?)?).unwrap();
            init.push(format!(".l = {l}"));
            format!("__auto_type {x} = pc_->l.data[i_]; ")
        };
        fields.push_str("int64_t depth; ");
        init.push(".depth = depth".into());
        if matches!(end, End::Collect) {
            let oc = self.decl(&elem(t, "List").ok_or("bad collect type")?)?;
            write!(fields, "{oc}* d; ").unwrap();
            init.push(".d = d_".into());
        }
        let rr = self.rr(&self.ret.clone())?;
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![scope]);
        let saved_know = std::mem::take(&mut self.know);
        let saved_decls = std::mem::take(&mut self.fn_decls);
        let saved_catch = std::mem::take(&mut self.catch_stack);
        let saved_tail = std::mem::take(&mut self.tail_spans);
        let saved_boxed = std::mem::take(&mut self.boxed);
        let saved_mode = std::mem::replace(&mut self.par_mode, true);
        if let Some(iv) = riv {
            self.know.facts.push(Fact::Range(x.clone(), iv));
        }
        let r = self.stage_body(stages, x, et0.clone());
        let decls = std::mem::replace(&mut self.fn_decls, saved_decls);
        self.scopes = saved_scopes;
        self.know = saved_know;
        self.catch_stack = saved_catch;
        self.tail_spans = saved_tail;
        self.boxed = saved_boxed;
        self.par_mode = saved_mode;
        let (body, xf) = r?;
        if !decls.is_empty() {
            return Err("worker needs function-level declarations".into());
        }
        let step = match end {
            End::Sum => format!("if (UNLIKELY(__builtin_add_overflow(acc_, {xf}, &acc_))) {{ sspur_jb = sv_; return 0; }} mn_ = acc_ < mn_ ? acc_ : mn_; mx_ = acc_ > mx_ ? acc_ : mx_; "),
            End::Len => "acc_++; ".into(),
            End::Collect => format!("pc_->d[i_] = {xf}; "),
        };
        writeln!(self.protos, "struct {id}_c {{ {fields}}};").unwrap();
        writeln!(
            self.lambdas,
            "#define RRT {rr}\n#define FIDX {}\nstatic int {id}(void* cv_, int64_t lo_, int64_t hi_, ParRes* pr_) {{ struct {id}_c* pc_ = (struct {id}_c*)cv_; Status st_; memset(&st_, 0, sizeof st_); Status* st = &st_; int64_t depth = pc_->depth; (void)depth; \
jmp_buf jb_; jmp_buf* sv_ = sspur_jb; sspur_jb = &jb_; if (setjmp(jb_)) {{ sspur_jb = sv_; return 0; }} \
int64_t acc_ = 0, mn_ = 0, mx_ = 0; for (int64_t i_ = lo_; i_ < hi_; i_++) {{ {head}{body}{step}}} \
sspur_jb = sv_; pr_->acc = acc_; pr_->mn = mn_; pr_->mx = mx_; return 1; }}\n#undef RRT\n#undef FIDX",
            self.fidx
        )
        .unwrap();
        Ok((id, init.join(", ")))
    }
}
