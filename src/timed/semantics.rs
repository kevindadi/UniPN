//! Discrete (untimed) firing for [`TimedNet`].
//!
//! Successor capacity never gates a transition: overflow is clamped on firing
//! instead. Clamping a *non-saturating* place is invalid behavior, so it is
//! recorded for the metrics layer.

use std::cell::RefCell;

use crate::analysis::Semantics;
use crate::net::{Marking, PlaceCapacity, State, TransitionId};

use super::kinds::{TimedNet, TimedPlaceKind, TimedState};

// Overflow recording: `fire` clamps every overflowing place to capacity, but a
// NON-saturating place being clamped is an invalid behavior and is recorded so
// the metrics layer can report it. Reset at the start of each build.
thread_local! {
    static OVERFLOW: RefCell<std::collections::BTreeSet<usize>> =
        const { RefCell::new(std::collections::BTreeSet::new()) };
}

pub fn reset_overflow_recording() {
    OVERFLOW.with(|o| o.borrow_mut().clear());
}

pub fn overflowed_places() -> Vec<usize> {
    OVERFLOW.with(|o| o.borrow().iter().copied().collect())
}

impl PlaceCapacity for TimedPlaceKind {
    fn capacity(&self) -> Option<usize> {
        self.capacity
    }
}

impl TimedNet {
    /// Structural enabling. Successor capacity is not consulted (overflow on
    /// non-saturating places is clamped on firing instead).
    pub fn is_enabled(&self, marking: &Marking, transition: TransitionId) -> bool {
        self.structurally_enabled(marking, transition)
    }

    /// Fires a transition: consumes input tokens and produces output tokens,
    /// clamping each output place to its capacity.
    ///
    /// This inherent method does not re-check enabling (the timed state-class
    /// explorer already has). [`NetLike::fire`](crate::analysis::NetLike::fire)
    /// returns `None` when the transition is not structurally enabled.
    pub fn fire(&self, marking: &Marking, transition: TransitionId) -> Marking {
        let mut next = marking.clone();
        self.consume_inputs(&mut next, transition);
        for place in self.produce_outputs_clamped(&mut next, transition) {
            if self.place(place).is_some_and(|p| !p.kind.saturate) {
                OVERFLOW.with(|o| o.borrow_mut().insert(place.index()));
            }
        }
        next
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
