//! Index-based identifiers: places/transitions use contiguous `usize` numbers
//! so the hot path has zero hashing.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Place identifier (index-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlaceId(pub usize);

/// Transition identifier (index-based).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TransitionId(pub usize);

impl From<usize> for PlaceId {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

impl From<usize> for TransitionId {
    fn from(v: usize) -> Self {
        Self(v)
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
