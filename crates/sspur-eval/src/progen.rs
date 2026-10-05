use std::collections::BTreeSet;

pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        for _ in 0..4 {
            r.next();
        }
        r
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }

    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }

    fn chance(&mut self, pct: u64) -> bool {
        self.below(100) < pct
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum T {
    Int,
    Bool,
    Str,
    List,
    Rec,
    Sum,
    Tree,
    Map,
    Set,
    HMap,
    Clo,
}

#[derive(Clone)]
struct Var {
    name: String,
    ty: T,
    mutable: bool,
    assignable: bool,
    index_of: Option<String>,
    eff: Vec<String>,
}

#[derive(Clone)]
struct Sig {
    name: String,
    params: Vec<T>,
    ret: T,
    effects: Vec<String>,
}

#[derive(Default)]
struct Cx {
    vars: Vec<Var>,
    eff: BTreeSet<String>,
    ask_ok: bool,
    lambda: bool,
    ret: Option<T>,
    no_risky: bool,
    marks: Vec<usize>,
}

const BOUNDARY: &[&str] = &[
    "9223372036854775807",
    "(-9223372036854775807 - 1)",
    "9223372036854775806",
    "(-9223372036854775807)",
    "4611686018427387904",
    "(-4611686018427387904)",
    "3037000499",
    "3037000500",
    "(-3037000500)",
    "2147483647",
    "(-2147483648)",
    "4294967296",
];

const STRS: &[&str] = &["\"\"", "\"a\"", "\"ab\"", "\"Hello\"", "\" x y \"", "\"é\"", "\"日本\"", "\"a,b,c\"", "\"42\"", "\"-7\"", "\"zz9\""];

pub struct Gen {
    r: Rng,
    p: String,
    c: String,
    sigs: Vec<Sig>,
    n: usize,
    out: String,
}

pub fn program(seed: u64, id: usize) -> String {
    let mut g = Gen { r: Rng::new(seed), p: format!("p{id}"), c: format!("P{id}"), sigs: vec![], n: 0, out: String::new() };
    g.module();
    g.out
}

fn ty_name(c: &str, t: T) -> String {
    match t {
        T::Int => "Int".into(),
        T::Bool => "Bool".into(),
        T::Str => "Str".into(),
        T::List => "List[Int]".into(),
        T::Rec => format!("{c}R"),
        T::Sum => format!("{c}S"),
        T::Tree => format!("{c}T"),
        T::Map => "Map[Int, Int]".into(),
        T::Set => "Set[Int]".into(),
        T::HMap => "HashMap[Str, Int]".into(),
        T::Clo => "Int -> Int".into(),
    }
}

impl Gen {
    fn fresh(&mut self, base: &str) -> String {
        self.n += 1;
        format!("{base}{}", self.n)
    }

    fn line(&mut self, ind: usize, s: &str) {
        for _ in 0..ind {
            self.out.push_str("  ");
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn eff_of(&self, cx: &Cx) -> String {
        if cx.eff.is_empty() {
            String::new()
        } else {
            format!(" ! {}", cx.eff.iter().cloned().collect::<Vec<_>>().join(", "))
        }
    }

    fn module(&mut self) {
        let (p, c) = (self.p.clone(), self.c.clone());
        let refine = ["_ >= 0", "_ > -100", "_ != 0", "_ >= -1000 and _ <= 1000"][self.r.below(4) as usize];
        self.out.push_str(&format!("type {c}R = {{a: Int, b: Int where {refine}}}\n"));
        self.out.push_str(&format!("type {c}S = {c}A{{x: Int}} | {c}B{{x: Int, y: Int}} | {c}C\n"));
        self.out.push_str(&format!("type {c}T = {c}Lf | {c}Nd{{l: {c}T, v: Int, r: {c}T}}\n"));
        self.out.push_str(&format!("type {c}E = {c}Bad{{c: Int}} | {c}Worse\n"));
        self.out.push_str(&format!("effect {p}ask(x: Int) -> Int\n\n"));
        self.tree_helpers();
        self.risky();
        self.asker();
        let k = 3 + self.r.below(5);
        for i in 0..k {
            match self.r.below(10) {
                0 | 1 => self.recursive(),
                2 => self.tree_rec(),
                _ => self.random_fn(i < 2),
            }
        }
        self.entry();
    }

    fn sig(&mut self, name: &str, params: &[T], ret: T, effects: &[&str]) {
        self.sigs.push(Sig { name: name.into(), params: params.to_vec(), ret, effects: effects.iter().map(|s| s.to_string()).collect() });
    }

    fn tree_helpers(&mut self) {
        let (p, c) = (self.p.clone(), self.c.clone());
        let cmp = ["x < v", "x <= v", "x > v"][self.r.below(3) as usize];
        self.out.push_str(&format!(
            "fn {p}_ins(t: {c}T, x: Int) -> {c}T\n= match t\n  | {c}Lf => {c}Nd{{l: {c}Lf, v: x, r: {c}Lf}}\n  | {c}Nd{{l, v, r}} => if {cmp} then {c}Nd{{l: {p}_ins(l, x), v: v, r: r}} else {c}Nd{{l: l, v: v, r: {p}_ins(r, x)}}\n\n"
        ));
        self.sig(&format!("{p}_ins"), &[T::Tree, T::Int], T::Tree, &[]);
        let upd = ["v + d", "v * d", "v - d", "(v + d) % 1000", "d / (v + 1)"][self.r.below(5) as usize];
        self.out.push_str(&format!(
            "fn {p}_inc(t: {c}T, d: Int) -> {c}T\n= match t\n  | {c}Lf => {c}Lf\n  | {c}Nd{{l, v, r}} => {c}Nd{{l: {p}_inc(l, d), v: {upd}, r: {p}_inc(r, d)}}\n\n"
        ));
        self.sig(&format!("{p}_inc"), &[T::Tree, T::Int], T::Tree, &[]);
        self.out.push_str(&format!("fn {p}_tsum(t: {c}T) -> Int\n= match t\n  | {c}Lf => 0\n  | {c}Nd{{l, v, r}} => {p}_tsum(l) + v + {p}_tsum(r)\n\n"));
        self.sig(&format!("{p}_tsum"), &[T::Tree], T::Int, &[]);
        self.out.push_str(&format!("fn {p}_items(t: {c}T) -> List[Int]\n= match t\n  | {c}Lf => []\n  | {c}Nd{{l, v, r}} => {p}_items(l) + [v] + {p}_items(r)\n\n"));
        self.sig(&format!("{p}_items"), &[T::Tree], T::List, &[]);
        self.out.push_str(&format!("fn {p}_build(xs: List[Int]) -> {c}T\n= do\n  var t = {c}Lf\n  for x in xs\n    t := {p}_ins(t, x)\n  t\n\n"));
        self.sig(&format!("{p}_build"), &[T::List], T::Tree, &[]);
        let kexpr = ["x % 7", "x / 3", "x", "x * x % 11"][self.r.below(4) as usize];
        self.out.push_str(&format!("fn {p}_mkmap(xs: List[Int]) -> Map[Int, Int]\n= xs.fold(empty_map(), (m, x) => m.put({kexpr}, m.get({kexpr}).or(0) + x))\n\n"));
        self.sig(&format!("{p}_mkmap"), &[T::List], T::Map, &[]);
        self.out.push_str(&format!("fn {p}_mkhm(xs: List[Int]) -> HashMap[Str, Int]\n= xs.fold(hash_map(), (m, x) => m.put(\"k{{x % 5}}\", x))\n\n"));
        self.sig(&format!("{p}_mkhm"), &[T::List], T::HMap, &[]);
    }

    fn risky(&mut self) {
        let (p, c) = (self.p.clone(), self.c.clone());
        let mut cx = Cx::default();
        cx.vars.push(Var { name: "a".into(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        let k1 = self.r.range(2, 7);
        let k2 = self.r.range(2, 9);
        cx.no_risky = true;
        let body = self.int(&mut cx, 2);
        cx.eff.remove(&format!("fail[{c}E]"));
        let eff = if cx.eff.is_empty() { format!(" ! fail[{c}E]") } else { format!(" ! fail[{c}E], {}", cx.eff.iter().cloned().collect::<Vec<_>>().join(", ")) };
        self.out.push_str(&format!(
            "fn {p}_risky(a: Int) -> Int{eff}\n= if a % {k1} == 0 then raise {c}Bad{{c: a}} else if a % {k2} == 1 then raise {c}Worse else {body}\n\n"
        ));
        let mut effs = vec![format!("fail[{c}E]")];
        effs.extend(cx.eff.iter().cloned());
        self.sigs.push(Sig { name: format!("{p}_risky"), params: vec![T::Int], ret: T::Int, effects: effs });
    }

    fn asker(&mut self) {
        let p = self.p.clone();
        let mut cx = Cx::default();
        cx.vars.push(Var { name: "a".into(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        let e = self.int(&mut cx, 1);
        let op = ["+", "-", "*"][self.r.below(3) as usize];
        cx.eff.insert(format!("{p}ask"));
        let eff = self.eff_of(&cx);
        self.out.push_str(&format!("fn {p}_eff(a: Int) -> Int{eff}\n= {p}ask(a) {op} {p}ask({e})\n\n"));
        let effs: Vec<String> = cx.eff.into_iter().collect();
        self.sigs.push(Sig { name: format!("{p}_eff"), params: vec![T::Int], ret: T::Int, effects: effs });
    }

    fn recursive(&mut self) {
        let p = self.p.clone();
        let name = self.fresh(&format!("{p}_rec"));
        let refined = self.r.chance(40);
        let mut cx = Cx::default();
        cx.vars.push(Var { name: "n".into(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        cx.vars.push(Var { name: "acc".into(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        let step = self.int(&mut cx, 2);
        let base = self.int(&mut cx, 1);
        let dec = ["n - 1", "n / 2", "n - 2", "n % 10 - 1"][self.r.below(4) as usize];
        let stop = if refined { "n <= 0" } else { ["n <= 0", "n < 2", "n == 0"][self.r.below(3) as usize] };
        let body = if self.r.chance(50) {
            format!("if {stop} then {base} else {name}({dec}, {step})")
        } else {
            format!("if {stop} then {base} else ({step}) + {name}({dec}, acc)")
        };
        let post = if self.r.chance(25) { format!("\n  post r {}", ["!= 7", ">= acc", ">= -1000000", "> -9223372036854775807"][self.r.below(4) as usize]) } else { String::new() };
        let pt = if refined { "Int where _ >= 0" } else { "Int" };
        let eff = self.eff_of(&cx);
        self.out.push_str(&format!("fn {name}(n: {pt}, acc: Int) -> Int{eff}{post}\n= {body}\n\n"));
        let effs: Vec<String> = cx.eff.into_iter().collect();
        self.sigs.push(Sig { name, params: vec![T::Int, T::Int], ret: T::Int, effects: effs });
    }

    fn tree_rec(&mut self) {
        let (p, c) = (self.p.clone(), self.c.clone());
        let name = self.fresh(&format!("{p}_tr"));
        let mut cx = Cx::default();
        cx.vars.push(Var { name: "k".into(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        cx.vars.push(Var { name: "v".into(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        let nv = self.int(&mut cx, 2);
        let swap = self.r.chance(30);
        let (lf, rt) = if swap { (format!("{name}(r, k)"), format!("{name}(l, k)")) } else { (format!("{name}(l, k)"), format!("{name}(r, k)")) };
        let eff = self.eff_of(&cx);
        let arm = if self.r.chance(30) {
            format!("if v > k then {c}Nd{{l: {lf}, v: {nv}, r: r}} else {c}Nd{{l: l, v: v, r: {rt}}}")
        } else {
            format!("{c}Nd{{l: {lf}, v: {nv}, r: {rt}}}")
        };
        self.out.push_str(&format!("fn {name}(t: {c}T, k: Int) -> {c}T{eff}\n= match t\n  | {c}Lf => {c}Lf\n  | {c}Nd{{l, v, r}} => {arm}\n\n"));
        let effs: Vec<String> = cx.eff.into_iter().collect();
        self.sigs.push(Sig { name, params: vec![T::Tree, T::Int], ret: T::Tree, effects: effs });
    }

    fn random_fn(&mut self, simple: bool) {
        let p = self.p.clone();
        let name = self.fresh(&format!("{p}_f"));
        let mut cx = Cx::default();
        let np = 1 + self.r.below(3) as usize;
        let mut params = vec![];
        let mut ptext = vec![];
        let mut pres = vec![];
        for i in 0..np {
            let t = if simple || self.r.chance(55) { T::Int } else { [T::List, T::Str, T::Rec, T::Sum, T::Tree, T::Map][self.r.below(6) as usize] };
            let pn = ["a", "b", "c"][i].to_string();
            let mut tn = ty_name(&self.c, t);
            if t == T::Int && self.r.chance(25) {
                tn = format!("Int where {}", ["_ >= 0", "_ > 0", "_ < 1000", "_ >= -50 and _ <= 50"][self.r.below(4) as usize]);
            }
            ptext.push(format!("{pn}: {tn}"));
            params.push(t);
            cx.vars.push(Var { name: pn, ty: t, mutable: false, assignable: false, index_of: None, eff: vec![] });
        }
        let ints: Vec<String> = cx.vars.iter().filter(|v| v.ty == T::Int).map(|v| v.name.clone()).collect();
        if !ints.is_empty() && self.r.chance(35) {
            let a = &ints[self.r.below(ints.len() as u64) as usize];
            let b = &ints[self.r.below(ints.len() as u64) as usize];
            pres.push(match self.r.below(4) {
                0 => format!("pre {a} <= {b}"),
                1 => format!("pre {a} > -1000 and {a} < 1000"),
                2 => format!("pre {a} != {b} or {a} == 0"),
                _ => format!("pre {a} >= 0"),
            });
        }
        let ret = if simple { T::Int } else { [T::Int, T::Int, T::Int, T::Bool, T::Str, T::List, T::Rec, T::Tree, T::Map, T::Set, T::HMap][self.r.below(11) as usize] };
        cx.ret = Some(ret);
        if ret == T::Int && self.r.chance(25) {
            pres.push(format!("post {}", ["r >= 0", "r != 0", "r > -1000000", "r >= a or r < a"][self.r.below(4) as usize]).replace(" a", &format!(" {}", ints.first().cloned().unwrap_or("0".into()))));
        }
        let save = std::mem::take(&mut self.out);
        if self.r.chance(30) && !matches!(ret, T::Clo) {
            let e = self.expr(&mut cx, ret, 3);
            self.out.push_str(&format!("= {e}\n"));
        } else {
            self.line(0, "= do");
            let nb = 2 + self.r.below(5) as usize; self.block(&mut cx, 1, nb, Some(ret), 0);
        }
        let body = std::mem::replace(&mut self.out, save);
        let eff = self.eff_of(&cx);
        self.out.push_str(&format!("fn {name}({}) -> {}{eff}\n", ptext.join(", "), ty_name(&self.c, ret)));
        for pr in pres {
            self.out.push_str(&format!("  {pr}\n"));
        }
        self.out.push_str(&body);
        self.out.push('\n');
        let effs: Vec<String> = cx.eff.into_iter().collect();
        self.sigs.push(Sig { name, params, ret, effects: effs });
    }

    fn entry(&mut self) {
        let p = self.p.clone();
        let mut cx = Cx { ret: Some(T::Str), ..Default::default() };
        let save = std::mem::take(&mut self.out);
        self.line(0, "= do");
        let nb = 3 + self.r.below(4) as usize; self.block(&mut cx, 1, nb, Some(T::Str), 0);
        let body = std::mem::replace(&mut self.out, save);
        let eff = self.eff_of(&cx);
        self.out.push_str(&format!("fn {p}_main() -> Str{eff}\n{body}\n"));
    }

    fn block(&mut self, cx: &mut Cx, ind: usize, n: usize, ret: Option<T>, loop_depth: u32) {
        let mark = cx.vars.len();
        cx.marks.push(mark);
        let start = self.out.len();
        for _ in 0..n {
            self.stmt(cx, ind, loop_depth);
        }
        if let Some(t) = ret {
            let e = self.expr(cx, t, 3);
            self.line(ind, &e);
        } else if !cx.vars.iter().any(|v| v.assignable) || self.out.len() == start {
            let e = self.int(cx, 1);
            self.line(ind, &format!("log(\"{{{e}}}\")"));
            cx.eff.insert("log".into());
        }
        cx.vars.truncate(mark);
        cx.marks.pop();
    }

    fn bind(&mut self, cx: &mut Cx, name: String, ty: T, mutable: bool) {
        cx.vars.push(Var { name, ty, mutable, assignable: mutable, index_of: None, eff: vec![] });
    }

    fn stmt(&mut self, cx: &mut Cx, ind: usize, loop_depth: u32) {
        let k = self.r.below(100);
        let assignable: Vec<Var> = cx.vars.iter().filter(|v| v.assignable).cloned().collect();
        if k < 14 {
            let t = self.any_ty();
            let e = self.expr(cx, t, 3);
            let shadow = cx.vars.iter().filter(|v| v.ty == t && !v.mutable).map(|v| v.name.clone()).collect::<Vec<_>>();
            let mark = cx.marks.last().copied().unwrap_or(0);
            let local: Vec<String> = cx.vars[mark..].iter().filter(|v| !v.mutable && v.ty != t && !cx.vars[..mark].iter().any(|o| o.name == v.name)).map(|v| v.name.clone()).collect();
            let name = if !shadow.is_empty() && self.r.chance(25) {
                shadow[self.r.below(shadow.len() as u64) as usize].clone()
            } else if !local.is_empty() && self.r.chance(15) {
                let n = local[self.r.below(local.len() as u64) as usize].clone();
                cx.vars.retain(|v| v.name != n);
                n
            } else {
                self.fresh("x")
            };
            self.line(ind, &format!("{name} = {e}"));
            self.bind(cx, name, t, false);
        } else if k < 28 {
            let t = [T::Int, T::Int, T::Int, T::List, T::Str, T::Tree, T::Map, T::Set, T::Rec][self.r.below(9) as usize];
            let e = self.expr(cx, t, 2);
            let name = self.fresh("v");
            self.line(ind, &format!("var {name} = {e}"));
            self.bind(cx, name, t, true);
        } else if k < 44 && !assignable.is_empty() {
            let v = &assignable[self.r.below(assignable.len() as u64) as usize];
            let e = self.update(cx, v);
            self.line(ind, &format!("{} := {e}", v.name));
        } else if k < 56 && loop_depth < 2 {
            self.for_loop(cx, ind, loop_depth);
        } else if k < 60 && loop_depth < 2 {
            let c = self.fresh("c");
            let lim = self.r.range(0, 6);
            self.ensure_acc(cx, ind);
            self.line(ind, &format!("var {c} = 0"));
            self.line(ind, &format!("while {c} < {lim}"));
            cx.vars.push(Var { name: c.clone(), ty: T::Int, mutable: true, assignable: false, index_of: None, eff: vec![] });
            let nb = 1 + self.r.below(3) as usize; self.block(cx, ind + 1, nb, None, loop_depth + 1);
            self.line(ind + 1, &format!("{c} := {c} + 1"));
            cx.eff.insert("div".into());
        } else if k < 68 && !assignable.is_empty() {
            let b = self.boolean(cx, 2);
            let v = &assignable[self.r.below(assignable.len() as u64) as usize];
            let e = self.update(cx, v);
            if self.r.chance(60) {
                self.line(ind, &format!("if {b} then {} := {e}", v.name));
            } else {
                self.line(ind, &format!("if {b} then do"));
                let nb = 1 + self.r.below(2) as usize; self.block(cx, ind + 1, nb, None, loop_depth + 1);
                self.line(ind, "else do");
                self.block(cx, ind + 1, 1, None, loop_depth + 1);
            }
        } else if k < 74 {
            self.multi(cx, ind);
        } else if k < 79 {
            let ints: Vec<Var> = cx.vars.iter().filter(|v| v.ty == T::Int && (v.mutable || self.r.chance(50))).cloned().collect();
            if !ints.is_empty() {
                let v = &ints[self.r.below(ints.len() as u64) as usize];
                let f = self.fresh("f");
                let x = self.fresh("y");
                let op = ["+", "*", "-", "%"][self.r.below(4) as usize];
                self.line(ind, &format!("{f} = {x} => {x} {op} {}", v.name));
                cx.vars.push(Var { name: f, ty: T::Clo, mutable: false, assignable: false, index_of: None, eff: vec![] });
            } else {
                let name = self.fresh("v");
                let e = self.int(cx, 2);
                self.line(ind, &format!("var {name} = {e}"));
                self.bind(cx, name, T::Int, true);
            }
        } else if k < 83 && !cx.lambda {
            if let Some(t) = cx.ret {
                let b = self.boolean(cx, 1);
                let e = self.expr(cx, t, 2);
                self.line(ind, &format!("if {b} then return {e}"));
            }
        } else if k < 86 {
            let e = self.int(cx, 2);
            let s = self.string(cx, 1);
            self.line(ind, &format!("log(\"{{{e}}} {{{s}}}\")"));
            cx.eff.insert("log".into());
        } else if k < 90 {
            self.local_fn(cx, ind);
        } else {
            let name = self.fresh("v");
            let e = self.int(cx, 2);
            self.line(ind, &format!("var {name} = {e}"));
            self.bind(cx, name, T::Int, true);
        }
    }

    fn ensure_acc(&mut self, cx: &mut Cx, ind: usize) {
        if !cx.vars.iter().any(|v| v.assignable) {
            let name = self.fresh("acc");
            self.line(ind, &format!("var {name} = 0"));
            self.bind(cx, name, T::Int, true);
        }
    }

    fn local_fn(&mut self, cx: &mut Cx, ind: usize) {
        let ints: Vec<Var> = cx.vars.iter().filter(|v| v.assignable && v.ty == T::Int).cloned().collect();
        let Some(v) = ints.last() else { return };
        let f = self.fresh("g");
        let k = self.fresh("k");
        let mut inner = Cx { vars: cx.vars.clone(), lambda: true, ..Default::default() };
        inner.vars.push(Var { name: k.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        let e = self.int(&mut inner, 2);
        let eff: Vec<String> = inner.eff.into_iter().collect();
        let row = if eff.is_empty() { String::new() } else { format!(" ! {}", eff.join(", ")) };
        self.line(ind, &format!("fn {f}({k}: Int) -> Int{row}"));
        self.line(ind, "= do");
        self.line(ind + 1, &format!("{} := {e}", v.name));
        self.line(ind + 1, &v.name);
        cx.vars.push(Var { name: f, ty: T::Clo, mutable: false, assignable: false, index_of: None, eff });
    }

    fn for_loop(&mut self, cx: &mut Cx, ind: usize, loop_depth: u32) {
        self.ensure_acc(cx, ind);
        let i = self.fresh("i");
        let lists: Vec<Var> = cx.vars.iter().filter(|v| v.ty == T::List).cloned().collect();
        let mut index_of = None;
        let src = match self.r.below(6) {
            0 | 1 if !lists.is_empty() => {
                let l = &lists[self.r.below(lists.len() as u64) as usize];
                index_of = Some(l.name.clone());
                format!("0..{}.len", l.name)
            }
            2 if !lists.is_empty() => lists[self.r.below(lists.len() as u64) as usize].name.clone(),
            3 => {
                let (a, b) = (self.r.range(-5, 5), self.r.range(-5, 12));
                let s = [1, 2, 3, -1, -2][self.r.below(5) as usize];
                format!("range({a}, {b}, {s})")
            }
            _ => {
                let hi = if self.r.chance(30) {
                    let e = self.int_atom(cx);
                    format!("{e} % 8")
                } else {
                    self.r.range(0, 7).to_string()
                };
                format!("0..{hi}")
            }
        };
        self.line(ind, &format!("for {i} in {src}"));
        cx.vars.push(Var { name: i.clone(), ty: T::Int, mutable: false, assignable: false, index_of, eff: vec![] });
        if self.r.chance(15) && !lists.is_empty() {
            let l = &lists[self.r.below(lists.len() as u64) as usize];
            if l.mutable {
                let k = self.r.range(0, 3);
                self.line(ind + 1, &format!("{} := {}.take({k})", l.name, l.name));
            } else {
                self.line(ind + 1, &format!("{} = {}.drop(1)", l.name, l.name));
            }
        }
        let nb = 1 + self.r.below(3) as usize; self.block(cx, ind + 1, nb, None, loop_depth + 1);
        cx.vars.retain(|v| v.name != i);
    }

    fn multi(&mut self, cx: &mut Cx, ind: usize) {
        let name = self.fresh("m");
        let c = self.c.clone();
        let p = self.p.clone();
        match self.r.below(5) {
            0 => {
                let s = self.expr(cx, T::Sum, 2);
                self.line(ind, &format!("{name} = match {s}"));
                let (x, y) = (self.fresh("q"), self.fresh("q"));
                cx.vars.push(Var { name: x.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
                let e1 = self.int(cx, 2);
                cx.vars.push(Var { name: y.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
                let e2 = self.int(cx, 2);
                cx.vars.truncate(cx.vars.len() - 2);
                let e3 = self.int(cx, 1);
                self.line(ind + 1, &format!("| {c}A{{x: {x}}} => {e1}"));
                if self.r.chance(30) {
                    let g = self.boolean(cx, 1);
                    let e4 = self.int(cx, 1);
                    self.line(ind + 1, &format!("| {c}B{{x: {x}, y: {y}}} if {g} => {e4}"));
                }
                cx.vars.push(Var { name: x.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
                cx.vars.push(Var { name: y.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
                cx.vars.truncate(cx.vars.len() - 2);
                self.line(ind + 1, &format!("| {c}B{{x: {x}, y: {y}}} => {e2}"));
                self.line(ind + 1, &format!("| {c}C => {e3}"));
            }
            1 => {
                let l = self.list(cx, 2);
                let q = self.fresh("q");
                self.line(ind, &format!("{name} = match {l}.first"));
                cx.vars.push(Var { name: q.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
                let e1 = self.int(cx, 2);
                cx.vars.pop();
                let e2 = self.int(cx, 1);
                self.line(ind + 1, &format!("| some({q}) => {e1}"));
                self.line(ind + 1, &format!("| none => {e2}"));
            }
            2 => {
                let mut inner = Cx { vars: cx.vars.clone(), ask_ok: cx.ask_ok, lambda: true, ..Default::default() };
                let arg = self.int(&mut inner, 2);
                let e = format!("{p}_risky({arg})");
                let more = self.int(&mut inner, 1);
                inner.eff.remove(&format!("fail[{c}E]"));
                cx.eff.extend(inner.eff);
                let q = self.fresh("q");
                self.line(ind, &format!("{name} = catch {e} + {more}"));
                cx.vars.push(Var { name: q.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
                let e1 = self.int(cx, 1);
                cx.vars.pop();
                let e2 = self.int(cx, 1);
                self.line(ind + 1, &format!("| {c}Bad{{c: {q}}} => {e1}"));
                self.line(ind + 1, &format!("| {c}Worse => {e2}"));
            }
            3 => {
                let mut inner = Cx { vars: cx.vars.clone(), ask_ok: true, lambda: true, ..Default::default() };
                let arg = self.int(&mut inner, 2);
                let more = self.int(&mut inner, 1);
                if let Some(sg) = self.sigs.iter().find(|s| s.name == format!("{p}_eff")) {
                    inner.eff.extend(sg.effects.iter().cloned());
                }
                inner.eff.remove(&format!("{p}ask"));
                cx.eff.extend(inner.eff);
                let q = self.fresh("q");
                self.line(ind, &format!("{name} = handle {p}_eff({arg}) + {more}"));
                cx.vars.push(Var { name: q.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
                let e1 = self.int(cx, 1);
                cx.vars.pop();
                if self.r.chance(80) {
                    self.line(ind + 1, &format!("| {p}ask({q}) => resume({e1})"));
                } else {
                    self.line(ind + 1, &format!("| {p}ask({q}) => {e1}"));
                }
            }
            _ => {
                let t = self.expr(cx, T::Tree, 2);
                let (l, v, r) = (self.fresh("q"), self.fresh("q"), self.fresh("q"));
                self.line(ind, &format!("{name} = match {t}"));
                let e2 = self.int(cx, 1);
                cx.vars.push(Var { name: v.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
                cx.vars.push(Var { name: l.clone(), ty: T::Tree, mutable: false, assignable: false, index_of: None, eff: vec![] });
                cx.vars.push(Var { name: r.clone(), ty: T::Tree, mutable: false, assignable: false, index_of: None, eff: vec![] });
                let e1 = self.int(cx, 2);
                cx.vars.truncate(cx.vars.len() - 3);
                self.line(ind + 1, &format!("| {c}Lf => {e2}"));
                self.line(ind + 1, &format!("| {c}Nd{{l: {l}, v: {v}, r: {r}}} => {e1}"));
            }
        }
        self.bind(cx, name, T::Int, false);
    }

    fn update(&mut self, cx: &mut Cx, v: &Var) -> String {
        let n = &v.name;
        let p = self.p.clone();
        match v.ty {
            T::Int => match self.r.below(4) {
                0 => {
                    let e = self.int(cx, 2);
                    format!("{n} + {e}")
                }
                1 => {
                    let e = self.int(cx, 1);
                    format!("{n} * {e}")
                }
                _ => self.int(cx, 2),
            },
            T::List => match self.r.below(4) {
                0 => {
                    let e = self.int(cx, 2);
                    format!("{n}.push({e})")
                }
                1 => {
                    let l = self.lambda(cx);
                    format!("{n}.map({l})")
                }
                _ => self.list(cx, 2),
            },
            T::Tree => match self.r.below(3) {
                0 => {
                    let e = self.int(cx, 2);
                    format!("{p}_inc({n}, {e})")
                }
                _ => {
                    let e = self.int(cx, 2);
                    format!("{p}_ins({n}, {e})")
                }
            },
            T::Map => {
                let (a, b) = (self.int(cx, 1), self.int(cx, 2));
                if self.r.chance(70) { format!("{n}.put({a}, {b})") } else { format!("{n}.remove({a})") }
            }
            T::Set => {
                let a = self.int(cx, 1);
                if self.r.chance(70) { format!("{n}.add({a})") } else { format!("{n}.remove({a})") }
            }
            T::Str => {
                let s = self.string(cx, 1);
                format!("{n} + {s}")
            }
            T::Rec => {
                let e = self.int(cx, 2);
                format!("{n} with {} := {e}", if self.r.chance(50) { "a" } else { "b" })
            }
            t => self.expr(cx, t, 2),
        }
    }

    fn any_ty(&mut self) -> T {
        [T::Int, T::Int, T::Int, T::Bool, T::Str, T::List, T::List, T::Rec, T::Sum, T::Tree, T::Map, T::Set, T::HMap][self.r.below(13) as usize]
    }

    fn expr(&mut self, cx: &mut Cx, t: T, d: u32) -> String {
        match t {
            T::Int => self.int(cx, d),
            T::Bool => self.boolean(cx, d),
            T::Str => self.string(cx, d),
            T::List => self.list(cx, d),
            T::Rec => self.rec(cx, d),
            T::Sum => self.sum(cx, d),
            T::Tree => self.tree(cx, d),
            T::Map => self.map(cx, d),
            T::Set => self.set(cx, d),
            T::HMap => self.hmap(cx, d),
            T::Clo => "(z => z)".into(),
        }
    }

    fn var_of(&mut self, cx: &Cx, t: T) -> Option<String> {
        let vs: Vec<&Var> = cx.vars.iter().filter(|v| v.ty == t).collect();
        if vs.is_empty() { None } else { Some(vs[self.r.below(vs.len() as u64) as usize].name.clone()) }
    }

    fn lit_int(&mut self) -> String {
        let k = self.r.below(100);
        if k < 70 {
            self.r.range(-12, 30).to_string()
        } else if k < 88 {
            self.r.range(-2000, 2000).to_string()
        } else {
            BOUNDARY[self.r.below(BOUNDARY.len() as u64) as usize].to_string()
        }
    }

    fn int_atom(&mut self, cx: &Cx) -> String {
        if self.r.chance(65)
            && let Some(v) = self.var_of(cx, T::Int)
        {
            return v;
        }
        let s = self.lit_int();
        if s.starts_with('-') { format!("({s})") } else { s }
    }

    fn call(&mut self, cx: &mut Cx, ret: T, d: u32) -> Option<String> {
        let cands: Vec<Sig> = self.sigs.iter().filter(|s| s.ret == ret && (cx.ask_ok || !s.effects.iter().any(|e| e.ends_with("ask"))) && !(cx.no_risky && s.name.ends_with("_risky"))).cloned().collect();
        if cands.is_empty() {
            return None;
        }
        let s = cands[self.r.below(cands.len() as u64) as usize].clone();
        let args: Vec<String> = s.params.iter().map(|t| self.expr(cx, *t, d.saturating_sub(1))).collect();
        for e in &s.effects {
            cx.eff.insert(e.clone());
        }
        Some(format!("{}({})", s.name, args.join(", ")))
    }

    fn int(&mut self, cx: &mut Cx, d: u32) -> String {
        if d == 0 || self.r.chance(20) {
            return self.int_atom(cx);
        }
        let d1 = d - 1;
        let p = self.p.clone();
        match self.r.below(40) {
            0..=7 => {
                let op = ["+", "-", "*", "+", "-", "*", "/", "%"][self.r.below(8) as usize];
                let (a, b) = (self.int(cx, d1), self.int(cx, d1));
                format!("({a} {op} {b})")
            }
            8 => {
                let a = self.int(cx, d1);
                format!("(-({a}))")
            }
            9 => {
                let (c, a, b) = (self.boolean(cx, d1), self.int(cx, d1), self.int(cx, d1));
                format!("(if {c} then {a} else {b})")
            }
            10 | 11 => self.call(cx, T::Int, d1).unwrap_or_else(|| "1".into()),
            12 => {
                let l = self.list(cx, d1);
                [format!("{l}.len"), format!("{l}.sum"), format!("{l}.fold(0, (s, x) => s + x * 2)"), format!("{l}.max.or(0)")][self.r.below(4) as usize].clone()
            }
            13 => {
                let (l, i) = (self.list(cx, d1), self.int(cx, d1));
                if self.r.chance(50) { format!("{l}[{i}]") } else { format!("{l}.get({i}).or(-1)") }
            }
            14 | 15 => {
                let idx: Vec<Var> = cx.vars.iter().filter(|v| v.index_of.is_some()).cloned().collect();
                if let Some(v) = idx.last() {
                    format!("{}[{}]", v.index_of.clone().unwrap(), v.name)
                } else {
                    self.int_atom(cx)
                }
            }
            16 => {
                let r = self.rec(cx, d1);
                format!("{r}.{}", if self.r.chance(50) { "a" } else { "b" })
            }
            17 => {
                let s = self.string(cx, d1);
                [format!("{s}.len"), format!("{s}.to_int.or(3)"), format!("{s}.byte_len")][self.r.below(3) as usize].clone()
            }
            18 => {
                let (m, k) = (self.map(cx, d1), self.int(cx, d1));
                if self.r.chance(60) { format!("{m}.get({k}).or(0)") } else { format!("{m}.len") }
            }
            19 => {
                let st = self.set(cx, d1);
                [format!("{st}.len"), format!("{st}.min.or(0)"), format!("{st}.items.sum")][self.r.below(3) as usize].clone()
            }
            20 => {
                let (h, s) = (self.hmap(cx, d1), self.string(cx, 0));
                if self.r.chance(60) { format!("{h}.get({s}).or(5)") } else { format!("{h}.len") }
            }
            21 => {
                let (a, b) = (self.int(cx, d1), self.int(cx, d1));
                let m = ["wrapping_add", "wrapping_mul", "band", "bxor", "saturating_mul", "saturating_sub", "gcd"][self.r.below(7) as usize];
                format!("{a}.{m}({b})")
            }
            22 => {
                let (a, b) = (self.int(cx, d1), self.int(cx, d1));
                let m = ["checked_add", "checked_mul", "checked_div", "checked_sub"][self.r.below(4) as usize];
                format!("{a}.{m}({b}).or(-9)")
            }
            23 => {
                let a = self.int(cx, d1);
                [format!("{a}.abs"), format!("({a} ** {})", self.r.range(0, 3)), format!("{a}.shl({})", self.r.range(0, 70)), format!("{a}.popcount")][self.r.below(4) as usize].clone()
            }
            24 => {
                let (a, b, c) = (self.int(cx, d1), self.int(cx, d1), self.int(cx, d1));
                [format!("min({a}, {b})"), format!("max({a}, {b})"), format!("clamp({a}, {b}, {c})")][self.r.below(3) as usize].clone()
            }
            25 => {
                let t = self.tree(cx, d1);
                format!("{p}_tsum({t})")
            }
            26 if cx.ask_ok => {
                let a = self.int(cx, d1);
                format!("{p}ask({a})")
            }
            27 => {
                let clos: Vec<String> = cx.vars.iter().filter(|v| v.ty == T::Clo).map(|v| v.name.clone()).collect();
                if let Some(f) = clos.last() {
                    let eff = cx.vars.iter().rev().find(|v| &v.name == f).map(|v| v.eff.clone()).unwrap_or_default();
                    cx.eff.extend(eff);
                    let a = self.int(cx, d1);
                    format!("{f}({a})")
                } else {
                    self.int_atom(cx)
                }
            }
            28 | 29 => self.pipeline(cx, d1),
            30 if !cx.no_risky => {
                let a = self.int(cx, d1);
                cx.eff.insert(format!("fail[{}E]", self.c));
                format!("{p}_risky({a})")
            }
            31 => {
                let sm = self.sum(cx, d1);
                format!("{sm}.str.len")
            }
            32 | 33 => {
                let s = self.string(cx, d1);
                let k = self.int_atom(cx);
                [
                    format!("{s}.split(\",\").len"),
                    format!("{s}.index_of(\"a\").or(-1)"),
                    format!("{s}.byte({k})"),
                    format!("{s}.words.len"),
                    format!("{s}.chars.len"),
                    format!("{s}.pad_left(3, \"*\").len"),
                ][self.r.below(6) as usize]
                    .clone()
            }
            34 | 35 => {
                let l = self.list(cx, d1);
                let k = self.int_atom(cx);
                let f = self.lambda(cx);
                [
                    format!("{l}.sort_by({f}).first.or(0)"),
                    format!("{l}.enumerate.map(p => p.0 * p.1).sum"),
                    format!("{l}.windows(2).len"),
                    format!("{l}.index_of({k}).or(-1)"),
                    format!("{l}.counts.len"),
                    format!("{l}.last.or(7)"),
                    format!("{l}.zip({l}.reverse).map(p => p.0 - p.1).sum"),
                    format!("{l}.chunks({k} % 4 + 1).len"),
                ][self.r.below(8) as usize]
                    .clone()
            }
            _ => self.int_atom(cx),
        }
    }

    fn pipeline(&mut self, cx: &mut Cx, d: u32) -> String {
        let src = if self.r.chance(40) {
            let (a, b) = (self.r.range(-4, 4), self.r.range(0, 40));
            let s = [1, 1, 2, 3, -1, -3][self.r.below(6) as usize];
            if s < 0 { format!("range({b}, {a}, {s})") } else { format!("range({a}, {b}, {s})") }
        } else if self.r.chance(30) {
            format!("(0..{})", self.r.range(0, 30))
        } else {
            self.list(cx, d)
        };
        let mut out = src;
        for _ in 0..1 + self.r.below(3) {
            if self.r.chance(55) {
                let l = self.lambda(cx);
                out = format!("{out}.map({l})");
            } else {
                let l = self.blambda(cx);
                out = format!("{out}.filter({l})");
            }
        }
        format!("{out}.{}", ["sum", "sum", "len"][self.r.below(3) as usize])
    }

    fn lambda(&mut self, cx: &mut Cx) -> String {
        let x = self.fresh("e");
        cx.vars.push(Var { name: x.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        let was = cx.lambda;
        cx.lambda = true;
        let b = if self.r.chance(50) {
            let op = ["+", "*", "-", "/", "%"][self.r.below(5) as usize];
            let k = self.int_atom(cx);
            format!("{x} {op} {k}")
        } else {
            self.int(cx, 2)
        };
        cx.lambda = was;
        cx.vars.pop();
        format!("{x} => {b}")
    }

    fn blambda(&mut self, cx: &mut Cx) -> String {
        let x = self.fresh("e");
        cx.vars.push(Var { name: x.clone(), ty: T::Int, mutable: false, assignable: false, index_of: None, eff: vec![] });
        let was = cx.lambda;
        cx.lambda = true;
        let b = if self.r.chance(60) {
            let op = ["<", ">", "==", "!=", ">="][self.r.below(5) as usize];
            let k = self.int_atom(cx);
            if self.r.chance(30) { format!("{x} % 2 {op} {k}") } else { format!("{x} {op} {k}") }
        } else {
            self.boolean(cx, 2)
        };
        cx.lambda = was;
        cx.vars.pop();
        format!("{x} => {b}")
    }

    fn boolean(&mut self, cx: &mut Cx, d: u32) -> String {
        if d == 0 {
            if let Some(v) = self.var_of(cx, T::Bool) {
                return v;
            }
            let (a, b) = (self.int_atom(cx), self.int_atom(cx));
            return format!("({a} < {b})");
        }
        let d1 = d - 1;
        match self.r.below(12) {
            0..=3 => {
                let op = ["<", "<=", ">", ">=", "==", "!="][self.r.below(6) as usize];
                let (a, b) = (self.int(cx, d1), self.int(cx, d1));
                format!("({a} {op} {b})")
            }
            4 => {
                let (a, b) = (self.boolean(cx, d1), self.boolean(cx, d1));
                format!("({a} {} {b})", if self.r.chance(50) { "and" } else { "or" })
            }
            5 => {
                let a = self.boolean(cx, d1);
                format!("(not {a})")
            }
            6 => {
                let (l, a) = (self.list(cx, d1), self.int(cx, d1));
                if self.r.chance(50) { format!("{l}.contains({a})") } else { format!("{l}.is_empty") }
            }
            7 => {
                let (st, a) = (self.set(cx, d1), self.int(cx, d1));
                format!("{st}.has({a})")
            }
            8 => {
                let (m, a) = (self.map(cx, d1), self.int(cx, d1));
                format!("{m}.has({a})")
            }
            9 => {
                let (s, t) = (self.string(cx, d1), self.string(cx, 0));
                let m = ["starts_with", "contains", "ends_with"][self.r.below(3) as usize];
                format!("{s}.{m}({t})")
            }
            10 => {
                let l = self.list(cx, d1);
                let b = self.blambda(cx);
                format!("{l}.{}({b})", if self.r.chance(50) { "any" } else { "all" })
            }
            _ => {
                let t = self.any_ty();
                if matches!(t, T::Clo) {
                    return "true".into();
                }
                let (a, b) = (self.expr(cx, t, d1), self.expr(cx, t, d1));
                format!("({a} == {b})")
            }
        }
    }

    fn string(&mut self, cx: &mut Cx, d: u32) -> String {
        if d == 0 || self.r.chance(20) {
            if self.r.chance(50)
                && let Some(v) = self.var_of(cx, T::Str)
            {
                return v;
            }
            return STRS[self.r.below(STRS.len() as u64) as usize].to_string();
        }
        let d1 = d - 1;
        match self.r.below(12) {
            0 => {
                let v = self.int_atom(cx);
                format!("\"n{{{v}}}\"")
            }
            1 => {
                let (a, b) = (self.string(cx, d1), self.string(cx, d1));
                format!("({a} + {b})")
            }
            2 => {
                let a = self.string(cx, d1);
                format!("{a}.{}", ["upper", "lower", "reverse", "trim"][self.r.below(4) as usize])
            }
            3 => {
                let (a, k) = (self.string(cx, d1), self.r.range(-1, 4));
                format!("{a}.{}({k})", if self.r.chance(50) { "take" } else { "drop" })
            }
            4 => {
                let l = self.list(cx, d1);
                format!("{l}.map(x => x.str).join(\",\")")
            }
            5 => {
                let a = self.int(cx, d1);
                format!("{a}.str")
            }
            6 => {
                let a = self.string(cx, d1);
                format!("{a}.replace(\"a\", \"xy\")")
            }
            7 => {
                let t = self.any_ty();
                if t == T::Clo {
                    return "\"c\"".into();
                }
                let e = self.expr(cx, t, d1);
                format!("{e}.str")
            }
            8 => {
                let (a, k) = (self.string(cx, d1), self.r.range(0, 3));
                format!("{a}.repeat({k})")
            }
            9 => {
                let (a, k) = (self.string(cx, d1), self.int_atom(cx));
                [
                    format!("{a}.split(\",\").join(\"-\")"),
                    format!("{a}.chars.reverse.join(\"\")"),
                    format!("{a}.get({k}).or(\"?\")"),
                    format!("{a}.pad_right(4, \"ab\")"),
                    format!("{a}.words.join(\"_\")"),
                ][self.r.below(5) as usize]
                    .clone()
            }
            _ => self.call(cx, T::Str, d1).unwrap_or_else(|| "\"s\"".into()),
        }
    }

    fn list(&mut self, cx: &mut Cx, d: u32) -> String {
        if d == 0 || self.r.chance(20) {
            if self.r.chance(60)
                && let Some(v) = self.var_of(cx, T::List)
            {
                return v;
            }
            let n = self.r.range(1, 5);
            let xs: Vec<String> = (0..n).map(|_| self.int_atom(cx)).collect();
            return format!("[{}]", xs.join(", "));
        }
        let d1 = d - 1;
        let p = self.p.clone();
        match self.r.below(18) {
            14 => {
                let l = self.list(cx, d1);
                format!("{l}.flat_map(x => [x, x + 1])")
            }
            15 => {
                let (l, f) = (self.list(cx, d1), self.blambda(cx));
                format!("{l}.{}({f})", ["take_while", "drop_while", "filter"][self.r.below(3) as usize])
            }
            16 => {
                let (l, k) = (self.list(cx, d1), self.int_atom(cx));
                format!("{l}.{}", [format!("rotate({k})"), format!("slice(1, {k})"), "unique.sort".to_string()][self.r.below(3) as usize])
            }
            17 => {
                let l = self.list(cx, d1);
                let f = self.lambda(cx);
                format!("{l}.sort_by({f})")
            }
            0 | 1 => {
                let (l, f) = (self.list(cx, d1), self.lambda(cx));
                format!("{l}.map({f})")
            }
            2 => {
                let (l, f) = (self.list(cx, d1), self.blambda(cx));
                format!("{l}.filter({f})")
            }
            3 => {
                let (a, b) = (self.r.range(-5, 5), self.r.range(-5, 15));
                let s = [1, 2, 3, -1, -2, 0][self.r.below(6) as usize];
                format!("range({a}, {b}, {s})")
            }
            4 => format!("(0..{})", self.r.range(0, 8)),
            5 => {
                let (l, a) = (self.list(cx, d1), self.int(cx, d1));
                format!("{l}.push({a})")
            }
            6 => {
                let (a, b) = (self.list(cx, d1), self.list(cx, d1));
                format!("{a}.concat({b})")
            }
            7 => {
                let l = self.list(cx, d1);
                format!("{l}.{}", ["sort", "reverse", "unique"][self.r.below(3) as usize])
            }
            8 => {
                let (l, k) = (self.list(cx, d1), self.r.range(-1, 4));
                format!("{l}.{}({k})", if self.r.chance(50) { "take" } else { "drop" })
            }
            9 => {
                let t = self.tree(cx, d1);
                format!("{p}_items({t})")
            }
            10 => {
                let m = self.map(cx, d1);
                format!("{m}.{}", if self.r.chance(50) { "keys" } else { "values" })
            }
            11 => {
                let l = self.list(cx, d1);
                format!("{l}.scan(0, (s, x) => s + x)")
            }
            12 => {
                let st = self.set(cx, d1);
                format!("{st}.items")
            }
            _ => self.call(cx, T::List, d1).unwrap_or_else(|| "[1]".into()),
        }
    }

    fn rec(&mut self, cx: &mut Cx, d: u32) -> String {
        if (d == 0 || self.r.chance(30))
            && let Some(v) = self.var_of(cx, T::Rec)
        {
            return v;
        }
        let d1 = d.saturating_sub(1);
        if d > 0 && self.r.chance(30) {
            if let Some(c) = self.call(cx, T::Rec, d1) {
                return c;
            }
            let r = self.rec(cx, d1);
            let e = self.int(cx, d1);
            return format!("({r} with {} := {e})", if self.r.chance(50) { "a" } else { "b" });
        }
        let (a, b) = (self.int(cx, d1), self.int(cx, d1));
        format!("{}R{{a: {a}, b: {b}}}", self.c)
    }

    fn sum(&mut self, cx: &mut Cx, d: u32) -> String {
        if (d == 0 || self.r.chance(30))
            && let Some(v) = self.var_of(cx, T::Sum)
        {
            return v;
        }
        let d1 = d.saturating_sub(1);
        let c = self.c.clone();
        match self.r.below(3) {
            0 => {
                let a = self.int(cx, d1);
                format!("{c}A{{x: {a}}}")
            }
            1 => {
                let (a, b) = (self.int(cx, d1), self.int(cx, d1));
                format!("{c}B{{x: {a}, y: {b}}}")
            }
            _ => format!("{c}C"),
        }
    }

    fn tree(&mut self, cx: &mut Cx, d: u32) -> String {
        if (d == 0 || self.r.chance(30))
            && let Some(v) = self.var_of(cx, T::Tree)
        {
            return v;
        }
        let d1 = d.saturating_sub(1);
        let p = self.p.clone();
        if d == 0 {
            return format!("{}Lf", self.c);
        }
        match self.r.below(4) {
            0 => {
                let l = self.list(cx, d1);
                format!("{p}_build({l})")
            }
            1 => {
                let (t, a) = (self.tree(cx, d1), self.int(cx, d1));
                format!("{p}_ins({t}, {a})")
            }
            2 => self.call(cx, T::Tree, d1).unwrap_or_else(|| format!("{}Lf", self.c)),
            _ => {
                let (t, a) = (self.tree(cx, d1), self.int(cx, d1));
                format!("{p}_inc({t}, {a})")
            }
        }
    }

    fn map(&mut self, cx: &mut Cx, d: u32) -> String {
        if (d == 0 || self.r.chance(30))
            && let Some(v) = self.var_of(cx, T::Map)
        {
            return v;
        }
        let d1 = d.saturating_sub(1);
        let p = self.p.clone();
        match self.r.below(3) {
            0 => {
                let (m, a, b) = (self.map(cx, d1), self.int(cx, d1), self.int(cx, d1));
                format!("{m}.put({a}, {b})")
            }
            1 if d > 0 => {
                let (m, a) = (self.map(cx, d1), self.int(cx, d1));
                format!("{m}.remove({a})")
            }
            _ => {
                let l = self.list(cx, d1);
                format!("{p}_mkmap({l})")
            }
        }
    }

    fn set(&mut self, cx: &mut Cx, d: u32) -> String {
        if (d == 0 || self.r.chance(30))
            && let Some(v) = self.var_of(cx, T::Set)
        {
            return v;
        }
        let d1 = d.saturating_sub(1);
        match self.r.below(4) {
            0 if d > 0 => {
                let (s, a) = (self.set(cx, d1), self.int(cx, d1));
                format!("{s}.add({a})")
            }
            1 if d > 0 => {
                let (s, t) = (self.set(cx, d1), self.set(cx, d1));
                format!("{s}.{}({t})", ["union", "inter", "diff"][self.r.below(3) as usize])
            }
            _ => {
                let l = self.list(cx, d1);
                format!("{l}.to_set")
            }
        }
    }

    fn hmap(&mut self, cx: &mut Cx, d: u32) -> String {
        if (d == 0 || self.r.chance(30))
            && let Some(v) = self.var_of(cx, T::HMap)
        {
            return v;
        }
        let d1 = d.saturating_sub(1);
        let p = self.p.clone();
        if d > 0 && self.r.chance(40) {
            let (h, s, a) = (self.hmap(cx, d1), self.string(cx, 0), self.int(cx, d1));
            return format!("{h}.put({s}, {a})");
        }
        let l = self.list(cx, d1);
        format!("{p}_mkhm({l})")
    }
}
