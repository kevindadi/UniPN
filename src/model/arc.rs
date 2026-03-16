//! Arc types for the CVN.
//!
//! Arcs connect places to transitions (input arcs) or transitions to places (output arcs).

use crate::model::{BoolExpr, Expr, PlaceId, TransitionId};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Variable update mapping: variable name -> expression to evaluate.
pub type VarUpdate = IndexMap<String, Expr>;

/// Data attached to an input arc (Place → Transition).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputArcData {
    /// Source place.
    pub place: PlaceId,
    /// Target transition.
    pub transition: TransitionId,
    /// Number of tokens consumed (must be ≥ 1).
    pub weight: u32,
    /// Guard condition; transition fires only if guard does not evaluate to `false`.
    pub guard: BoolExpr,
}

/// Data attached to an output arc (Transition → Place).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputArcData {
    /// Source transition.
    pub transition: TransitionId,
    /// Target place.
    pub place: PlaceId,
    /// Number of tokens produced (must be ≥ 1).
    pub weight: u32,
    /// Optional variable updates applied when this arc fires.
    pub update: Option<VarUpdate>,
}
