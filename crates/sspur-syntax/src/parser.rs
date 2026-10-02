use crate::ast::*;
use crate::lexer::{lex, Tok, Token};
use crate::SyntaxError;

type PResult<T> = Result<T, SyntaxError>;

pub fn parse(src: &str) -> PResult<Module> {
    let toks = lex(src)?;
    let mut m = Parser::new(toks).module()?;
    normalize(&mut m);
    Ok(m)
}

const BUILTIN_TYPE_NAMES: &[&str] = &["Int", "I8", "I16", "I32", "U8", "U16", "U32", "U64", "F32", "F64", "Bool", "Str", "Unit", "List", "Opt", "Res", "Map"];

fn normalize(m: &mut Module) {
    let names: Vec<String> = m.defs.iter().filter_map(|d| if let Def::Type(t) = d { Some(t.name.clone()) } else { None }).collect();
    for d in &mut m.defs {
        let Def::Type(t) = d else { continue };
        if let TypeBody::Alias(Ty::Named { name, args, .. }, None) = &t.body {
            let known = names.contains(name) || BUILTIN_TYPE_NAMES.contains(&name.as_str()) || t.params.iter().any(|p| &p.name == name);
            if args.is_empty() && !known {
                t.body = TypeBody::Sum(vec![Variant { name: name.clone(), fields: None }]);
            }
        }
    }
}

pub fn parse_expr(src: &str) -> PResult<Expr> {
    let toks = lex(src)?;
    let mut p = Parser::new(toks);
    let e = p.expr_seq()?;
    p.skip_newlines();
    p.expect_eof()?;
    Ok(e)
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    line_indent: u32,
    in_refine: bool,
}

fn is_upper(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

impl Parser {
    fn new(toks: Vec<Token>) -> Self {
        Parser { toks, pos: 0, line_indent: 0, in_refine: false }
    }

    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn peek_at(&self, n: usize) -> &Tok {
        let i = (self.pos + n).min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    fn span(&self) -> Span {
        self.toks[self.pos].span
    }

    fn prev_span(&self) -> Span {
        self.toks[self.pos.saturating_sub(1)].span
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Sym(x) if *x == s)
    }

    fn is_kw(&self, k: &str) -> bool {
        matches!(self.peek(), Tok::Kw(x) if *x == k)
    }

    fn eat_sym(&mut self, s: &str) -> bool {
        if self.is_sym(s) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn err<T>(&self, code: &'static str, msg: impl Into<String>) -> PResult<T> {
        Err(SyntaxError::new(code, msg.into(), self.span()))
    }

    fn describe(&self) -> String {
        match self.peek() {
            Tok::Ident(s) => format!("'{s}'"),
            Tok::Int(n) => format!("'{n}'"),
            Tok::Float(n) => format!("'{n}'"),
            Tok::Str(_) => "string".into(),
            Tok::Hash(h) => format!("'#{h}'"),
            Tok::Kw(k) => format!("'{k}'"),
            Tok::Sym(s) => format!("'{s}'"),
            Tok::Newline(_) => "end of line".into(),
            Tok::Eof => "end of input".into(),
        }
    }

    fn expect_sym(&mut self, s: &str) -> PResult<Span> {
        if self.is_sym(s) {
            Ok(self.bump().span)
        } else {
            self.err("E_PARSE_EXPECTED", format!("expected '{s}', found {}", self.describe()))
        }
    }

    fn expect_kw(&mut self, k: &str) -> PResult<Span> {
        if self.is_kw(k) {
            Ok(self.bump().span)
        } else {
            self.err("E_PARSE_EXPECTED", format!("expected '{k}', found {}", self.describe()))
        }
    }

    fn expect_ident(&mut self) -> PResult<String> {
        match self.peek().clone() {
            Tok::Ident(s) => {
                self.bump();
                Ok(s)
            }
            _ => self.err("E_PARSE_EXPECTED", format!("expected identifier, found {}", self.describe())),
        }
    }

    fn expect_eof(&mut self) -> PResult<()> {
        if matches!(self.peek(), Tok::Eof) {
            Ok(())
        } else {
            self.err("E_PARSE_TRAILING", format!("unexpected {}", self.describe()))
        }
    }

    fn skip_newlines(&mut self) {
        while let Tok::Newline(c) = *self.peek() {
            self.line_indent = c;
            self.bump();
        }
    }

    fn newline_then(&self, pred: impl Fn(&Tok) -> bool) -> Option<u32> {
        match self.peek() {
            Tok::Newline(c) if pred(self.peek_at(1)) => Some(*c),
            _ => None,
        }
    }

    fn module(&mut self) -> PResult<Module> {
        self.skip_newlines();
        let mut profile = None;
        if self.eat_kw("profile") {
            profile = Some(self.expect_ident()?);
        }
        let mut defs = Vec::new();
        loop {
            self.skip_newlines();
            if matches!(self.peek(), Tok::Eof) {
                break;
            }
            defs.push(self.def()?);
            match self.peek() {
                Tok::Newline(_) | Tok::Eof => {}
                _ => return self.err("E_PARSE_TRAILING", format!("unexpected {} after definition", self.describe())),
            }
        }
        Ok(Module { profile, defs })
    }

    fn def(&mut self) -> PResult<Def> {
        let start = self.span();
        match self.peek() {
            Tok::Kw("type") => self.type_def().map(Def::Type),
            Tok::Kw("fn") => self.fn_def().map(Def::Fn),
            Tok::Kw("test") => {
                self.bump();
                let name = self.expect_ident()?;
                self.expect_sym("=")?;
                let body = self.expr_seq()?;
                Ok(Def::Test(TestDef { name, span: start.to(self.prev_span()), body }))
            }
            Tok::Kw(k @ ("trait" | "impl" | "store" | "svc" | "queue" | "effect")) => {
                let k = *k;
                self.err("E_UNSUPPORTED", format!("'{k}' definitions are not supported by this compiler version yet"))
            }
            _ => self.err("E_PARSE_DEF", format!("expected a definition, found {}", self.describe())),
        }
    }

    fn tparams(&mut self) -> PResult<Vec<TParam>> {
        let mut out = Vec::new();
        if !self.eat_sym("[") {
            return Ok(out);
        }
        loop {
            let name = self.expect_ident()?;
            let mut kind = None;
            let mut refine = None;
            if self.eat_sym(":") {
                kind = Some(self.ty()?);
                if self.eat_kw("where") {
                    refine = Some(self.refine_expr()?);
                }
            }
            out.push(TParam { name, kind, refine });
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym("]")?;
        Ok(out)
    }

    fn refine_expr(&mut self) -> PResult<Expr> {
        let saved = self.in_refine;
        self.in_refine = true;
        let e = self.binary(1);
        self.in_refine = saved;
        e
    }

    fn type_def(&mut self) -> PResult<TypeDef> {
        let start = self.expect_kw("type")?;
        let name = self.expect_ident()?;
        let params = self.tparams()?;
        self.expect_sym("=")?;
        let body = self.type_body()?;
        let mut derives = Vec::new();
        if self.eat_kw("derive") {
            loop {
                derives.push(self.expect_ident()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
        }
        Ok(TypeDef { name, params, body, derives, span: start.to(self.prev_span()) })
    }

    fn type_body(&mut self) -> PResult<TypeBody> {
        if self.is_sym("{") {
            return Ok(TypeBody::Record(self.fields()?));
        }
        if self.eat_kw("new") {
            return Ok(TypeBody::New(self.ty()?));
        }
        let first_is_variant = matches!(self.peek(), Tok::Ident(s) if is_upper(s)) && matches!(self.peek_at(1), Tok::Sym("{"));
        let first = if first_is_variant {
            None
        } else {
            Some(self.ty()?)
        };
        let has_bar = self.is_sym("|") || self.newline_then(|t| matches!(t, Tok::Sym("|"))).is_some();
        if let (Some(ty), false) = (&first, has_bar) {
            let refine = if self.eat_kw("where") { Some(self.refine_expr()?) } else { None };
            return Ok(TypeBody::Alias(ty.clone(), refine));
        }
        let mut variants = Vec::new();
        match first {
            Some(Ty::Named { name, args, .. }) if args.is_empty() && is_upper(&name) => {
                variants.push(Variant { name, fields: None })
            }
            Some(_) => return self.err("E_PARSE_VARIANT", "sum variants must be bare constructor names"),
            None => variants.push(self.variant()?),
        }
        loop {
            if self.newline_then(|t| matches!(t, Tok::Sym("|"))).is_some() {
                self.bump();
            }
            if !self.eat_sym("|") {
                break;
            }
            variants.push(self.variant()?);
        }
        Ok(TypeBody::Sum(variants))
    }

    fn variant(&mut self) -> PResult<Variant> {
        let name = self.expect_ident()?;
        if !is_upper(&name) {
            return self.err("E_PARSE_VARIANT", "variant names must start with an uppercase letter");
        }
        let fields = if self.is_sym("{") { Some(self.fields()?) } else { None };
        Ok(Variant { name, fields })
    }

    fn fields(&mut self) -> PResult<Vec<Field>> {
        self.expect_sym("{")?;
        let mut out = Vec::new();
        while !self.is_sym("}") {
            let name = self.expect_ident()?;
            self.expect_sym(":")?;
            let ty = self.ty()?;
            let refine = if self.eat_kw("where") { Some(self.refine_expr()?) } else { None };
            out.push(Field { name, ty, refine });
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym("}")?;
        Ok(out)
    }

    fn ty(&mut self) -> PResult<Ty> {
        let base = if self.eat_sym("(") {
            let mut items = Vec::new();
            while !self.is_sym(")") {
                items.push(self.ty()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(")")?;
            if self.is_sym("->") {
                return self.fn_ty(items);
            }
            match items.len() {
                0 => Ty::Named { name: "Unit".into(), args: vec![], span: self.prev_span() },
                1 => items.pop().unwrap(),
                _ => Ty::Tuple(items),
            }
        } else {
            let span = self.span();
            let name = self.expect_ident()?;
            let mut args = Vec::new();
            if self.eat_sym("[") {
                loop {
                    args.push(self.ty()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym("]")?;
            }
            Ty::Named { name, args, span: span.to(self.prev_span()) }
        };
        if self.is_sym("->") {
            return self.fn_ty(vec![base]);
        }
        Ok(base)
    }

    fn fn_ty(&mut self, params: Vec<Ty>) -> PResult<Ty> {
        self.expect_sym("->")?;
        let ret = self.ty()?;
        let effects = if self.eat_sym("!") { self.effects()? } else { vec![] };
        Ok(Ty::Fn { params, ret: Box::new(ret), effects })
    }

    fn effects(&mut self) -> PResult<Vec<Effect>> {
        let mut out = Vec::new();
        loop {
            let start = self.span();
            let mut name = self.expect_ident()?;
            while self.is_sym(".") && matches!(self.peek_at(1), Tok::Ident(_)) {
                self.bump();
                name.push('.');
                name.push_str(&self.expect_ident()?);
            }
            let mut args = Vec::new();
            if self.eat_sym("[") {
                loop {
                    args.push(self.ty()?);
                    if !self.eat_sym("|") && !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym("]")?;
            }
            out.push(Effect { name, args, span: start.to(self.prev_span()) });
            let continues = self.is_sym(",") && matches!(self.peek_at(1), Tok::Ident(_)) && !matches!(self.peek_at(2), Tok::Sym(":"));
            if !continues {
                break;
            }
            self.bump();
        }
        Ok(out)
    }

    fn fn_def(&mut self) -> PResult<FnDef> {
        let start = self.expect_kw("fn")?;
        let name = self.expect_ident()?;
        let tparams = self.tparams()?;
        self.expect_sym("(")?;
        let mut params = Vec::new();
        while !self.is_sym(")") {
            let pname = self.expect_ident()?;
            self.expect_sym(":")?;
            let ty = self.ty()?;
            let refine = if self.eat_kw("where") { Some(self.refine_expr()?) } else { None };
            params.push(Param { name: pname, ty, refine });
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        let ret = if self.eat_sym("->") { Some(self.ty()?) } else { None };
        let effects = if self.eat_sym("!") { self.effects()? } else { vec![] };
        let sig_span = start.to(self.prev_span());
        let mut pres = Vec::new();
        let mut posts = Vec::new();
        loop {
            if self.newline_then(|t| matches!(t, Tok::Kw("pre" | "post"))).is_some() {
                self.bump();
            }
            if self.eat_kw("pre") {
                pres.push(self.refine_expr()?);
            } else if self.eat_kw("post") {
                posts.push(self.refine_expr()?);
            } else {
                break;
            }
        }
        if self.newline_then(|t| matches!(t, Tok::Sym("="))).is_some() {
            let Tok::Newline(c) = *self.peek() else { unreachable!() };
            self.line_indent = c;
            self.bump();
        }
        self.expect_sym("=")?;
        let body = self.expr_seq()?;
        Ok(FnDef { name, tparams, params, ret, effects, pres, posts, body, span: start.to(self.prev_span()), sig_span })
    }

    fn expr_seq(&mut self) -> PResult<Expr> {
        let first = self.expr()?;
        if !self.is_sym(";") {
            return Ok(first);
        }
        let start = first.span;
        let mut stmts = vec![Stmt::Expr(first)];
        while self.eat_sym(";") {
            stmts.push(self.stmt()?);
        }
        Ok(Expr::new(ExprKind::Block(stmts), start.to(self.prev_span())))
    }

    fn lambda_ahead(&self) -> bool {
        match self.peek() {
            Tok::Ident(_) => matches!(self.peek_at(1), Tok::Sym("=>")),
            Tok::Sym("(") => {
                let mut i = 1;
                loop {
                    match self.peek_at(i) {
                        Tok::Ident(_) => {}
                        _ => return false,
                    }
                    match self.peek_at(i + 1) {
                        Tok::Sym(",") => i += 2,
                        Tok::Sym(")") => return matches!(self.peek_at(i + 2), Tok::Sym("=>")),
                        _ => return false,
                    }
                }
            }
            _ => false,
        }
    }

    fn expr(&mut self) -> PResult<Expr> {
        let start = self.span();
        if self.lambda_ahead() {
            let mut params = Vec::new();
            if self.eat_sym("(") {
                loop {
                    params.push(self.expect_ident()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym(")")?;
            } else {
                params.push(self.expect_ident()?);
            }
            self.expect_sym("=>")?;
            let body = self.expr()?;
            return Ok(Expr::new(ExprKind::Lambda { params, body: Box::new(body), implicit: false }, start.to(self.prev_span())));
        }
        match self.peek() {
            Tok::Kw("if") => {
                self.bump();
                let c = self.expr()?;
                self.expect_kw("then")?;
                let t = self.branch()?;
                let e = if self.eat_kw("else") { Some(Box::new(self.branch()?)) } else { None };
                Ok(Expr::new(ExprKind::If(Box::new(c), Box::new(t), e), start.to(self.prev_span())))
            }
            Tok::Kw(k @ ("match" | "catch")) => {
                let is_match = *k == "match";
                let indent = self.line_indent;
                self.bump();
                let scrut = self.expr()?;
                let arms = self.arms(indent)?;
                if arms.is_empty() {
                    return self.err("E_PARSE_ARMS", "expected at least one '|' arm");
                }
                let kind = if is_match {
                    ExprKind::Match(Box::new(scrut), arms)
                } else {
                    ExprKind::Catch(Box::new(scrut), arms)
                };
                Ok(Expr::new(kind, start.to(self.prev_span())))
            }
            Tok::Kw("do") => {
                self.bump();
                self.block()
            }
            Tok::Kw("raise") => {
                self.bump();
                let e = self.expr()?;
                Ok(Expr::new(ExprKind::Raise(Box::new(e)), start.to(self.prev_span())))
            }
            Tok::Kw("return") => {
                self.bump();
                let e = self.expr()?;
                Ok(Expr::new(ExprKind::Return(Box::new(e)), start.to(self.prev_span())))
            }
            _ => self.binary(1),
        }
    }

    fn assign_ahead(&self) -> bool {
        matches!((self.peek(), self.peek_at(1)), (Tok::Ident(_), Tok::Sym(":=")))
    }

    fn branch(&mut self) -> PResult<Expr> {
        if !self.assign_ahead() {
            return self.expr();
        }
        let span = self.span();
        let Tok::Ident(name) = self.bump().tok else { unreachable!() };
        self.bump();
        let value = self.expr()?;
        let full = span.to(self.prev_span());
        Ok(Expr::new(ExprKind::Block(vec![Stmt::Assign(name, value, span)]), full))
    }

    fn arms(&mut self, indent: u32) -> PResult<Vec<Arm>> {
        let mut arms = Vec::new();
        loop {
            if let Some(c) = self.newline_then(|t| matches!(t, Tok::Sym("|"))) {
                if c < indent {
                    break;
                }
                self.line_indent = c;
                self.bump();
            }
            if !self.eat_sym("|") {
                break;
            }
            let pat = self.pat()?;
            let guard = if self.eat_kw("if") { Some(self.expr()?) } else { None };
            self.expect_sym("=>")?;
            let saved = self.line_indent;
            let body = if self.assign_ahead() { self.branch()? } else { self.expr_seq()? };
            self.line_indent = saved;
            arms.push(Arm { pat, guard, body });
        }
        Ok(arms)
    }

    fn block(&mut self) -> PResult<Expr> {
        let start = self.prev_span();
        let outer = self.line_indent;
        let col = match *self.peek() {
            Tok::Newline(c) if c > outer => c,
            _ => return self.err("E_PARSE_BLOCK", "expected an indented block on the next line"),
        };
        let mut stmts = Vec::new();
        while let Tok::Newline(c) = *self.peek() {
            if c != col {
                if c > col {
                    return self.err("E_PARSE_INDENT", "unexpected indentation");
                }
                break;
            }
            self.line_indent = c;
            self.bump();
            stmts.push(self.stmt()?);
        }
        self.line_indent = outer;
        Ok(Expr::new(ExprKind::Block(stmts), start.to(self.prev_span())))
    }

    fn stmt(&mut self) -> PResult<Stmt> {
        if self.eat_kw("var") {
            let name = self.expect_ident()?;
            self.expect_sym("=")?;
            return Ok(Stmt::Var(name, self.expr()?));
        }
        if self.eat_kw("for") {
            let pat = self.pat()?;
            self.expect_kw("in")?;
            let iter = self.expr()?;
            let body = if matches!(self.peek(), Tok::Newline(c) if *c > self.line_indent) {
                self.block()?
            } else {
                self.expr()?
            };
            return Ok(Stmt::For(pat, iter, body));
        }
        if let (Tok::Ident(name), Tok::Sym(":=")) = (self.peek().clone(), self.peek_at(1)) {
            let span = self.span();
            self.bump();
            self.bump();
            return Ok(Stmt::Assign(name, self.expr()?, span));
        }
        let saved = self.pos;
        if let Ok(p) = self.pat()
            && self.eat_sym("=") {
                return Ok(Stmt::Let(p, self.expr()?));
            }
        self.pos = saved;
        Ok(Stmt::Expr(self.expr()?))
    }

    fn pat(&mut self) -> PResult<Pat> {
        match self.peek().clone() {
            Tok::Sym("_") => {
                self.bump();
                Ok(Pat::Wild)
            }
            Tok::Int(n) => {
                self.bump();
                Ok(Pat::Int(n))
            }
            Tok::Sym("-") if matches!(self.peek_at(1), Tok::Int(_)) => {
                self.bump();
                let Tok::Int(n) = self.bump().tok else { unreachable!() };
                Ok(Pat::Int(-n))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Pat::Str(s))
            }
            Tok::Kw("true") => {
                self.bump();
                Ok(Pat::Bool(true))
            }
            Tok::Kw("false") => {
                self.bump();
                Ok(Pat::Bool(false))
            }
            Tok::Sym("(") => {
                self.bump();
                let mut items = Vec::new();
                while !self.is_sym(")") {
                    items.push(self.pat()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym(")")?;
                Ok(if items.len() == 1 { items.pop().unwrap() } else { Pat::Tuple(items) })
            }
            Tok::Ident(name) => {
                self.bump();
                if self.eat_sym("(") {
                    let mut items = Vec::new();
                    while !self.is_sym(")") {
                        items.push(self.pat()?);
                        if !self.eat_sym(",") {
                            break;
                        }
                    }
                    self.expect_sym(")")?;
                    return Ok(Pat::Ctor { name, args: CtorArgs::Positional(items) });
                }
                if is_upper(&name) && self.eat_sym("{") {
                    let mut fields = Vec::new();
                    while !self.is_sym("}") {
                        let f = self.expect_ident()?;
                        let p = if self.eat_sym(":") { self.pat()? } else { Pat::Bind(f.clone()) };
                        fields.push((f, p));
                        if !self.eat_sym(",") {
                            break;
                        }
                    }
                    self.expect_sym("}")?;
                    return Ok(Pat::Ctor { name, args: CtorArgs::Record(fields) });
                }
                if is_upper(&name) || name == "none" {
                    Ok(Pat::Ctor { name, args: CtorArgs::None })
                } else {
                    Ok(Pat::Bind(name))
                }
            }
            _ => self.err("E_PARSE_PATTERN", format!("expected a pattern, found {}", self.describe())),
        }
    }

    fn binop(&self) -> Option<BinOp> {
        Some(match self.peek() {
            Tok::Sym("+") => BinOp::Add,
            Tok::Sym("-") => BinOp::Sub,
            Tok::Sym("*") => BinOp::Mul,
            Tok::Sym("/") => BinOp::Div,
            Tok::Sym("%") => BinOp::Rem,
            Tok::Sym("**") => BinOp::Pow,
            Tok::Sym("==") => BinOp::Eq,
            Tok::Sym("!=") => BinOp::Ne,
            Tok::Sym("<") => BinOp::Lt,
            Tok::Sym("<=") => BinOp::Le,
            Tok::Sym(">") => BinOp::Gt,
            Tok::Sym(">=") => BinOp::Ge,
            Tok::Kw("and") => BinOp::And,
            Tok::Kw("or") => BinOp::Or,
            _ => return None,
        })
    }

    fn binary(&mut self, min: u8) -> PResult<Expr> {
        let mut lhs = self.unary()?;
        loop {
            if self.is_sym("..") && min <= 5 {
                self.bump();
                let rhs = self.binary(6)?;
                let span = lhs.span.to(rhs.span);
                lhs = Expr::new(ExprKind::Range(Box::new(lhs), Box::new(rhs)), span);
                continue;
            }
            let Some(op) = self.binop() else { break };
            let prec = op.prec();
            if prec < min {
                break;
            }
            self.bump();
            let next_min = if op == BinOp::Pow { prec } else { prec + 1 };
            let rhs = self.binary(next_min)?;
            let span = lhs.span.to(rhs.span);
            lhs = Expr::new(ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)), span);
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> PResult<Expr> {
        let start = self.span();
        if self.eat_kw("not") {
            let e = self.binary(4)?;
            return Ok(Expr::new(ExprKind::Unary(UnOp::Not, Box::new(e)), start.to(self.prev_span())));
        }
        if self.eat_sym("-") {
            let e = self.unary()?;
            if let ExprKind::Int(n) = e.kind {
                return Ok(Expr::new(ExprKind::Int(-n), start.to(e.span)));
            }
            if let ExprKind::Float(n) = e.kind {
                return Ok(Expr::new(ExprKind::Float(-n), start.to(e.span)));
            }
            return Ok(Expr::new(ExprKind::Unary(UnOp::Neg, Box::new(e)), start.to(self.prev_span())));
        }
        let e = self.primary()?;
        self.postfix(e)
    }

    fn args(&mut self) -> PResult<Vec<Expr>> {
        self.expect_sym("(")?;
        let mut out = Vec::new();
        while !self.is_sym(")") {
            let a = self.expr()?;
            out.push(self.wrap_placeholder(a));
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        Ok(out)
    }

    fn wrap_placeholder(&self, e: Expr) -> Expr {
        if self.in_refine || !has_placeholder(&e) {
            return e;
        }
        let span = e.span;
        Expr::new(ExprKind::Lambda { params: vec!["_".into()], body: Box::new(e), implicit: true }, span)
    }

    fn postfix(&mut self, mut e: Expr) -> PResult<Expr> {
        loop {
            if self.eat_sym(".") {
                let name = match self.peek().clone() {
                    Tok::Ident(s) => s,
                    Tok::Int(n) => n.to_string(),
                    Tok::Kw(k) => k.to_string(),
                    _ => return self.err("E_PARSE_EXPECTED", format!("expected a field or method name, found {}", self.describe())),
                };
                self.bump();
                let mut targs = Vec::new();
                if self.is_sym("[") && matches!(self.peek_at(1), Tok::Ident(s) if is_upper(s)) {
                    self.bump();
                    loop {
                        targs.push(self.ty()?);
                        if !self.eat_sym(",") {
                            break;
                        }
                    }
                    self.expect_sym("]")?;
                }
                if self.is_sym("(") || !targs.is_empty() {
                    let args = if self.is_sym("(") { self.args()? } else { vec![] };
                    let span = e.span.to(self.prev_span());
                    e = Expr::new(ExprKind::Method { recv: Box::new(e), name, targs, args }, span);
                } else {
                    let span = e.span.to(self.prev_span());
                    e = Expr::new(ExprKind::Field(Box::new(e), name), span);
                }
            } else if self.is_sym("(") {
                let args = self.args()?;
                let span = e.span.to(self.prev_span());
                e = Expr::new(ExprKind::Call(Box::new(e), args), span);
            } else if self.eat_sym("[") {
                let idx = self.expr()?;
                self.expect_sym("]")?;
                let span = e.span.to(self.prev_span());
                e = Expr::new(ExprKind::Index(Box::new(e), Box::new(idx)), span);
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> PResult<Expr> {
        let start = self.span();
        let tok = self.peek().clone();
        let kind = match tok {
            Tok::Int(n) => {
                self.bump();
                ExprKind::Int(n)
            }
            Tok::Float(n) => {
                self.bump();
                ExprKind::Float(n)
            }
            Tok::Str(s) => {
                self.bump();
                ExprKind::Str(self.interp(&s, start)?)
            }
            Tok::Kw("true") => {
                self.bump();
                ExprKind::Bool(true)
            }
            Tok::Kw("false") => {
                self.bump();
                ExprKind::Bool(false)
            }
            Tok::Sym("_") => {
                self.bump();
                ExprKind::Placeholder
            }
            Tok::Sym("?") => {
                self.bump();
                let adjacent = self.span().start == start.end;
                match self.peek().clone() {
                    Tok::Ident(n) if adjacent => {
                        self.bump();
                        ExprKind::Hole(Some(n))
                    }
                    _ => ExprKind::Hole(None),
                }
            }
            Tok::Kw("par") => {
                self.bump();
                ExprKind::Par(self.args()?)
            }
            Tok::Ident(name) => {
                self.bump();
                if is_upper(&name) && self.is_sym("{") {
                    let fields = self.record_fields()?;
                    ExprKind::Record { ctor: Some(name), fields }
                } else {
                    ExprKind::Name(name)
                }
            }
            Tok::Sym("{") => ExprKind::Record { ctor: None, fields: self.record_fields()? },
            Tok::Sym("[") => {
                self.bump();
                let mut items = Vec::new();
                while !self.is_sym("]") {
                    items.push(self.expr()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym("]")?;
                ExprKind::List(items)
            }
            Tok::Sym("(") => {
                self.bump();
                let mut items = Vec::new();
                while !self.is_sym(")") {
                    items.push(self.expr()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym(")")?;
                match items.len() {
                    0 => ExprKind::Unit,
                    1 => return Ok(items.pop().unwrap()),
                    _ => ExprKind::Tuple(items),
                }
            }
            Tok::Hash(_) => return self.err("E_UNSUPPORTED", "hash references are not supported in text input yet"),
            _ => return self.err("E_PARSE_EXPR", format!("expected an expression, found {}", self.describe())),
        };
        Ok(Expr::new(kind, start.to(self.prev_span())))
    }

    fn record_fields(&mut self) -> PResult<Vec<(String, Expr)>> {
        self.expect_sym("{")?;
        let mut out = Vec::new();
        while !self.is_sym("}") {
            let name = self.expect_ident()?;
            let value = if self.eat_sym(":") {
                self.expr()?
            } else {
                Expr::new(ExprKind::Name(name.clone()), self.prev_span())
            };
            out.push((name, value));
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym("}")?;
        Ok(out)
    }

    fn interp(&self, s: &str, span: Span) -> PResult<Vec<StrPart>> {
        let mut parts = Vec::new();
        let mut lit = String::new();
        let mut rest = s;
        while let Some(i) = rest.find(['{', '\\']) {
            lit.push_str(&rest[..i]);
            if rest[i..].starts_with("\\{") {
                lit.push('{');
                rest = &rest[i + 2..];
                continue;
            }
            if rest.as_bytes()[i] == b'\\' {
                lit.push('\\');
                rest = &rest[i + 1..];
                continue;
            }
            let mut depth = 0;
            let close = rest[i..].char_indices().find_map(|(j, c)| {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(j);
                        }
                    }
                    _ => {}
                }
                None
            });
            let Some(close) = close else {
                return Err(SyntaxError::new("E_LEX_STRING", "unclosed '{' in string".into(), span));
            };
            let inner = &rest[i + 1..i + close];
            let mut e = parse_expr(inner).map_err(|mut e| {
                e.span = span;
                e
            })?;
            reset_spans(&mut e, span);
            if !lit.is_empty() {
                parts.push(StrPart::Lit(std::mem::take(&mut lit)));
            }
            parts.push(StrPart::Expr(e));
            rest = &rest[i + close + 1..];
        }
        lit.push_str(rest);
        if !lit.is_empty() || parts.is_empty() {
            parts.push(StrPart::Lit(lit));
        }
        Ok(parts)
    }
}

pub fn has_placeholder(e: &Expr) -> bool {
    let mut found = false;
    crate::visit::walk_expr(e, &mut |x| {
        if matches!(x.kind, ExprKind::Placeholder) {
            found = true;
        }
        !matches!(x.kind, ExprKind::Lambda { .. })
    });
    found
}

fn reset_spans(e: &mut Expr, span: Span) {
    crate::visit::walk_expr_mut(e, &mut |x| x.span = span);
}
