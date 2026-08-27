//! What a node's *kind* can tell an analysis about the role it plays.
//!
//! These are the questions more than one frontend has to answer the same way.
//! "A thread is stuck" is a shared definition — a control token sitting on a
//! place it cannot leave — but *which* places a thread may rest on is the
//! frontend's own knowledge, so the question is a trait on the kind, exactly
//! like [`PlaceCapacity`](crate::net::PlaceCapacity).
//!
//! Not every frontend answers every question. The timed net's places carry no
//! control/resource split and PTPN classifies schedulability rather than
//! deadlocks, so `TimedPlaceKind` implements neither trait; a bound is only
//! paid where it is used.

use crate::net::{Marking, Net, PlaceId};

/// A place kind that separates shared resources from control-flow points and
/// knows where a thread may legitimately come to rest.
pub trait PlaceRole {
    /// A shared resource — a lock, a channel, a semaphore — rather than a point
    /// in the control flow.
    fn is_resource(&self) -> bool;

    /// A place a thread legitimately ends on. A token resting here is a
    /// finished thread, not a stuck one.
    fn is_terminal(&self) -> bool;
}

/// A transition kind that classifies the concurrency operation it stands for.
///
/// The two frontends that implement this disagree on *shape*: P/T's variants
/// carry pointer-analysis payloads (`Lock(alias)`, `AtomicLoad(alias,
/// ordering, …)`) while the CVN's are bare tags, because in ConcIR the resource
/// identity already *is* the place identity. That is why the shared vocabulary
/// is a set of predicates and not a shared enum.
///
/// An operation that fits none of them answers `false` everywhere. P/T's
/// condvar `Wait` is a single transition that both drops and retakes a lock, so
/// it is neither an acquire nor a release; the CVN splits the same operation
/// into `CondvarWaitEnter` and `CondvarReacquire` and classifies both halves.
/// That is a difference in the two lowerings, not in this vocabulary.
pub trait TransitionRole {
    /// Takes a shared resource: a lock, a semaphore permit, a channel message.
    fn is_acquire(&self) -> bool;

    /// Gives a shared resource back.
    fn is_release(&self) -> bool;

    /// Creates a new thread.
    fn is_thread_spawn(&self) -> bool;

    /// Waits for another thread to finish.
    fn is_thread_join(&self) -> bool;

    /// An atomic load, store, or compare-exchange.
    fn is_atomic(&self) -> bool;

    /// A memory access through a raw pointer.
    fn is_unsafe_access(&self) -> bool;
}

impl<PK: PlaceRole, TK, AK> Net<PK, TK, AK> {
    /// Whether `place` is a shared resource (`false` for an unknown id).
    pub fn is_resource(&self, place: PlaceId) -> bool {
        self.place(place).is_some_and(|p| p.kind.is_resource())
    }

    /// Whether `place` is somewhere a thread legitimately ends.
    pub fn is_terminal(&self, place: PlaceId) -> bool {
        self.place(place).is_some_and(|p| p.kind.is_terminal())
    }

    /// The places whose tokens say a thread is stuck: control-flow places that
    /// are not a thread terminal.
    ///
    /// A resource token is not evidence of anything — an unlocked mutex sits on
    /// its own resource place — and a token on a thread end is a thread that
    /// finished.
    pub fn blocked_places(&self, marking: &Marking) -> Vec<PlaceId> {
        marking
            .iter_nonzero()
            .map(|(place, _)| place)
            .filter(|&place| !self.is_resource(place) && !self.is_terminal(place))
            .collect()
    }

    /// Whether a marking that has nothing left to fire is a genuine deadlock.
    ///
    /// Being blocked is not enough: a run where every thread reached its end and
    /// gave every lock back also has no enabled transition, and that is a normal
    /// termination.
    pub fn is_deadlock(&self, marking: &Marking) -> bool {
        marking
            .iter_nonzero()
            .any(|(place, _)| !self.is_resource(place) && !self.is_terminal(place))
    }
}
