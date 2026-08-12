use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

use super::sort::Symbol;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Bool3 {
    False,
    Unknown,
    True,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Unit,
    Bool(bool),
    Bool3(Bool3),
    Int(i64),
    Enum(Symbol),
    Tuple(Vec<Value>),
    Record(Vec<(Symbol, Value)>),
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Unit => {}
            Self::Bool(v) => v.hash(state),
            Self::Bool3(v) => v.hash(state),
            Self::Int(v) => v.hash(state),
            Self::Enum(v) => v.hash(state),
            Self::Tuple(items) => items.hash(state),
            Self::Record(fields) => fields.hash(state),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Token {
    pub value: Value,
}

impl Token {
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}
