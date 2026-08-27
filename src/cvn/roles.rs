//! How the CVN's kinds answer the shared [`TransitionRole`] questions.
//!
//! Only the transition side is here. The place side is not the CVN's to answer
//! any more: [`PlaceKind`](super::kinds::PlaceKind) *is* the shared
//! [`PlaceClass`](crate::net::PlaceClass), so it inherits the one
//! [`PlaceRole`](crate::net::PlaceRole) implementation both frontends use.
//!
//! The transition side is where
//! the CVN's finer lowering shows: because it splits a condvar wait into
//! [`CondvarWaitEnter`](TransitionKind::CondvarWaitEnter) and
//! [`CondvarReacquire`](TransitionKind::CondvarReacquire), both halves get
//! classified, while P/T's single `Wait` transition can be classified as
//! neither.

use crate::net::TransitionRole;

use super::kinds::{CvnTransition, TransitionKind};

impl TransitionRole for CvnTransition {
    /// A channel `Recv` and a condvar reacquire are acquisitions too: both take
    /// a token off a resource place.
    fn is_acquire(&self) -> bool {
        matches!(
            self.kind,
            TransitionKind::Lock
                | TransitionKind::ReadLock
                | TransitionKind::Acquire
                | TransitionKind::Recv
                | TransitionKind::CondvarReacquire
        )
    }

    /// `CondvarWaitEnter` counts: entering a wait is where the lock goes back.
    fn is_release(&self) -> bool {
        matches!(
            self.kind,
            TransitionKind::Unlock
                | TransitionKind::ReadUnlock
                | TransitionKind::Release
                | TransitionKind::Send
                | TransitionKind::CondvarWaitEnter
        )
    }

    fn is_thread_spawn(&self) -> bool {
        matches!(self.kind, TransitionKind::Spawn)
    }

    fn is_thread_join(&self) -> bool {
        matches!(self.kind, TransitionKind::Join)
    }

    fn is_atomic(&self) -> bool {
        matches!(
            self.kind,
            TransitionKind::AtomicLoad
                | TransitionKind::AtomicStore
                | TransitionKind::AtomicCmpXchg
                | TransitionKind::CasSuccess
                | TransitionKind::CasFailure
        )
    }

    fn is_unsafe_access(&self) -> bool {
        matches!(
            self.kind,
            TransitionKind::UnsafeRead | TransitionKind::UnsafeWrite | TransitionKind::UnsafeAccess
        )
    }

    /// The two wake variants are what a parked waiter is waiting for; `Recv`
    /// waits for a message and `Join` for another thread to finish.
    ///
    /// `CondvarReacquire` is deliberately absent: by then the notification has
    /// arrived and the thread is queueing for the lock, which is the acquire
    /// side. That is exactly the split that lets the CVN say "the notification
    /// was lost" instead of "something is stuck".
    fn is_blocking_wait(&self) -> bool {
        matches!(
            self.kind,
            TransitionKind::CondvarWakeByNotify
                | TransitionKind::CondvarWakeByNotifyAll
                | TransitionKind::Recv
                | TransitionKind::Join
        )
    }
}
