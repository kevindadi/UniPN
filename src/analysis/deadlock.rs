//! Deadlock classification: a blocked state is a deadlock iff at least one
//! control-flow token is not at a terminal place and not a resource token.
//!
//! The explorer only reports *blocked* states; the caller decides which of them
//! are real deadlocks via [`find_deadlocks`] (or `is_deadlock` directly).

use crate::ids::PlaceId;
use crate::netlike::NetLike;
use crate::state::State;

use super::{Counterexample, PropertyViolation, ReachabilityGraph};

/// Decide whether a state with no enabled transitions is a deadlock.
///
/// A token is a "live thread" when it sits on a place that is neither a
/// resource nor a thread terminal. If any live-thread token remains, the
/// blocked state is a deadlock; otherwise every thread has finished normally.
///
/// This depends on the net's optional `is_resource` / `is_thread_terminal`
/// predicates, which default to `false` (i.e. any non-empty blocked state is a
/// deadlock).
pub fn is_deadlock(net: &dyn NetLike, state: &State) -> bool {
    state
        .marking
        .iter_nonzero()
        .any(|(p, _)| !net.is_resource(p) && !net.is_thread_terminal(p))
}

/// The set of blocked (control-flow, non-terminal) places in a deadlock state
/// (for diagnostics).
pub fn blocked_places(net: &dyn NetLike, state: &State) -> Vec<PlaceId> {
    state
        .marking
        .iter_nonzero()
        .filter(|(p, _)| !net.is_resource(*p) && !net.is_thread_terminal(*p))
        .map(|(p, _)| p)
        .collect()
}

/// Classify the blocked states of a reachability graph into deadlock
/// counterexamples, each with a reconstructed firing trace.
pub fn find_deadlocks(net: &dyn NetLike, rg: &ReachabilityGraph) -> Vec<Counterexample> {
    rg.blocked
        .iter()
        .filter(|&&state| is_deadlock(net, &rg.states[state]))
        .map(|&state| Counterexample {
            kind: PropertyViolation::Deadlock,
            trace: rg.trace_to(state),
            final_state: rg.states[state].clone(),
        })
        .collect()
}
