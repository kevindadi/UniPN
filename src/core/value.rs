use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

use crate::ids::{SortId, Symbol};

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
            Self::Bool(value) => value.hash(state),
            Self::Bool3(value) => value.hash(state),
            Self::Int(value) => value.hash(state),
            Self::Enum(value) => value.hash(state),
            Self::Tuple(values) => values.hash(state),
            Self::Record(fields) => fields.hash(state),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Token {
    pub sort: SortId,
    pub value: Value,
}

impl Token {
    pub fn new(sort: SortId, value: Value) -> Self {
        Self { sort, value }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypedValue {
    pub sort: SortId,
    pub value: Value,
}

impl TypedValue {
    pub fn new(sort: SortId, value: Value) -> Self {
        Self { sort, value }
    }
}
