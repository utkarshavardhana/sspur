use super::*;

const UNARY: &[&str] = &["neg", "conj", "exp", "ln", "sqrt", "sin", "cos", "tan", "sinh", "cosh", "tanh", "asin", "acos", "atan", "asinh", "acosh", "atanh"];

impl Cx<'_> {
    pub(super) fn cx_global(&mut self, n: &str, vals: &[String], t: &Type) -> G<Option<String>> {
        if !matches!(n, "complex" | "polar") {
            return Ok(None);
        }
        let rc = self.cty(t)?;
        Ok(Some(if n == "complex" {
            format!("({{ double a_ = {}; double b_ = {}; ({rc}){{a_, b_}}; }})", vals[0], vals[1])
        } else {
            format!("({{ double r_ = {}; double t_ = {}; ({rc}){{r_ * cos(t_), r_ * sin(t_)}}; }})", vals[0], vals[1])
        }))
    }

    pub(super) fn cx_method(&mut self, r: &str, name: &str, args: &[Expr]) -> G {
        self.std("cx");
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let rc = self.cty(&Type::con("#Complex"))?;
        let head = format!("__auto_type z_ = {r}; SsC a_ = {{z_.re, z_.im}};");
        let back = |e: &str| format!("SsC o_ = {e}; ({rc}){{o_.re, o_.im}};");
        if let Some(k) = UNARY.iter().position(|u| *u == name) {
            return Ok(format!("({{ {head} {} }})", back(&format!("ss_cx_un(a_, {k})"))));
        }
        Ok(match name {
            "abs" => format!("({{ {head} ss_cx_abs(a_); }})"),
            "arg" => format!("({{ {head} atan2(a_.im, a_.re); }})"),
            "norm" => format!("({{ {head} double x_ = a_.re * a_.re; double y_ = a_.im * a_.im; x_ + y_; }})"),
            "scale" => format!("({{ {head} double k_ = {}; ({rc}){{a_.re * k_, a_.im * k_}}; }})", vals[0]),
            "add" | "sub" | "mul" | "div" | "pow" => format!("({{ {head} __auto_type w_ = {}; SsC b_ = {{w_.re, w_.im}}; {} }})", vals[0], back(&format!("ss_cx_{name}(a_, b_)"))),
            _ => return Err(format!("uses Complex.{name}")),
        })
    }
}
