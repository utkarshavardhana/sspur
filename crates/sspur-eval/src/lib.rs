#![allow(clippy::mutable_key_type)]
mod builtins;
pub mod fuzz;
pub mod value;

use sspur_syntax::*;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
pub use value::Value;
use value::{Closure, Env};

pub enum Ctrl {
    Raise(Value),
    Return(Value),
    Trap(String),
}

pub type R<T = Value> = Result<T, Ctrl>;

pub fn trap<T>(msg: impl Into<String>) -> R<T> {
    Err(Ctrl::Trap(msg.into()))
}

struct Refine {
    field: Option<String>,
    expr: Expr,
}

pub struct Interp {
    fns: HashMap<String, Rc<FnDef>>,
    tests: Vec<TestDef>,
    variant_fields: HashMap<String, Option<Vec<String>>>,
    record_fields: HashMap<String, Vec<String>>,
    newtypes: HashSet<String>,
    alias_refines: HashMap<String, Expr>,
    type_refines: HashMap<String, Vec<Refine>>,
    field_types: HashMap<(String, String), String>,
    record_types: HashMap<(u32, u32), String>,
    user_methods: HashSet<(u32, u32)>,
    globals: Rc<Env>,
    types: HashMap<String, TypeDef>,
    pub output: RefCell<Option<Vec<String>>>,
    depth: Cell<u32>,
    pub fuel: Cell<u64>,
}

const MAX_DEPTH: u32 = 20_000;
pub const OUT_OF_FUEL: &str = "evaluation step budget exhausted";

fn ty_name(t: &Ty) -> Option<&str> {
    match t {
        Ty::Named { name, .. } => Some(name),
        _ => None,
    }
}

impl Interp {
    pub fn new(m: &Module, record_types: HashMap<(u32, u32), String>, user_methods: HashSet<(u32, u32)>) -> Self {
        let mut it = Interp {
            fns: HashMap::new(),
            tests: vec![],
            variant_fields: HashMap::new(),
            record_fields: HashMap::new(),
            newtypes: HashSet::new(),
            alias_refines: HashMap::new(),
            type_refines: HashMap::new(),
            field_types: HashMap::new(),
            record_types,
            user_methods,
            globals: Rc::new(Env::default()),
            types: HashMap::new(),
            output: RefCell::new(None),
            depth: Cell::new(0),
            fuel: Cell::new(u64::MAX),
        };
        for d in &m.defs {
            match d {
                Def::Fn(f) => {
                    it.fns.insert(f.name.clone(), Rc::new(f.clone()));
                }
                Def::Test(t) => it.tests.push(t.clone()),
                Def::Type(t) => {
                    it.types.insert(t.name.clone(), t.clone());
                    it.add_type(t)
                }
            }
        }
        it
    }

    fn add_type(&mut self, t: &TypeDef) {
        let add_fields = |owner: &str, fs: &[Field], refines: &mut HashMap<String, Vec<Refine>>, ftypes: &mut HashMap<(String, String), String>| {
            for f in fs {
                if let Some(r) = &f.refine {
                    refines.entry(owner.to_string()).or_default().push(Refine { field: Some(f.name.clone()), expr: r.clone() });
                }
                if let Some(n) = ty_name(&f.ty) {
                    ftypes.insert((owner.to_string(), f.name.clone()), n.to_string());
                }
            }
        };
        match &t.body {
            TypeBody::Record(fs) => {
                self.record_fields.insert(t.name.clone(), fs.iter().map(|f| f.name.clone()).collect());
                add_fields(&t.name, fs, &mut self.type_refines, &mut self.field_types);
            }
            TypeBody::Sum(vs) => {
                for v in vs {
                    self.variant_fields.insert(v.name.clone(), v.fields.as_ref().map(|fs| fs.iter().map(|f| f.name.clone()).collect()));
                    if let Some(fs) = &v.fields {
                        add_fields(&v.name, fs, &mut self.type_refines, &mut self.field_types);
                    }
                }
            }
            TypeBody::Alias(_, Some(r)) => {
                self.alias_refines.insert(t.name.clone(), r.clone());
            }
            TypeBody::Alias(_, None) => {}
            TypeBody::New(_) => {
                self.newtypes.insert(t.name.clone());
            }
        }
    }

    fn emit(&self, line: String) {
        match &mut *self.output.borrow_mut() {
            Some(buf) => buf.push(line),
            None => println!("{line}"),
        }
    }

    pub fn run_main(&self) -> Result<(), String> {
        let Some(main) = self.fns.get("main").cloned() else {
            return Err("no 'main' function".into());
        };
        if !main.params.is_empty() {
            return Err("'main' must take no parameters".into());
        }
        match self.call_fn(&main, vec![]) {
            Ok(_) => Ok(()),
            Err(c) => Err(describe(c)),
        }
    }

    pub fn run_tests(&self) -> Vec<(String, Result<(), String>)> {
        let mut out = Vec::new();
        let mut cases: Vec<(String, Expr)> = Vec::new();
        let mut names: Vec<&String> = self.fns.keys().collect();
        names.sort();
        for n in names {
            for (i, ex) in self.fns[n].examples.iter().enumerate() {
                cases.push((format!("{n}.ex{}", i + 1), ex.clone()));
            }
        }
        cases.extend(self.tests.iter().map(|t| (t.name.clone(), t.body.clone())));
        for (name, body) in &cases {
            let t = TestDef { name: name.clone(), body: body.clone(), span: Span::default() };
            *self.output.borrow_mut() = Some(vec![]);
            let r = match self.eval(&t.body, &Env::child(&self.globals)) {
                Ok(Value::Bool(true)) => Ok(()),
                Ok(Value::Bool(false)) => Err("evaluated to false".to_string()),
                Ok(v) => Err(format!("expected Bool, got {v}")),
                Err(c) => Err(describe(c)),
            };
            *self.output.borrow_mut() = None;
            out.push((t.name.clone(), r));
        }
        out
    }

    pub fn call_fn(&self, f: &FnDef, args: Vec<Value>) -> R {
        let globals = self.globals.clone();
        self.call_fn_in(f, args, &globals)
    }

    fn call_fn_in(&self, f: &FnDef, args: Vec<Value>, parent: &Rc<Env>) -> R {
        let d = self.depth.get() + 1;
        if d > MAX_DEPTH {
            return trap(format!("stack overflow in {}", f.name));
        }
        self.depth.set(d);
        let r = self.call_fn_inner(f, args, parent);
        self.depth.set(d - 1);
        r
    }

    fn call_fn_inner(&self, f: &FnDef, args: Vec<Value>, parent: &Rc<Env>) -> R {
        let env = Env::child(parent);
        for (p, v) in f.params.iter().zip(args) {
            self.check_value_type(&p.ty, &v, &format!("parameter '{}' of {}", p.name, f.name))?;
            if let Some(r) = &p.refine {
                self.check_refine(r, &v, &format!("parameter '{}' of {}", p.name, f.name))?;
            }
            env.define(&p.name, v);
        }
        for pre in &f.pres {
            if !self.truthy(pre, &env)? {
                return trap(format!("contract violated: pre {} in {}", printer::expr(pre, 0), f.name));
            }
        }
        let body = match &f.body.kind {
            ExprKind::Table(rows) => self.eval_table(rows, f, &env),
            _ => self.eval(&f.body, &env),
        };
        let result = match body {
            Ok(v) | Err(Ctrl::Return(v)) => v,
            Err(c) => return Err(c),
        };
        if let Some(rt) = &f.ret {
            self.check_value_type(rt, &result, &format!("result of {}", f.name))?;
        }
        if !f.posts.is_empty() {
            let penv = Env::child(&env);
            penv.define("r", result.clone());
            for post in &f.posts {
                if !self.truthy(post, &penv)? {
                    return trap(format!("contract violated: post {} in {} (r = {})", printer::expr(post, 0), f.name, value::Quoted(&result)));
                }
            }
        }
        Ok(result)
    }

    fn check_value_type(&self, t: &Ty, v: &Value, ctx: &str) -> R<()> {
        if let Some(name) = ty_name(t)
            && let Some(r) = self.alias_refines.get(name) {
                self.check_refine(r, v, &format!("type {name} in {ctx}"))?;
            }
        Ok(())
    }

    fn check_refine(&self, r: &Expr, v: &Value, ctx: &str) -> R<()> {
        let env = Env::child(&self.globals);
        env.define("_", v.clone());
        if self.truthy(r, &env)? {
            Ok(())
        } else {
            trap(format!("contract violated: {} where {} (value = {})", ctx, printer::expr(r, 0), value::Quoted(v)))
        }
    }

    fn truthy(&self, e: &Expr, env: &Rc<Env>) -> R<bool> {
        match self.eval(e, env)? {
            Value::Bool(b) => Ok(b),
            v => trap(format!("expected Bool, got {v}")),
        }
    }

    fn build_record(&self, owner: &str, fields: Vec<(Rc<str>, Value)>) -> R<value::Fields> {
        if let Some(rs) = self.type_refines.get(owner) {
            for r in rs {
                let fname = r.field.as_deref().unwrap();
                let v = &fields.iter().find(|(n, _)| &**n == fname).unwrap().1;
                self.check_refine(&r.expr, v, &format!("field '{fname}' of {owner}"))?;
            }
        }
        for (fname, v) in &fields {
            if let Some(tn) = self.field_types.get(&(owner.to_string(), fname.to_string()))
                && let Some(r) = self.alias_refines.get(tn) {
                    self.check_refine(r, v, &format!("field '{fname}' of {owner}"))?;
                }
        }
        Ok(Rc::new(fields))
    }

    pub fn apply(&self, f: &Value, args: Vec<Value>) -> R {
        match f {
            Value::Closure(c) => {
                let env = Env::child(&c.env);
                for (p, v) in c.params.iter().zip(args) {
                    env.define(p, v);
                }
                self.eval(&c.body, &env)
            }
            Value::Func(d) => self.call_fn(d, args),
            Value::LocalFn(d, env) => self.call_fn_in(d, args, env),
            Value::Builtin(n) => self.call_global(n, args),
            v => trap(format!("cannot call {v}")),
        }
    }

    fn eval_args(&self, args: &[Expr], env: &Rc<Env>) -> R<Vec<Value>> {
        args.iter().map(|a| self.eval(a, env)).collect()
    }

    fn method(&self, span: Span, name: &str, recv: Value, args: Vec<Value>) -> R {
        if self.user_methods.contains(&(span.start, span.end))
            && let Some(f) = self.fns.get(name).cloned() {
                let mut all = vec![recv];
                all.extend(args);
                return self.call_fn(&f, all);
            }
        self.call_method(name, recv, args)
    }

    pub fn eval(&self, e: &Expr, env: &Rc<Env>) -> R {
        let fuel = self.fuel.get();
        if fuel == 0 {
            return trap(OUT_OF_FUEL);
        }
        self.fuel.set(fuel - 1);
        match &e.kind {
            ExprKind::Int(n) => Ok(Value::Int(*n)),
            ExprKind::Float(x) => Ok(Value::Float(*x)),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::Unit => Ok(Value::Unit),
            ExprKind::Str(parts) => {
                let mut s = String::new();
                for p in parts {
                    match p {
                        StrPart::Lit(l) => s.push_str(l),
                        StrPart::Expr(x) => s.push_str(&self.eval(x, env)?.to_string()),
                    }
                }
                Ok(Value::str(&s))
            }
            ExprKind::Placeholder => env.get("_").map_or_else(|| trap("unbound '_'"), Ok),
            ExprKind::Hole(_) => trap("reached a typed hole"),
            ExprKind::Name(n) => self.eval_name(n, env),
            ExprKind::Field(x, f) => {
                let v = self.eval(x, env)?;
                if let Some(fv) = v.field(f) {
                    return Ok(fv);
                }
                if let (Value::Tuple(items), Ok(i)) = (&v, f.parse::<usize>()) {
                    return items.get(i).cloned().map_or_else(|| trap("tuple index out of range"), Ok);
                }
                if let (Value::New(_, inner), "raw") = (&v, f.as_str()) {
                    return Ok((**inner).clone());
                }
                self.method(e.span, f, v, vec![])
            }
            ExprKind::Method { recv, name, args, .. } => {
                let r = self.eval(recv, env)?;
                let a = self.eval_args(args, env)?;
                self.method(e.span, name, r, a)
            }
            ExprKind::Call(f, args) => {
                if let ExprKind::Name(n) = &f.kind
                    && env.cell(n).is_none() {
                        if self.newtypes.contains(n) {
                            let v = self.eval(&args[0], env)?;
                            return Ok(Value::New(n.as_str().into(), Rc::new(v)));
                        }
                        if let Some(fd) = self.fns.get(n).cloned() {
                            let a = self.eval_args(args, env)?;
                            return self.call_fn(&fd, a);
                        }
                        if builtins::is_global(n) {
                            let a = self.eval_args(args, env)?;
                            return self.call_global(n, a);
                        }
                    }
                let fv = self.eval(f, env)?;
                let a = self.eval_args(args, env)?;
                self.apply(&fv, a)
            }
            ExprKind::Index(a, i) => {
                let av = self.eval(a, env)?;
                let iv = self.eval(i, env)?;
                match (av, iv) {
                    (Value::List(xs), Value::Int(i)) => {
                        if i < 0 || i as usize >= xs.len() {
                            trap(format!("index {i} out of bounds for list of length {}", xs.len()))
                        } else {
                            Ok(xs[i as usize].clone())
                        }
                    }
                    (a, _) => trap(format!("cannot index {a}")),
                }
            }
            ExprKind::Lambda { params, body, .. } => Ok(Value::Closure(Rc::new(Closure { params: params.clone(), body: (**body).clone(), env: env.clone() }))),
            ExprKind::Binary(op, l, r) => self.eval_binary(*op, l, r, env),
            ExprKind::Unary(UnOp::Neg, x) => match self.eval(x, env)? {
                Value::Int(n) => n.checked_neg().map(Value::Int).map_or_else(|| trap("integer overflow"), Ok),
                Value::Float(f) => Ok(Value::Float(-f)),
                v => trap(format!("cannot negate {v}")),
            },
            ExprKind::Unary(UnOp::Not, x) => Ok(Value::Bool(!self.truthy(x, env)?)),
            ExprKind::Range(a, b) => match (self.eval(a, env)?, self.eval(b, env)?) {
                (Value::Int(a), Value::Int(b)) => Ok(Value::list((a..b).map(Value::Int).collect())),
                _ => trap("range bounds must be Int"),
            },
            ExprKind::If(c, t, f) => {
                if self.truthy(c, env)? {
                    self.eval(t, env)
                } else if let Some(f) = f {
                    self.eval(f, env)
                } else {
                    Ok(Value::Unit)
                }
            }
            ExprKind::Match(s, arms) => {
                let v = self.eval(s, env)?;
                match self.match_arms(&v, arms, env)? {
                    Some(r) => Ok(r),
                    None => trap(format!("no match arm for {v}")),
                }
            }
            ExprKind::Catch(body, arms) => match self.eval(body, env) {
                Err(Ctrl::Raise(v)) => match self.match_arms(&v, arms, env)? {
                    Some(r) => Ok(r),
                    None => Err(Ctrl::Raise(v)),
                },
                other => other,
            },
            ExprKind::Block(stmts) => {
                let env = Env::child(env);
                for s in stmts {
                    if let Stmt::Fn(f) = s {
                        env.define(&f.name, Value::LocalFn(Rc::new((**f).clone()), env.clone()));
                    }
                }
                let mut last = Value::Unit;
                for s in stmts {
                    last = self.exec(s, &env)?;
                }
                Ok(last)
            }
            ExprKind::Record { ctor, fields } => {
                let owner = match ctor {
                    Some(c) => c.clone(),
                    None => match self.record_types.get(&(e.span.start, e.span.end)) {
                        Some(n) => n.clone(),
                        None => return trap("untyped record literal"),
                    },
                };
                let order: Vec<String> = match self.variant_fields.get(&owner) {
                    Some(Some(fs)) => fs.clone(),
                    _ => self.record_fields.get(&owner).cloned().unwrap_or_else(|| fields.iter().map(|(n, _)| n.clone()).collect()),
                };
                let mut vals = Vec::with_capacity(order.len());
                let mut given = HashMap::new();
                for (n, x) in fields {
                    given.insert(n.as_str(), self.eval(x, env)?);
                }
                for n in &order {
                    let v = given.remove(n.as_str()).map_or_else(|| trap(format!("missing field {n}")), Ok)?;
                    vals.push((Rc::<str>::from(n.as_str()), v));
                }
                let fs = self.build_record(&owner, vals)?;
                if self.variant_fields.contains_key(&owner) {
                    Ok(Value::Variant(owner.as_str().into(), Some(fs)))
                } else {
                    Ok(Value::Record(owner.as_str().into(), fs))
                }
            }
            ExprKind::List(xs) => Ok(Value::list(self.eval_args(xs, env)?)),
            ExprKind::Tuple(xs) | ExprKind::Par(xs) => Ok(Value::Tuple(Rc::new(self.eval_args(xs, env)?))),
            ExprKind::Raise(x) => Err(Ctrl::Raise(self.eval(x, env)?)),
            ExprKind::Return(x) => Err(Ctrl::Return(self.eval(x, env)?)),
            ExprKind::Table(_) => trap("a rule table can only be the body of a rule"),
            ExprKind::With(base, ups) => {
                let mut v = self.eval(base, env)?;
                for (path, value) in ups {
                    let mut keys = Vec::with_capacity(path.len());
                    for seg in path {
                        keys.push(match seg {
                            PathSeg::Field(f) => Ok(f.as_str()),
                            PathSeg::Index(i) => match self.eval(i, env)? {
                                Value::Int(n) => Err(n),
                                other => return trap(format!("index must be Int, got {other}")),
                            },
                        });
                    }
                    let nv = self.eval(value, env)?;
                    v = self.set_path(&v, &keys, nv)?;
                }
                Ok(v)
            }
        }
    }

    fn eval_table(&self, rows: &[TableRow], f: &FnDef, env: &Rc<Env>) -> R {
        'rows: for r in rows {
            let inner = Env::child(env);
            for (c, p) in r.cells.iter().zip(&f.params) {
                let ok = match c {
                    sspur_syntax::Cell::Any => true,
                    sspur_syntax::Cell::Pat(pat) => self.bind_pat(pat, &env.get(&p.name).unwrap_or(Value::Unit), &inner),
                    sspur_syntax::Cell::Cond(e) => self.truthy(e, &inner)?,
                };
                if !ok {
                    continue 'rows;
                }
            }
            return self.eval(&r.out, &inner);
        }
        let args: Vec<String> = f.params.iter().map(|p| format!("{} = {}", p.name, env.get(&p.name).map(|v| value::Quoted(&v).to_string()).unwrap_or_default())).collect();
        trap(format!("no row of rule {} matched ({})", f.name, args.join(", ")))
    }

    fn set_path(&self, v: &Value, keys: &[Result<&str, i64>], nv: Value) -> R {
        let Some((k, rest)) = keys.split_first() else { return Ok(nv) };
        match (k, v) {
            (Ok(f), Value::Record(n, fs)) => {
                let mut out = Vec::with_capacity(fs.len());
                for (name, fv) in fs.iter() {
                    let nfv = if &**name == *f { self.set_path(fv, rest, nv.clone())? } else { fv.clone() };
                    out.push((name.clone(), nfv));
                }
                Ok(Value::Record(n.clone(), self.build_record(n, out)?))
            }
            (Err(i), Value::List(xs)) => {
                if *i < 0 || *i as usize >= xs.len() {
                    return trap(format!("index {i} out of bounds for list of length {}", xs.len()));
                }
                let mut ys = (**xs).clone();
                ys[*i as usize] = self.set_path(&xs[*i as usize], rest, nv)?;
                Ok(Value::list(ys))
            }
            (_, v) => trap(format!("cannot update a path through {v}")),
        }
    }

    fn eval_name(&self, n: &str, env: &Rc<Env>) -> R {
        if let Some(v) = env.get(n) {
            return Ok(v);
        }
        if n == "none" {
            return Ok(Value::Opt(None));
        }
        if let Some(None) = self.variant_fields.get(n) {
            return Ok(Value::Variant(n.into(), None));
        }
        if let Some(f) = self.fns.get(n) {
            return Ok(Value::Func(f.clone()));
        }
        if builtins::is_global(n) {
            return Ok(Value::Builtin(n.into()));
        }
        trap(format!("unbound name '{n}'"))
    }

    fn exec(&self, s: &Stmt, env: &Rc<Env>) -> R {
        match s {
            Stmt::Expr(e) => self.eval(e, env),
            Stmt::Let(p, e) => {
                let v = self.eval(e, env)?;
                if !self.bind_pat(p, &v, env) {
                    return trap(format!("pattern {} did not match {v}", printer::pat(p)));
                }
                Ok(Value::Unit)
            }
            Stmt::Var(n, e) => {
                let v = self.eval(e, env)?;
                env.define(n, v);
                Ok(Value::Unit)
            }
            Stmt::Assign(n, e, _) => {
                let v = self.eval(e, env)?;
                match env.cell(n) {
                    Some(c) => *c.borrow_mut() = v,
                    None => return trap(format!("unbound name '{n}'")),
                }
                Ok(Value::Unit)
            }
            Stmt::Fn(_) => Ok(Value::Unit),
            Stmt::While(c, body) => {
                while self.truthy(c, env)? {
                    self.eval(body, &Env::child(env))?;
                }
                Ok(Value::Unit)
            }
            Stmt::For(p, it, body) => {
                let Value::List(xs) = self.eval(it, env)? else { return trap("for expects a List") };
                for x in xs.iter() {
                    let inner = Env::child(env);
                    if self.bind_pat(p, x, &inner) {
                        self.eval(body, &inner)?;
                    }
                }
                Ok(Value::Unit)
            }
        }
    }

    fn match_arms(&self, v: &Value, arms: &[Arm], env: &Rc<Env>) -> R<Option<Value>> {
        for a in arms {
            let inner = Env::child(env);
            if !self.bind_pat(&a.pat, v, &inner) {
                continue;
            }
            if let Some(g) = &a.guard
                && !self.truthy(g, &inner)? {
                    continue;
                }
            return self.eval(&a.body, &inner).map(Some);
        }
        Ok(None)
    }

    fn bind_pat(&self, p: &Pat, v: &Value, env: &Rc<Env>) -> bool {
        match (p, v) {
            (Pat::Wild, _) => true,
            (Pat::Bind(n), _) => {
                env.define(n, v.clone());
                true
            }
            (Pat::Int(a), Value::Int(b)) => a == b,
            (Pat::Str(a), Value::Str(b)) => a.as_str() == &**b,
            (Pat::Bool(a), Value::Bool(b)) => a == b,
            (Pat::Tuple(ps), Value::Tuple(vs)) => ps.len() == vs.len() && ps.iter().zip(vs.iter()).all(|(p, v)| self.bind_pat(p, v, env)),
            (Pat::Ctor { name, args }, _) => match (name.as_str(), args, v) {
                ("some", CtorArgs::Positional(ps), Value::Opt(Some(x))) => self.bind_pat(&ps[0], x, env),
                ("none", _, Value::Opt(None)) => true,
                ("ok", CtorArgs::Positional(ps), Value::Res(Ok(x))) => self.bind_pat(&ps[0], x, env),
                ("err", CtorArgs::Positional(ps), Value::Res(Err(x))) => self.bind_pat(&ps[0], x, env),
                (_, _, Value::Variant(n, _) | Value::Record(n, _)) if &**n != name.as_str() => false,
                (_, CtorArgs::None, Value::Variant(..) | Value::Record(..)) => true,
                (_, CtorArgs::Record(pfs), Value::Variant(_, Some(_)) | Value::Record(..)) => pfs.iter().all(|(f, sp)| v.field(f).is_some_and(|fv| self.bind_pat(sp, &fv, env))),
                _ => false,
            },
            _ => false,
        }
    }

    fn eval_binary(&self, op: BinOp, l: &Expr, r: &Expr, env: &Rc<Env>) -> R {
        use BinOp::*;
        if op == And {
            return Ok(Value::Bool(self.truthy(l, env)? && self.truthy(r, env)?));
        }
        if op == Or {
            return Ok(Value::Bool(self.truthy(l, env)? || self.truthy(r, env)?));
        }
        let a = self.eval(l, env)?;
        let b = self.eval(r, env)?;
        Ok(match op {
            Eq => Value::Bool(a == b),
            Ne => Value::Bool(a != b),
            Lt => Value::Bool(a < b),
            Le => Value::Bool(a <= b),
            Gt => Value::Bool(a > b),
            Ge => Value::Bool(a >= b),
            _ => return arith(op, a, b),
        })
    }
}

fn arith(op: BinOp, a: Value, b: Value) -> R {
    use BinOp::*;
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => {
            let r = match op {
                Add => x.checked_add(y),
                Sub => x.checked_sub(y),
                Mul => x.checked_mul(y),
                Div if y == 0 => return trap("division by zero"),
                Div => x.checked_div(y),
                Rem if y == 0 => return trap("division by zero"),
                Rem => x.checked_rem(y),
                Pow if y < 0 => return trap("negative exponent"),
                Pow => u32::try_from(y).ok().and_then(|e| x.checked_pow(e)),
                _ => unreachable!(),
            };
            r.map(Value::Int).map_or_else(|| trap("integer overflow"), Ok)
        }
        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(match op {
            Add => x + y,
            Sub => x - y,
            Mul => x * y,
            Div => x / y,
            Rem => x % y,
            Pow => x.powf(y),
            _ => unreachable!(),
        })),
        (Value::Str(x), Value::Str(y)) if op == Add => Ok(Value::str(&format!("{x}{y}"))),
        (Value::List(x), Value::List(y)) if op == Add => Ok(Value::list(x.iter().chain(y.iter()).cloned().collect())),
        (a, b) => trap(format!("operator '{}' cannot apply to {a} and {b}", op.symbol())),
    }
}

pub fn describe(c: Ctrl) -> String {
    match c {
        Ctrl::Trap(m) => m,
        Ctrl::Raise(v) => format!("unhandled error: {v}"),
        Ctrl::Return(_) => "return outside of a function".into(),
    }
}
