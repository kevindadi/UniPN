//! Place/transition kind annotations.
//!
//! `kind` is only an annotation and does not participate in firing semantics —
//! engines always treat every element as an ordinary P/T element. Semantics such
//! as "thread terminal / wait point / resource" are exposed through
//! [`crate::netlike::NetLike`] predicates.

use serde::{Deserialize, Serialize};

use crate::ids::Weight;

/// Place kind (two classes: control flow / resource).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    /// Control-flow place: a token means a thread instance is at a control point.
    Control(ControlSub),
    /// Resource place: the token count is the number of available units
    /// (initial marking is supplied by the frontend).
    Resource(ResourceType),
}

/// Control-place structural sub-class (only for visualization/anchoring and the
/// default semantic predicates).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ControlSub {
    /// A single ConcIR statement.
    Statement,
    /// A MIR basic block.
    BasicBlock,
    FunctionStart,
    FunctionEnd,
    /// Function return point (ordinary control transfer, **not** thread end).
    Return,
    /// Thread-scope terminal (the return place of the entry / a spawned
    /// function, annotated by the frontend).
    ThreadEnd,
    /// Synchronous call parking place.
    CallWait,
    /// Condvar wait point (a control-flow annotation used for signal-loss
    /// classification).
    WaitPoint,
    /// Condvar re-acquire place.
    Reacquire,
    /// Spawn skeleton bridge.
    SpawnBridge,
    /// Test orchestration point.
    TestPoint,
}

/// Resource type (determines the initial-token semantics).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    /// Mutex (initial tokens = 1).
    Mutex,
    /// Reader-writer lock (initial tokens = N = number of concurrent entities).
    RwLock { max_readers: u32 },
    /// Counting semaphore (initial tokens = count).
    Semaphore { count: u32 },
    /// Channel (initial tokens = 0).
    Channel,
    /// Condition variable (used with `WaitPoint` control places).
    Condvar,
}

/// Unified transition classification (annotation; firing semantics is decided
/// by the arc structure).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionKind {
    // ── Sequential / control ──
    Sequential,
    Goto,
    FunctionEnter,
    FunctionExit,
    /// Function return (ordinary control transfer, not thread end).
    Return,
    Drop,
    BranchTrue,
    BranchFalse,
    Switch { label: String },
    // ── Synchronization resources ──
    /// Mutex / RwLock write lock.
    Lock,
    Unlock,
    ReadLock,
    ReadUnlock,
    /// Semaphore acquire/release.
    Acquire,
    Release,
    /// Channel send/receive.
    Send,
    Recv,
    // ── Data ──
    VarRead,
    VarWrite,
    AtomicLoad,
    AtomicStore,
    AtomicCmpXchg,
    CasSuccess,
    CasFailure,
    /// MIR unsafe access (used by the data-race detector).
    UnsafeRead,
    UnsafeWrite,
    UnsafeAccess,
    // ── Threads ──
    Spawn,
    Join,
    Call,
    // ── Condvar ──
    CondvarWaitEnter,
    CondvarWakeByNotify,
    CondvarWakeByNotifyAll,
    CondvarReacquire,
    CondvarNotify,
    CondvarNotifyLost,
    CondvarNotifyAll,
    CondvarNotifyAllLost,
    // ── Test orchestration ──
    TestBarrier,
    TestInject,
    TestPoint,
    // ── Fallback ──
    Other(String),
}

/// A place. `capacity` is optional (ConcBugDect has it, CVN does not).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    pub id: crate::ids::PlaceId,
    pub name: String,
    pub kind: PlaceKind,
    pub capacity: Option<Weight>,
}

/// A transition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transition {
    pub id: crate::ids::TransitionId,
    pub name: String,
    pub kind: TransitionKind,
    /// Source scope (function/def) that produced this transition, for
    /// attribution (e.g. the ConcIR function or MIR def path).
    pub scope: Option<String>,
    /// Anchoring back to the source (ConcIR sid or source line).
    pub anchors: Vec<String>,
    /// Disjunctive OR family (mutually-exclusive variants, to avoid false
    /// dead-transition reports).
    pub family: Option<String>,
    /// Time extension (feature `timed`): static delay interval.
    #[cfg(feature = "timed")]
    pub timing: Option<crate::timed::StaticInterval>,
    /// Time extension (feature `timed`): fixed priority.
    #[cfg(feature = "timed")]
    pub priority: Option<crate::timed::Priority>,
}
