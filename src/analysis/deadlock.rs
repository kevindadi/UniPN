//! Deadlock detection for CVN states.

use crate::model::{PlaceId, State};
use crate::net::CvnNet;

/// Check if a state is terminal (all control tokens are in return places).
pub fn is_terminal(net: &CvnNet, state: &State) -> bool {
    for (place_id, &count) in &state.marking {
        if count == 0 {
            continue;
        }
        if let Some(place) = net.place(place_id) {
            if place.is_control_flow() && !place.is_return {
                return false;
            }
        }
    }
    true
}

/// Check if a state is a deadlock.
///
/// A state is a deadlock iff it is not terminal and no transitions are enabled.
pub fn is_deadlock(net: &CvnNet, state: &State) -> bool {
    !is_terminal(net, state) && net.enabled_transitions(state).is_empty()
}

/// Find all place IDs that have tokens and represent blocked control flow.
///
/// Useful for diagnosing deadlocks.
pub fn blocked_places(net: &CvnNet, state: &State) -> Vec<PlaceId> {
    state
        .marking
        .iter()
        .filter(|(_, count)| **count > 0)
        .filter_map(|(pid, _)| {
            net.place(pid)
                .filter(|p| p.is_control_flow() && !p.is_return)
                .map(|_| pid.clone())
        })
        .collect()
}
