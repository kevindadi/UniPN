//! Reserved time extension (feature `timed`).
//!
//! Goal: bridge to [PTPN](https://github.com/kevindadi/PTPN)'s priority timed
//! Petri nets, using state-class (DBM) reachability analysis to verify
//! time-related properties (WCET, schedulability, deadlines, real-time mutual
//! exclusion).
//!
//! Integration path: consistent with PTPN's own Romeo `.cts` / PToPNer `.ppn`
//! exports — the unified net is exported (via an export bridge) as PTPN's
//! `.ptpn` / TDG JSON, PTPN runs the state-class analysis, and the results
//! (DBM zones, scheduling states) come back. On the IR side only optional
//! annotations are added; the core firing semantics is untouched.

use serde::{Deserialize, Serialize};

/// Static time interval `[dmin, dmax]` (T-timed: a transition may only fire at
/// some instant within the interval after becoming enabled). The unit of `dmin`
/// is decided by the consumer (e.g. 1 = one time unit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StaticInterval {
    pub dmin: u64,
    pub dmax: u64,
}

/// Fixed priority (higher value = higher priority; for preemption/scheduling).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Priority(pub u32);

/// Clock class: groups several places/transitions under one clock (a clock
/// variable in state-class analysis).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClockClass {
    pub name: String,
    /// Id prefixes of the places/transitions belonging to this clock.
    pub members: Vec<String>,
}
