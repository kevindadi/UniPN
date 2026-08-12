//! The ordinary P/T net backend (ConcBugDect's MIR→PN lowering target).
//!
//! `PtNet` is [`Net`] instantiated with ConcBugDect's place/transition metadata
//! kinds and no arc kind (the `ArcDir` already distinguishes input/output/read/
//! inhibitor/reset). Weight and token counts are `usize`.

use serde::{Deserialize, Serialize};

use crate::analysis::NetLike;
use crate::ids::{PlaceId, TransitionId};
use crate::net::{ArcDir, Marking, Net};

/// Capacity overflow policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapacityMode {
    /// A transition that would overflow the place is not fireable.
    #[default]
    Reject,
    /// Overflow is clamped to the place's capacity.
    Saturate,
}

/// ConcBugDect place classification.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceType {
    Resource,
    FunctionStart,
    FunctionEnd,
    BasicBlock,
    Other(String),
}

/// A source location (file/line/column).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Atomic memory ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtomicOrdering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

/// An unsafe memory access.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnsafeOp {
    pub alias: u64,
    pub is_write: bool,
    pub span: Option<SourceLocation>,
    pub basic_block: usize,
    pub ty: String,
}

/// ConcBugDect transition classification.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionType {
    Start { thread: u64 },
    Goto,
    Switch,
    Return { thread: u64 },
    Lock { resource: u64 },
    Unlock { resource: u64 },
    RwLockRead { resource: u64 },
    RwLockWrite { resource: u64 },
    Wait { resource: u64 },
    Notify { resource: u64 },
    Spawn { thread: u64 },
    Join { thread: u64 },
    UnsafeRead(UnsafeOp),
    UnsafeWrite(UnsafeOp),
    UnsafeAccess(Vec<UnsafeOp>),
    AtomicLoad {
        alias: u64,
        ordering: AtomicOrdering,
        thread: u64,
    },
    AtomicStore {
        alias: u64,
        ordering: AtomicOrdering,
        thread: u64,
    },
    AtomicCmpXchg {
        alias: u64,
        success: AtomicOrdering,
        failure: AtomicOrdering,
        thread: u64,
    },
    Function,
    Normal,
    Inhibitor,
    Reset,
    Other(String),
}

/// ConcBugDect place attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtPlaceKind {
    pub place_type: PlaceType,
    pub span: Option<SourceLocation>,
    pub capacity: Option<usize>,
    pub capacity_mode: CapacityMode,
}

impl PtPlaceKind {
    pub fn new(place_type: PlaceType) -> Self {
        Self {
            place_type,
            span: None,
            capacity: None,
            capacity_mode: CapacityMode::Reject,
        }
    }
}

/// ConcBugDect transition attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtTransitionKind {
    pub transition_type: TransitionType,
    pub span: Option<SourceLocation>,
    pub priority: Option<u32>,
}

impl PtTransitionKind {
    pub fn new(transition_type: TransitionType) -> Self {
        Self {
            transition_type,
            span: None,
            priority: None,
        }
    }
}

/// The ordinary P/T net (no arc payload).
pub type PtNet = Net<PtPlaceKind, PtTransitionKind, ()>;

impl NetLike for PtNet {
    type State = Marking;

    fn num_places(&self) -> usize {
        self.places.len()
    }

    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    fn enabled(&self, state: &Self::State) -> Vec<TransitionId> {
        self.transitions
            .iter()
            .filter(|t| self.is_enabled(state, t.id))
            .map(|t| t.id)
            .collect()
    }

    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        if !self.is_enabled(state, transition) {
            return None;
        }
        let mut next = state.clone();

        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = next.tokens(arc.place);
            next.set(arc.place, current.checked_sub(arc.weight)?);
        }

        for arc in self.arcs_of(transition, ArcDir::Output) {
            let current = next.tokens(arc.place);
            let produced = current.checked_add(arc.weight)?;
            next.set(arc.place, self.apply_capacity(arc.place, produced)?);
        }

        for arc in self.arcs_of(transition, ArcDir::Reset) {
            next.set(arc.place, 0);
        }

        Some(next)
    }
}

impl PtNet {
    fn is_enabled(&self, state: &Marking, transition: TransitionId) -> bool {
        // Aggregate input-arc weights per place.
        let mut required: Vec<(PlaceId, usize)> = Vec::new();
        for arc in self.arcs_for(transition) {
            match arc.direction {
                ArcDir::Input => {
                    if let Some((_, total)) =
                        required.iter_mut().find(|(p, _)| *p == arc.place)
                    {
                        *total = total.checked_add(arc.weight).unwrap_or(usize::MAX);
                    } else {
                        required.push((arc.place, arc.weight));
                    }
                }
                ArcDir::Read => {
                    if state.tokens(arc.place) < arc.weight {
                        return false;
                    }
                }
                ArcDir::Inhibitor => {
                    if state.tokens(arc.place) >= arc.weight {
                        return false;
                    }
                }
                ArcDir::Output | ArcDir::Reset => {}
            }
        }
        required
            .into_iter()
            .all(|(place, count)| state.tokens(place) >= count)
    }

    fn apply_capacity(&self, place: PlaceId, tokens: usize) -> Option<usize> {
        let place = self.place(place)?;
        let Some(capacity) = place.kind.capacity else {
            return Some(tokens);
        };
        if tokens <= capacity {
            return Some(tokens);
        }
        match place.kind.capacity_mode {
            CapacityMode::Reject => None,
            CapacityMode::Saturate => Some(capacity),
        }
    }
}

/// Convenience: build a P/T net's initial marking from per-place token counts.
pub fn initial_marking<P>(places: &[P], tokens: impl Fn(usize) -> usize) -> Marking {
    Marking::new((0..places.len()).map(tokens).collect())
}

/// Convenience: build a marking directly from a slice of counts.
pub fn marking(counts: impl IntoIterator<Item = usize>) -> Marking {
    Marking::new(counts.into_iter().collect())
}
