//! Discrete (untimed) firing for [`TimedNet`].
//!
//! Enabling is input-driven only: successor capacity never gates a transition,
//! overflow is clamped on firing instead. Clamping a *non-saturating* place is
//! invalid behavior, so it is recorded for the metrics layer.

use std::cell::RefCell;

use crate::analysis::NetLike;
use crate::net::{ArcDir, Marking, State, TransitionId};

use super::kinds::{TimedNet, TimedState};

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

impl TimedNet {
    /// Structural (input-driven) enabling: every input place holds enough
    /// tokens. Successor capacity never gates a transition (overflow on
    /// non-saturating places is clamped on firing).
    pub fn is_enabled(&self, marking: &Marking, transition: TransitionId) -> bool {
        self.arcs_of(transition, ArcDir::Input)
            .all(|arc| marking.tokens(arc.place) >= arc.weight)
    }

    /// Fires a transition: consumes input tokens and produces output tokens,
    /// clamping each output place to its capacity.
    ///
    /// This inherent method does not re-check enabling (the timed state-class
    /// explorer already has). [`NetLike::fire`] returns `None` when the
    /// transition is not structurally enabled.
    pub fn fire(&self, marking: &Marking, transition: TransitionId) -> Marking {
        self.fire_marking(marking, transition)
    }

    fn fire_marking(&self, marking: &Marking, transition: TransitionId) -> Marking {
        let mut next = marking.clone();

        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = next.tokens(arc.place);
            next.set(arc.place, current.saturating_sub(arc.weight));
        }

        for arc in self.arcs_of(transition, ArcDir::Output) {
            let current = next.tokens(arc.place);
            let produced = current.saturating_add(arc.weight);
            let place = self.place(arc.place);
            let clamped = match place.and_then(|p| p.kind.capacity) {
                Some(cap) if produced > cap => {
                    // Firing always happens (enabling is input-driven). Overflow
                    // is clamped; a non-saturating place being clamped is an
                    // invalid behavior recorded for the metrics layer.
                    if place.is_some_and(|p| !p.kind.saturate) {
                        OVERFLOW.with(|o| o.borrow_mut().insert(arc.place.index()));
                    }
                    cap
                }
                _ => produced,
            };
            next.set(arc.place, clamped);
        }

        next
    }
}

impl NetLike for TimedNet {
    type State = TimedState;

    fn num_places(&self) -> usize {
        self.places.len()
    }

    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    fn enabled(&self, state: &Self::State) -> Vec<TransitionId> {
        self.transitions
            .iter()
            .filter(|t| self.is_enabled(&state.marking, t.id))
            .map(|t| t.id)
            .collect()
    }

    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        if !self.is_enabled(&state.marking, transition) {
            return None;
        }
        Some(State::new(
            self.fire_marking(&state.marking, transition),
            state.extra,
        ))
    }
}
