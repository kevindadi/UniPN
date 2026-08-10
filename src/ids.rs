//! 索引化标识：库位/变迁用连续 usize 编号，保证热路径零哈希。

use std::fmt;

/// 库位标识（索引制）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlaceId(pub usize);

/// 变迁标识（索引制）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TransitionId(pub usize);

/// 弧权重（u32 足够覆盖库位容量与并发实体数）。
pub type Weight = u32;

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
    pub fn index(self) -> usize {
        self.0
    }
}

impl TransitionId {
    pub fn index(self) -> usize {
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
