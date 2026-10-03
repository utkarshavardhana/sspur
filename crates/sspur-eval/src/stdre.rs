use crate::value::Value;
use crate::{trap, R};
use std::rc::Rc;

const LIMIT: usize = 20_000;
const MAX_REP: u32 = 1000;
const MAX_GROUPS: usize = 100;
const ANYC: u32 = u32::MAX;

#[derive(Clone)]
enum Node {
    Empty,
    Char(u32),
    Any,
    Class(usize),
    Assert(u8),
    Group(Option<usize>, Box<Node>),
    Cat(Vec<Node>),
    Alt(Vec<Node>),
    Rep(Box<Node>, u32, Option<u32>, bool),
}

#[derive(Clone, Copy)]
enum Inst {
    Char(u32),
    Any,
    Class(usize),
    Split(usize, usize),
    Jmp(usize),
    Save(usize),
    Assert(u8),
    Match,
}

pub struct Re {
    pub src: String,
    prog: Vec<Inst>,
    classes: Vec<(Vec<(u32, u32)>, bool)>,
    pub groups: usize,
}

const DIGIT: &[(u32, u32)] = &[(0x30, 0x39)];
const WORD: &[(u32, u32)] = &[(0x30, 0x39), (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A)];
const SPACE: &[(u32, u32)] = &[(0x09, 0x0D), (0x20, 0x20)];

fn complement(r: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    let mut lo = 0u32;
    for &(a, b) in r {
        if a > lo {
            out.push((lo, a - 1));
        }
        lo = b + 1;
    }
    if lo <= 0x10FFFF {
        out.push((lo, 0x10FFFF));
    }
    out
}

fn perl_class(e: u32) -> Option<Vec<(u32, u32)>> {
    let (set, neg) = match char::from_u32(e)? {
        'd' => (DIGIT, false),
        'D' => (DIGIT, true),
        'w' => (WORD, false),
        'W' => (WORD, true),
        's' => (SPACE, false),
        'S' => (SPACE, true),
        _ => return None,
    };
    Some(if neg { complement(set) } else { set.to_vec() })
}

fn simple_escape(e: u32) -> Option<u32> {
    Some(match e {
        0x6E => 0x0A,
        0x74 => 0x09,
        0x72 => 0x0D,
        0x66 => 0x0C,
        0x76 => 0x0B,
        0x30 => 0,
        e if e < 0x80 && !(e as u8).is_ascii_alphanumeric() => e,
        _ => return None,
    })
}

struct Parser {
    c: Vec<u32>,
    pos: usize,
    groups: usize,
    classes: Vec<(Vec<(u32, u32)>, bool)>,
}

enum ClassAtom {
    One(u32),
    Set(Vec<(u32, u32)>),
}

impl Parser {
    fn peek(&self) -> Option<u32> {
        self.c.get(self.pos).copied()
    }

    fn quant_at(&self, p: usize) -> Option<(u32, Option<u32>, usize)> {
        let c = &self.c;
        match c.get(p).copied()? {
            0x2A => Some((0, None, p + 1)),
            0x2B => Some((1, None, p + 1)),
            0x3F => Some((0, Some(1), p + 1)),
            0x7B => {
                let digits = |mut q: usize| -> (Option<u32>, usize) {
                    let st = q;
                    let mut v = 0u32;
                    while q < c.len() && (0x30..=0x39).contains(&c[q]) {
                        v = (v * 10 + (c[q] - 0x30)).min(100_000);
                        q += 1;
                    }
                    ((q > st).then_some(v), q)
                };
                let (n, q) = digits(p + 1);
                let n = n?;
                match c.get(q).copied()? {
                    0x7D => Some((n, Some(n), q + 1)),
                    0x2C => {
                        let (m, q) = digits(q + 1);
                        (c.get(q) == Some(&0x7D)).then_some((n, m, q + 1))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn alt(&mut self) -> Result<Node, &'static str> {
        let mut bs = vec![self.cat()?];
        while self.peek() == Some(0x7C) {
            self.pos += 1;
            bs.push(self.cat()?);
        }
        Ok(if bs.len() == 1 { bs.pop().unwrap() } else { Node::Alt(bs) })
    }

    fn cat(&mut self) -> Result<Node, &'static str> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == 0x7C || c == 0x29 {
                break;
            }
            items.push(self.repeat()?);
        }
        Ok(if items.is_empty() { Node::Empty } else { Node::Cat(items) })
    }

    fn repeat(&mut self) -> Result<Node, &'static str> {
        let atom = self.atom()?;
        let Some((min, max, next)) = self.quant_at(self.pos) else { return Ok(atom) };
        if min > MAX_REP || max.is_some_and(|m| m > MAX_REP || m < min) {
            return Err("bad repetition");
        }
        self.pos = next;
        let greedy = if self.peek() == Some(0x3F) {
            self.pos += 1;
            false
        } else {
            true
        };
        if self.quant_at(self.pos).is_some() {
            return Err("nothing to repeat");
        }
        Ok(Node::Rep(Box::new(atom), min, max, greedy))
    }

    fn class_node(&mut self, ranges: Vec<(u32, u32)>, neg: bool) -> Node {
        self.classes.push((ranges, neg));
        Node::Class(self.classes.len() - 1)
    }

    fn atom(&mut self) -> Result<Node, &'static str> {
        let c = self.c[self.pos];
        self.pos += 1;
        Ok(match c {
            0x28 => {
                let idx = if self.peek() == Some(0x3F) {
                    if self.c.get(self.pos + 1) != Some(&0x3A) {
                        return Err("bad group");
                    }
                    self.pos += 2;
                    None
                } else {
                    self.groups += 1;
                    if self.groups > MAX_GROUPS {
                        return Err("too many groups");
                    }
                    Some(self.groups)
                };
                let inner = self.alt()?;
                if self.peek() != Some(0x29) {
                    return Err("unclosed group");
                }
                self.pos += 1;
                Node::Group(idx, Box::new(inner))
            }
            0x5B => self.class()?,
            0x2E => Node::Any,
            0x5E => Node::Assert(0),
            0x24 => Node::Assert(1),
            0x5C => {
                let e = self.peek().ok_or("bad escape")?;
                self.pos += 1;
                match e {
                    0x62 => Node::Assert(2),
                    0x42 => Node::Assert(3),
                    _ => match perl_class(e) {
                        Some(set) => self.class_node(set, false),
                        None => Node::Char(simple_escape(e).ok_or("bad escape")?),
                    },
                }
            }
            0x2A | 0x2B | 0x3F => return Err("nothing to repeat"),
            0x7B if self.quant_at(self.pos - 1).is_some() => return Err("nothing to repeat"),
            c => Node::Char(c),
        })
    }

    fn class_atom(&mut self) -> Result<ClassAtom, &'static str> {
        let c = self.c[self.pos];
        self.pos += 1;
        if c != 0x5C {
            return Ok(ClassAtom::One(c));
        }
        let e = self.peek().ok_or("bad escape")?;
        self.pos += 1;
        match perl_class(e) {
            Some(set) => Ok(ClassAtom::Set(set)),
            None => Ok(ClassAtom::One(simple_escape(e).ok_or("bad escape")?)),
        }
    }

    fn class(&mut self) -> Result<Node, &'static str> {
        let neg = self.peek() == Some(0x5E);
        if neg {
            self.pos += 1;
        }
        let mut ranges = Vec::new();
        let mut first = true;
        loop {
            let c = self.peek().ok_or("unclosed class")?;
            if c == 0x5D && !first {
                self.pos += 1;
                break;
            }
            first = false;
            match self.class_atom()? {
                ClassAtom::Set(s) => ranges.extend(s),
                ClassAtom::One(a) => {
                    if self.peek() == Some(0x2D) && self.c.get(self.pos + 1).is_some_and(|&x| x != 0x5D) {
                        self.pos += 1;
                        let ClassAtom::One(b) = self.class_atom()? else { return Err("bad class range") };
                        if a > b {
                            return Err("bad class range");
                        }
                        ranges.push((a, b));
                    } else {
                        ranges.push((a, a));
                    }
                }
            }
        }
        Ok(self.class_node(ranges, neg))
    }
}

struct Compiler {
    prog: Vec<Inst>,
}

impl Compiler {
    fn emit(&mut self, i: Inst) -> Result<usize, &'static str> {
        if self.prog.len() >= LIMIT {
            return Err("regex too large");
        }
        self.prog.push(i);
        Ok(self.prog.len() - 1)
    }

    fn node(&mut self, n: &Node) -> Result<(), &'static str> {
        match n {
            Node::Empty => {}
            Node::Char(c) => {
                self.emit(Inst::Char(*c))?;
            }
            Node::Any => {
                self.emit(Inst::Any)?;
            }
            Node::Class(k) => {
                self.emit(Inst::Class(*k))?;
            }
            Node::Assert(k) => {
                self.emit(Inst::Assert(*k))?;
            }
            Node::Group(idx, inner) => {
                if let Some(i) = idx {
                    self.emit(Inst::Save(2 * i))?;
                }
                self.node(inner)?;
                if let Some(i) = idx {
                    self.emit(Inst::Save(2 * i + 1))?;
                }
            }
            Node::Cat(xs) => {
                for x in xs {
                    self.node(x)?;
                }
            }
            Node::Alt(bs) => {
                let mut jumps = Vec::new();
                for (k, b) in bs.iter().enumerate() {
                    if k + 1 < bs.len() {
                        let s = self.emit(Inst::Split(0, 0))?;
                        self.node(b)?;
                        jumps.push(self.emit(Inst::Jmp(0))?);
                        let here = self.prog.len();
                        self.prog[s] = Inst::Split(s + 1, here);
                    } else {
                        self.node(b)?;
                    }
                }
                let end = self.prog.len();
                for j in jumps {
                    self.prog[j] = Inst::Jmp(end);
                }
            }
            Node::Rep(inner, min, max, greedy) => {
                for _ in 0..*min {
                    self.node(inner)?;
                }
                match max {
                    None => {
                        let l = self.emit(Inst::Split(0, 0))?;
                        self.node(inner)?;
                        self.emit(Inst::Jmp(l))?;
                        let end = self.prog.len();
                        self.prog[l] = if *greedy { Inst::Split(l + 1, end) } else { Inst::Split(end, l + 1) };
                    }
                    Some(mx) => {
                        let mut splits = Vec::new();
                        for _ in *min..*mx {
                            splits.push(self.emit(Inst::Split(0, 0))?);
                            self.node(inner)?;
                        }
                        let end = self.prog.len();
                        for s in splits {
                            self.prog[s] = if *greedy { Inst::Split(s + 1, end) } else { Inst::Split(end, s + 1) };
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn compile(src: &str) -> Result<Re, &'static str> {
    let mut p = Parser { c: src.chars().map(|c| c as u32).collect(), pos: 0, groups: 0, classes: Vec::new() };
    let root = p.alt()?;
    if p.pos < p.c.len() {
        return Err("unmatched ')'");
    }
    let mut c = Compiler { prog: Vec::new() };
    c.emit(Inst::Save(0))?;
    c.node(&root)?;
    c.emit(Inst::Save(1))?;
    c.emit(Inst::Match)?;
    Ok(Re { src: src.to_string(), prog: c.prog, classes: p.classes, groups: p.groups })
}

struct List {
    dense: Vec<usize>,
    sparse: Vec<usize>,
    caps: Vec<Vec<isize>>,
}

impl List {
    fn new(n: usize, slots: usize) -> List {
        List { dense: Vec::with_capacity(n), sparse: vec![0; n], caps: vec![vec![-1; slots]; n] }
    }

    fn contains(&self, pc: usize) -> bool {
        let i = self.sparse[pc];
        i < self.dense.len() && self.dense[i] == pc
    }

    fn insert(&mut self, pc: usize) {
        self.sparse[pc] = self.dense.len();
        self.dense.push(pc);
    }
}

fn is_word(b: Option<&u8>) -> bool {
    b.is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
}

fn decode(t: &[u8], i: usize) -> (u32, usize) {
    let c = t[i];
    if c < 0x80 {
        return (u32::from(c), i + 1);
    }
    let k = if c >= 0xF0 { 3 } else if c >= 0xE0 { 2 } else { 1 };
    let mut cp = u32::from(c) & (0x3F >> k);
    for j in 1..=k {
        cp = (cp << 6) | (u32::from(t[i + j]) & 0x3F);
    }
    (cp, i + k + 1)
}

impl Re {
    fn holds(&self, kind: u8, t: &[u8], pos: usize) -> bool {
        match kind {
            0 => pos == 0,
            1 => pos == t.len(),
            k => {
                let b = is_word(pos.checked_sub(1).and_then(|p| t.get(p))) != is_word(t.get(pos));
                if k == 2 { b } else { !b }
            }
        }
    }

    fn add(&self, l: &mut List, pc0: usize, pos: usize, caps: &mut [isize], t: &[u8]) {
        enum F {
            Explore(usize),
            Restore(usize, isize),
        }
        let mut stack = vec![F::Explore(pc0)];
        while let Some(f) = stack.pop() {
            match f {
                F::Restore(k, v) => caps[k] = v,
                F::Explore(mut pc) => loop {
                    if l.contains(pc) {
                        break;
                    }
                    l.insert(pc);
                    match self.prog[pc] {
                        Inst::Jmp(x) => pc = x,
                        Inst::Split(x, y) => {
                            stack.push(F::Explore(y));
                            pc = x;
                        }
                        Inst::Save(k) => {
                            stack.push(F::Restore(k, caps[k]));
                            caps[k] = pos as isize;
                            pc += 1;
                        }
                        Inst::Assert(k) => {
                            if !self.holds(k, t, pos) {
                                break;
                            }
                            pc += 1;
                        }
                        _ => {
                            l.caps[pc].copy_from_slice(caps);
                            break;
                        }
                    }
                },
            }
        }
    }

    fn class_hit(&self, k: usize, c: u32) -> bool {
        let (r, neg) = &self.classes[k];
        r.iter().any(|&(a, b)| a <= c && c <= b) != *neg
    }

    pub fn search(&self, t: &[u8], start: usize) -> Option<Vec<isize>> {
        let n = self.prog.len();
        let slots = 2 * (self.groups + 1);
        let (mut cl, mut nl) = (List::new(n, slots), List::new(n, slots));
        let mut matched: Option<Vec<isize>> = None;
        let mut pos = start;
        let mut caps = vec![-1isize; slots];
        loop {
            if matched.is_none() {
                caps.fill(-1);
                self.add(&mut cl, 0, pos, &mut caps, t);
            }
            if cl.dense.is_empty() {
                break;
            }
            let (c, next) = if pos < t.len() { decode(t, pos) } else { (ANYC, pos) };
            for i in 0..cl.dense.len() {
                let pc = cl.dense[i];
                let hit = match self.prog[pc] {
                    Inst::Char(x) => pos < t.len() && c == x,
                    Inst::Any => pos < t.len() && c != 0x0A,
                    Inst::Class(k) => pos < t.len() && self.class_hit(k, c),
                    Inst::Match => {
                        matched = Some(cl.caps[pc].clone());
                        break;
                    }
                    _ => false,
                };
                if hit {
                    caps.copy_from_slice(&cl.caps[pc]);
                    self.add(&mut nl, pc + 1, next, &mut caps, t);
                }
            }
            std::mem::swap(&mut cl, &mut nl);
            nl.dense.clear();
            if pos >= t.len() {
                break;
            }
            pos = next;
        }
        matched
    }

    fn matches(&self, t: &str) -> Vec<Vec<isize>> {
        let b = t.as_bytes();
        let mut out = Vec::new();
        let mut pos = 0;
        while pos <= b.len() {
            let Some(m) = self.search(b, pos) else { break };
            let (s, e) = (m[0] as usize, m[1] as usize);
            out.push(m);
            if e == s {
                if e >= b.len() {
                    break;
                }
                pos = decode(b, e).1;
            } else {
                pos = e;
            }
        }
        out
    }
}

fn group_text<'a>(t: &'a str, m: &[isize], g: usize) -> &'a str {
    match (m[2 * g], m[2 * g + 1]) {
        (s, e) if s >= 0 && e >= s => &t[s as usize..e as usize],
        _ => "",
    }
}

fn expand(rep: &str, t: &str, m: &[isize], groups: usize, out: &mut String) {
    let b = rep.as_bytes();
    let mut i = 0;
    let mut run = 0;
    while i < b.len() {
        if b[i] == b'$' && i + 1 < b.len() {
            let d = b[i + 1];
            if d == b'$' {
                out.push_str(&rep[run..i + 1]);
                i += 2;
                run = i;
                continue;
            }
            if d.is_ascii_digit() && usize::from(d - b'0') <= groups {
                out.push_str(&rep[run..i]);
                out.push_str(group_text(t, m, usize::from(d - b'0')));
                i += 2;
                run = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&rep[run..]);
}

pub fn global(a: &[Value]) -> R<Value> {
    let Value::Str(p) = &a[0] else { return trap("expected Str") };
    Ok(Value::Res(match compile(p) {
        Ok(re) => Ok(Rc::new(Value::Regex(Rc::new(re)))),
        Err(m) => Err(Rc::new(Value::str(m))),
    }))
}

pub fn method(name: &str, re: &Re, a: &[Value]) -> R<Value> {
    let Some(Value::Str(t)) = a.first() else { return trap("expected Str") };
    let t: &str = t;
    let s = |x: &str| Value::str(x);
    Ok(match name {
        "is_match" => Value::Bool(re.search(t.as_bytes(), 0).is_some()),
        "find" => Value::Opt(re.search(t.as_bytes(), 0).map(|m| Rc::new(s(group_text(t, &m, 0))))),
        "span" => Value::Opt(re.search(t.as_bytes(), 0).map(|m| {
            let cs = |b: isize| Value::Int(t[..b as usize].chars().count() as i64);
            Rc::new(Value::Tuple(Rc::new(vec![cs(m[0]), cs(m[1])])))
        })),
        "find_all" => Value::list(re.matches(t).iter().map(|m| s(group_text(t, m, 0))).collect()),
        "captures" => Value::Opt(re.search(t.as_bytes(), 0).map(|m| Rc::new(Value::list((0..=re.groups).map(|g| s(group_text(t, &m, g))).collect())))),
        "replace" | "split" => {
            let rep = match a.get(1) {
                Some(Value::Str(r)) => r.to_string(),
                _ => String::new(),
            };
            let mut out = String::new();
            let mut parts = Vec::new();
            let mut last = 0usize;
            for m in re.matches(t) {
                let (st, en) = (m[0] as usize, m[1] as usize);
                if name == "split" {
                    parts.push(s(&t[last..st]));
                } else {
                    out.push_str(&t[last..st]);
                    expand(&rep, t, &m, re.groups, &mut out);
                }
                last = en;
            }
            if name == "split" {
                parts.push(s(&t[last..]));
                Value::list(parts)
            } else {
                out.push_str(&t[last..]);
                s(&out)
            }
        }
        _ => return trap(format!("no method '{name}' on Regex")),
    })
}
