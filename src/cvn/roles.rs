//! How the CVN's kinds answer the shared [`PlaceRole`] / [`TransitionRole`]
//! questions.
//!
//! The place side is the definition the CVN deadlock check already used, now
//! stated once for both frontends. The transition side is where the CVN's finer
//! lowering shows: because it splits a condvar wait into
//! [`CondvarWaitEnter`](TransitionKind::CondvarWaitEnter) and
//! [`CondvarReacquire`](TransitionKind::CondvarReacquire), both halves get
//! classified, while P/T's single `Wait` transition can be classified as
//! neither.

use crate::net::{PlaceRole, TransitionRole};

use super::kinds::{ControlSub, CvnTransition, PlaceKind, TransitionKind};

impl PlaceRole for PlaceKind {
    fn is_resource(&self) -> bool {
        matches!(self, Self::Resource(_))
    }

    fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Control(ControlSub::ThreadEnd | ControlSub::FunctionEnd)
        )
    }
}

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
}
