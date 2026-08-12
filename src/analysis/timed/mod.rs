//! State-class (DBM) reachability analysis for priority timed Petri nets.
//!
//! Ported from PTPN's `src/analysis/` — the core timed-net analysis: DBM clock
//! zones, symbolic state classes, canonicalization, scheduling (priority/
//! suspension), and the state-class reachability graph.

// The port keeps PTPN's index-computation style (`0 * n + i`, dense loops,
// `Default` + field assignment) verbatim for auditable parity; these lints are
// stylistic only.
#![allow(
    clippy::erasing_op,
    clippy::identity_op,
    clippy::needless_range_loop,
    clippy::field_reassign_with_default,
    clippy::inherent_to_string
)]

pub mod canonicalization;
pub mod dbm;
pub mod reachability;
pub mod scheduling;
pub mod state_class;

pub use canonicalization::{CanonicalizationMode, can_merge_into, check_equality, check_inclusion};
pub use dbm::{DBM, INF_TIME};
pub use reachability::{
    StateClassGraph, StateClassReachabilityGraph, Statistics, TimedReachabilityConfig,
    format_marking_of, out_edge_transitions, reachable_markings,
};
pub use scheduling::Scheduling;
pub use state_class::{
    ClockKind, ClockVar, FiringEdge, StateClass, TransitionSet, contains, hash_combine,
    hash_state_class,
};
