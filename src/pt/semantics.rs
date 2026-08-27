//! P/T firing semantics (ConcBugDect's).
//!
//! Enabling is purely structural. Firing consumes the inputs, produces the
//! outputs *clamped* to each place's capacity, and empties the places reached
//! by reset arcs.

use crate::analysis::{NetLike, Semantics};
use crate::net::{Marking, PlaceCapacity, TransitionId};

use super::kinds::{PtNet, PtPlaceKind};

impl PlaceCapacity for PtPlaceKind {
    fn capacity(&self) -> Option<usize> {
        self.capacity
    }
}

impl Semantics for PtNet {
    type State = Marking;

    fn can_fire(&self, state: &Self::State, transition: TransitionId) -> bool {
        self.structurally_enabled(state, transition)
    }

    fn fire_enabled(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        let mut next = state.clone();
        self.consume_inputs(&mut next, transition);
        self.produce_outputs_clamped(&mut next, transition);
        self.apply_resets(&mut next, transition);
        Some(next)
    }
}

impl PtNet {
    /// `Result`-shaped firing (mirrors ConcBugDect's `fire_transition`).
    // The `()` error type is part of the mirrored ConcBugDect signature.
    #[allow(clippy::result_unit_err)]
    pub fn fire_transition(
        &self,
        marking: &Marking,
        transition: TransitionId,
    ) -> Result<Marking, ()> {
        NetLike::fire(self, marking, transition).ok_or(())
    }

    /// The enabled transitions under a marking (mirrors ConcBugDect's API).
    pub fn enabled_transitions(&self, marking: &Marking) -> Vec<TransitionId> {
        NetLike::enabled(self, marking)
    }
}
