//! Untimed (P/T skeleton) firing for [`TpnNet`].
//!
//! A `TpnNet`'s [`Semantics`] is the plain P/T
//! firing rule over a bare [`Marking`]: structural
//! enabling, consume inputs, produce outputs (rejecting a `usize` overflow
//! rather than saturating), apply resets. The firing interval on a transition
//! is an *annotation* that this skeleton ignores — exploring a `TpnNet`
//! directly therefore computes the untimed over-approximation of its timed
//! behavior, exactly as documented in [`kinds`](super::kinds). The dense-time
//! semantics lives in `analysis::tpn`, not here.

use crate::analysis::Semantics;
use crate::net::{Marking, TransitionId};

use super::kinds::TpnNet;

impl<PL, TL> Semantics for TpnNet<PL, TL> {
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
