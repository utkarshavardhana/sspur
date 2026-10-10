//! Exhaustiveness and redundancy of `match` and `catch` arms (ADR 0029): the usefulness
//! algorithm over constructors, with or-patterns, and list patterns split by length.

use super::*;

/// A pattern with its constructor resolved against the scrutinee's type.
#[derive(Clone, Debug)]
enum Dp {
    Wild,
    Ctor(K, Vec<Dp>),
    Or(Vec<Dp>),
    /// Head elements, and with `Some` a rest followed by the tail elements.
    List(Vec<Dp>, Option<Vec<Dp>>),
}

#[derive(Clone, Debug, PartialEq)]
enum K {
    /// A variant of a sum, `some`/`none`/`ok`/`err`, or `true`/`false`.
    Named(String),
    /// The one constructor of a tuple or a record.
    Product,
    Int(i64),
    Str(String),
    /// A list of exactly this many elements.
    Len(usize),
    /// A list of at least `min` elements, seen through its first `pre` and last `suf`.
    AtLeast { min: usize, pre: usize, suf: usize },
}

enum Shape {
    /// The variants, each with its fields (names are empty for `some(x)` and friends).
    Finite(Vec<(String, Vec<(String, Type)>)>),
    /// A tuple's elements, or a record's fields with the record's name.
    Product(Vec<(String, Type)>, Option<String>),
    List(Type),
    Open,
}

/// Steps of the algorithm before it gives up on a match (huge or-pattern products).
const BUDGET: usize = 200_000;
const MAX_WITNESSES: usize = 8;

struct Pm<'a> {
    ck: &'a Checker,
    steps: usize,
}

fn canon_ctor(name: &str) -> &str {
    match name {
        "None" | "Nothing" | "Nil" => "none",
        "Some" | "Just" => "some",
        "Ok" => "ok",
        "Err" => "err",
        "True" => "true",
        "False" => "false",
        n => n,
    }
}

impl Pm<'_> {
    fn tick(&mut self) -> bool {
        self.steps += 1;
        self.steps > BUDGET
    }

    fn shape(&self, t: &Type) -> Shape {
        let ck = self.ck;
        match ck.resolve(t) {
            Type::Tuple(ts) => Shape::Product(ts.into_iter().enumerate().map(|(i, t)| (i.to_string(), t)).collect(), None),
            Type::Con(n, a) => match (n.as_str(), a.as_slice()) {
                ("Bool", []) => Shape::Finite(vec![("true".into(), vec![]), ("false".into(), vec![])]),
                ("Opt", [x]) => Shape::Finite(vec![("some".into(), vec![(String::new(), x.clone())]), ("none".into(), vec![])]),
                ("Res", [x, e]) => Shape::Finite(vec![("ok".into(), vec![(String::new(), x.clone())]), ("err".into(), vec![(String::new(), e.clone())])]),
                ("List", [x]) => Shape::List(x.clone()),
                _ => match ck.types.get(&n) {
                    Some(TypeInfo { params, kind: TypeKind::Sum(vs) }) => {
                        let map: HashMap<String, Type> = params.iter().cloned().zip(a.iter().cloned()).collect();
                        Shape::Finite(
                            vs.iter()
                                .map(|v| {
                                    let fs = ck.ctors.get(v).and_then(|c| c.fields.clone()).unwrap_or_default();
                                    (v.clone(), fs.into_iter().map(|(f, t)| (f, subst_params(&t, &map))).collect())
                                })
                                .collect(),
                        )
                    }
                    Some(TypeInfo { params, kind: TypeKind::Record(fs) }) => {
                        let map: HashMap<String, Type> = params.iter().cloned().zip(a.iter().cloned()).collect();
                        Shape::Product(fs.iter().map(|(f, t)| (f.clone(), subst_params(t, &map))).collect(), Some(n.clone()))
                    }
                    _ => Shape::Open,
                },
            },
            _ => Shape::Open,
        }
    }

    fn lower(&self, p: &Pat, t: &Type) -> Dp {
        match p {
            Pat::Wild | Pat::Bind(_) => Dp::Wild,
            Pat::Int(n) => Dp::Ctor(K::Int(*n), vec![]),
            Pat::Str(s) => Dp::Ctor(K::Str(s.clone()), vec![]),
            Pat::Bool(b) => Dp::Ctor(K::Named(b.to_string()), vec![]),
            Pat::Or(alts) => Dp::Or(alts.iter().map(|a| self.lower(a, t)).collect()),
            Pat::Tuple(xs) => match self.shape(t) {
                Shape::Product(ts, None) if ts.len() == xs.len() => Dp::Ctor(K::Product, xs.iter().zip(&ts).map(|(x, (_, t))| self.lower(x, t)).collect()),
                _ => Dp::Wild,
            },
            Pat::List { head, rest, tail } => match self.shape(t) {
                Shape::List(et) => Dp::List(head.iter().map(|x| self.lower(x, &et)).collect(), rest.as_ref().map(|_| tail.iter().map(|x| self.lower(x, &et)).collect())),
                _ => Dp::Wild,
            },
            Pat::Ctor { name, args } => {
                let name = name.as_str();
                let fields = |fs: &[(String, Type)]| -> Vec<Dp> {
                    match args {
                        CtorArgs::None => fs.iter().map(|_| Dp::Wild).collect(),
                        CtorArgs::Positional(xs) => fs.iter().enumerate().map(|(i, (_, t))| xs.get(i).map_or(Dp::Wild, |x| self.lower(x, t))).collect(),
                        CtorArgs::Record(ps) => fs.iter().map(|(f, t)| ps.iter().find(|(n, _)| n == f).map_or(Dp::Wild, |(_, x)| self.lower(x, t))).collect(),
                    }
                };
                match self.shape(t) {
                    Shape::Finite(vs) => match vs.iter().find(|(v, _)| v == name).or_else(|| vs.iter().find(|(v, _)| v == canon_ctor(name))) {
                        Some((v, fs)) => Dp::Ctor(K::Named(v.clone()), fields(fs)),
                        None => Dp::Ctor(K::Named(name.to_string()), vec![]),
                    },
                    Shape::Product(fs, Some(r)) if r == name => Dp::Ctor(K::Product, fields(&fs)),
                    _ => Dp::Wild,
                }
            }
        }
    }

    fn sub_types(&self, t: &Type, k: &K) -> Vec<Type> {
        match (self.shape(t), k) {
            (Shape::Finite(vs), K::Named(n)) => vs.into_iter().find(|(v, _)| v == n).map(|(_, fs)| fs.into_iter().map(|(_, t)| t).collect()).unwrap_or_default(),
            (Shape::Product(fs, _), K::Product) => fs.into_iter().map(|(_, t)| t).collect(),
            (Shape::List(et), K::Len(n)) => vec![et; *n],
            (Shape::List(et), K::AtLeast { pre, suf, .. }) => vec![et; pre + suf],
            _ => vec![],
        }
    }

    /// The length constructors that split a column of list patterns into classes.
    fn list_ctors(heads: &[&Dp]) -> Option<Vec<K>> {
        let (mut exact, mut pre, mut suf, mut any) = (None::<usize>, 0, 0, false);
        fn scan(p: &Dp, exact: &mut Option<usize>, pre: &mut usize, suf: &mut usize, any: &mut bool) {
            match p {
                Dp::Or(xs) => xs.iter().for_each(|x| scan(x, exact, pre, suf, any)),
                Dp::List(h, None) => {
                    *any = true;
                    *exact = Some(exact.map_or(h.len(), |e| e.max(h.len())));
                }
                Dp::List(h, Some(t)) => {
                    *any = true;
                    *pre = (*pre).max(h.len());
                    *suf = (*suf).max(t.len());
                }
                _ => {}
            }
        }
        for h in heads {
            scan(h, &mut exact, &mut pre, &mut suf, &mut any);
        }
        if !any {
            return None;
        }
        let min = exact.map_or(0, |e| e + 1).max(pre + suf);
        let mut out: Vec<K> = (0..min).map(K::Len).collect();
        out.push(K::AtLeast { min, pre, suf });
        Some(out)
    }

    fn arity(k: &K, sub: &[Type]) -> usize {
        match k {
            K::Len(n) => *n,
            K::AtLeast { pre, suf, .. } => pre + suf,
            _ => sub.len(),
        }
    }

    /// The rows `p :: rest` becomes when the value is built with `k`.
    fn spec_row(p: &Dp, rest: &[Dp], k: &K, arity: usize, out: &mut Vec<Vec<Dp>>) {
        let row = |mut v: Vec<Dp>| {
            v.extend_from_slice(rest);
            v
        };
        match p {
            Dp::Wild => out.push(row(vec![Dp::Wild; arity])),
            Dp::Or(xs) => xs.iter().for_each(|x| Self::spec_row(x, rest, k, arity, out)),
            Dp::Ctor(c, args) if c == k => out.push(row(args.clone())),
            Dp::Ctor(..) => {}
            Dp::List(h, t) => match (k, t) {
                (K::Len(n), None) if h.len() == *n => out.push(row(h.clone())),
                (K::Len(n), Some(t)) if h.len() + t.len() <= *n => {
                    let mut v = h.clone();
                    v.extend(std::iter::repeat_n(Dp::Wild, n - h.len() - t.len()));
                    v.extend(t.iter().cloned());
                    out.push(row(v));
                }
                (K::AtLeast { pre, suf, .. }, Some(t)) => {
                    let mut v = h.clone();
                    v.extend(std::iter::repeat_n(Dp::Wild, pre - h.len() + suf - t.len()));
                    v.extend(t.iter().cloned());
                    out.push(row(v));
                }
                _ => {}
            },
        }
    }

    fn specialize(rows: &[Vec<Dp>], k: &K, arity: usize) -> Vec<Vec<Dp>> {
        let mut out = Vec::new();
        for r in rows {
            Self::spec_row(&r[0], &r[1..], k, arity, &mut out);
        }
        out
    }

    fn default_rows(rows: &[Vec<Dp>]) -> Vec<Vec<Dp>> {
        fn go(p: &Dp, rest: &[Dp], out: &mut Vec<Vec<Dp>>) {
            match p {
                Dp::Wild => out.push(rest.to_vec()),
                Dp::Or(xs) => xs.iter().for_each(|x| go(x, rest, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for r in rows {
            go(&r[0], &r[1..], &mut out);
        }
        out
    }

    fn heads(rows: &[Vec<Dp>]) -> Vec<K> {
        fn go(p: &Dp, out: &mut Vec<K>) {
            match p {
                Dp::Ctor(k, _) if !out.contains(k) => out.push(k.clone()),
                Dp::Or(xs) => xs.iter().for_each(|x| go(x, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for r in rows {
            go(&r[0], &mut out);
        }
        out
    }

    /// The constructors to split on when every one of them is covered, or `None` when the
    /// default rows decide (an open type, or a sum with a variant no row names).
    fn split(&self, t: &Type, rows: &[Vec<Dp>], extra: Option<&Dp>) -> Option<Vec<K>> {
        let mut col: Vec<&Dp> = rows.iter().map(|r| &r[0]).collect();
        col.extend(extra);
        match self.shape(t) {
            Shape::List(_) => Self::list_ctors(&col),
            Shape::Product(..) => Self::heads(rows).contains(&K::Product).then(|| vec![K::Product]),
            Shape::Finite(vs) => {
                let used = Self::heads(rows);
                let all: Vec<K> = vs.into_iter().map(|(v, _)| K::Named(v)).collect();
                all.iter().all(|k| used.contains(k)).then_some(all)
            }
            Shape::Open => None,
        }
    }

    fn useful(&mut self, rows: &[Vec<Dp>], v: &[Dp], tys: &[Type]) -> bool {
        if v.is_empty() {
            return rows.is_empty();
        }
        if self.tick() {
            return true;
        }
        let t = &tys[0];
        let expand = |pm: &mut Self, k: &K, prow: Vec<Vec<Dp>>| -> bool {
            let sub = pm.sub_types(t, k);
            let n = Self::arity(k, &sub);
            let srows = Self::specialize(rows, k, n);
            let stys: Vec<Type> = sub.into_iter().chain(tys[1..].iter().cloned()).collect();
            prow.iter().any(|pv| pm.useful(&srows, pv, &stys))
        };
        match &v[0] {
            Dp::Or(xs) => xs.iter().any(|x| {
                let mut w = vec![x.clone()];
                w.extend_from_slice(&v[1..]);
                self.useful(rows, &w, tys)
            }),
            Dp::Ctor(k, _) => {
                let sub = self.sub_types(t, k);
                let mut prow = Vec::new();
                Self::spec_row(&v[0], &v[1..], k, Self::arity(k, &sub), &mut prow);
                expand(self, k, prow)
            }
            Dp::List(..) => {
                let ks = self.split(t, rows, Some(&v[0])).unwrap_or_default();
                ks.iter().any(|k| {
                    let sub = self.sub_types(t, k);
                    let mut prow = Vec::new();
                    Self::spec_row(&v[0], &v[1..], k, Self::arity(k, &sub), &mut prow);
                    !prow.is_empty() && expand(self, k, prow)
                })
            }
            Dp::Wild => match self.split(t, rows, None) {
                Some(ks) => ks.iter().any(|k| {
                    let sub = self.sub_types(t, k);
                    let mut prow = Vec::new();
                    Self::spec_row(&Dp::Wild, &v[1..], k, Self::arity(k, &sub), &mut prow);
                    expand(self, k, prow)
                }),
                None => self.useful(&Self::default_rows(rows), &v[1..], &tys[1..]),
            },
        }
    }

    /// Values (one string per column) that no row matches, at most `MAX_WITNESSES`.
    fn missing(&mut self, rows: &[Vec<Dp>], tys: &[Type]) -> Vec<Vec<String>> {
        if tys.is_empty() {
            return if rows.is_empty() { vec![vec![]] } else { vec![] };
        }
        if self.tick() {
            return vec![vec!["_".to_string(); tys.len()]];
        }
        let t = &tys[0];
        let mut out = Vec::new();
        match self.split(t, rows, None) {
            Some(ks) => {
                for k in &ks {
                    let sub = self.sub_types(t, k);
                    let n = Self::arity(k, &sub);
                    let srows = Self::specialize(rows, k, n);
                    let stys: Vec<Type> = sub.into_iter().chain(tys[1..].iter().cloned()).collect();
                    for w in self.missing(&srows, &stys) {
                        let mut row = vec![self.show(t, k, &w[..n])];
                        row.extend_from_slice(&w[n..]);
                        out.push(row);
                        if out.len() >= MAX_WITNESSES {
                            return out;
                        }
                    }
                }
            }
            None => {
                let rest = self.missing(&Self::default_rows(rows), &tys[1..]);
                if rest.is_empty() {
                    return out;
                }
                let used = Self::heads(rows);
                let heads: Vec<String> = match self.shape(t) {
                    Shape::Finite(vs) if !used.is_empty() => vs
                        .iter()
                        .filter(|(v, _)| !used.contains(&K::Named(v.clone())))
                        .map(|(v, fs)| {
                            let k = K::Named(v.clone());
                            self.show(t, &k, &vec!["_".to_string(); fs.len()])
                        })
                        .collect(),
                    _ => vec!["_".into()],
                };
                for h in heads {
                    for r in &rest {
                        let mut row = vec![h.clone()];
                        row.extend_from_slice(r);
                        out.push(row);
                        if out.len() >= MAX_WITNESSES {
                            return out;
                        }
                    }
                }
            }
        }
        out
    }

    fn show(&self, t: &Type, k: &K, args: &[String]) -> String {
        let wild = |s: &String| s == "_";
        match (k, self.shape(t)) {
            (K::Named(n), Shape::Finite(vs)) => {
                let fs = vs.into_iter().find(|(v, _)| v == n).map(|(_, fs)| fs).unwrap_or_default();
                if fs.is_empty() {
                    n.clone()
                } else if fs[0].0.is_empty() {
                    format!("{n}({})", args.join(", "))
                } else if args.iter().all(wild) {
                    format!("{n}{{..}}")
                } else {
                    let items: Vec<String> = fs.iter().zip(args).filter(|(_, a)| !wild(a)).map(|((f, _), a)| format!("{f}: {a}")).collect();
                    format!("{n}{{{}}}", items.join(", "))
                }
            }
            (K::Product, Shape::Product(fs, Some(r))) => {
                if args.iter().all(wild) {
                    "_".into()
                } else {
                    let items: Vec<String> = fs.iter().zip(args).filter(|(_, a)| !wild(a)).map(|((f, _), a)| format!("{f}: {a}")).collect();
                    format!("{r}{{{}}}", items.join(", "))
                }
            }
            (K::Product, _) => format!("({})", args.join(", ")),
            (K::Len(_), _) => format!("[{}]", args.join(", ")),
            (K::AtLeast { min, pre, suf }, _) => {
                let mut items: Vec<String> = args[..*pre].to_vec();
                items.extend(std::iter::repeat_n("_".to_string(), min - pre - suf));
                items.push("..".into());
                items.extend_from_slice(&args[*pre..]);
                format!("[{}]", items.join(", "))
            }
            (K::Int(n), _) => n.to_string(),
            (K::Str(s), _) => format!("{s:?}"),
            (K::Named(n), _) => n.clone(),
        }
    }
}

/// An arm, or one alternative of an arm's or-pattern, that can never match.
pub(crate) struct Unreachable {
    pub arm: usize,
    pub alt: Option<usize>,
}

impl Checker {
    /// Cases the unguarded arms miss, as patterns to add (`B{..}`, `some(none)`, `[_, ..]`).
    pub(crate) fn missing_cases(&self, st: &Type, arms: &[Arm]) -> Vec<String> {
        let mut pm = Pm { ck: self, steps: 0 };
        let rows: Vec<Vec<Dp>> = arms.iter().filter(|a| a.guard.is_none()).map(|a| vec![pm.lower(&a.pat, st)]).collect();
        let tys = [self.resolve(st)];
        let mut out: Vec<String> = pm.missing(&rows, &tys).into_iter().map(|mut w| w.remove(0)).collect();
        out.dedup();
        out
    }

    /// Arms and top-level alternatives that earlier unguarded arms already cover.
    pub(crate) fn unreachable_arms(&self, st: &Type, arms: &[Arm]) -> Vec<Unreachable> {
        let mut pm = Pm { ck: self, steps: 0 };
        let tys = [self.resolve(st)];
        let mut rows: Vec<Vec<Dp>> = Vec::new();
        let mut out = Vec::new();
        for (i, a) in arms.iter().enumerate() {
            let p = pm.lower(&a.pat, st);
            match &p {
                Dp::Or(alts) => {
                    let mut seen = rows.clone();
                    let mut any = false;
                    for (j, x) in alts.iter().enumerate() {
                        let v = vec![x.clone()];
                        if pm.useful(&seen, &v, &tys) {
                            any = true;
                        } else {
                            out.push(Unreachable { arm: i, alt: Some(j) });
                        }
                        seen.push(v);
                    }
                    if !any {
                        out.retain(|u| u.arm != i);
                        out.push(Unreachable { arm: i, alt: None });
                    }
                }
                _ => {
                    if !pm.useful(&rows, std::slice::from_ref(&p), &tys) {
                        out.push(Unreachable { arm: i, alt: None });
                    }
                }
            }
            if pm.steps > BUDGET {
                return vec![];
            }
            if a.guard.is_none() {
                rows.push(vec![p]);
            }
        }
        out
    }
}
