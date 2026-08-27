//! The one place classification the two control-flow frontends share.
//!
//! P/T and the CVN lower the same thing at two precisions — a control-flow
//! skeleton over shared resources — and they had arrived at the *same* set of
//! control-place roles independently: `BasicBlock` / `FunctionStart` /
//! `FunctionEnd`, with "shared resource" as the other side of the dichotomy.
//! Keeping that as two enums meant the agreement was a claim in the docs that
//! nothing checked; here it is the type, so it cannot drift.
//!
//! Where they genuinely differ is the resource arm, and that is what `R` is for.
//! The CVN's resource *type* decides the place's capacity ([`PlaceCapacity`] is
//! implemented on `PlaceClass<ResourceType>`), because in ConcIR the resource
//! identity already is the place identity. ConcBugDect's resource identity lives
//! on the transition instead (`Lock(alias)`, `RwLockRead(alias)`) and its
//! capacity is a separate field, so its resource arm carries nothing: `R = ()`.
//!
//! This is deliberately a shared *enum*, unlike
//! [`TransitionRole`](crate::net::TransitionRole), which is a set of predicates
//! precisely because the two frontends' transition variants disagree on shape.
//! The control variants carry no payload in either frontend and mean the same
//! thing in both, so the argument that blocks merging transitions does not reach
//! them — and `R` keeps the one arm that does differ in the frontend's hands.
//!
//! [`PlaceCapacity`]: crate::net::PlaceCapacity

use serde::{Deserialize, Serialize};

use crate::net::PlaceRole;

/// Which role a control-flow place plays: one variant per distinction some
/// analysis makes, and no more.
///
/// There is no `Statement` / `BasicBlock` split. ConcIR's flat statement list
/// gives one control point per statement and MIR one per basic block, but that
/// difference *is* the precision tier, not a difference in what the place is.
///
/// There is no `ThreadEnd`. In ConcIR a thread **is** a function — `spawn`,
/// `scope`, and `async_call` all target an ordinary function, and the same
/// function can be both called and spawned — so being a thread's terminal is a
/// property of the call site, which a place kind cannot express.
///
/// There is no `WaitPoint` or `CallWait`. Both named the *operation* a token was
/// waiting on, and both frontends already keep that on the transition; the
/// question they existed to answer is
/// [`Net::is_wait_point`](crate::net::Net::is_wait_point), derived from the
/// transitions that can carry the token away.
///
/// A lowering still creates whatever intermediate places it needs — a condvar
/// wait or reacquire point, a spawn bridge, a call's return point — and those
/// are [`BasicBlock`](ControlSub::BasicBlock) with a telling name. Adding a
/// variant means naming the analysis that would branch on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ControlSub {
    /// A point in the control flow: one ConcIR statement, or one MIR basic
    /// block.
    BasicBlock,
    FunctionStart,
    /// Where a function's control comes to rest, and so also where a spawned
    /// function's thread ends.
    FunctionEnd,
}

/// A place is either a point in some thread's control flow or a shared
/// resource — the dichotomy every analysis in the crate branches on.
///
/// `R` is the frontend's resource payload: [`ResourceType`] for the CVN, `()`
/// for P/T. Deriving `Ord` is conditional on `R`, so `PlaceClass<()>` is
/// orderable whether or not the CVN's payload is.
///
/// [`ResourceType`]: crate::cvn::ResourceType
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PlaceClass<R> {
    Control(ControlSub),
    Resource(R),
}

impl<R> PlaceClass<R> {
    /// The control sub-role, or `None` for a resource place.
    pub fn control(&self) -> Option<ControlSub> {
        match self {
            Self::Control(sub) => Some(*sub),
            Self::Resource(_) => None,
        }
    }

    /// The resource payload, or `None` for a control place.
    pub fn resource(&self) -> Option<&R> {
        match self {
            Self::Resource(r) => Some(r),
            Self::Control(_) => None,
        }
    }
}

impl PlaceClass<()> {
    /// The payload-free resource place, spelled without the `Resource(())`
    /// noise.
    pub const RESOURCE: Self = Self::Resource(());
}

/// One definition for both frontends: a resource is the `Resource` arm, and the
/// only place a thread legitimately comes to rest is a function's end.
impl<R> PlaceRole for PlaceClass<R> {
    fn is_resource(&self) -> bool {
        matches!(self, Self::Resource(_))
    }

    fn is_terminal(&self) -> bool {
        matches!(self, Self::Control(ControlSub::FunctionEnd))
    }
}
