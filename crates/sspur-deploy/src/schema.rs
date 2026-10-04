use sspur_syntax::printer::{expr as pexpr, ty as pty};
use sspur_syntax::*;
use std::collections::{BTreeMap, HashMap};
use std::fmt;

pub type Fields = Vec<(String, Shape)>;

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Prim(String),
    List(Box<Shape>),
    Opt(Box<Shape>),
    Map(Box<Shape>, Box<Shape>),
    Tuple(Vec<Shape>),
    Rec(Vec<(String, Shape)>),
    Sum(Vec<(String, Option<Fields>)>),
    Where(Box<Shape>, String),
    Ref(String),
}

fn fields_str(f: &mut fmt::Formatter, fs: &[(String, Shape)]) -> fmt::Result {
    write!(f, "{{")?;
    for (i, (n, s)) in fs.iter().enumerate() {
        write!(f, "{}{n}: {s}", if i > 0 { ", " } else { "" })?;
    }
    write!(f, "}}")
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Shape::Prim(p) | Shape::Ref(p) => write!(f, "{p}"),
            Shape::List(x) => write!(f, "List[{x}]"),
            Shape::Opt(x) => write!(f, "Opt[{x}]"),
            Shape::Map(k, v) => write!(f, "Map[{k}, {v}]"),
            Shape::Tuple(xs) => write!(f, "({})", xs.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")),
            Shape::Rec(fs) => fields_str(f, fs),
            Shape::Sum(vs) => {
                for (i, (n, fs)) in vs.iter().enumerate() {
                    write!(f, "{}{n}", if i > 0 { " | " } else { "" })?;
                    if let Some(fs) = fs {
                        fields_str(f, fs)?;
                    }
                }
                Ok(())
            }
            Shape::Where(x, r) => write!(f, "{x} where {r}"),
        }
    }
}

pub struct Shapes<'a> {
    types: HashMap<&'a str, &'a TypeDef>,
    hashes: HashMap<String, String>,
    stack: Vec<String>,
}

impl<'a> Shapes<'a> {
    pub fn new(m: &'a Module) -> Self {
        let types = m.defs.iter().filter_map(|d| if let Def::Type(t) = d { Some((t.name.as_str(), t)) } else { None }).collect();
        Shapes { types, hashes: sspur_hash::hash_module(m).into_iter().collect(), stack: Vec::new() }
    }

    fn fields(&mut self, fs: &[Field]) -> Vec<(String, Shape)> {
        fs.iter().map(|f| (f.name.clone(), self.refined(&f.ty, f.refine.as_ref()))).collect()
    }

    fn refined(&mut self, ty: &Ty, r: Option<&Expr>) -> Shape {
        let s = self.ty(ty);
        match r {
            Some(e) => Shape::Where(Box::new(s), pexpr(e, 0)),
            None => s,
        }
    }

    pub fn ty(&mut self, ty: &Ty) -> Shape {
        let Ty::Named { name, args, .. } = ty else {
            return match ty {
                Ty::Tuple(xs) => Shape::Tuple(xs.iter().map(|x| self.ty(x)).collect()),
                _ => Shape::Prim(pty(ty)),
            };
        };
        match (name.as_str(), args.as_slice()) {
            ("Int" | "Str" | "Bool" | "F64" | "Unit", []) => return Shape::Prim(name.clone()),
            ("List", [x]) => return Shape::List(Box::new(self.ty(x))),
            ("Opt", [x]) => return Shape::Opt(Box::new(self.ty(x))),
            ("Pii", [x]) => return self.ty(x),
            ("Map", [k, v]) => return Shape::Map(Box::new(self.ty(k)), Box::new(self.ty(v))),
            _ => {}
        }
        let Some(t) = self.types.get(name.as_str()).copied() else { return Shape::Prim(pty(ty)) };
        if !t.params.is_empty() || !args.is_empty() {
            return Shape::Prim(format!("{}#{}", pty(ty), self.hashes.get(name).map_or("", |h| &h[..12])));
        }
        if self.stack.contains(name) {
            return Shape::Ref(format!("#{}", self.hashes.get(name).map_or("", |h| &h[..12])));
        }
        self.stack.push(name.clone());
        let s = match &t.body {
            TypeBody::Record(fs) => Shape::Rec(self.fields(fs)),
            TypeBody::Sum(vs) => Shape::Sum(vs.iter().map(|v| (v.name.clone(), v.fields.as_ref().map(|fs| self.fields(fs)))).collect()),
            TypeBody::Alias(x, r) => self.refined(x, r.as_ref()),
            TypeBody::New(x) => self.ty(x),
        };
        self.stack.pop();
        s
    }

    pub fn type_hash(&self, ty: &Ty) -> String {
        match ty {
            Ty::Named { name, args, .. } => {
                let head = self.hashes.get(name).cloned().unwrap_or_else(|| name.clone());
                if args.is_empty() {
                    head
                } else {
                    format!("{head}[{}]", args.iter().map(|a| self.type_hash(a)).collect::<Vec<_>>().join(","))
                }
            }
            _ => pty(ty),
        }
    }

    pub fn version(&mut self, ty: &Ty) -> String {
        let shape = self.ty(ty);
        let h = blake3::hash(format!("{}\0{shape}", self.type_hash(ty)).as_bytes());
        sspur_hash::base32(h.as_bytes())[..12].to_string()
    }
}

fn opt(s: &Shape) -> bool {
    matches!(s, Shape::Opt(_)) || matches!(s, Shape::Where(x, _) if opt(x))
}

pub fn diff(old: &Shape, new: &Shape, path: &str, out: &mut Vec<(bool, String)>) {
    if old == new {
        return;
    }
    let at = |p: &str| if p.is_empty() { "value".to_string() } else { p.to_string() };
    let sub = |n: &str| if path.is_empty() { n.to_string() } else { format!("{path}.{n}") };
    match (old, new) {
        (Shape::Rec(a), Shape::Rec(b)) => rec_diff(a, b, path, out),
        (Shape::Sum(a), Shape::Sum(b)) => {
            for (n, fs) in a {
                match b.iter().find(|(m, _)| m == n) {
                    None => out.push((false, format!("variant {} removed", sub(n)))),
                    Some((_, gs)) => match (fs, gs) {
                        (Some(x), Some(y)) => rec_diff(x, y, &sub(n), out),
                        (None, None) => {}
                        _ => out.push((false, format!("variant {} changed between bare and with fields", sub(n)))),
                    },
                }
            }
            for (n, _) in b.iter().filter(|(n, _)| !a.iter().any(|(m, _)| m == n)) {
                out.push((true, format!("variant {} added", sub(n))));
            }
        }
        (Shape::List(a), Shape::List(b)) | (Shape::Opt(a), Shape::Opt(b)) => diff(a, b, &format!("{}[]", at(path)), out),
        (Shape::Map(k1, v1), Shape::Map(k2, v2)) if k1 == k2 => diff(v1, v2, &format!("{}[]", at(path)), out),
        (Shape::Tuple(a), Shape::Tuple(b)) if a.len() == b.len() => {
            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                diff(x, y, &format!("{}.{i}", at(path)), out);
            }
        }
        (Shape::Where(x, r1), Shape::Where(y, r2)) => {
            if r1 != r2 {
                out.push((false, format!("{}: refinement `{r1}` changed to `{r2}`; stored values are not re-checked", at(path))));
            }
            diff(x, y, path, out);
        }
        (Shape::Where(x, r), y) => {
            out.push((true, format!("{}: refinement `{r}` dropped", at(path))));
            diff(x, y, path, out);
        }
        (x, Shape::Where(y, r)) => {
            out.push((false, format!("{}: refinement `{r}` added; stored values are not re-checked", at(path))));
            diff(x, y, path, out);
        }
        _ => out.push((false, format!("{}: {old} changed to {new}", at(path)))),
    }
}

fn rec_diff(a: &[(String, Shape)], b: &[(String, Shape)], path: &str, out: &mut Vec<(bool, String)>) {
    let sub = |n: &str| if path.is_empty() { n.to_string() } else { format!("{path}.{n}") };
    let bm: BTreeMap<&str, &Shape> = b.iter().map(|(n, s)| (n.as_str(), s)).collect();
    for (n, s) in a {
        match bm.get(n.as_str()) {
            None => out.push((false, format!("field {} removed", sub(n)))),
            Some(t) => diff(s, t, &sub(n), out),
        }
    }
    for (n, s) in b.iter().filter(|(n, _)| !a.iter().any(|(m, _)| m == n)) {
        if opt(s) {
            out.push((true, format!("field {}: {s} added (optional)", sub(n))));
        } else {
            out.push((false, format!("field {}: {s} added (required)", sub(n))));
        }
    }
}
