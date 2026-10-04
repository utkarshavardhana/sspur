use crate::{Service, Source};
use sspur_check::Type;
use sspur_native::nval::Layouts;
use std::collections::HashMap;
use std::fmt::Write;

const PRELUDE: &str = include_str!("rt_prelude.c");

fn c_str(s: &str) -> String {
    let mut out = String::from("\"");
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b" _-./{}[]:,()<>=!+*'".contains(&b) {
            out.push(b as char);
        } else {
            write!(out, "\\{b:03o}").unwrap();
        }
    }
    out.push('"');
    out
}

struct Codecs<'a> {
    layouts: &'a Layouts,
    names: HashMap<(char, String), String>,
    protos: String,
    bodies: String,
}

impl<'a> Codecs<'a> {
    fn new(layouts: &'a Layouts) -> Self {
        Codecs { layouts, names: HashMap::new(), protos: String::new(), bodies: String::new() }
    }

    fn strip(&self, t: &Type) -> Type {
        match t {
            Type::Con(n, a) if n == "Pii" && a.len() == 1 => self.strip(&a[0]),
            Type::Con(n, a) if a.is_empty() && self.layouts.newtypes.contains_key(n) => self.strip(&self.layouts.newtypes[n]),
            _ => t.clone(),
        }
    }

    fn name(&mut self, kind: char, t: &Type) -> (String, bool) {
        let key = (kind, t.to_string());
        if let Some(n) = self.names.get(&key) {
            return (n.clone(), false);
        }
        let n = format!("rt_{kind}{}", self.names.len());
        self.names.insert(key, n.clone());
        (n, true)
    }

    fn enc(&mut self, t: &Type) -> Result<String, String> {
        let t = self.strip(t);
        let (name, fresh) = self.name('e', &t);
        if !fresh {
            return Ok(name);
        }
        writeln!(self.protos, "static void {name}(const int64_t** p, RtB* o);").unwrap();
        let body = match &t {
            Type::Con(n, a) => match (n.as_str(), a.as_slice()) {
                ("Int", _) => "rt_i64(o, *(*p)++);".to_string(),
                ("Bool", _) => "rt_s(o, *(*p)++ ? \"true\" : \"false\");".to_string(),
                ("F64", _) => "int64_t w = *(*p)++; double d; memcpy(&d, &w, 8); rt_f64(o, d);".to_string(),
                ("Unit", _) => "(*p)++; rt_s(o, \"null\");".to_string(),
                ("Str", _) => "int64_t n = *(*p)++; rt_jstr(o, (const char*)*p, (size_t)n); *p += (n + 7) / 8;".to_string(),
                ("List", [x]) => {
                    let e = self.enc(x)?;
                    format!("int64_t n = *(*p)++; rt_s(o, \"[\"); for (int64_t i = 0; i < n; i++) {{ if (i) rt_s(o, \",\"); {e}(p, o); }} rt_s(o, \"]\");")
                }
                ("Opt", [x]) => {
                    let e = self.enc(x)?;
                    format!("if (*(*p)++) {e}(p, o); else rt_s(o, \"null\");")
                }
                ("Map", [k, v]) => {
                    let (ek, ev) = (self.enc(k)?, self.enc(v)?);
                    if self.strip(k) == Type::str() {
                        format!("int64_t n = *(*p)++; rt_s(o, \"{{\"); for (int64_t i = 0; i < n; i++) {{ if (i) rt_s(o, \",\"); {ek}(p, o); rt_s(o, \":\"); {ev}(p, o); }} rt_s(o, \"}}\");")
                    } else {
                        format!("int64_t n = *(*p)++; rt_s(o, \"[\"); for (int64_t i = 0; i < n; i++) {{ if (i) rt_s(o, \",\"); rt_s(o, \"[\"); {ek}(p, o); rt_s(o, \",\"); {ev}(p, o); rt_s(o, \"]\"); }} rt_s(o, \"]\");")
                    }
                }
                _ if self.layouts.records.contains_key(n) => {
                    let fs = self.layouts.record_fields(n, a).unwrap();
                    let mut s = String::from("rt_s(o, \"{\"); ");
                    for (i, (f, ft)) in fs.iter().enumerate() {
                        let e = self.enc(ft)?;
                        write!(s, "rt_s(o, {}); {e}(p, o); ", c_str(&format!("{}\"{f}\":", if i > 0 { "," } else { "" }))).unwrap();
                    }
                    s.push_str("rt_s(o, \"}\");");
                    s
                }
                _ if self.layouts.sums.contains_key(n) => {
                    let vs = self.layouts.sum_variants(n, a).unwrap();
                    let mut s = String::from("switch (*(*p)++) { ");
                    for (k, (v, fs)) in vs.iter().enumerate() {
                        match fs {
                            None => write!(s, "case {k}: rt_s(o, {}); break; ", c_str(&format!("\"{v}\""))).unwrap(),
                            Some(fs) => {
                                write!(s, "case {k}: rt_s(o, {}); ", c_str(&format!("{{\"tag\":\"{v}\""))).unwrap();
                                for (f, ft) in fs {
                                    let e = self.enc(ft)?;
                                    write!(s, "rt_s(o, {}); {e}(p, o); ", c_str(&format!(",\"{f}\":"))).unwrap();
                                }
                                s.push_str("rt_s(o, \"}\"); break; ");
                            }
                        }
                    }
                    s.push_str("default: rt_s(o, \"null\"); }");
                    s
                }
                _ => return Err(format!("{t} has no JSON form")),
            },
            Type::Tuple(xs) => {
                let mut s = String::from("rt_s(o, \"[\"); ");
                for (i, x) in xs.iter().enumerate() {
                    let e = self.enc(x)?;
                    if i > 0 {
                        s.push_str("rt_s(o, \",\"); ");
                    }
                    write!(s, "{e}(p, o); ").unwrap();
                }
                s.push_str("rt_s(o, \"]\");");
                s
            }
            _ => return Err(format!("{t} has no JSON form")),
        };
        writeln!(self.bodies, "static void {name}(const int64_t** p, RtB* o) {{ {body} }}").unwrap();
        Ok(name)
    }

    fn dec(&mut self, t: &Type) -> Result<String, String> {
        let t = self.strip(t);
        let (name, fresh) = self.name('d', &t);
        if !fresh {
            return Ok(name);
        }
        writeln!(self.protos, "static int {name}(RtJ* v, Buf* b);").unwrap();
        let want = c_str(&t.to_string());
        let body = match &t {
            Type::Con(n, a) => match (n.as_str(), a.as_slice()) {
                ("Int", _) => format!("int64_t x; if (!rt_num_i64(v, &x)) return rt_jerr({want}, v); buf_push(b, x); return 1;"),
                ("F64", _) => format!("double d; if (!rt_num_f64(v, &d)) return rt_jerr({want}, v); int64_t w; memcpy(&w, &d, 8); buf_push(b, w); return 1;"),
                ("Bool", _) => format!("if (!v || (v->t != RJ_TRUE && v->t != RJ_FALSE)) return rt_jerr({want}, v); buf_push(b, v->t == RJ_TRUE); return 1;"),
                ("Unit", _) => "(void)v; buf_push(b, 0); return 1;".to_string(),
                ("Str", _) => format!("if (!v || v->t != RJ_STR || !rt_utf8_ok(v->s, v->len)) return rt_jerr({want}, v); rt_wstr(b, v->s, v->len); return 1;"),
                ("List", [x]) => {
                    let d = self.dec(x)?;
                    format!("if (!v || v->t != RJ_ARR) return rt_jerr({want}, v); buf_push(b, v->len); for (int64_t i = 0; i < v->len; i++) {{ rt_pushi(i); if (!{d}(&v->items[i], b)) return 0; rt_pop(); }} return 1;")
                }
                ("Opt", [x]) => {
                    let d = self.dec(x)?;
                    format!("if (!v || v->t == RJ_NULL) {{ buf_push(b, 0); return 1; }} buf_push(b, 1); return {d}(v, b);")
                }
                ("Map", [k, val]) => {
                    let (dk, dv) = (self.dec(k)?, self.dec(val)?);
                    if self.strip(k) == Type::str() {
                        format!("if (!v || v->t != RJ_OBJ) return rt_jerr({want}, v); buf_push(b, v->len); for (int64_t i = 0; i < v->len; i++) {{ RtJ kj = {{0}}; kj.t = RJ_STR; kj.s = v->keys[i]; kj.len = v->klens[i]; if (!{dk}(&kj, b)) return 0; rt_push(v->keys[i]); if (!{dv}(&v->items[i], b)) return 0; rt_pop(); }} return 1;")
                    } else {
                        format!("if (!v || v->t != RJ_ARR) return rt_jerr({want}, v); buf_push(b, v->len); for (int64_t i = 0; i < v->len; i++) {{ RtJ* e = &v->items[i]; rt_pushi(i); if (e->t != RJ_ARR || e->len != 2) return rt_jerr(\"[key, value]\", e); if (!{dk}(&e->items[0], b) || !{dv}(&e->items[1], b)) return 0; rt_pop(); }} return 1;")
                    }
                }
                _ if self.layouts.records.contains_key(n) => {
                    let fs = self.layouts.record_fields(n, a).unwrap();
                    let mut s = format!("if (!v || v->t != RJ_OBJ) return rt_jerr({want}, v); ");
                    for (f, ft) in &fs {
                        let d = self.dec(ft)?;
                        write!(s, "rt_push({}); if (!{d}(rt_get(v, {}), b)) return 0; rt_pop(); ", c_str(f), c_str(f)).unwrap();
                    }
                    s.push_str("return 1;");
                    s
                }
                _ if self.layouts.sums.contains_key(n) => {
                    let vs = self.layouts.sum_variants(n, a).unwrap();
                    let mut s = format!("RtJ* tag = v && v->t == RJ_OBJ ? rt_get(v, \"tag\") : v; if (!tag || tag->t != RJ_STR) return rt_jerr({want}, v); ");
                    for (k, (vn, fs)) in vs.iter().enumerate() {
                        write!(s, "if (rt_eq(tag, {})) {{ buf_push(b, {k}); ", c_str(vn)).unwrap();
                        for (f, ft) in fs.iter().flatten() {
                            let d = self.dec(ft)?;
                            write!(s, "rt_push({}); if (!{d}(rt_get(v, {}), b)) return 0; rt_pop(); ", c_str(f), c_str(f)).unwrap();
                        }
                        s.push_str("return 1; } ");
                    }
                    write!(s, "return rt_jerr({}, tag);", c_str(&format!("one of {}", vs.iter().map(|v| v.0.as_str()).collect::<Vec<_>>().join(", ")))).unwrap();
                    s
                }
                _ => return Err(format!("{t} has no JSON form")),
            },
            Type::Tuple(xs) => {
                let mut s = format!("if (!v || v->t != RJ_ARR || v->len != {}) return rt_jerr({want}, v); ", xs.len());
                for (i, x) in xs.iter().enumerate() {
                    let d = self.dec(x)?;
                    write!(s, "rt_pushi({i}); if (!{d}(&v->items[{i}], b)) return 0; rt_pop(); ").unwrap();
                }
                s.push_str("return 1;");
                s
            }
            _ => return Err(format!("{t} has no JSON form")),
        };
        writeln!(self.bodies, "static int {name}(RtJ* v, Buf* b) {{ {body} }}").unwrap();
        Ok(name)
    }

    fn key(&mut self, t: &Type) -> Result<String, String> {
        let t = self.strip(t);
        let (name, fresh) = self.name('k', &t);
        if !fresh {
            return Ok(name);
        }
        let body = if t == Type::int() {
            "rt_s(o, \"{\\\"N\\\":\\\"\"); rt_i64(o, *(*p)++); rt_s(o, \"\\\"}\");"
        } else if t == Type::str() {
            "rt_s(o, \"{\\\"S\\\":\"); int64_t n = *(*p)++; rt_jstr(o, (const char*)*p, (size_t)n); *p += (n + 7) / 8; rt_s(o, \"}\");"
        } else {
            return Err(format!("{t} cannot be a table key"));
        };
        writeln!(self.protos, "static void {name}(const int64_t** p, RtB* o);").unwrap();
        writeln!(self.bodies, "static void {name}(const int64_t** p, RtB* o) {{ {body} }}").unwrap();
        Ok(name)
    }
}

pub fn status_for(variant: &str) -> u16 {
    match variant {
        "NotFound" | "Missing" => 404,
        "Conflict" | "Exists" | "AlreadyExists" => 409,
        "Forbidden" | "Denied" => 403,
        "Unauthorized" => 401,
        "BadRequest" | "Invalid" => 400,
        "TooMany" | "RateLimited" => 429,
        _ => 422,
    }
}

pub fn generate(svc: &Service) -> Result<String, String> {
    let mut g = Codecs::new(&svc.layouts);
    let mut tail = String::new();
    let mut stores = Vec::new();
    for (i, s) in svc.stores.iter().enumerate() {
        let k = g.key(&s.key)?;
        let e = g.enc(&s.val)?;
        let d = g.dec(&s.val)?;
        let mut arrays = Vec::new();
        for (tag, ms) in [("migs", &s.migs), ("backs", &s.backs)] {
            if ms.is_empty() {
                arrays.push("0, 0".to_string());
                continue;
            }
            let mut items = Vec::new();
            for m in ms {
                items.push(format!("{{{}, {}, {}, sspur_wentry_{}}}", c_str(&m.sv), g.dec(&m.ty)?, g.enc(&m.ty)?, m.fun));
            }
            writeln!(tail, "static const RtMig rt_{tag}{i}[] = {{{}}};", items.join(", ")).unwrap();
            arrays.push(format!("rt_{tag}{i}, {}", ms.len()));
        }
        stores.push(format!("{{{}, {k}, {e}, {d}, {}, {}, {}}}", c_str(&s.name), c_str(&s.val.to_string()), c_str(&s.sv), arrays.join(", ")));
    }
    writeln!(tail, "static const RtStore rt_stores[] = {{{}}};", if stores.is_empty() { "{0}".to_string() } else { stores.join(", ") }).unwrap();
    writeln!(tail, "static const RtStore* rt_store(const char* name) {{ for (size_t i = 0; i < {}; i++) if (!strcmp(rt_stores[i].name, name)) return &rt_stores[i]; return 0; }}", svc.stores.len()).unwrap();

    let mut hs = Vec::new();
    for h in &svc.handlers {
        let mut args = format!("static int rt_args_{}(RtJ* ev, Buf* b) {{ (void)ev; (void)b; RtJ* body = 0; ", h.name);
        if h.params.iter().any(|p| p.2 == Source::Body) {
            args.push_str("if (!rt_body(ev, &body)) { snprintf(rt_errbuf, sizeof rt_errbuf, \"request body is not valid JSON\"); return 0; } ");
        }
        for (name, t, src) in &h.params {
            let d = g.dec(t)?;
            match src {
                Source::Path => {
                    let num = i32::from(g.strip(t) == Type::int());
                    write!(args, "rt_np = 0; rt_push({}); if (!{d}(rt_pathp(ev, {}, {num}), b)) return 0; ", c_str(name), c_str(name)).unwrap();
                }
                Source::Body => write!(args, "rt_np = 0; rt_push(\"body\"); if (!{d}(body, b)) return 0; ").unwrap(),
            }
        }
        args.push_str("return 1; }");
        writeln!(tail, "{args}").unwrap();
        let routes: Vec<String> = svc.routes.iter().filter(|r| r.handler == h.name).map(|r| c_str(&r.key())).collect();
        writeln!(tail, "static const char* const rt_routes_{}[] = {{{}}};", h.name, routes.join(", ")).unwrap();
        let (kind, ret) = match &h.ret {
            Type::Con(n, a) if n == "Opt" && a.len() == 1 => (1, a[0].clone()),
            Type::Con(n, _) if n == "Unit" => (2, h.ret.clone()),
            t => (0, t.clone()),
        };
        let enc = g.enc(&ret)?;
        let ok = if svc.routes.iter().any(|r| r.handler == h.name && r.method == "post") { 201 } else { 200 };
        let validate = h.validator.as_ref().map_or("0".to_string(), |v| format!("sspur_wentry_{v}"));
        hs.push(format!("{{{}, rt_routes_{n}, {}, rt_args_{n}, {validate}, sspur_wentry_{n}, {enc}, {kind}, {ok}}}", c_str(&h.name), routes.len(), n = h.name));
    }
    writeln!(tail, "static const RtHandler rt_handlers[] = {{{}}};", hs.join(", ")).unwrap();
    let nh = svc.handlers.len();
    writeln!(tail, "static const RtHandler* rt_handler_named(const char* name) {{ for (size_t i = 0; i < {nh}; i++) if (!strcmp(rt_handlers[i].name, name)) return &rt_handlers[i]; return 0; }}").unwrap();
    writeln!(tail, "static const RtHandler* rt_handler_for(RtJ* route) {{ if (!route || route->t != RJ_STR) return 0; for (size_t i = 0; i < {nh}; i++) for (int j = 0; j < rt_handlers[i].nroutes; j++) if (rt_eq(route, rt_handlers[i].routes[j])) return &rt_handlers[i]; return 0; }}").unwrap();

    let mut errs = String::from("static int rt_err_json(int64_t ty, const int64_t* p, RtB* o) { switch (ty) { ");
    for (i, t) in svc.program.err_types.iter().enumerate() {
        let Ok(e) = g.enc(t) else { continue };
        let statuses: Vec<String> = match t {
            Type::Con(n, a) => svc.layouts.sum_variants(n, a).map(|vs| vs.iter().map(|(v, _)| status_for(v).to_string()).collect()).unwrap_or_default(),
            _ => vec![],
        };
        if statuses.is_empty() {
            write!(errs, "case {i}: {e}(&p, o); return 422; ").unwrap();
        } else {
            write!(errs, "case {i}: {{ static const int st[] = {{{}}}; int64_t tag = *p; {e}(&p, o); return tag >= 0 && tag < {} ? st[tag] : 422; }} ", statuses.join(", "), statuses.len()).unwrap();
        }
    }
    errs.push_str("default: rt_s(o, \"\\\"error\\\"\"); return 500; } }");
    writeln!(tail, "{errs}").unwrap();
    let refines: Vec<String> = svc.program.refines.iter().map(|(ctx, e, _)| c_str(&format!("{ctx} where {e}"))).collect();
    writeln!(tail, "static const char* const rt_refines[] = {{{}}};", if refines.is_empty() { "0".to_string() } else { refines.join(", ") }).unwrap();
    writeln!(tail, "static const char* rt_refine_text(int64_t c) {{ return c >= 0 && c < {} ? rt_refines[c] : 0; }}", refines.len()).unwrap();

    let mut out = format!("/* generated by sspur deploy plan: svc {} #{}. Do not edit; change the SSPUR source instead. */\n", svc.name, svc.hash);
    out.push_str(&svc.program.src);
    out.push_str(PRELUDE);
    out.push_str(&g.protos);
    out.push_str(&g.bodies);
    out.push_str(&tail);
    Ok(out)
}
