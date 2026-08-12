use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlaceId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TransitionId(pub usize);

pub type SortId = usize;
pub type FuncId = usize;
pub type Symbol = String;

impl From<usize> for PlaceId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl From<usize> for TransitionId {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl PlaceId {
    pub const fn index(self) -> usize {
        self.0
    }
}

impl TransitionId {
    pub const fn index(self) -> usize {
        self.0
    }
}

impl fmt::Display for PlaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "p{}", self.0)
    }
}

impl fmt::Display for TransitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}
