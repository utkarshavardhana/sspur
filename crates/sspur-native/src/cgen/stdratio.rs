use super::*;

impl Cx<'_> {
    pub(super) fn ratio_global(&mut self, n: &str, vals: &[String], t: &Type) -> G<Option<String>> {
        if !matches!(n, "ratio" | "ratio_big" | "parse_ratio") {
            return Ok(None);
        }
        self.std("ratio");
        let rc = self.cty(&Type::con("#Ratio"))?;
        let back = format!("({rc}){{o_.n, o_.d}}");
        Ok(Some(match n {
            "ratio" => format!("({{ SBig a_ = ss_big_from({}); SBig b_ = ss_big_from({}); SRat o_ = ss_rat_mk(a_, b_, st); {back}; }})", vals[0], vals[1]),
            "ratio_big" => format!("({{ SBig a_ = {}; SBig b_ = {}; SRat o_ = ss_rat_mk(a_, b_, st); {back}; }})", vals[0], vals[1]),
            _ => {
                let oc = self.cty(t)?;
                format!("({{ Str s_ = {}; SRat o_; {oc} r_; memset(&r_, 0, sizeof r_); if (ss_rat_parse(s_, &o_, st)) {{ r_.some = 1; r_.v = {back}; }} r_; }})", vals[0])
            }
        }))
    }

    pub(super) fn ratio_method(&mut self, r: &str, name: &str, args: &[Expr]) -> G {
        self.std("ratio");
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let rc = self.cty(&Type::con("#Ratio"))?;
        let head = format!("__auto_type z_ = {r}; SRat a_ = {{z_.num, z_.den}};");
        let back = |e: &str| format!("SRat o_ = {e}; ({rc}){{o_.n, o_.d}};");
        let round = ["floor", "ceil", "trunc", "round"];
        if let Some(k) = round.iter().position(|u| *u == name) {
            return Ok(format!("({{ {head} ss_rat_round(a_, {k}, st); }})"));
        }
        let bins = ["add", "sub", "mul", "div"];
        if let Some(k) = bins.iter().position(|u| *u == name) {
            return Ok(format!("({{ {head} __auto_type w_ = {}; SRat b_ = {{w_.num, w_.den}}; {} }})", vals[0], back(&format!("ss_rat_bin(a_, b_, {k}, st)"))));
        }
        Ok(match name {
            "neg" => format!("({{ {head} {} }})", back("(SRat){ss_big_neg(a_.n), a_.d}")),
            "abs" => format!("({{ {head} {} }})", back("(SRat){ss_big_abs(a_.n), a_.d}")),
            "inv" => format!("({{ {head} {} }})", back("ss_rat_mk(a_.d, a_.n, st)")),
            "pow" => format!("({{ {head} int64_t e_ = {}; {} }})", vals[0], back("ss_rat_pow(a_, e_, st)")),
            "sign" => format!("({{ {head} ss_big_sign(a_.n); }})"),
            "is_int" => format!("({{ {head} (int64_t)ss_big_one(a_.d); }})"),
            "to_f64" => format!("({{ {head} ss_rat_f64(a_, st); }})"),
            "to_dec" => format!("({{ {head} int64_t k_ = {}; SDec x_ = ss_dec_mk(a_.n, 0); SDec y_ = ss_dec_mk(a_.d, 0); ss_dec_div(x_, y_, k_, st); }})", vals[0]),
            _ => return Err(format!("uses Ratio.{name}")),
        })
    }
}
