//! Runtime state types for CVN simulation and analysis.

use crate::model::{PlaceId, Val};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Sparse marking: maps place IDs to their token counts.
///
/// Only places with tokens > 0 are stored.
pub type Marking = FxHashMap<PlaceId, u32>;

/// Ordered variable store: maps variable names to their current values.
pub type VarStore = IndexMap<String, Val>;

/// A complete CVN state comprising a marking and a variable store.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    /// Current token distribution across places.
    pub marking: Marking,
    /// Current variable values.
    pub vars: VarStore,
}

impl State {
    /// Create a new state.
    pub fn new(marking: Marking, vars: VarStore) -> Self {
        Self { marking, vars }
    }

    /// Get the token count at a place (0 if absent).
    pub fn tokens(&self, place: &PlaceId) -> u32 {
        self.marking.get(place).copied().unwrap_or(0)
    }

    /// Set the token count at a place. Removes the entry if count is 0.
    pub fn set_tokens(&mut self, place: PlaceId, count: u32) {
        if count == 0 {
            self.marking.remove(&place);
        } else {
            self.marking.insert(place, count);
        }
    }
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.marking == other.marking && self.vars == other.vars
    }
}

impl Eq for State {}

impl Hash for State {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Sort marking entries for deterministic hashing (FxHashMap iteration order is unstable)
        let mut entries: Vec<_> = self.marking.iter().collect();
        entries.sort_by(|a, b| a.0 .0.cmp(&b.0 .0));
        entries.len().hash(state);
        for (k, v) in &entries {
            k.hash(state);
            v.hash(state);
        }

        self.vars.len().hash(state);
        for (k, v) in &self.vars {
            k.hash(state);
            v.hash(state);
        }
    }
}
