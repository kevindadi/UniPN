//! Shared analysis engines.
//!
//! Two families live here:
//!
//! - **NetLike-based** (this module's root): the domain-neutral engines consumed
//!   by every frontend net — reachability exploration (BFS/DFS/POR), deadlock,
//!   dead-transition, conflict sets, boundness, and (feature-gated) invariants
//!   and timed analysis. They depend only on [`crate::netlike::NetLike`].
//! - **`generic`**: the [`crate::semantics::Semantics`]-trait-based explorer
//!   (see [`generic`]).

pub mod generic;

mod boundness;
mod conflict;
mod dead_transition;
mod deadlock;
mod explore;
#[cfg(feature = "invariants")]
pub mod invariants;
mod pt_graph;
#[cfg(feature = "timed")]
pub mod timed;

use crate::ids::TransitionId;
use crate::state::State;

pub use boundness::{BoundnessAnalyzer, BoundnessResult, check_boundness, check_place_boundness};
pub use conflict::*;
pub use dead_transition::*;
pub use deadlock::*;
pub use explore::*;
pub use pt_graph::{
    PtEdge, PtPlaceSnapshot, PtSearchStrategy, PtState, PtStateGraph, PtStateGraphConfig,
    PtStateId, PtTransitionFailure, TokenChange,
};

/// Analysis mode.
///
/// `Timed` is a reserved mode (feature `timed`): it runs state-class (DBM)
/// reachability analysis, bridging to PTPN for time/real-time-scheduling
/// properties. When disabled it degrades to the untimed reachability graph.
#[derive(Clone, Debug)]
pub enum AnalysisMode {
    Untimed,
    #[cfg(feature = "timed")]
    Timed {
        clock_classes: Vec<crate::timed::ClockClass>,
        /// Whether to enable fixed-priority preemption semantics.
        priorities: bool,
    },
}

/// Exploration configuration.
#[derive(Clone, Debug)]
pub struct AnalysisConfig {
    pub mode: AnalysisMode,
    pub strategy: SearchStrategy,
    pub max_states: usize,
    /// Partial-order reduction (sleep-set).
    pub por: bool,
    /// Run net reduction (loop/sequence/intermediate) before building the
    /// graph (reserved).
    pub reduce: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            mode: AnalysisMode::Untimed,
            strategy: SearchStrategy::Bfs,
            max_states: 100_000,
            por: false,
            reduce: false,
        }
    }
}

/// Search strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchStrategy {
    /// Breadth-first (shortest counterexamples).
    #[default]
    Bfs,
    /// Depth-first (lower memory).
    Dfs,
}

/// A single firing step.
#[derive(Clone, Debug)]
pub struct FiringStep {
    pub transition: TransitionId,
    pub anchors: Vec<String>,
}

/// Type of property violation.
#[derive(Clone, Debug)]
pub enum PropertyViolation {
    Deadlock,
    DeadTransition {
        transition: TransitionId,
        anchors: Vec<String>,
    },
    GoalUnmet {
        goal: String,
    },
}

/// A counterexample: firing sequence + final state + violation type.
#[derive(Clone, Debug)]
pub struct Counterexample {
    pub kind: PropertyViolation,
    pub trace: Vec<FiringStep>,
    pub final_state: State,
}
