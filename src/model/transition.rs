//! Transition types for the CVN.

use serde::{Deserialize, Serialize};
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
    /// Acquire a lock.
    Lock,
    /// Release a lock.
    Unlock,
    /// Send on a channel.
    Send,
    /// Receive from a channel.
    Recv,
    /// Write to a variable.
    VarWrite,
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
    /// Condvar wait (releases mutex, blocks on condvar).
    CondvarWait,
    /// Condvar notify_one (wakes a specific wait site).
    CondvarNotify {
        /// The wait-place site that this notify targets.
        target_wait_place: String,
    },
    /// Condvar notify_all (wakes all waiters).
    CondvarNotifyAll,
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
    pub anchor_sids: SmallVec<[String; 2]>,
}

impl Transition {
    /// Create a new transition.
    pub fn new(
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

    /// Returns `true` if this is a return transition.
    pub fn is_return(&self) -> bool {
        matches!(self.kind, TransitionKind::Return)
    }
}
