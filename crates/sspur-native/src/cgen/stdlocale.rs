use super::*;

impl Cx<'_> {
    pub(super) fn locale_global(&mut self, n: &str, vals: &[String], t: &Type) -> G<Option<String>> {
        if n != "locale" {
            return Ok(None);
        }
        self.cty(&Type::con("#BigInt"))?;
        self.cty(&Type::con("#Dec"))?;
        self.std("locale");
        let rc = self.cty(&Type::con("#Locale"))?;
        let oc = self.cty(t)?;
        Ok(Some(format!("({{ Str s_ = {}; {oc} r_; memset(&r_, 0, sizeof r_); for (int i_ = 0; i_ < 6; i_++) {{ Str g_ = ss_loc_str(ss_loc_tag[i_]); if (g_.len == s_.len && !memcmp(g_.p, s_.p, (size_t)g_.len)) {{ r_.some = 1; r_.v = ({rc}){{g_}}; }} }} r_; }})", vals[0])))
    }

    pub(super) fn locale_method(&mut self, r: &str, name: &str, args: &[Expr], t: &Type) -> G {
        self.cty(&Type::con("#BigInt"))?;
        self.cty(&Type::con("#Dec"))?;
        self.std("locale");
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let head = format!("int64_t i_ = ss_loc_idx(({r}).tag);");
        let kinds = ["format_date", "format_date_long", "format_time"];
        if let Some(k) = kinds.iter().position(|u| *u == name) {
            return Ok(format!("({{ {head} int64_t t_ = {}; ss_loc_time(i_, {k}, t_); }})", vals[0]));
        }
        Ok(match name {
            "compare" => format!("({{ {head} (void)i_; Str a_ = {}; Str b_ = {}; (int64_t)ss_coll_cmp(a_, b_); }})", vals[0], vals[1]),
            "sort" => {
                let lc = self.cty(t)?;
                format!("({{ {head} (void)i_; __auto_type xs_ = {}; int64_t n_ = xs_.len; RawL r_ = raw_alloc(n_, sizeof(Str)); if (n_) memcpy(r_.data, xs_.data, (size_t)n_ * sizeof(Str)); r_.len = n_; r_.hdr[1] = n_; if (n_ > 1) raw_msort((char*)r_.data, n_, sizeof(Str), ss_coll_cmp_p, (char*)sspur_alloc((size_t)n_ * sizeof(Str))); ({lc}){{r_.len, (Str*)r_.data, r_.hdr}}; }})", vals[0])
            }
            "format_int" => format!("({{ {head} int64_t n_ = {}; SB_INIT(b_); sb_int(&b_, n_); ss_loc_num(i_, sb_done(&b_)); }})", vals[0]),
            "format_f64" => format!("({{ {head} double x_ = {}; int64_t d_ = {}; ss_loc_num(i_, ss_fmt_fixed(x_, d_, st)); }})", vals[0], vals[1]),
            "format_dec" => format!("({{ {head} SDec d_ = {}; ss_loc_num(i_, ss_dec_str(d_)); }})", vals[0]),
            _ => return Err(format!("uses Locale.{name}")),
        })
    }
}
