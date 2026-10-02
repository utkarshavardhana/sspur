pub mod ast;
pub mod lexer;
pub mod parser;
pub mod printer;
pub mod visit;

pub use ast::*;
pub use parser::{parse, parse_expr};
pub use printer::print_module;

#[derive(Clone, Debug, PartialEq)]
pub struct SyntaxError {
    pub code: &'static str,
    pub msg: String,
    pub span: Span,
}

impl SyntaxError {
    pub fn new(code: &'static str, msg: String, span: Span) -> Self {
        SyntaxError { code, msg, span }
    }
}

pub fn line_col(src: &str, offset: u32) -> (usize, usize) {
    let off = (offset as usize).min(src.len());
    let before = &src[..off];
    let line = before.matches('\n').count() + 1;
    let col = off - before.rfind('\n').map_or(0, |i| i + 1) + 1;
    (line, col)
}
