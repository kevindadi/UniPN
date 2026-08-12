//! CVN (Concurrency Verification Net) kind annotations — ConcPlanVerify's
//! lowering target. These are the `PK`/`TK`/`AK` payloads for [`crate::net::Net`].

use serde::{Deserialize, Serialize};

/// Place kind (two classes: control flow / resource).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    Control(ControlSub),
    Resource(ResourceType),
}

/// Control-place structural sub-class (visualization/anchoring + the default
/// semantic predicates).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ControlSub {
    Statement,
    BasicBlock,
    FunctionStart,
    FunctionEnd,
    /// Function return point (ordinary control transfer, **not** thread end).
    Return,
    /// Thread-scope terminal (the return place of the entry / a spawned
    /// function, annotated by the frontend).
    ThreadEnd,
    CallWait,
    WaitPoint,
    Reacquire,
    SpawnBridge,
    TestPoint,
}

/// Resource type (determines the initial-token semantics).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Mutex,
    RwLock { max_readers: usize },
    Semaphore { count: usize },
    Channel,
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
