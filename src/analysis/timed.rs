//! Reserved time-analysis module (feature `timed`).
//!
//! State-class reachability analysis: each state class = `(marking, DBM)`, where
//! the DBM constrains clock differences and absolute values. Canonicalization
//! combines equality / max-lower / intersection (aligned with PTPN's
//! `canonicalization` module).
//!
//! TODO: port PTPN's state-class exploration (DBM Floyd-Warshall + difference
//! constraints), plus the enable/fire semantics extension for
//! `Transition.timing/.priority`.

use crate::netlike::NetLike;
use crate::state::State;

/// A state class: marking + clock difference-bound matrix (DBM).
#[derive(Clone, Debug)]
pub struct StateClass {
    pub state: State,
    pub dbm: Vec<Vec<i64>>,
}

/// Timed-analysis configuration.
#[derive(Clone, Debug)]
pub struct TimedConfig {
    /// Bounds of each clock (clock id → upper bound).
    pub clock_bounds: Vec<i64>,
    /// Whether to enable priority preemption.
    pub priorities: bool,
}

/// Explore the state-class reachability graph (reserved).
pub fn explore_timed(_net: &dyn NetLike, _config: &TimedConfig) -> Result<Vec<StateClass>, String> {
    Err("timed state-class analysis not implemented yet — see PTPN bridge".into())
}
