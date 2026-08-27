//! Core CVN analysis (ConcPlanVerify's checks): deadlock classification and
//! dead-transition detection, both reported as counterexamples with ConcIR
//! anchors.
//!
//! These consume a [`ReachabilityGraph`](crate::analysis::ReachabilityGraph)
//! produced by [`explore`](crate::analysis::explore) over a
//! [`CvnNet`](crate::cvn::CvnNet). ConcPlanVerify keeps only its translator,
//! repair, and goal checking (which depend on ConcIR).
//!
//! Conflict sets moved to [`analysis::conflict_sets`](crate::analysis::conflict_sets):
//! sharing an input place is a structural property, and ConcBugDect's race
//! candidates need the same answer.

pub mod deadlock;

pub use deadlock::{blocked_places, find_dead_transitions, find_deadlocks, is_deadlock};
