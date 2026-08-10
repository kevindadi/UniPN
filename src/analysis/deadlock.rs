//! Deadlock detection: no enabled transition, and at least one thread is not at
//! a thread-terminal place.

use crate::netlike::NetLike;
use crate::state::State;

/// Decide whether a state with no enabled transitions is a deadlock.
///
/// Resource tokens (Mutex/RwLock/Semaphore/Channel) are not control flow; if
/// every control-flow token sits on a [`NetLike::is_thread_terminal`] place,
/// all threads have finished and it is not a deadlock.
pub fn is_deadlock(net: &dyn NetLike, state: &State) -> bool {
    state
        .marking
        .iter_nonzero()
        .any(|(p, _)| !net.is_resource(p) && !net.is_thread_terminal(p))
}

/// The set of blocked (control-flow, non-terminal) places in a deadlock state
/// (for diagnostics).
pub fn blocked_places(net: &dyn NetLike, state: &State) -> Vec<crate::ids::PlaceId> {
    state
        .marking
        .iter_nonzero()
        .filter(|(p, _)| !net.is_resource(*p) && !net.is_thread_terminal(*p))
        .map(|(p, _)| p)
        .collect()
}
