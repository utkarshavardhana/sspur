#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span { start: start as u32, end: end as u32 }
    }

    pub fn to(self, other: Span) -> Span {
        Span { start: self.start.min(other.start), end: self.end.max(other.end) }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub profile: Option<String>,
    pub defs: Vec<Def>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Def {
    Type(TypeDef),
    Fn(FnDef),
    Test(TestDef),
    Effect(EffectDef),
    Store(StoreDef),
    Svc(SvcDef),
}

impl Def {
    pub fn name(&self) -> &str {
        match self {
            Def::Type(t) => &t.name,
            Def::Fn(f) => &f.name,
            Def::Test(t) => &t.name,
            Def::Effect(e) => &e.name,
            Def::Store(s) => &s.name,
            Def::Svc(s) => &s.name,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Def::Type(t) => t.span,
            Def::Fn(f) => f.span,
            Def::Test(t) => t.span,
            Def::Effect(e) => e.span,
            Def::Store(s) => s.span,
            Def::Svc(s) => s.span,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TParam {
    pub name: String,
    pub kind: Option<Ty>,
    pub refine: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeDef {
    pub name: String,
    pub params: Vec<TParam>,
    pub body: TypeBody,
    pub derives: Vec<String>,
    pub res: bool,
    pub drop: Option<String>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeBody {
    Record(Vec<Field>),
    Sum(Vec<Variant>),
    Alias(Ty, Option<Expr>),
    New(Ty),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: Ty,
    pub refine: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Option<Vec<Field>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Named { name: String, args: Vec<Ty>, span: Span },
    Tuple(Vec<Ty>),
    Fn { params: Vec<Ty>, ret: Box<Ty>, effects: Vec<Effect> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub name: String,
    pub args: Vec<Ty>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Ty,
    pub refine: Option<Expr>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FnDef {
    pub name: String,
    pub tparams: Vec<TParam>,
    pub params: Vec<Param>,
    pub ret: Option<Ty>,
    pub effects: Vec<Effect>,
    pub pres: Vec<Expr>,
    pub posts: Vec<Expr>,
    pub examples: Vec<Expr>,
    pub trusted: Option<String>,
    pub interrupt: Option<String>,
    pub body: Expr,
    pub span: Span,
    pub sig_span: Span,
    pub ext: Option<Extern>,
    pub kernel: Option<Box<KernelSpec>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KernelSpec {
    pub grid: Expr,
    pub group: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Extern {
    pub lib: Option<String>,
    pub symbol: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectDef {
    pub name: String,
    pub params: Vec<TParam>,
    pub ops: Vec<OpSig>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpSig {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<Ty>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoreDef {
    pub name: String,
    pub kind: String,
    pub key: Ty,
    pub val: Ty,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SvcDef {
    pub name: String,
    pub eps: Vec<Endpoint>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Endpoint {
    pub method: String,
    pub path: String,
    pub handler: String,
    pub span: Span,
}

impl Endpoint {
    pub fn path_params(&self) -> Vec<String> {
        self.path.split('/').filter_map(|s| s.strip_prefix('{').and_then(|s| s.strip_suffix('}'))).map(str::to_string).collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TestDef {
    pub name: String,
    pub body: Expr,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Pow => "**",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        }
    }

    pub fn prec(self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 4,
            BinOp::Add | BinOp::Sub => 6,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 7,
            BinOp::Pow => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    Neg,
    Not,
    Ref,
    RefMut,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StrPart {
    Lit(String),
    Expr(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Str(Vec<StrPart>),
    Bool(bool),
    Unit,
    Name(String),
    Hole(Option<String>),
    Placeholder,
    Field(Box<Expr>, String),
    Call(Box<Expr>, Vec<Expr>),
    Method { recv: Box<Expr>, name: String, targs: Vec<Ty>, args: Vec<Expr> },
    Index(Box<Expr>, Box<Expr>),
    Lambda { params: Vec<String>, body: Box<Expr>, implicit: bool },
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Match(Box<Expr>, Vec<Arm>),
    Catch(Box<Expr>, Vec<Arm>),
    Handle(Box<Expr>, Vec<Arm>),
    Block(Vec<Stmt>),
    Record { ctor: Option<String>, fields: Vec<(String, Expr)> },
    List(Vec<Expr>),
    Tuple(Vec<Expr>),
    Par(Vec<Expr>),
    Raise(Box<Expr>),
    Return(Box<Expr>),
    With(Box<Expr>, Vec<(Vec<PathSeg>, Expr)>),
    Table(Vec<TableRow>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub cells: Vec<Cell>,
    pub out: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Any,
    Pat(Pat),
    Cond(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PathSeg {
    Field(String),
    Index(Expr),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Arm {
    pub pat: Pat,
    pub guard: Option<Expr>,
    pub body: Expr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stmt {
    Let(Pat, Expr),
    Var(String, Expr),
    Assign(String, Expr, Span),
    Expr(Expr),
    For(Pat, Expr, Expr),
    While(Expr, Expr),
    Fn(Box<FnDef>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pat {
    Wild,
    Bind(String),
    Int(i64),
    Str(String),
    Bool(bool),
    Tuple(Vec<Pat>),
    Ctor { name: String, args: CtorArgs },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CtorArgs {
    None,
    Positional(Vec<Pat>),
    Record(Vec<(String, Pat)>),
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Expr { kind, span }
    }
}
