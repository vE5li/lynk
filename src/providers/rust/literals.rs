//! The values of Rust literals, compared with the values links state.

use std::fmt;

use syn::{Expr, Lit, UnOp};

/// The value a literal stands for, however it is written: `0x10`, `16` and
/// `1_6u8` are one integer.
#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// A string, byte string, C string, character or byte, by its content.
    Text(String),
    /// An integer, by its base-10 digits, without suffix or sign.
    Integer {
        negative: bool,
        digits: String,
    },
    Float(f64),
    Bool(bool),
}

impl Literal {
    /// The value of an expression that is a literal, maybe negated; `None`
    /// for any other expression.
    pub fn of_expression(expression: &Expr) -> Option<Self> {
        match expression {
            Expr::Lit(literal) => Self::of_lit(&literal.lit),
            Expr::Group(group) => Self::of_expression(&group.expr),
            Expr::Paren(parenthesized) => Self::of_expression(&parenthesized.expr),
            Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => Self::of_expression(&unary.expr)?.negated(),
            _ => None,
        }
    }

    /// The value [`written`](lynk:rust.argument) in a link states: a Rust
    /// literal, maybe negated, or else a text as it is.
    pub fn written(written: &str) -> Self {
        let (negative, unsigned) = match written.strip_prefix('-') {
            Some(unsigned) => (true, unsigned),
            None => (false, written),
        };
        let literal = syn::parse_str::<Lit>(unsigned).ok().and_then(|literal| Self::of_lit(&literal));
        let literal = if negative { literal.and_then(Self::negated) } else { literal };
        literal.unwrap_or_else(|| Self::Text(written.to_owned()))
    }

    /// Whether a link stating [`written`](lynk:rust.argument) states this
    /// value. A text matches as it is or written as a Rust literal, and an
    /// integer may be written for a float.
    pub fn matches(&self, written: &str) -> bool {
        match (self, Self::written(written)) {
            (Self::Text(text), _) if text == written => true,
            (Self::Float(value), Self::Integer { negative, digits }) => {
                let sign = if negative { "-" } else { "" };
                format!("{sign}{digits}").parse::<f64>().is_ok_and(|written| written == *value)
            }
            (value, written) => *value == written,
        }
    }

    fn of_lit(literal: &Lit) -> Option<Self> {
        Some(match literal {
            Lit::Str(text) => Self::Text(text.value()),
            Lit::ByteStr(bytes) => Self::Text(String::from_utf8_lossy(&bytes.value()).into_owned()),
            Lit::CStr(text) => Self::Text(text.value().to_string_lossy().into_owned()),
            Lit::Char(character) => Self::Text(character.value().to_string()),
            Lit::Byte(byte) => Self::Text(char::from(byte.value()).to_string()),
            Lit::Int(integer) => Self::Integer {
                negative: false,
                digits: integer.base10_digits().to_owned(),
            },
            Lit::Float(float) => Self::Float(float.base10_digits().parse().ok()?),
            Lit::Bool(boolean) => Self::Bool(boolean.value),
            _ => return None,
        })
    }

    /// The value negated, `None` for one that is not a number. Zero has no
    /// sign, so `-0` is `0`.
    fn negated(self) -> Option<Self> {
        match self {
            Self::Integer { negative, digits } => Some(Self::Integer {
                negative: !negative && !digits.trim_start_matches('0').is_empty(),
                digits,
            }),
            Self::Float(value) => Some(Self::Float(-value)),
            _ => None,
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Text(text) => write!(formatter, "{text}"),
            Self::Integer { negative, digits } => write!(formatter, "{}{digits}", if *negative { "-" } else { "" }),
            Self::Float(value) => write!(formatter, "{value}"),
            Self::Bool(value) => write!(formatter, "{value}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(source: &str) -> Option<Literal> {
        Literal::of_expression(&syn::parse_str(source).unwrap())
    }

    #[test]
    fn reads_literals_maybe_negated() {
        assert_eq!(value("\"tool\""), Some(Literal::Text("tool".to_owned())));
        assert_eq!(value("b'a'"), Some(Literal::Text("a".to_owned())));
        assert_eq!(
            value("-(0x10)"),
            Some(Literal::Integer {
                negative: true,
                digits: "16".to_owned()
            })
        );
        assert_eq!(value("1.5e1"), Some(Literal::Float(15.0)));
        assert_eq!(value("true"), Some(Literal::Bool(true)));
        assert_eq!(value("-\"tool\""), None);
        assert_eq!(value("1 + 2"), None);
        assert_eq!(value("LIMIT"), None);
    }

    #[test]
    fn matches_the_value_however_it_is_written() {
        let integer = value("0x10").unwrap();
        assert!(integer.matches("16"));
        assert!(integer.matches("1_6usize"));
        assert!(!integer.matches("-16"));
        assert!(!integer.matches("17"));
        assert!(value("-0").unwrap().matches("0"));
        let float = value("-1.50").unwrap();
        assert!(float.matches("-1.5"));
        assert!(value("2.0").unwrap().matches("2"));
        let text = value("\"a tool\"").unwrap();
        assert!(text.matches("a tool"));
        assert!(text.matches("\"a tool\""));
        assert!(!text.matches("tool"));
        // Text only matches as a Rust literal of the same value.
        assert!(value("\"16\"").unwrap().matches("16"));
        assert!(!integer.matches("\"16\""));
        assert!(value("true").unwrap().matches("true"));
        assert!(!value("true").unwrap().matches("yes"));
    }
}
