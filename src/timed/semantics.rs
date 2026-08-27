//! Discrete (untimed) firing for [`TimedNet`].
//!
//! Successor capacity never gates a transition: overflow is clamped on firing
//! instead. Clamping a *non-saturating* place is invalid behavior, so firing
//! reports it — see [`TimedNet::fire_reporting_overflow`]. The report is a
//! return value, not a global: the state-class explorer accumulates it into the
//! graph it builds.

use crate::analysis::Semantics;
use crate::net::{Marking, PlaceCapacity, PlaceId, State, TransitionId};

use super::kinds::{TimedNet, TimedPlaceKind, TimedState};

impl PlaceCapacity for TimedPlaceKind {
    fn capacity(&self) -> Option<usize> {
        self.capacity
    }
}

impl TimedNet {
    /// Structural enabling. Successor capacity is not consulted (overflow is
    /// clamped on firing instead).
    pub fn is_enabled(&self, marking: &Marking, transition: TransitionId) -> bool {
        self.structurally_enabled(marking, transition)
    }

    /// Fires a transition, reporting the places whose count had to be clamped.
    ///
    /// Only *non-saturating* places are reported: a saturating place absorbing
    /// overflow is intended behavior, while a non-saturating one being clamped
    /// means the net lost tokens it should not have.
    ///
    /// Enabling is not re-checked (the state-class explorer already has).
    pub fn fire_reporting_overflow(
        &self,
        marking: &Marking,
        transition: TransitionId,
    ) -> (Marking, Vec<PlaceId>) {
        let mut next = marking.clone();
        self.consume_inputs(&mut next, transition);
        let overflowed = self
            .produce_outputs_clamped(&mut next, transition)
            .into_iter()
            .filter(|place| self.place(*place).is_some_and(|p| !p.kind.saturate))
            .collect();
        (next, overflowed)
    }

    /// Fires a transition, discarding the overflow report.
    pub fn fire(&self, marking: &Marking, transition: TransitionId) -> Marking {
        self.fire_reporting_overflow(marking, transition).0
    }
}

impl Semantics for TimedNet {
    type State = TimedState;

    fn can_fire(&self, state: &Self::State, transition: TransitionId) -> bool {
        self.is_enabled(&state.marking, transition)
    }

    fn fire_enabled(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        Some(State::new(
            self.fire(&state.marking, transition),
            state.extra,
        ))
    }
}
