//! How ConcBugDect's kinds answer the shared [`PlaceRole`] / [`TransitionRole`]
//! questions.
//!
//! [`PlaceType`] already carries the control/resource split the analyses need,
//! it was simply never exposed to them: before this, P/T reachability treated
//! every blocked state as a deadlock, including a run where all threads reached
//! `FunctionEnd` and returned every lock.

use crate::net::{PlaceRole, TransitionRole};

use super::kinds::{PlaceType, PtPlaceKind, PtTransitionKind, TransitionType};

impl PlaceRole for PtPlaceKind {
    fn is_resource(&self) -> bool {
        self.place_type == PlaceType::Resources
    }

    fn is_terminal(&self) -> bool {
        self.place_type == PlaceType::FunctionEnd
    }
}

impl TransitionRole for PtTransitionKind {
    fn is_acquire(&self) -> bool {
        matches!(
            self.transition_type,
            TransitionType::Lock(_)
                | TransitionType::RwLockRead(_)
                | TransitionType::RwLockWrite(_)
        )
    }

    /// `Drop*` counts: dropping a guard is how a Rust lock is released.
    fn is_release(&self) -> bool {
        matches!(
            self.transition_type,
            TransitionType::Unlock(_) | TransitionType::DropRead(_) | TransitionType::DropWrite(_)
        )
    }

    fn is_thread_spawn(&self) -> bool {
        matches!(self.transition_type, TransitionType::Spawn(_))
    }

    fn is_thread_join(&self) -> bool {
        matches!(self.transition_type, TransitionType::Join(_))
    }

    fn is_atomic(&self) -> bool {
        matches!(
            self.transition_type,
            TransitionType::AtomicLoad(..)
                | TransitionType::AtomicStore(..)
                | TransitionType::AtomicCmpXchg(..)
        )
    }

    fn is_unsafe_access(&self) -> bool {
        matches!(
            self.transition_type,
            TransitionType::UnsafeRead(..)
                | TransitionType::UnsafeWrite(..)
                | TransitionType::UnsafeAccess(_)
        )
    }

    /// `Wait` is a single transition that drops the lock, waits, and takes it
    /// back, so the place *before* it is where a thread parks for a
    /// notification — which is what [`Net::is_wait_point`](crate::net::Net::is_wait_point)
    /// asks about, and why the question is consumer-side.
    fn is_blocking_wait(&self) -> bool {
        matches!(
            self.transition_type,
            TransitionType::Wait | TransitionType::Join(_)
        )
    }
}
