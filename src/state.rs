//! State: dense marking + optional variable store.

use indexmap::IndexMap;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;

use crate::expr::Val;
use crate::ids::PlaceId;

/// Dense marking: index = place id, value = token count.
///
/// A dense vector serves the matrix hot path (index-direct access) and gives
/// deterministic hashing; token counts are usually tiny, and the vector length
/// is |P|, so the cost is negligible.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Marking(pub Vec<u32>);

impl Marking {
    pub fn new(counts: Vec<u32>) -> Self {
        Self(counts)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn tokens(&self, p: PlaceId) -> u32 {
        self.0.get(p.index()).copied().unwrap_or(0)
    }

    pub fn set(&mut self, p: PlaceId, count: u32) {
        if let Some(c) = self.0.get_mut(p.index()) {
            *c = count;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (PlaceId, u32)> + '_ {
        self.0.iter().enumerate().map(|(i, &c)| (PlaceId(i), c))
    }

    /// Iterate only over places with non-zero tokens.
    pub fn iter_nonzero(&self) -> impl Iterator<Item = (PlaceId, u32)> + '_ {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, c)| **c > 0)
            .map(|(i, &c)| (PlaceId(i), c))
    }
}

impl Hash for Marking {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Ordered variable store.
pub type VarStore = IndexMap<String, Val>;

/// A complete state.
#[derive(Clone, Debug, Default)]
pub struct State {
    pub marking: Marking,
    /// `None` means the net does not model data (pure P/T).
    pub vars: Option<VarStore>,
}

impl State {
    pub fn new(marking: Marking, vars: Option<VarStore>) -> Self {
        Self { marking, vars }
    }

    pub fn vars(&self) -> &IndexMap<String, Val> {
        match &self.vars {
            Some(v) => v,
            None => &EMPTY_VARS,
        }
    }
}

static EMPTY_VARS: LazyLock<IndexMap<String, Val>> = LazyLock::new(IndexMap::new);

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.marking == other.marking && self.vars == other.vars
    }
}

impl Eq for State {}

impl Hash for State {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.marking.hash(state);
        match &self.vars {
            Some(vars) => {
                vars.len().hash(state);
                for (k, v) in vars {
                    k.hash(state);
                    v.hash(state);
                }
            }
            None => 0usize.hash(state),
        }
    }
}
