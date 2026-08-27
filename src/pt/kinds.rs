//! ConcBugDect place/transition kind payloads plus the [`PtNet`] alias.
//!
//! The kinds mirror ConcBugDect's `net/structure.rs`, with the rustc-private
//! `AliasId` decoupled to plain integers. There is no arc kind:
//! [`ArcDir`](crate::net::ArcDir) already distinguishes
//! input/output/read/inhibitor/reset.

use serde::{Deserialize, Serialize};

use crate::net::Net;

/// ConcBugDect place classification.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PlaceType {
    Resources,
    FunctionStart,
    FunctionEnd,
    BasicBlock,
}

/// Atomic memory ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtomicOrdering {
    Relaxed,
    Release,
    Acquire,
    AcqRel,
    SeqCst,
}

/// A decoupled pointer-analysis alias identifier (ConcBugDect's rustc-private
/// `AliasId`, reduced to plain integers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AliasId {
    pub instance_id: usize,
    pub local: usize,
    pub array_index: Option<u64>,
    pub field: Option<u32>,
}

/// An unsafe memory access.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnsafeOp {
    /// Unsafe alias-group id.
    pub alias: usize,
    pub is_write: bool,
    pub span: String,
    pub basic_block: usize,
    pub ty: String,
}

/// ConcBugDect transition classification.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionType {
    Start(usize),
    Goto,
    Switch,
    Return(usize),
    Unlock(usize),
    DropRead(usize),
    DropWrite(usize),
    Drop,
    Assert,

    UnsafeRead(usize, String, usize, String),
    UnsafeWrite(usize, String, usize, String),
    /// One merged transition per basic block summarizing every unsafe access.
    UnsafeAccess(Vec<UnsafeOp>),

    Lock(usize),
    RwLockRead(usize),
    RwLockWrite(usize),
    Notify(usize),
    Wait,

    AtomicLoad(AliasId, AtomicOrdering, String, usize),
    AtomicStore(AliasId, AtomicOrdering, String, usize),
    AtomicCmpXchg(AliasId, AtomicOrdering, AtomicOrdering, String, usize),
    Spawn(String),
    Join(String),

    Function,
    Normal,
    Inhibitor,
    Reset,
}

/// ConcBugDect place attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtPlaceKind {
    pub place_type: PlaceType,
    pub span: String,
    /// `None` = unbounded.
    pub capacity: Option<usize>,
}

impl PtPlaceKind {
    pub fn new(place_type: PlaceType) -> Self {
        Self {
            place_type,
            span: String::new(),
            capacity: None,
        }
    }
}

/// ConcBugDect transition attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtTransitionKind {
    pub transition_type: TransitionType,
}

impl PtTransitionKind {
    pub fn new(transition_type: TransitionType) -> Self {
        Self { transition_type }
    }
}

/// The ordinary P/T net (no arc payload).
pub type PtNet = Net<PtPlaceKind, PtTransitionKind, ()>;
