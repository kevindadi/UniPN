use thiserror::Error;

use crate::core::model::ModelError;
use crate::ids::{PlaceId, TransitionId};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("transition is not enabled")]
    NotEnabled,
    #[error("token type does not match the place")]
    TypeMismatch,
    #[error("execution feature is unsupported by this semantics")]
    Unsupported,
    #[error("invalid model: {0}")]
    InvalidModel(#[from] ModelError),
    #[error("invalid P/T model: {0}")]
    InvalidPtModel(String),
    #[error("unknown transition {0}")]
    UnknownTransition(TransitionId),
    #[error("marking has {actual} places, expected {expected}")]
    InvalidMarkingLength { expected: usize, actual: usize },
    #[error("token count overflow at place {place}")]
    ArithmeticOverflow { place: PlaceId },
    #[error("token count underflow at place {place}")]
    ArithmeticUnderflow { place: PlaceId },
    #[error("capacity exceeded at place {place}")]
    CapacityExceeded { place: PlaceId },
}
