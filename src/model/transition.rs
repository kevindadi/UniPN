//! Transition types for the CVN.

use serde::{Deserialize, Serialize};
#[cfg(feature = "cir-anchor")]
use smallvec::SmallVec;
use std::fmt;

/// Unique identifier for a transition in the CVN.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransitionId(pub String);

impl TransitionId {
    /// Create a new transition ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for TransitionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<S: Into<String>> From<S> for TransitionId {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

/// Classification of a transition (for debugging/visualization only; does not affect semantics).
///
/// All transition behavior is determined by its connected arcs' weight/guard/update.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TransitionKind {
    /// A sequential (non-synchronizing) step.
    Sequential,
    /// Acquire a Mutex lock or RwLock write-lock.
    Lock,
    /// Release a Mutex lock or RwLock write-lock.
    Unlock,
    /// Acquire an RwLock read-lock.
    ReadLock,
    /// Release an RwLock read-lock.
    ReadUnlock,
    /// Acquire a Semaphore permit.
    Acquire,
    /// Release a Semaphore permit.
    Release,
    /// Send on a channel.
    Send,
    /// Receive from a channel.
    Recv,
    /// Read a variable (Var.read or Atomic.load followed by next).
    VarRead,
    /// Write to a variable.
    VarWrite,
    /// Atomic load.
    AtomicLoad,
    /// Store to an atomic variable.
    AtomicStore,
    /// Branch taken when guard is true.
    BranchTrue,
    /// Branch taken when guard is false.
    BranchFalse,
    /// Switch on a specific label.
    Switch {
        /// The matched label/variant.
        label: String,
    },
    /// Compare-and-swap success.
    CasSuccess,
    /// Compare-and-swap failure.
    CasFailure,
    /// Spawn a new concurrent entity.
    Spawn,
    /// Join (wait for) a concurrent entity.
    Join,
    /// Function call.
    Call,
    /// Condvar wait enter: releases mutex, moves to wait place, increments nw.
    CondvarWaitEnter,
    /// Condvar wake by notify_one: consumes rp(cv) token, moves to reacquire place.
    CondvarWakeByNotify,
    /// Condvar wake by notify_all: guarded by na flag, moves to reacquire place.
    CondvarWakeByNotifyAll,
    /// Re-acquire mutex after condvar wake-up.
    CondvarReacquire,
    /// Condvar notify_one: produces rp(cv) token when nw > 0.
    CondvarNotify,
    /// Condvar notify_one lost: fires when nw == 0 (signal loss detection point).
    CondvarNotifyLost,
    /// Condvar notify_all: sets all na flags when nw > 0.
    CondvarNotifyAll,
    /// Condvar notify_all lost: fires when nw == 0 (signal loss detection point).
    CondvarNotifyAllLost,
    /// Return from a function (terminal transition).
    Return,
}

/// A transition in the CVN.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    /// Unique identifier for this transition.
    pub id: TransitionId,
    /// Classification of this transition.
    pub kind: TransitionKind,
    /// Anchor mapping μ(t): SIDs from the CIR that this transition corresponds to.
    /// Only available with the `cir-anchor` feature.
    #[cfg(feature = "cir-anchor")]
    #[serde(default, skip_serializing_if = "SmallVec::is_empty")]
    pub anchor_sids: SmallVec<[String; 2]>,
}

impl Transition {
    /// Create a new transition without anchor information.
    pub fn new(id: impl Into<TransitionId>, kind: TransitionKind) -> Self {
        Self {
            id: id.into(),
            kind,
            #[cfg(feature = "cir-anchor")]
            anchor_sids: SmallVec::new(),
        }
    }

    /// Create a new transition with CIR statement ID anchors.
    #[cfg(feature = "cir-anchor")]
    pub fn with_anchor(
        id: impl Into<TransitionId>,
        kind: TransitionKind,
        sids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            anchor_sids: sids.into_iter().map(Into::into).collect(),
        }
    }

    /// Returns the CIR statement ID anchors for this transition.
    #[cfg(feature = "cir-anchor")]
    pub fn anchor_sids(&self) -> &SmallVec<[String; 2]> {
        &self.anchor_sids
    }

    /// Returns `true` if this is a return transition.
    pub fn is_return(&self) -> bool {
        matches!(self.kind, TransitionKind::Return)
    }
}
