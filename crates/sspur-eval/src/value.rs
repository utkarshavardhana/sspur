use sspur_syntax::{Expr, FnDef};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::rc::Rc;

pub type Fields = Rc<Vec<(Rc<str>, Value)>>;

#[derive(Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(Rc<str>),
    List(Rc<Vec<Value>>),
    Tuple(Rc<Vec<Value>>),
    Record(Rc<str>, Fields),
    Variant(Rc<str>, Option<Fields>),
    Opt(Option<Rc<Value>>),
    Res(Result<Rc<Value>, Rc<Value>>),
    Map(Rc<BTreeMap<Value, Value>>),
    New(Rc<str>, Rc<Value>),
    Closure(Rc<Closure>),
    Func(Rc<FnDef>),
    Builtin(Rc<str>),
}

pub struct Closure {
    pub params: Vec<String>,
    pub body: Expr,
    pub env: Rc<Env>,
}

#[derive(Default)]
pub struct Env {
    pub vars: RefCell<HashMap<Rc<str>, Rc<RefCell<Value>>>>,
    pub parent: Option<Rc<Env>>,
}

impl Env {
    pub fn child(parent: &Rc<Env>) -> Rc<Env> {
        Rc::new(Env { vars: RefCell::new(HashMap::new()), parent: Some(parent.clone()) })
    }

    pub fn define(&self, name: &str, v: Value) {
        self.vars.borrow_mut().insert(name.into(), Rc::new(RefCell::new(v)));
    }

    pub fn cell(&self, name: &str) -> Option<Rc<RefCell<Value>>> {
        if let Some(c) = self.vars.borrow().get(name) {
            return Some(c.clone());
        }
        self.parent.as_ref().and_then(|p| p.cell(name))
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.cell(name).map(|c| c.borrow().clone())
    }
}

impl Value {
    fn rank(&self) -> u8 {
        match self {
            Value::Unit => 0,
            Value::Bool(_) => 1,
            Value::Int(_) => 2,
            Value::Float(_) => 3,
            Value::Str(_) => 4,
            Value::List(_) => 5,
            Value::Tuple(_) => 6,
            Value::Record(..) => 7,
            Value::Variant(..) => 8,
            Value::Opt(_) => 9,
            Value::Res(_) => 10,
            Value::Map(_) => 11,
            Value::New(..) => 12,
            Value::Closure(_) | Value::Func(_) | Value::Builtin(_) => 13,
        }
    }

    pub fn str(s: &str) -> Value {
        Value::Str(s.into())
    }

    pub fn list(v: Vec<Value>) -> Value {
        Value::List(Rc::new(v))
    }

    pub fn some(v: Value) -> Value {
        Value::Opt(Some(Rc::new(v)))
    }

    pub fn field(&self, name: &str) -> Option<Value> {
        match self {
            Value::Record(_, fs) | Value::Variant(_, Some(fs)) => fs.iter().find(|(n, _)| &**n == name).map(|(_, v)| v.clone()),
            _ => None,
        }
    }
}

fn cmp_fields(a: &Fields, b: &Fields) -> Ordering {
    a.iter().map(|(_, v)| v).cmp(b.iter().map(|(_, v)| v))
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        use Value::*;
        match (self, other) {
            (Unit, Unit) => Ordering::Equal,
            (Bool(a), Bool(b)) => a.cmp(b),
            (Int(a), Int(b)) => a.cmp(b),
            (Float(a), Float(b)) => a.total_cmp(b),
            (Str(a), Str(b)) => a.cmp(b),
            (List(a), List(b)) | (Tuple(a), Tuple(b)) => a.iter().cmp(b.iter()),
            (Record(n1, a), Record(n2, b)) => n1.cmp(n2).then_with(|| cmp_fields(a, b)),
            (Variant(n1, a), Variant(n2, b)) => n1.cmp(n2).then_with(|| match (a, b) {
                (Some(a), Some(b)) => cmp_fields(a, b),
                (a, b) => a.is_some().cmp(&b.is_some()),
            }),
            (Opt(a), Opt(b)) => a.cmp(b),
            (Res(a), Res(b)) => match (a, b) {
                (Ok(x), Ok(y)) | (Err(x), Err(y)) => x.cmp(y),
                (Ok(_), Err(_)) => Ordering::Less,
                (Err(_), Ok(_)) => Ordering::Greater,
            },
            (Map(a), Map(b)) => a.iter().cmp(b.iter()),
            (New(n1, a), New(n2, b)) => n1.cmp(n2).then_with(|| a.cmp(b)),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Value {}

fn write_fields(f: &mut fmt::Formatter<'_>, fs: &Fields) -> fmt::Result {
    write!(f, "{{")?;
    for (i, (n, v)) in fs.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{n}: {}", Quoted(v))?;
    }
    write!(f, "}}")
}

fn write_seq(f: &mut fmt::Formatter<'_>, open: &str, close: &str, xs: &[Value]) -> fmt::Result {
    write!(f, "{open}")?;
    for (i, v) in xs.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{}", Quoted(v))?;
    }
    write!(f, "{close}")
}

pub struct Quoted<'a>(pub &'a Value);

impl fmt::Display for Quoted<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Value::Str(s) => write!(f, "{s:?}"),
            v => write!(f, "{v}"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Int(n) => write!(f, "{n}"),
            Value::Float(x) => {
                let s = format!("{x:?}");
                write!(f, "{s}")
            }
            Value::Str(s) => write!(f, "{s}"),
            Value::List(xs) => write_seq(f, "[", "]", xs),
            Value::Tuple(xs) => write_seq(f, "(", ")", xs),
            Value::Record(n, fs) => {
                write!(f, "{n}")?;
                write_fields(f, fs)
            }
            Value::Variant(n, None) => write!(f, "{n}"),
            Value::Variant(n, Some(fs)) => {
                write!(f, "{n}")?;
                write_fields(f, fs)
            }
            Value::Opt(None) => write!(f, "none"),
            Value::Opt(Some(v)) => write!(f, "some({})", Quoted(v)),
            Value::Res(Ok(v)) => write!(f, "ok({})", Quoted(v)),
            Value::Res(Err(v)) => write!(f, "err({})", Quoted(v)),
            Value::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", Quoted(k), Quoted(v))?;
                }
                write!(f, "}}")
            }
            Value::New(n, v) => write!(f, "{n}({})", Quoted(v)),
            Value::Closure(_) => write!(f, "<fn>"),
            Value::Func(d) => write!(f, "<fn {}>", d.name),
            Value::Builtin(n) => write!(f, "<builtin {n}>"),
        }
    }
}
