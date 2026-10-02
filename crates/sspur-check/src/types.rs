use std::collections::BTreeSet;
use std::fmt;

pub const NUMERIC: &[&str] = &["Int", "I8", "I16", "I32", "U8", "U16", "U32", "U64", "F32", "F64"];

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Con(String, Vec<Type>),
    Tuple(Vec<Type>),
    Fn(Vec<Type>, Box<Type>, Row),
    Var(u32),
    Param(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row {
    pub atoms: BTreeSet<String>,
    pub var: Option<u32>,
}

impl Row {
    pub fn closed(atoms: impl IntoIterator<Item = String>) -> Self {
        Row { atoms: atoms.into_iter().collect(), var: None }
    }
}

impl Type {
    pub fn con(name: &str) -> Type {
        Type::Con(name.to_string(), vec![])
    }

    pub fn app(name: &str, args: Vec<Type>) -> Type {
        Type::Con(name.to_string(), args)
    }

    pub fn int() -> Type {
        Type::con("Int")
    }

    pub fn bool() -> Type {
        Type::con("Bool")
    }

    pub fn str() -> Type {
        Type::con("Str")
    }

    pub fn unit() -> Type {
        Type::con("Unit")
    }

    pub fn list(t: Type) -> Type {
        Type::app("List", vec![t])
    }

    pub fn opt(t: Type) -> Type {
        Type::app("Opt", vec![t])
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Con(n, a) if a.is_empty() && NUMERIC.contains(&n.as_str()))
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Con(n, a) if a.is_empty() => write!(f, "{n}"),
            Type::Con(n, a) => {
                write!(f, "{n}[")?;
                for (i, t) in a.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, "]")
            }
            Type::Tuple(xs) => {
                write!(f, "(")?;
                for (i, t) in xs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, ")")
            }
            Type::Fn(ps, r, row) => {
                if ps.len() == 1 {
                    write!(f, "{} -> {r}", ps[0])?;
                } else {
                    write!(f, "(")?;
                    for (i, t) in ps.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{t}")?;
                    }
                    write!(f, ") -> {r}")?;
                }
                if !row.atoms.is_empty() {
                    write!(f, " ! {}", row.atoms.iter().cloned().collect::<Vec<_>>().join(", "))?;
                }
                Ok(())
            }
            Type::Var(v) => write!(f, "?{v}"),
            Type::Param(p) => write!(f, "{p}"),
        }
    }
}
