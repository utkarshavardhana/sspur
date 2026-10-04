use super::*;

impl Cx<'_> {
    fn res_wrap(&mut self, t: &Type, body: &str, ok: &str, v: Option<&str>) -> G {
        let c = self.cty(t)?;
        let set = v.map_or(String::new(), |v| format!(" r_.v = {v};"));
        Ok(format!("({{ {c} r_; memset(&r_, 0, sizeof r_); Str e_ = {{0, 0}}; {body} if ({ok}) {{ r_.ok = 1;{set} }} else r_.e = e_; r_; }})"))
    }

    pub(super) fn file_global(&mut self, n: &str, args: &[Expr], vals: &[String], t: &Type) -> G<Option<String>> {
        if !matches!(n, "open_file" | "with_file") {
            return Ok(None);
        }
        self.std("file");
        let ft = Type::con("#File");
        let fc = self.cty(&ft)?;
        let rec = |h: &str| format!("({fc}){{{h}.fd, {h}.path, {h}.dev, {h}.ino}}");
        if n == "open_file" {
            let body = format!("Str p_ = {}; Str m_ = {}; SsF h_; int k_ = ss_f_open(p_, m_, &h_, &e_, st);", vals[0], vals[1]);
            return self.res_wrap(t, &body, "k_", Some(&rec("h_"))).map(Some);
        }
        let fv = self.fresh("fh");
        let call = self.apply(&args[2], vec![(fv.clone(), ft)])?;
        let c = self.cty(t)?;
        Ok(Some(format!(
            "({{ {c} r_; memset(&r_, 0, sizeof r_); Str e_ = {{0, 0}}; Str p_ = {}; Str m_ = {}; SsF hf_ __attribute__((cleanup(ss_f_cleanup))) = {{0}}; if (ss_f_open(p_, m_, &hf_, &e_, st)) {{ __auto_type {fv} = {}; r_.v = {call}; r_.ok = 1; }} else r_.e = e_; r_; }})",
            vals[0],
            vals[1],
            rec("hf_")
        )))
    }

    pub(super) fn file_method(&mut self, r: &str, name: &str, args: &[Expr], t: &Type) -> G {
        self.std("file");
        let head = format!("__auto_type rf_ = {r}; SsF f_ = {{rf_.fd, rf_.path, rf_.dev, rf_.ino}};");
        if name == "fold_lines" {
            let at = self.ty(&args[0])?;
            let init = self.expr(&args[0])?;
            let (acc, line) = (self.fresh("acc"), self.fresh("ln"));
            let ac = self.cty(&at)?;
            let body = self.apply(&args[1], vec![(acc.clone(), at), (line.clone(), Type::con("Str"))])?;
            let c = self.cty(t)?;
            return Ok(format!("({{ {head} {ac} {acc} = {init}; {c} r_; memset(&r_, 0, sizeof r_); Str e_ = {{0, 0}}; if (ss_f_live(f_, &e_)) {{ int ok_ = 1; for (;;) {{ Str {line}; int g_ = 0; if (!ss_f_line(f_, &{line}, &g_, &e_)) {{ ok_ = 0; break; }} if (!g_) break; {acc} = {body}; }} if (ok_) {{ r_.ok = 1; r_.v = {acc}; }} else r_.e = e_; }} else r_.e = e_; r_; }})"));
        }
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let live = "ss_f_live(f_, &e_) &&";
        let (body, ok, v): (String, String, Option<String>) = match name {
            "read" => {
                let lc = self.cty(&Type::app("List", vec![Type::int()]))?;
                (format!("{head} int64_t n_ = {}; if (n_ < 0) ss_fail(st, \"read size must be >= 0\"); RawL o_ = {{0}};", vals[0]), format!("{live} ss_f_read(f_, n_, &o_, &e_)"), Some(format!("({lc}){{o_.len, (int64_t*)o_.data, o_.hdr}}")))
            }
            "read_line" => {
                let oc = self.cty(&arg0_ty(t))?;
                (format!("{head} Str o_ = {{0, 0}}; int g_ = 0;"), format!("{live} ss_f_line(f_, &o_, &g_, &e_)"), Some(format!("({{ {oc} x_; memset(&x_, 0, sizeof x_); if (g_) {{ x_.some = 1; x_.v = o_; }} x_; }})")))
            }
            "read_all" => (format!("{head} Str o_ = {{0, 0}};"), format!("{live} ss_f_all(f_, &o_, &e_)"), Some("o_".into())),
            "lines" => {
                let lc = self.cty(&arg0_ty(t))?;
                (format!("{head} RawL o_ = raw_alloc(8, sizeof(Str)); int k_ = 0; Str e0_ = {{0, 0}}; if (ss_f_live(f_, &e0_)) {{ k_ = 1; for (;;) {{ Str l_; int g_ = 0; if (!ss_f_line(f_, &l_, &g_, &e0_)) {{ k_ = 0; break; }} if (!g_) break; o_ = raw_push(o_, &l_, sizeof(Str)); }} }} e_ = e0_;"), "k_".into(), Some(format!("({lc}){{o_.len, (Str*)o_.data, o_.hdr}}")))
            }
            "write" => (format!("{head} Str s_ = {};", vals[0]), format!("{live} ss_f_write(f_, s_.p, s_.len, &e_)"), None),
            "write_bytes" => (format!("{head} __auto_type b_ = {};", vals[0]), format!("{live} ss_f_write_bytes(f_, b_.data, b_.len, &e_)"), None),
            "seek" => (format!("{head} int64_t p_ = {}; int64_t o_ = 0;", vals[0]), format!("{live} ss_f_seek(f_, p_, &o_, SEEK_SET, &e_)"), None),
            "tell" => (format!("{head} int64_t o_ = 0;"), format!("{live} ss_f_seek(f_, 0, &o_, SEEK_CUR, &e_)"), Some("o_".into())),
            "size" => (format!("{head} int64_t o_ = 0;"), format!("{live} ss_f_size(f_, &o_, &e_)"), Some("o_".into())),
            "close" => (head.clone(), "ss_f_close(f_, &e_)".into(), None),
            _ => return Err(format!("uses File.{name}")),
        };
        self.res_wrap(t, &body, &ok, v.as_deref())
    }
}

fn arg0_ty(t: &Type) -> Type {
    match t {
        Type::Con(_, a) if !a.is_empty() => a[0].clone(),
        _ => Type::unit(),
    }
}
