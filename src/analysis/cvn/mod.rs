//! Core CVN analysis (ConcPlanVerify's checks): deadlock classification,
//! dead-transition detection, and structural conflict sets.
//!
//! These consume a [`ReachabilityGraph`](crate::analysis::ReachabilityGraph)
//! produced by [`explore`](crate::analysis::explore) over a
//! [`CvnNet`](crate::cvn::CvnNet). ConcPlanVerify keeps only its translator,
//! repair, and goal checking (which depend on ConcIR).

pub mod deadlock;

pub use deadlock::{
    blocked_places, conflict_sets, find_dead_transitions, find_deadlocks, is_deadlock,
};
