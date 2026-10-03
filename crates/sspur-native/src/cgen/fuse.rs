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

enum End {
    Sum,
    Len,
    Collect,
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
        if stages.len() < 2 && (matches!(end, End::Collect) || stages.is_empty()) {
            return None;
        }
        let mut codes = BTreeSet::new();
        for (_, f, _) in &stages {
            let ExprKind::Lambda { body, params, .. } = &f.kind else { return None };
            if params.len() != 1 || !traps(body, &mut codes) {
                return None;
            }
        }
        let src_t = self.ty(cur).ok()?;
        let et0 = elem(&src_t, "List")?;
        if matches!(end, End::Sum) && is(&elem(&self.ty(recv).ok()?, "List")?, "Int") {
            codes.insert(T_OVERFLOW);
        }
        if codes.len() > 1 {
            return None;
        }
        Some(self.fused_code(cur, et0, &stages, end, t))
    }

    fn fused_code(&mut self, src: &Expr, et0: Type, stages: &[(&str, &Expr, &Expr)], end: End, t: &Type) -> G {
        let (i, n) = (self.fresh("i"), self.fresh("n"));
        let mut x = self.fresh("x");
        let mark = self.know.facts.len();
        let (setup, head) = match &src.kind {
            ExprKind::Range(a, b) => {
                let (av, bv) = (self.expr(a)?, self.expr(b)?);
                self.for_range_facts(&x, a, b);
                (format!("int64_t s_ = {av}; int64_t e_ = {bv}; int64_t {n} = e_ > s_ ? e_ - s_ : 0; "), format!("int64_t {x} = s_ + {i}; "))
            }
            _ => {
                let l = self.fresh("l");
                let lv = self.expr(src)?;
                (format!("__auto_type {l} = {lv}; int64_t {n} = {l}.len; "), format!("__auto_type {x} = {l}.data[{i}]; "))
            }
        };
        let mut body = head;
        let mut et = et0;
        let mut out = Ok(());
        for (kind, f, stage) in stages {
            let r = self.apply(f, vec![(x.clone(), et.clone())]);
            let v = match r {
                Ok(v) => v,
                Err(err) => {
                    out = Err(err);
                    break;
                }
            };
            if *kind == "map" {
                let nx = self.fresh("x");
                write!(body, "__auto_type {nx} = {v}; ").unwrap();
                x = nx;
                et = match self.ty(stage).ok().and_then(|st| elem(&st, "List")) {
                    Some(t) => t,
                    None => {
                        out = Err("bad fused map type".into());
                        break;
                    }
                };
            } else {
                write!(body, "if (!({v})) continue; ").unwrap();
            }
        }
        self.know.facts.truncate(mark);
        out?;
        Ok(match end {
            End::Sum if is(&et, "F64") => format!("({{ {setup}double acc_ = 0.0; for (int64_t {i} = 0; {i} < {n}; {i}++) {{ {body}acc_ += {x}; }} acc_; }})"),
            End::Sum => format!("({{ {setup}int64_t acc_ = 0; for (int64_t {i} = 0; {i} < {n}; {i}++) {{ {body}if (UNLIKELY(__builtin_add_overflow(acc_, {x}, &acc_))) TRAPV({T_OVERFLOW}, 0, 0); }} acc_; }})"),
            End::Len => format!("({{ {setup}int64_t acc_ = 0; for (int64_t {i} = 0; {i} < {n}; {i}++) {{ {body}acc_++; }} acc_; }})"),
            End::Collect => {
                let oc = self.decl(&et)?;
                let lc = self.cty(t)?;
                format!("({{ {setup}RawL r_ = raw_alloc({n}, sizeof({oc})); {oc}* d_ = ({oc}*)r_.data; int64_t k_ = 0; for (int64_t {i} = 0; {i} < {n}; {i}++) {{ {body}d_[k_++] = {x}; }} r_.hdr[1] = k_; ({lc}){{k_, d_, r_.hdr}}; }})")
            }
        })
    }
}
