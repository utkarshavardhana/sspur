use super::*;

impl Cx<'_> {
    fn zone_rec(&mut self, zb: &str) -> G {
        let zc = self.cty(&Type::con("#Zone"))?;
        let mut parts = vec![format!("{zb}.name")];
        for (f, et) in [("tr", Type::int()), ("ix", Type::int()), ("off", Type::int()), ("dst", Type::bool()), ("ab", Type::con("Str")), ("rule", Type::int())] {
            let lc = self.cty(&Type::app("List", vec![et.clone()]))?;
            let ec = self.cty(&et)?;
            parts.push(format!("({lc}){{{zb}.{f}.len, ({ec}*){zb}.{f}.data, {zb}.{f}.hdr}}"));
        }
        Ok(format!("({zc}){{{}}}", parts.join(", ")))
    }

    pub(super) fn tz_global(&mut self, n: &str, vals: &[String], t: &Type) -> G<Option<String>> {
        if !matches!(n, "time_zone" | "local_zone" | "fixed_zone") {
            return Ok(None);
        }
        self.std("tz");
        let rec = self.zone_rec("z_")?;
        Ok(Some(match n {
            "fixed_zone" => format!("({{ int64_t m_ = {}; if (!(-1440 < m_ && m_ < 1440)) ss_fail(st, \"fixed_zone needs -1440 < minutes < 1440\"); SB_INIT(b_); ss_tz_offname(&b_, m_ * 60); SsZB z_ = ss_tz_fixed(sb_done(&b_), m_ * 60); {rec}; }})", vals[0]),
            _ => {
                let c = self.cty(t)?;
                let call = if n == "time_zone" { format!("Str n_ = {}; int k_ = ss_tz_load(n_, &z_, &e_);", vals[0]) } else { "int k_ = ss_tz_local(&z_, &e_);".into() };
                format!("({{ SsZB z_; Str e_ = {{0, 0}}; {call} {c} r_; memset(&r_, 0, sizeof r_); if (k_) {{ r_.ok = 1; r_.v = {rec}; }} else r_.e = e_; r_; }})")
            }
        }))
    }

    fn zone_view(z: &str) -> String {
        format!("SsZ zv_ = {{{z}.trans.len < {z}.idx.len ? {z}.trans.len : {z}.idx.len, {z}.trans.data, {z}.idx.data, {z}.offs.data, {z}.dst.data, {z}.abbrs.data, {z}.tail.len, {z}.tail.data}}; ")
    }

    pub(super) fn tz_method(&mut self, r: &str, name: &str, args: &[Expr], t: &Type) -> G {
        self.std("tz");
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let head = format!("__auto_type z_ = {r}; int64_t t_ = {}; {}", vals[0], Self::zone_view("z_"));
        Ok(match name {
            "offset" => format!("({{ {head}ss_tz_off(&zv_, t_); }})"),
            "abbr" => format!("({{ {head}zv_.ab[ss_tz_type(&zv_, t_)]; }})"),
            "is_dst" => format!("({{ {head}zv_.dst[ss_tz_type(&zv_, t_)]; }})"),
            "local" => format!("({{ {head}ss_tz_localt(&zv_, t_, st); }})"),
            "utc" => {
                let oc = self.cty(t)?;
                format!("({{ {head}int64_t u_; {oc} o_; memset(&o_, 0, sizeof o_); if (ss_tz_utc(&zv_, t_, &u_, st)) {{ o_.some = 1; o_.v = u_; }} o_; }})")
            }
            _ => return Err(format!("uses Zone.{name}")),
        })
    }

    pub(super) fn tz_time_method(&mut self, r: &str, name: &str, args: &[Expr]) -> G {
        self.std("tz");
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let head = format!("int64_t t_ = {r}; __auto_type z_ = {}; ", vals[0]);
        let view = Self::zone_view("z_");
        Ok(if name == "iso_in" {
            format!("({{ {head}{view}ss_tz_iso(&zv_, t_, st); }})")
        } else {
            format!("({{ {head}Str p_ = {}; {view}ss_tz_format(&zv_, t_, p_, st); }})", vals[1])
        })
    }
}
