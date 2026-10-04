use super::*;

const DISTS: &[&str] = &["binomial", "poisson", "geometric", "gamma", "beta", "weighted"];

impl Cx<'_> {
    fn rng_draw(&mut self, name: &str, a: &[String]) -> Option<String> {
        Some(match name {
            "next" => "int64_t v_ = (int64_t)(ss_r_next(&q_) >> 1);".into(),
            "int" => format!("int64_t lo_ = {}; int64_t hi_ = {}; int64_t v_ = ss_r_int(&q_, lo_, hi_, st);", a[0], a[1]),
            "f64" => "double v_ = ss_r_f64(&q_);".into(),
            "normal" => format!("double a_ = {}; double b_ = {}; double v_ = ss_r_normal(&q_, a_, b_);", a[0], a[1]),
            "uniform" => format!("double a_ = {}; double b_ = {}; double u_ = ss_r_f64(&q_); double w_ = b_ - a_; double t_ = w_ * u_; double v_ = a_ + t_;", a[0], a[1]),
            "exp" => format!("double a_ = {}; double v_ = -log(1.0 - ss_r_f64(&q_)) / a_;", a[0]),
            "bool" => format!("double a_ = {}; int64_t v_ = ss_r_f64(&q_) < a_;", a[0]),
            "binomial" => format!("int64_t n_ = {}; double p_ = {}; int64_t v_ = ss_r_binom(&q_, n_, p_, st);", a[0], a[1]),
            "poisson" => format!("double m_ = {}; int64_t v_ = ss_r_poisson(&q_, m_, st);", a[0]),
            "geometric" => format!("double p_ = {}; int64_t v_ = ss_r_geom(&q_, p_, st);", a[0]),
            "gamma" => format!("double a_ = {}; double b_ = {}; double v_ = ss_r_gamma(&q_, a_, b_, st);", a[0], a[1]),
            "beta" => format!("double a_ = {}; double b_ = {}; double v_ = ss_r_beta(&q_, a_, b_, st);", a[0], a[1]),
            "weighted" => format!("__auto_type w_ = {}; int64_t v_ = ss_r_weighted(&q_, w_.data, w_.len, st);", a[0]),
            _ => return None,
        })
    }

    pub(super) fn rng_global(&mut self, n: &str, vals: &[String], t: &Type) -> G<Option<String>> {
        if n == "rng" {
            self.std("rng2");
            let rc = self.cty(t)?;
            return Ok(Some(format!("(({rc}){{(int64_t)({}), (int64_t)0x9E3779B97F4A7C15ULL}})", vals[0])));
        }
        let Some(d) = n.strip_prefix("rand_").filter(|d| DISTS.contains(d)) else { return Ok(None) };
        self.std("rng2");
        let c = self.cty(t)?;
        let call = self.rng_draw(d, &vals[1..]).unwrap();
        Ok(Some(format!("({{ SsR q_ = {{(uint64_t)({}), 0x9E3779B97F4A7C15ULL}}; {call} ({c}){{v_, (int64_t)q_.s}}; }})", vals[0])))
    }

    pub(super) fn rng_method(&mut self, r: &str, name: &str, args: &[Expr], t: &Type) -> G {
        self.std("rng2");
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let rc = self.cty(&Type::con("#Rng"))?;
        let c = self.cty(t)?;
        let head = format!("__auto_type r_ = {r}; SsR q_ = {{(uint64_t)r_.state, (uint64_t)r_.gamma}};");
        let back = |q: &str| format!("({rc}){{(int64_t){q}.s, (int64_t){q}.g}}");
        Ok(match name {
            "split" => format!("({{ {head} SsR o_ = ss_r_split(&q_); ({c}){{{}, {}}}; }})", back("q_"), back("o_")),
            "stream" => format!("({{ {head} int64_t k_ = {}; SsR o_ = ss_r_stream(q_, k_); {}; }})", vals[0], back("o_")),
            _ => {
                let call = self.rng_draw(name, &vals).ok_or_else(|| format!("uses Rng.{name}"))?;
                format!("({{ {head} {call} ({c}){{v_, {}}}; }})", back("q_"))
            }
        })
    }
}
