//! PTPN place/transition kind payloads plus the [`TimedNet`] and [`TimedState`]
//! aliases.
//!
//! Time is an *annotation* on transitions: the interval, priority, core, and
//! suspendability live in [`TimedTransitionKind`], while the clock zone that
//! interprets them is built by the state-class analysis in
//! `analysis::timed`, not stored on the state.

use serde::{Deserialize, Serialize};

use crate::net::{Marking, Net, State};

use super::interval::TimeInterval;

/// The "control core" (negative core id) for zero-time control transitions.
pub const CONTROL_TRANSITION_CORE: i32 = -1;

/// PTPN place attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimedPlaceKind {
    /// `None` = unbounded.
    pub capacity: Option<usize>,
    /// Saturating places absorb overflow (a transition producing into a full
    /// saturating place stays enabled and the count is clamped on firing).
    pub saturate: bool,
}

/// PTPN transition attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimedTransitionKind {
    pub interval: TimeInterval,
    pub priority: i32,
    pub core: i32,
    pub suspendable: bool,
}

/// The priority timed Petri net (no arc payload).
pub type TimedNet = Net<TimedPlaceKind, TimedTransitionKind, ()>;

/// Extra payload of the discrete timed state.
///
/// Empty on purpose: [`NetLike`](crate::analysis::NetLike) for [`TimedNet`] is
/// untimed token flow. A clock zone is a *set* of valuations (a DBM), not a
/// single extra field that `fire` can update, so it stays in
/// `analysis::timed::StateClass`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimedExtra;

/// Discrete timed state: marking + empty extra (same shape as
/// [`CvnState`](crate::cvn::CvnState)).
pub type TimedState = State<TimedExtra>;

impl From<Marking> for TimedState {
    fn from(marking: Marking) -> Self {
        State::new(marking, TimedExtra)
    }
}
