use super::*;

impl Cx<'_> {
    fn list_of(&mut self, et: &Type, n: &str, data: &str, hdr: &str) -> G {
        let lc = self.cty(&Type::app("List", vec![et.clone()]))?;
        let ec = self.decl(et)?;
        Ok(format!("({lc}){{{n}, ({ec}*){data}, {hdr}}}"))
    }

    pub(super) fn flat_global(&mut self, n: &str, vals: &[String], t: &Type) -> G<Option<String>> {
        Ok(Some(match n {
            "empty_flat_map" => format!("(({}){{0}})", self.cty(t)?),
            "mdspan" => {
                self.std("md");
                let mc = self.cty(t)?;
                let ints = self.list_of(&Type::int(), "s_.len", "s_.data", "s_.hdr")?;
                format!("({{ __auto_type d_ = {}; __auto_type h_ = {}; RawL s_ = ss_md_new(h_.data, h_.len, d_.len, st); ({mc}){{d_, 0, h_, {ints}}}; }})", vals[0], vals[1])
            }
            _ => return Ok(None),
        }))
    }

    pub(super) fn flat_from_pairs(&mut self, l: &str, t: &Type) -> G {
        let Type::Con(_, a) = t else { return Err("bad flat map type".into()) };
        let (kt, vt) = (a[0].clone(), a[1].clone());
        let (mm, node) = self.map_helpers(&Type::app("Map", vec![kt.clone(), vt.clone()]))?;
        let fc = self.cty(t)?;
        let (kc, vc) = (self.decl(&kt)?, self.decl(&vt)?);
        let (kl, vl) = (self.list_of(&kt, "n_", "ka_.data", "ka_.hdr")?, self.list_of(&vt, "n_", "va_.data", "va_.hdr")?);
        Ok(format!("({{ __auto_type p_ = {l}; {node}* t_ = 0; for (int64_t i_ = 0; i_ < p_.len; i_++) t_ = mput_{mm}(t_, p_.data[i_].f0, p_.data[i_].f1, sspur_prio()); int64_t n_ = sz_{mm}(t_); {node}** xs_ = ({node}**)sspur_alloc((size_t)(n_ + 1) * sizeof({node}*)); int64_t c_ = 0; fill_{mm}(t_, xs_, &c_); RawL ka_ = raw_alloc(n_, sizeof({kc})); RawL va_ = raw_alloc(n_, sizeof({vc})); for (int64_t i_ = 0; i_ < n_; i_++) {{ (({kc}*)ka_.data)[i_] = xs_[i_]->k; (({vc}*)va_.data)[i_] = xs_[i_]->v; }} ka_.hdr[1] = n_; va_.hdr[1] = n_; ({fc}){{{kl}, {vl}}}; }})"))
    }

    pub(super) fn flat_method(&mut self, r: &str, rt: &Type, name: &str, args: &[Expr], t: &Type) -> G {
        let Type::Con(_, a) = rt else { return Err("bad flat map type".into()) };
        let (kt, vt) = (a[0].clone(), a[1].clone());
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let cmp = self.helper_cmp(&kt)?;
        let fc = self.cty(rt)?;
        let (kc, vc) = (self.decl(&kt)?, self.decl(&vt)?);
        let head = format!("__auto_type m_ = {r}; int64_t n_ = m_.keys.len < m_.values.len ? m_.keys.len : m_.values.len; ");
        let bound = |k: &str, o: &str| format!("int64_t {o} = 0; {{ int64_t hi_ = n_; while ({o} < hi_) {{ int64_t md_ = {o} + (hi_ - {o}) / 2; if ({cmp}(m_.keys.data[md_], {k}) < 0) {o} = md_ + 1; else hi_ = md_; }} }} ");
        let lower = |k: &str| bound(k, "lo_");
        let find = |k: &str| format!("{}int found_ = lo_ < n_ && {cmp}(m_.keys.data[lo_], {k}) == 0; ", lower(k));
        let (kl, vl) = (self.list_of(&kt, "q_", "ka_.data", "ka_.hdr")?, self.list_of(&vt, "q_", "va_.data", "va_.hdr")?);
        let copy = format!("RawL ka_ = raw_alloc(q_, sizeof({kc})); RawL va_ = raw_alloc(q_, sizeof({vc})); {kc}* kd_ = ({kc}*)ka_.data; {vc}* vd_ = ({vc}*)va_.data; ka_.hdr[1] = q_; va_.hdr[1] = q_; ");
        let build = format!("({fc}){{{kl}, {vl}}}");
        Ok(match name {
            "len" => format!("({{ {head}n_; }})"),
            "is_empty" => format!("({{ {head}(int64_t)(n_ == 0); }})"),
            "lower_bound" => format!("({{ {head}__auto_type k_ = {}; {}lo_; }})", vals[0], lower("k_")),
            "has" => format!("({{ {head}__auto_type k_ = {}; {}(int64_t)found_; }})", vals[0], find("k_")),
            "get" => {
                let oc = self.cty(t)?;
                format!("({{ {head}__auto_type k_ = {}; {}{oc} o_; memset(&o_, 0, sizeof o_); if (found_) {{ o_.some = 1; o_.v = m_.values.data[lo_]; }} o_; }})", vals[0], find("k_"))
            }
            "items" => {
                let lt = t.clone();
                let et = elem(&lt, "List").ok_or("bad items type")?;
                let ec = self.decl(&et)?;
                let out = self.list_of(&et, "n_", "a_.data", "a_.hdr")?;
                format!("({{ {head}RawL a_ = raw_alloc(n_, sizeof({ec})); for (int64_t i_ = 0; i_ < n_; i_++) {{ (({ec}*)a_.data)[i_].f0 = m_.keys.data[i_]; (({ec}*)a_.data)[i_].f1 = m_.values.data[i_]; }} a_.hdr[1] = n_; {out}; }})")
            }
            "put" => format!("({{ {head}__auto_type k_ = {}; __auto_type v_ = {}; {}int64_t q_ = n_ + !found_; {copy}for (int64_t i_ = 0; i_ < lo_; i_++) {{ kd_[i_] = m_.keys.data[i_]; vd_[i_] = m_.values.data[i_]; }} kd_[lo_] = k_; vd_[lo_] = v_; for (int64_t i_ = lo_ + found_; i_ < n_; i_++) {{ kd_[i_ + !found_] = m_.keys.data[i_]; vd_[i_ + !found_] = m_.values.data[i_]; }} {build}; }})", vals[0], vals[1], find("k_")),
            "remove" => format!("({{ {head}__auto_type k_ = {}; {}int64_t q_ = n_ - found_; {copy}for (int64_t i_ = 0, j_ = 0; i_ < n_; i_++) if (!found_ || i_ != lo_) {{ kd_[j_] = m_.keys.data[i_]; vd_[j_] = m_.values.data[i_]; j_++; }} {build}; }})", vals[0], find("k_")),
            "between" => {
                let (ks, vs) = (self.list_of(&kt, "b_ - a_", "(m_.keys.data + a_)", "m_.keys.hdr")?, self.list_of(&vt, "b_ - a_", "(m_.values.data + a_)", "m_.values.hdr")?);
                format!("({{ {head}__auto_type x_ = {}; __auto_type y_ = {}; {}{}if (b_ < a_) b_ = a_; ({fc}){{{ks}, {vs}}}; }})", vals[0], vals[1], bound("x_", "a_"), bound("y_", "b_"))
            }
            _ => return Err(format!("uses FlatMap.{name}")),
        })
    }

    pub(super) fn md_method(&mut self, r: &str, rt: &Type, name: &str, args: &[Expr]) -> G {
        self.std("md");
        let et = arg0_of(rt);
        let vals: Vec<String> = args.iter().map(|a| self.expr(a)).collect::<G<_>>()?;
        let ec = self.decl(&et)?;
        let head = format!("__auto_type s_ = {r}; ");
        let chk = "ss_md_check(s_.shape.data, s_.shape.len, s_.strides.len, st); ";
        let ints = |me: &mut Self, v: &str| me.list_of(&Type::int(), &format!("{v}.len"), &format!("{v}.data"), &format!("{v}.hdr"));
        let index = "int64_t f_ = ss_md_index(s_.offset, s_.shape.data, s_.strides.data, s_.shape.len, s_.data.len, i_.data, i_.len, st); ";
        Ok(match name {
            "rank" => format!("({{ {head}{chk}s_.shape.len; }})"),
            "size" => format!("({{ {head}{chk}ss_md_prod(s_.shape.data, s_.shape.len, st); }})"),
            "get" => format!("({{ {head}__auto_type i_ = {}; {chk}{index}s_.data.data[f_]; }})", vals[0]),
            "set" => {
                let dl = self.list_of(&et, "s_.data.len", "a_.data", "a_.hdr")?;
                format!("({{ {head}__auto_type i_ = {}; __auto_type x_ = {}; {chk}{index}RawL a_ = raw_alloc(s_.data.len, sizeof({ec})); if (s_.data.len) memcpy(a_.data, s_.data.data, (size_t)s_.data.len * sizeof({ec})); (({ec}*)a_.data)[f_] = x_; a_.hdr[1] = s_.data.len; s_.data = {dl}; s_; }})", vals[0], vals[1])
            }
            "transpose" => {
                let (a, b) = (ints(self, "h_")?, ints(self, "k_")?);
                format!("({{ {head}{chk}RawL h_ = ss_md_ints(s_.shape.data, s_.shape.len, 1); RawL k_ = ss_md_ints(s_.strides.data, s_.strides.len, 1); s_.shape = {a}; s_.strides = {b}; s_; }})")
            }
            "slice" => {
                let a = ints(self, "h_")?;
                format!("({{ {head}int64_t d_ = {}; int64_t lo_ = {}; int64_t hi_ = {}; {chk}RawL h_; s_.offset = ss_md_slice(s_.offset, s_.shape.data, s_.strides.data, s_.shape.len, d_, lo_, hi_, &h_, st); s_.shape = {a}; s_; }})", vals[0], vals[1], vals[2])
            }
            "sub" => format!("({{ {head}int64_t i_ = {}; {chk}if (s_.shape.len == 0) ss_md_dim(0, 0, st); if (i_ < 0 || i_ >= s_.shape.data[0]) ss_md_coord(i_, s_.shape.data[0], st); s_.offset = ss_md_shift(s_.offset, i_, s_.strides.data[0], st); s_.shape.len -= 1; s_.shape.data += 1; s_.strides.len -= 1; s_.strides.data += 1; s_; }})", vals[0]),
            "to_list" => {
                let out = self.list_of(&et, "f_.len", "a_.data", "a_.hdr")?;
                format!("({{ {head}{chk}RawL f_ = ss_md_flats(s_.offset, s_.shape.data, s_.strides.data, s_.shape.len, s_.data.len, st); RawL a_ = raw_alloc(f_.len, sizeof({ec})); for (int64_t k_ = 0; k_ < f_.len; k_++) (({ec}*)a_.data)[k_] = s_.data.data[((int64_t*)f_.data)[k_]]; a_.hdr[1] = f_.len; {out}; }})")
            }
            _ => return Err(format!("uses MdSpan.{name}")),
        })
    }
}

fn arg0_of(t: &Type) -> Type {
    match t {
        Type::Con(_, a) if !a.is_empty() => a[0].clone(),
        _ => Type::unit(),
    }
}
