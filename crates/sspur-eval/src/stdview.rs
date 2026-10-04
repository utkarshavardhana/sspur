//! Lazy views: a pipeline description that is pulled one element at a time when consumed.
use crate::value::Value;
use crate::{trap, Interp, R};
use std::rc::Rc;

pub enum Node {
    List(Rc<Vec<Value>>),
    Iota(i64, i64),
    Iterate(Value, Value, i64),
    Map(Rc<Node>, Value),
    Filter(Rc<Node>, Value),
    Take(Rc<Node>, i64),
    Drop(Rc<Node>, i64),
    TakeWhile(Rc<Node>, Value),
    DropWhile(Rc<Node>, Value),
    Enumerate(Rc<Node>),
    Zip(Rc<Node>, Rc<Node>),
    Chain(Rc<Node>, Rc<Node>),
}

enum It {
    List(Rc<Vec<Value>>, usize),
    Iota(i64, i64),
    Iterate(Value, Value, i64, i64),
    Map(Box<It>, Value),
    Filter(Box<It>, Value),
    Take(Box<It>, i64),
    Drop(Box<It>, i64),
    TakeWhile(Box<It>, Value, bool),
    DropWhile(Box<It>, Value, bool),
    Enumerate(Box<It>, i64),
    Zip(Box<It>, Box<It>),
    Chain(Box<It>, Option<Box<It>>, Rc<Node>),
}

fn start(n: &Node) -> It {
    match n {
        Node::List(xs) => It::List(xs.clone(), 0),
        Node::Iota(a, b) => It::Iota(*a, *b),
        Node::Iterate(x, f, k) => It::Iterate(x.clone(), f.clone(), 0, *k),
        Node::Map(s, f) => It::Map(Box::new(start(s)), f.clone()),
        Node::Filter(s, f) => It::Filter(Box::new(start(s)), f.clone()),
        Node::Take(s, k) => It::Take(Box::new(start(s)), *k),
        Node::Drop(s, k) => It::Drop(Box::new(start(s)), *k),
        Node::TakeWhile(s, f) => It::TakeWhile(Box::new(start(s)), f.clone(), false),
        Node::DropWhile(s, f) => It::DropWhile(Box::new(start(s)), f.clone(), true),
        Node::Enumerate(s) => It::Enumerate(Box::new(start(s)), 0),
        Node::Zip(a, b) => It::Zip(Box::new(start(a)), Box::new(start(b))),
        Node::Chain(a, b) => It::Chain(Box::new(start(a)), None, b.clone()),
    }
}

fn truthy(v: Value) -> R<bool> {
    match v {
        Value::Bool(b) => Ok(b),
        v => trap(format!("expected Bool, got {v}")),
    }
}

impl It {
    fn next(&mut self, ip: &Interp) -> R<Option<Value>> {
        Ok(match self {
            It::List(xs, i) => {
                let v = xs.get(*i).cloned();
                *i += 1;
                v
            }
            It::Iota(c, e) => {
                if *c < *e {
                    *c += 1;
                    Some(Value::Int(*c - 1))
                } else {
                    None
                }
            }
            It::Iterate(cur, f, k, n) => {
                if *k >= *n {
                    return Ok(None);
                }
                if *k > 0 {
                    *cur = ip.apply(f, vec![cur.clone()])?;
                }
                *k += 1;
                Some(cur.clone())
            }
            It::Map(s, f) => match s.next(ip)? {
                Some(x) => Some(ip.apply(f, vec![x])?),
                None => None,
            },
            It::Filter(s, f) => loop {
                match s.next(ip)? {
                    Some(x) => {
                        if truthy(ip.apply(f, vec![x.clone()])?)? {
                            break Some(x);
                        }
                    }
                    None => break None,
                }
            },
            It::Take(s, left) => {
                if *left <= 0 {
                    return Ok(None);
                }
                *left -= 1;
                s.next(ip)?
            }
            It::Drop(s, n) => {
                while *n > 0 {
                    *n -= 1;
                    if s.next(ip)?.is_none() {
                        return Ok(None);
                    }
                }
                s.next(ip)?
            }
            It::TakeWhile(s, f, done) => {
                if *done {
                    return Ok(None);
                }
                match s.next(ip)? {
                    Some(x) => {
                        if truthy(ip.apply(f, vec![x.clone()])?)? {
                            Some(x)
                        } else {
                            *done = true;
                            None
                        }
                    }
                    None => None,
                }
            }
            It::DropWhile(s, f, dropping) => loop {
                match s.next(ip)? {
                    Some(x) => {
                        if *dropping && truthy(ip.apply(f, vec![x.clone()])?)? {
                            continue;
                        }
                        *dropping = false;
                        break Some(x);
                    }
                    None => break None,
                }
            },
            It::Enumerate(s, k) => match s.next(ip)? {
                Some(x) => {
                    *k += 1;
                    Some(Value::Tuple(Rc::new(vec![Value::Int(*k - 1), x])))
                }
                None => None,
            },
            It::Zip(a, b) => match a.next(ip)? {
                Some(x) => b.next(ip)?.map(|y| Value::Tuple(Rc::new(vec![x, y]))),
                None => None,
            },
            It::Chain(a, second, b) => {
                if second.is_none() {
                    if let Some(x) = a.next(ip)? {
                        return Ok(Some(x));
                    }
                    *second = Some(Box::new(start(b)));
                }
                second.as_mut().unwrap().next(ip)?
            }
        })
    }
}

fn node(v: &Value) -> R<Rc<Node>> {
    match v {
        Value::View(n) => Ok(n.clone()),
        v => trap(format!("expected View, got {v}")),
    }
}

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

pub fn of_list(xs: Rc<Vec<Value>>) -> Value {
    Value::View(Rc::new(Node::List(xs)))
}

pub fn global(n: &str, a: &[Value]) -> R<Option<Value>> {
    Ok(Some(match n {
        "iota" => Value::View(Rc::new(Node::Iota(int(&a[0])?, int(&a[1])?))),
        "iterate" => Value::View(Rc::new(Node::Iterate(a[0].clone(), a[1].clone(), int(&a[2])?))),
        _ => return Ok(None),
    }))
}

impl Interp {
    pub(crate) fn for_view(&self, n: &Rc<Node>, mut body: impl FnMut(Value) -> R<()>) -> R<()> {
        let mut it = start(n);
        while let Some(x) = it.next(self)? {
            body(x)?;
        }
        Ok(())
    }

    pub(crate) fn view_method(&self, name: &str, n: Rc<Node>, a: Vec<Value>) -> R {
        let wrap = |k: Node| Ok(Value::View(Rc::new(k)));
        let f = || a[0].clone();
        match name {
            "map" => return wrap(Node::Map(n, f())),
            "filter" => return wrap(Node::Filter(n, f())),
            "take" => return wrap(Node::Take(n, int(&a[0])?)),
            "drop" => return wrap(Node::Drop(n, int(&a[0])?)),
            "take_while" => return wrap(Node::TakeWhile(n, f())),
            "drop_while" => return wrap(Node::DropWhile(n, f())),
            "enumerate" => return wrap(Node::Enumerate(n)),
            "zip" => return wrap(Node::Zip(n, node(&a[0])?)),
            "chain" => return wrap(Node::Chain(n, node(&a[0])?)),
            _ => {}
        }
        let mut it = start(&n);
        Ok(match name {
            "to_list" => {
                let mut out = Vec::new();
                while let Some(x) = it.next(self)? {
                    if out.len() == 1 << 26 {
                        return trap("out of memory");
                    }
                    out.push(x);
                }
                Value::list(out)
            }
            "fold" => {
                let mut acc = a[0].clone();
                while let Some(x) = it.next(self)? {
                    acc = self.apply(&a[1], vec![acc, x])?;
                }
                acc
            }
            "len" => {
                let mut k = 0i64;
                while it.next(self)?.is_some() {
                    k += 1;
                }
                Value::Int(k)
            }
            "sum" => {
                let mut s = 0i64;
                while let Some(x) = it.next(self)? {
                    s = match s.checked_add(int(&x)?) {
                        Some(v) => v,
                        None => return trap("integer overflow"),
                    };
                }
                Value::Int(s)
            }
            "first" => Value::Opt(it.next(self)?.map(Rc::new)),
            "find" | "any" | "all" => {
                let want = name != "all";
                let mut hit = None;
                while let Some(x) = it.next(self)? {
                    if truthy(self.apply(&a[0], vec![x.clone()])?)? == want {
                        hit = Some(x);
                        break;
                    }
                }
                match name {
                    "find" => Value::Opt(hit.map(Rc::new)),
                    "any" => Value::Bool(hit.is_some()),
                    _ => Value::Bool(hit.is_none()),
                }
            }
            _ => return trap(format!("no method '{name}' on View")),
        })
    }
}
