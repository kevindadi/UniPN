//! CVN kind payloads plus the net and state aliases.
//!
//! [`PlaceKind`], [`CvnTransition`] and [`CvnArcKind`] are the `PK` / `TK` /
//! `AK` arguments of [`Net`] for ConcPlanVerify's lowering
//! target. Guards live on input arcs, variable updates on output arcs, and the
//! variable store is the state's `extra` payload.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::net::{Net, State};

use super::expr::{BoolExpr, Val, VarUpdate};

/// Place kind (two classes: control flow / resource).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    Control(ControlSub),
    Resource(ResourceType),
}

/// Control-place structural sub-class: one variant per role an analysis
/// distinguishes, and no more.
///
/// There is no `Statement` / `BasicBlock` split, because the two are the same
/// role at two granularities — ConcIR's flat statement list yields one control
/// point per statement, MIR one per basic block — and that is exactly the
/// precision difference between the CVN and the P/T net, not a difference in
/// what the place *is*.
///
/// There is no `ThreadEnd` either. In ConcIR a thread **is** a function:
/// `spawn`, `scope`, and `async_call` all target an ordinary function, and the
/// same function can be both called and spawned. Being a thread's terminal is
/// therefore a property of the call site, which a place kind cannot express.
///
/// There is no `CallWait` or `WaitPoint`. Both named the *operation* a token is
/// waiting on, and in both frontends that belongs to the transition:
/// `TransitionType::Wait` / `Function` on the P/T side,
/// [`CondvarWaitEnter`](TransitionKind::CondvarWaitEnter) / [`Call`](TransitionKind::Call)
/// here, with source attribution in a *field* (`PtPlaceKind::span`,
/// [`CvnTransition::anchors`]) rather than a variant. Whether a token is parked
/// on an event that may never arrive is still asked — as
/// [`Net::is_wait_point`](crate::net::Net::is_wait_point), derived from the
/// transitions that can carry the token away.
///
/// Lowering still creates whatever intermediate places it needs — a condvar
/// wait or reacquire point, a spawn bridge, a call's return point — those are
/// [`ControlSub::BasicBlock`] with a telling name rather than variants of their
/// own.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ControlSub {
    /// A point in the control flow: one ConcIR statement, or one MIR basic
    /// block.
    BasicBlock,
    FunctionStart,
    /// Where a function's control comes to rest, and so also where a spawned
    /// function's thread ends.
    FunctionEnd,
}

/// Resource type (determines the initial-token semantics and the capacity).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Mutex,
    RwLock {
        max_readers: usize,
    },
    Semaphore {
        count: usize,
    },
    /// A channel with `capacity` in-flight payload slots. `0` is a rendezvous:
    /// there is nowhere to put a message, so a send can only fire together with
    /// a receive.
    Channel {
        capacity: usize,
    },
    Condvar,
}

/// Unified transition classification (annotation; firing semantics is decided
/// by the arc structure).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionKind {
    Sequential,
    Goto,
    FunctionEnter,
    FunctionExit,
    Return,
    Drop,
    BranchTrue,
    BranchFalse,
    Switch { label: String },
    Lock,
    Unlock,
    ReadLock,
    ReadUnlock,
    Acquire,
    Release,
    Send,
    Recv,
    VarRead,
    VarWrite,
    AtomicLoad,
    AtomicStore,
    AtomicCmpXchg,
    CasSuccess,
    CasFailure,
    UnsafeRead,
    UnsafeWrite,
    UnsafeAccess,
    Spawn,
    Join,
    Call,
    CondvarWaitEnter,
    CondvarWakeByNotify,
    CondvarWakeByNotifyAll,
    CondvarReacquire,
    CondvarNotify,
    CondvarNotifyLost,
    CondvarNotifyAll,
    CondvarNotifyAllLost,
    TestBarrier,
    TestInject,
    TestPoint,
    Other(String),
}

/// The per-arc payload: guards (input arcs), updates and scope ends (output
/// arcs).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CvnArcKind {
    Plain,
    Guard(BoolExpr),
    Update(VarUpdate),
    /// Remove these variables from the store when the transition fires — a
    /// scope ending.
    ///
    /// This exists for the state space, not for the data model: a local left in
    /// the store after its scope ends keeps distinguishing states that are
    /// otherwise equal, so reachability would explore the same behavior many
    /// times over. `Update` cannot express it, since setting a variable to
    /// `Unknown` still leaves the key present.
    DropVars(Vec<String>),
}

/// A CVN transition: the kind annotation plus source-attribution metadata used
/// by the repair layer (scope = source function, anchors = ConcIR sids, family
/// = disjunctive OR group).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CvnTransition {
    pub kind: TransitionKind,
    pub scope: Option<String>,
    pub anchors: Vec<String>,
    pub family: Option<String>,
}

impl CvnTransition {
    /// A transition with no source attribution yet (the builder's `set_scope` /
    /// `set_anchor` / `set_family` fill it in).
    pub fn new(kind: TransitionKind) -> Self {
        Self {
            kind,
            scope: None,
            anchors: Vec::new(),
            family: None,
        }
    }
}

/// Ordered variable store.
pub type VarStore = BTreeMap<String, Val>;

/// The CVN state extra: the variable store plus bounded-Int domains (an update
/// leaving a domain disables the transition, keeping the state space finite).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CvnExtra {
    pub vars: VarStore,
    pub domains: BTreeMap<String, (i64, i64)>,
}

/// The CVN net.
pub type CvnNet = Net<PlaceKind, CvnTransition, CvnArcKind>;

/// The CVN state: marking + variable store + bounded-Int domains.
pub type CvnState = State<CvnExtra>;
