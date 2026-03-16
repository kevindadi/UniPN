//! Counterexample and firing step types for reporting property violations.

use crate::model::{State, TransitionId};
use serde::{Deserialize, Serialize};
#[cfg(feature = "cir-anchor")]
use smallvec::SmallVec;

/// The kind of property violation detected.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PropertyViolation {
    /// Deadlock: no transitions enabled and not all threads have returned.
    Deadlock,
    /// Liveness violation: a transition can never fire.
    Liveness,
    /// Signal loss: a condvar notify occurs with no waiter.
    SignalLoss,
}

/// A single step in a counterexample trace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiringStep {
    /// The transition that fired.
    pub transition_id: TransitionId,
    /// The CIR statement IDs anchored to this transition (μ(t)).
    /// Only available with the `cir-anchor` feature.
    #[cfg(feature = "cir-anchor")]
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub anchor_sids: SmallVec<[String; 2]>,
}

/// A counterexample demonstrating a property violation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Counterexample {
    /// The kind of violation.
    pub kind: PropertyViolation,
    /// The sequence of firing steps leading to the violation.
    pub trace: Vec<FiringStep>,
    /// The final (violating) state.
    pub final_state: State,
}
