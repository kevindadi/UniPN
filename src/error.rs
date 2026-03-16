//! Unified error types for CVN validation and analysis.
//!
//! Error codes use the `V` prefix (to distinguish from CIR's `E` prefix):
//! - V0xx: structural errors
//! - V1xx: well-formedness violations
//! - V2xx: branch completeness violations
//! - V3xx: analysis-phase errors
//! - V4xx: resource semantics errors

use crate::model::{PlaceId, TransitionId};
use std::fmt;
use thiserror::Error;

/// Error code identifying a specific CVN validation or analysis error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ErrorCode {
    // V0xx — structural errors
    /// Duplicate place ID.
    V001,
    /// Duplicate transition ID.
    V002,
    /// Arc references a non-existent place.
    V003,
    /// Arc references a non-existent transition.
    V004,
    /// Input arc weight is zero.
    V005,
    /// Output arc weight is zero.
    V006,
    /// Initial token references a non-existent place.
    V007,
    /// Initial variable references a non-existent variable name.
    V008,

    // V1xx — well-formedness violations
    /// Transition has no control input arc (violates W2).
    V101,
    /// Transition has multiple control input arcs (violates W2).
    V102,
    /// Non-return transition has no control output arc (violates W3).
    V103,
    /// Same transition has conflicting variable updates (violates W4).
    V104,
    /// Transition has no anchor SID (violates W7).
    #[cfg(feature = "cir-anchor")]
    V105,

    // V2xx — branch completeness
    /// Branch transition is unpaired (violates W8).
    V201,
    /// Branch pair guards are not complementary (violates W8).
    V202,
    /// Switch transitions do not cover all enum variants (violates W9).
    V203,

    // V3xx — analysis-phase errors
    /// Insufficient tokens when firing (runtime error).
    V301,
    /// State space explosion (exceeded configured limit).
    V302,
    /// Expression type error during evaluation.
    V303,

    // V4xx — resource semantics errors
    /// Resource place initial tokens don't match declared type.
    V401,
    /// RwLock N value is less than 1.
    V402,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::V001 => "V001",
            Self::V002 => "V002",
            Self::V003 => "V003",
            Self::V004 => "V004",
            Self::V005 => "V005",
            Self::V006 => "V006",
            Self::V007 => "V007",
            Self::V008 => "V008",
            Self::V101 => "V101",
            Self::V102 => "V102",
            Self::V103 => "V103",
            Self::V104 => "V104",
            #[cfg(feature = "cir-anchor")]
            Self::V105 => "V105",
            Self::V201 => "V201",
            Self::V202 => "V202",
            Self::V203 => "V203",
            Self::V301 => "V301",
            Self::V302 => "V302",
            Self::V303 => "V303",
            Self::V401 => "V401",
            Self::V402 => "V402",
        };
        write!(f, "{code}")
    }
}

/// Locator pinpointing the source of an error within the CVN.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ErrorLocation {
    /// Error relates to a specific place.
    Place(PlaceId),
    /// Error relates to a specific transition.
    Transition(TransitionId),
    /// Error relates to a specific variable.
    Variable(String),
    /// Error relates to an arc between a place and a transition.
    Arc {
        /// The place end of the arc.
        place: PlaceId,
        /// The transition end of the arc.
        transition: TransitionId,
    },
    /// Error relates to a pair of transitions (e.g. branch pairs).
    TransitionPair(TransitionId, TransitionId),
    /// No specific location.
    None,
}

/// A CVN validation or analysis error.
#[derive(Debug, Clone, Error, serde::Serialize, serde::Deserialize)]
#[error("[{code}] {message}")]
pub struct CvnError {
    /// Machine-readable error code.
    pub code: ErrorCode,
    /// Human-readable description of the error.
    pub message: String,
    /// Location within the CVN where the error was detected.
    pub location: ErrorLocation,
}

impl CvnError {
    /// Create a new CVN error.
    pub fn new(code: ErrorCode, message: impl Into<String>, location: ErrorLocation) -> Self {
        Self {
            code,
            message: message.into(),
            location,
        }
    }
}
