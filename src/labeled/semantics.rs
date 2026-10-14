//! Plain P/T firing for [`LabeledNet`].
//!
//! Labels classify; they do not change how tokens move. Enabling is purely
//! structural (read and inhibitor arcs included), firing consumes the inputs,
//! produces the outputs, and empties the places reached by reset arcs. A
//! labeled place has no capacity — a bound would be a frontend's choice, and a
//! net that needs one is a frontend.

use crate::analysis::Semantics;
use crate::net::{Marking, PlaceCapacity, TransitionId};

use super::kinds::{LabeledNet, Tag};

/// Unbounded: this is the labeled net's semantics, not a placeholder — there is
/// no capacity for a bare label to carry.
impl<L> PlaceCapacity for Tag<L> {
    fn capacity(&self) -> Option<usize> {
        None
    }
}

impl<PL, TL> Semantics for LabeledNet<PL, TL> {
    type State = Marking;

    fn can_fire(&self, state: &Marking, transition: TransitionId) -> bool {
        self.structurally_enabled(state, transition)
    }

    /// `None` only if a token count would overflow `usize` — rejected rather
    /// than saturated, since saturating would silently lose tokens.
    fn fire_enabled(&self, state: &Marking, transition: TransitionId) -> Option<Marking> {
        let mut next = state.clone();
        self.consume_inputs(&mut next, transition);
        self.produce_outputs_bounded(&mut next, transition)?;
        self.apply_resets(&mut next, transition);
        Some(next)
    }
}
