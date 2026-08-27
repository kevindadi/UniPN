//! ConcBugDect place/transition kind payloads plus the [`PtNet`] alias.
//!
//! The kinds mirror ConcBugDect's `net/structure.rs`, with the rustc-private
//! `AliasId` decoupled to plain integers. There is no arc kind:
//! [`ArcDir`](crate::net::ArcDir) already distinguishes
//! input/output/read/inhibitor/reset.

use serde::{Deserialize, Deserializer, Serialize};

use crate::net::{ControlSub, Net, PlaceClass};

/// ConcBugDect's place classification: the shared [`PlaceClass`] with an empty
/// resource arm.
///
/// The arm carries nothing because P/T keeps a resource's *identity* on the
/// transition that touches it (`TransitionType::Lock(alias)`) and its bound in
/// [`PtPlaceKind::capacity`], where the CVN instead derives both from a
/// `ResourceType`. The control arm is shared verbatim — the two frontends had
/// converged on the same three roles, and now the type says so.
pub type PlaceType = PlaceClass<()>;

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
    #[serde(deserialize_with = "place_type_accepting_legacy")]
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

    /// A control-flow place. Shorter than spelling the class out, and it keeps
    /// `PlaceClass::Resource(())` from appearing at call sites.
    pub fn control(sub: ControlSub) -> Self {
        Self::new(PlaceClass::Control(sub))
    }

    /// A shared-resource place.
    pub fn resource() -> Self {
        Self::new(PlaceType::RESOURCE)
    }
}

/// Read the current `{"Control": "BasicBlock"}` shape *and* the flat
/// `"BasicBlock"` / `"Resources"` strings ConcBugDect wrote when `PlaceType` was
/// its own four-variant enum, so nets serialized before the two frontends' place
/// classifications were unified still load.
///
/// Only reading is compatible: serializing now writes the nested shape. The
/// legacy names are frontend-specific — `"Resources"`, plural — which is why
/// this sits here rather than on the shared [`PlaceClass`].
fn place_type_accepting_legacy<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<PlaceType, D::Error> {
    #[derive(Deserialize)]
    enum Legacy {
        Resources,
        FunctionStart,
        FunctionEnd,
        BasicBlock,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Either {
        // A bare string matches this arm; the nested map cannot, since `Legacy`
        // has no `Control` or `Resource` variant.
        Flat(Legacy),
        Current(PlaceType),
    }

    Ok(match Either::deserialize(deserializer)? {
        Either::Flat(Legacy::Resources) => PlaceType::RESOURCE,
        Either::Flat(Legacy::BasicBlock) => PlaceClass::Control(ControlSub::BasicBlock),
        Either::Flat(Legacy::FunctionStart) => PlaceClass::Control(ControlSub::FunctionStart),
        Either::Flat(Legacy::FunctionEnd) => PlaceClass::Control(ControlSub::FunctionEnd),
        Either::Current(class) => class,
    })
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
