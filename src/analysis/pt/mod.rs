//! Core P/T net analysis (ConcBugDect's `analysis/` module): reachability-graph
//! construction and boundness via the coverability tree.

pub mod boundness;
pub mod reachability;

pub use boundness::{BoundnessAnalyzer, BoundnessResult, check_boundness, check_place_boundness};
pub use reachability::{
    ArcSnapshot, StateEdge, StateGraph, StateGraphConfig, StateGraphStats, StateNode,
    StatePlaceSnapshot, TokenChange, TransitionFailure, TransitionSummary,
};
