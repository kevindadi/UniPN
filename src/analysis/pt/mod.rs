//! Core P/T net analysis (ConcBugDect's `analysis/` module): reachability-graph
//! construction, boundness via the coverability tree, and net reduction.

pub mod boundness;
pub mod reachability;
pub mod reduce;

pub use boundness::{BoundnessAnalyzer, BoundnessResult, check_boundness, check_place_boundness};
pub use reachability::{
    ArcSnapshot, StateEdge, StateGraph, StateGraphConfig, StateGraphStats, StateNode,
    StatePlaceSnapshot, TokenChange, TransitionFailure, TransitionSummary,
};
pub use reduce::{
    Reducer, ReductionError, ReductionOptions, ReductionResult, ReductionStageNets, ReductionStep,
    ReductionTrace, reduce,
};
