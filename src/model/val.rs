//! Value types for the CVN variable store.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A concrete (fully-known) value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConcreteVal {
    /// Boolean value.
    Bool(bool),
    /// 64-bit signed integer.
    Int(i64),
    /// 64-bit floating point.
    Float(f64),
    /// String value.
    Str(String),
    /// Enum variant name.
    Enum(String),
}

impl Eq for ConcreteVal {}

impl std::hash::Hash for ConcreteVal {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::Bool(b) => b.hash(state),
            Self::Int(i) => i.hash(state),
            Self::Float(f) => f.to_bits().hash(state),
            Self::Str(s) => s.hash(state),
            Self::Enum(s) => s.hash(state),
        }
    }
}

impl fmt::Display for ConcreteVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Enum(s) => write!(f, "{s}"),
        }
    }
}

/// A value in the CVN variable store.
///
/// `Unknown` (⊤) is the absorbing element: any operation involving `Unknown` yields `Unknown`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Val {
    /// A fully-known concrete value.
    Concrete(ConcreteVal),
    /// Unknown / top (⊤). Absorbs through all operations.
    Unknown,
}

impl Val {
    /// Create a boolean value.
    pub fn bool(b: bool) -> Self {
        Self::Concrete(ConcreteVal::Bool(b))
    }

    /// Create an integer value.
    pub fn int(i: i64) -> Self {
        Self::Concrete(ConcreteVal::Int(i))
    }

    /// Create a float value.
    pub fn float(f: f64) -> Self {
        Self::Concrete(ConcreteVal::Float(f))
    }

    /// Create a string value.
    pub fn string(s: impl Into<String>) -> Self {
        Self::Concrete(ConcreteVal::Str(s.into()))
    }

    /// Create an enum variant value.
    pub fn enum_val(s: impl Into<String>) -> Self {
        Self::Concrete(ConcreteVal::Enum(s.into()))
    }

    /// Returns `true` if this value is `Unknown`.
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    /// Returns the concrete value, if any.
    pub fn as_concrete(&self) -> Option<&ConcreteVal> {
        match self {
            Self::Concrete(v) => Some(v),
            Self::Unknown => None,
        }
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Concrete(v) => write!(f, "{v}"),
            Self::Unknown => write!(f, "⊤"),
        }
    }
}
