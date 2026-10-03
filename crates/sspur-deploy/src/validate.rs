use sspur_syntax::printer::{expr as pexpr, ty as pty};
use sspur_syntax::*;
use std::collections::{BTreeMap, HashMap, HashSet};

pub struct Gen<'a> {
    types: HashMap<&'a str, &'a TypeDef>,
    memo: HashMap<String, bool>,
    out: BTreeMap<String, String>,
}

impl<'a> Gen<'a> {
    pub fn new(m: &'a Module) -> Self {
        let types = m.defs.iter().filter_map(|d| if let Def::Type(t) = d { Some((t.name.as_str(), t)) } else { None }).collect();
        Gen { types, memo: HashMap::new(), out: BTreeMap::new() }
    }

    fn fields_of(t: &TypeDef) -> Vec<&Field> {
        match &t.body {
            TypeBody::Record(fs) => fs.iter().collect(),
            TypeBody::Sum(vs) => vs.iter().flat_map(|v| v.fields.iter().flatten()).collect(),
            _ => vec![],
        }
    }

    fn refined_alias(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Named { name, .. } if matches!(self.types.get(name.as_str()).map(|t| &t.body), Some(TypeBody::Alias(_, Some(_)))))
    }

    pub fn needs(&mut self, ty: &Ty) -> bool {
        match ty {
            Ty::Tuple(xs) => xs.iter().any(|x| self.needs(x)),
            Ty::Fn { .. } => false,
            Ty::Named { name, args, .. } => {
                if matches!(name.as_str(), "List" | "Opt" | "Map" | "Pii") {
                    return args.iter().any(|a| self.needs(a));
                }
                let Some(t) = self.types.get(name.as_str()).copied() else { return false };
                if let Some(v) = self.memo.get(name) {
                    return *v;
                }
                self.memo.insert(name.clone(), false);
                let v = match &t.body {
                    TypeBody::Alias(target, r) => r.is_some() || self.needs(target),
                    TypeBody::New(_) => false,
                    _ => Self::fields_of(t).iter().any(|f| f.refine.is_some() || self.refined_alias(&f.ty) || self.needs(&f.ty)),
                };
                self.memo.insert(name.clone(), v);
                v
            }
        }
    }

    fn raw(&mut self, ty: &Ty) -> Result<String, String> {
        if !self.needs(ty) {
            return Ok(pty(ty));
        }
        match ty {
            Ty::Tuple(xs) => Ok(format!("({})", xs.iter().map(|x| self.raw(x)).collect::<Result<Vec<_>, _>>()?.join(", "))),
            Ty::Named { name, args, .. } if matches!(name.as_str(), "List" | "Opt") => Ok(format!("{name}[{}]", self.raw(&args[0])?)),
            Ty::Named { name, args, .. } => {
                let t = self.types[name.as_str()];
                if !args.is_empty() || !t.params.is_empty() {
                    return Err(format!("validating generic type {} in a request is not supported yet", pty(ty)));
                }
                match &t.body {
                    TypeBody::Alias(target, _) => self.raw(&target.clone()),
                    _ => {
                        self.define(t)?;
                        Ok(format!("SspurRaw{name}"))
                    }
                }
            }
            Ty::Fn { .. } => Ok(pty(ty)),
        }
    }

    fn rb(&mut self, ty: &Ty, x: &str, depth: usize) -> Result<String, String> {
        if !self.needs(ty) {
            return Ok(x.to_string());
        }
        let v = format!("v{depth}");
        match ty {
            Ty::Tuple(xs) => {
                let parts = xs.iter().enumerate().map(|(i, t)| self.rb(t, &format!("{x}.{i}"), depth + 1)).collect::<Result<Vec<_>, _>>()?;
                Ok(format!("({})", parts.join(", ")))
            }
            Ty::Named { name, args, .. } if matches!(name.as_str(), "List" | "Opt") => Ok(format!("{x}.map({v} => {})", self.rb(&args[0], &v, depth + 1)?)),
            Ty::Named { name, .. } if matches!(name.as_str(), "Map" | "Pii") => Err(format!("validating {} in a request is not supported yet", pty(ty))),
            Ty::Named { name, .. } => match &self.types[name.as_str()].body {
                TypeBody::Alias(target, _) => self.rb(&target.clone(), x, depth),
                _ => Ok(format!("sspur_rb_{name}({x})")),
            },
            Ty::Fn { .. } => Ok(x.to_string()),
        }
    }

    fn define(&mut self, t: &'a TypeDef) -> Result<(), String> {
        let key = t.name.clone();
        if self.out.contains_key(&key) {
            return Ok(());
        }
        self.out.insert(key.clone(), String::new());
        let raw_fields = |g: &mut Self, fs: &[Field]| -> Result<String, String> {
            let items = fs.iter().map(|f| Ok(format!("{}: {}", f.name, g.raw(&f.ty)?))).collect::<Result<Vec<_>, String>>()?;
            Ok(format!("{{{}}}", items.join(", ")))
        };
        let n = &t.name;
        let src = match &t.body {
            TypeBody::Record(fs) => {
                let inits = fs.iter().map(|f| Ok(format!("{}: {}", f.name, self.rb(&f.ty, &format!("x.{}", f.name), 1)?))).collect::<Result<Vec<_>, String>>()?;
                format!("type SspurRaw{n} = {}\n\nfn sspur_rb_{n}(x: SspurRaw{n}) -> {n}\n= {n}{{{}}}", raw_fields(self, fs)?, inits.join(", "))
            }
            TypeBody::Sum(vs) => {
                let mut variants = Vec::new();
                let mut arms = String::new();
                for v in vs {
                    match &v.fields {
                        Some(fs) => {
                            variants.push(format!("SspurRaw{}{}", v.name, raw_fields(self, fs)?));
                            let names: Vec<&str> = fs.iter().map(|f| f.name.as_str()).collect();
                            let inits = fs.iter().map(|f| Ok(format!("{}: {}", f.name, self.rb(&f.ty, &f.name, 1)?))).collect::<Result<Vec<_>, String>>()?;
                            arms.push_str(&format!("\n  | SspurRaw{}{{{}}} => {}{{{}}}", v.name, names.join(", "), v.name, inits.join(", ")));
                        }
                        None => {
                            variants.push(format!("SspurRaw{}", v.name));
                            arms.push_str(&format!("\n  | SspurRaw{} => {}", v.name, v.name));
                        }
                    }
                }
                format!("type SspurRaw{n} = {}\n\nfn sspur_rb_{n}(x: SspurRaw{n}) -> {n}\n= match x{arms}", variants.join(" | "))
            }
            _ => return Err(format!("cannot validate type {n}")),
        };
        self.out.insert(key, src);
        Ok(())
    }

    pub fn entry(&mut self, f: &FnDef, path: &HashSet<String>) -> Result<Option<String>, String> {
        let body_needs = f.params.iter().any(|p| !path.contains(&p.name) && self.needs(&p.ty));
        if !body_needs && f.params.iter().all(|p| p.refine.is_none()) {
            return Ok(None);
        }
        let mut params = Vec::new();
        let mut outs = Vec::new();
        for p in &f.params {
            let raw = !path.contains(&p.name) && self.needs(&p.ty);
            let t = if raw { self.raw(&p.ty)? } else { pty(&p.ty) };
            params.push(match &p.refine {
                Some(r) => format!("{}: {t} where {}", p.name, pexpr(r, 0)),
                None => format!("{}: {t}", p.name),
            });
            outs.push(if raw { self.rb(&p.ty, &p.name, 1)? } else { p.name.clone() });
        }
        let name = format!("sspur_in_{}", f.name);
        let tys: Vec<String> = f.params.iter().map(|p| pty(&p.ty)).collect();
        let (ret, body) = if outs.len() == 1 { (tys[0].clone(), outs[0].clone()) } else { (format!("({})", tys.join(", ")), format!("({})", outs.join(", "))) };
        self.out.insert(format!("~{name}"), format!("fn {name}({}) -> {ret}\n= {body}", params.join(", ")));
        Ok(Some(name))
    }

    pub fn source(&self) -> String {
        self.out.values().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join("\n\n")
    }
}
