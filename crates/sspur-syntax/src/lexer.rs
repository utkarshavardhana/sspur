use crate::ast::Span;
use crate::SyntaxError;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Hash(String),
    Kw(&'static str),
    Sym(&'static str),
    Newline(u32),
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub const KEYWORDS: &[&str] = &[
    "fn", "type", "test", "var", "for", "in", "if", "then", "else", "match", "catch", "do", "raise",
    "return", "with", "profile", "derive", "where", "pre", "post", "dec", "cost", "new", "true",
    "false", "and", "or", "not", "par", "ex", "trait", "impl", "store", "svc", "queue", "effect",
];

const SYMBOLS: &[&str] = &[
    "...", ":=", "==", "!=", "<=", ">=", "->", "=>", "**", "..", "(", ")", "[", "]", "{", "}", ",",
    ":", ".", "=", "<", ">", "+", "-", "*", "/", "%", "|", "!", "?", ";", "&",
];

pub fn lex(src: &str) -> Result<Vec<Token>, SyntaxError> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let mut depth: i32 = 0;
    let mut at_line_start = true;
    let mut layout: Vec<(i32, u32)> = Vec::new();

    while i < bytes.len() {
        if at_line_start {
            let (indent, next) = measure_indent(bytes, i);
            if next >= bytes.len() {
                break;
            }
            if bytes[next] == b'\n' || bytes[next] == b'\r' {
                i = next + 1;
                continue;
            }
            if depth == 0 && !out.is_empty() {
                out.push(Token { tok: Tok::Newline(indent), span: Span::new(next, next) });
            } else if depth > 0 {
                while layout.last().is_some_and(|(d, ind)| *d == depth && indent < *ind) {
                    layout.pop();
                }
                let opens = matches!(out.last().map(|t: &Token| &t.tok), Some(Tok::Kw("do" | "then" | "else")) | Some(Tok::Sym("=>" | "=")));
                let active = layout.last().is_some_and(|(d, _)| *d == depth);
                if !active && opens {
                    layout.push((depth, indent));
                }
                if layout.last().is_some_and(|(d, _)| *d == depth) {
                    out.push(Token { tok: Tok::Newline(indent), span: Span::new(next, next) });
                }
            }
            i = next;
            at_line_start = false;
            continue;
        }

        let c = bytes[i];
        match c {
            b' ' | b'\t' | b'\r' => i += 1,
            b'\n' => {
                i += 1;
                at_line_start = true;
            }
            b'"' | b'\'' => {
                let (s, end) = lex_string(src, i)?;
                out.push(Token { tok: Tok::Str(s), span: Span::new(i, end) });
                i = end;
            }
            b'#' => {
                let start = i;
                i += 1;
                while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                    i += 1;
                }
                out.push(Token { tok: Tok::Hash(src[start + 1..i].to_string()), span: Span::new(start, i) });
            }
            b'0'..=b'9' => {
                let (tok, end) = lex_number(src, i)?;
                out.push(Token { tok, span: Span::new(i, end) });
                i = end;
            }
            c if c == b'_' || c.is_ascii_alphabetic() => {
                let start = i;
                while i < bytes.len() && (bytes[i] == b'_' || bytes[i].is_ascii_alphanumeric()) {
                    i += 1;
                }
                let word = &src[start..i];
                let tok = match KEYWORDS.iter().find(|k| **k == word) {
                    Some(k) => Tok::Kw(k),
                    None if word == "_" => Tok::Sym("_"),
                    None => Tok::Ident(word.to_string()),
                };
                out.push(Token { tok, span: Span::new(start, i) });
            }
            _ => {
                let rest = &src[i..];
                let Some(sym) = SYMBOLS.iter().find(|s| rest.starts_with(**s)) else {
                    let ch = rest.chars().next().unwrap();
                    return Err(SyntaxError::new("E_LEX_CHAR", format!("unexpected character '{ch}'"), Span::new(i, i + ch.len_utf8())));
                };
                match *sym {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => {
                        while layout.last().is_some_and(|(d, _)| *d == depth) {
                            layout.pop();
                        }
                        depth -= 1
                    }
                    _ => {}
                }
                out.push(Token { tok: Tok::Sym(sym), span: Span::new(i, i + sym.len()) });
                i += sym.len();
            }
        }
    }
    out.push(Token { tok: Tok::Eof, span: Span::new(src.len(), src.len()) });
    Ok(out)
}

fn measure_indent(bytes: &[u8], mut i: usize) -> (u32, usize) {
    let mut n = 0;
    while i < bytes.len() {
        match bytes[i] {
            b' ' => n += 1,
            b'\t' => n += 4,
            _ => break,
        }
        i += 1;
    }
    (n, i)
}

fn lex_string(src: &str, start: usize) -> Result<(String, usize), SyntaxError> {
    let mut out = String::new();
    let quote = src[start..].chars().next().unwrap();
    let mut chars = src[start + 1..].char_indices();
    let mut depth = 0;
    let mut skip_to = 0;
    while let Some((off, ch)) = chars.next() {
        let pos = start + 1 + off;
        if pos < skip_to {
            continue;
        }
        match ch {
            '{' => {
                depth += 1;
                out.push(ch);
            }
            '}' if depth > 0 => {
                depth -= 1;
                out.push(ch);
            }
            '"' | '\'' if depth > 0 => {
                let (_, end) = lex_string(src, pos)?;
                out.push_str(&src[pos..end]);
                skip_to = end;
            }
            c if c == quote => return Ok((out, pos + 1)),
            '\\' => match chars.next() {
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, '{')) => out.push_str("\\{"),
                Some((_, c)) => {
                    return Err(SyntaxError::new("E_LEX_ESCAPE", format!("unknown escape '\\{c}'"), Span::new(pos, pos + 2)));
                }
                None => break,
            },
            '\n' => break,
            c => out.push(c),
        }
    }
    Err(SyntaxError::new("E_LEX_STRING", "unterminated string".into(), Span::new(start, start + 1)))
}

fn lex_number(src: &str, start: usize) -> Result<(Tok, usize), SyntaxError> {
    let bytes = src.as_bytes();
    let mut i = start;
    if src[i..].starts_with("0x") {
        i += 2;
        while i < bytes.len() && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
            i += 1;
        }
        let digits: String = src[start + 2..i].chars().filter(|c| *c != '_').collect();
        let v = i64::from_str_radix(&digits, 16)
            .map_err(|_| SyntaxError::new("E_LEX_NUMBER", "invalid hex literal".into(), Span::new(start, i)))?;
        return Ok((Tok::Int(v), i));
    }
    let mut is_float = false;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_digit() || c == b'_' {
            i += 1;
        } else if c == b'.' && !is_float && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            is_float = true;
            i += 1;
        } else if (c == b'e' || c == b'E') && i + 1 < bytes.len() && (bytes[i + 1].is_ascii_digit() || bytes[i + 1] == b'-') {
            is_float = true;
            i += 2;
        } else {
            break;
        }
    }
    if i < bytes.len() && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
        return Err(SyntaxError::new("E_LEX_NUMBER", "unsupported numeric suffix".into(), Span::new(start, i + 1)));
    }
    let text: String = src[start..i].chars().filter(|c| *c != '_').collect();
    let span = Span::new(start, i);
    if is_float {
        let v = text.parse().map_err(|_| SyntaxError::new("E_LEX_NUMBER", "invalid float".into(), span))?;
        Ok((Tok::Float(v), i))
    } else {
        let v = text.parse().map_err(|_| SyntaxError::new("E_LEX_NUMBER", "integer out of range".into(), span))?;
        Ok((Tok::Int(v), i))
    }
}
