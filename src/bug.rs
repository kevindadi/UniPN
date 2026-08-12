use serde::{Deserialize, Serialize};

use crate::ids::{PlaceId, TransitionId};
use crate::pt::{PtNet, PtPlace, PtTransition};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AliasId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThreadId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

impl SourceLocation {
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnsafeOp {
    pub alias: AliasId,
    pub is_write: bool,
    pub span: Option<SourceLocation>,
    pub basic_block: usize,
    pub ty: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceType {
    Resource,
    FunctionStart,
    FunctionEnd,
    BasicBlock,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtomicOrdering {
    Relaxed,
    Acquire,
    Release,
    AcqRel,
    SeqCst,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionType {
    Start {
        thread: ThreadId,
    },
    Goto,
    Switch,
    Return {
        thread: ThreadId,
    },
    Lock {
        resource: ResourceId,
    },
    Unlock {
        resource: ResourceId,
    },
    RwLockRead {
        resource: ResourceId,
    },
    RwLockWrite {
        resource: ResourceId,
    },
    Wait {
        resource: ResourceId,
    },
    Notify {
        resource: ResourceId,
    },
    Spawn {
        thread: ThreadId,
    },
    Join {
        thread: ThreadId,
    },
    UnsafeRead(UnsafeOp),
    UnsafeWrite(UnsafeOp),
    UnsafeAccess(Vec<UnsafeOp>),
    AtomicLoad {
        alias: AliasId,
        ordering: AtomicOrdering,
        thread: ThreadId,
    },
    AtomicStore {
        alias: AliasId,
        ordering: AtomicOrdering,
        thread: ThreadId,
    },
    AtomicCmpXchg {
        alias: AliasId,
        success: AtomicOrdering,
        failure: AtomicOrdering,
        thread: ThreadId,
    },
    Function,
    Normal,
    Inhibitor,
    Reset,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaceMetadata {
    pub place_type: PlaceType,
    pub span: Option<SourceLocation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransitionMetadata {
    pub transition_type: TransitionType,
    pub span: Option<SourceLocation>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BugNet {
    pub net: PtNet,
    pub places: Vec<PlaceMetadata>,
    pub transitions: Vec<TransitionMetadata>,
}

impl BugNet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_place(&mut self, place: PtPlace, metadata: PlaceMetadata) -> PlaceId {
        let id = self.net.add_place(place);
        assert_eq!(id.index(), self.places.len());
        self.places.push(metadata);
        id
    }

    pub fn add_transition(
        &mut self,
        transition: PtTransition,
        metadata: TransitionMetadata,
    ) -> TransitionId {
        let id = self.net.add_transition(transition);
        assert_eq!(id.index(), self.transitions.len());
        self.transitions.push(metadata);
        id
    }

    pub fn place_metadata(&self, place: PlaceId) -> Option<&PlaceMetadata> {
        self.places.get(place.index())
    }

    pub fn transition_metadata(&self, transition: TransitionId) -> Option<&TransitionMetadata> {
        self.transitions.get(transition.index())
    }

    pub fn validate(&self) -> Result<(), crate::pt::PtModelError> {
        self.net.validate()?;
        if self.places.len() != self.net.places.len()
            || self.transitions.len() != self.net.transitions.len()
        {
            return Err(crate::pt::PtModelError::MetadataLengthMismatch);
        }
        Ok(())
    }
}

pub mod prelude {
    pub use super::{
        AliasId, AtomicOrdering, BugNet, PlaceMetadata, PlaceType, ResourceId, SourceLocation,
        ThreadId, TransitionMetadata, TransitionType, UnsafeOp,
    };
}
