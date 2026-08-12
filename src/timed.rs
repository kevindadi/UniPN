//! Priority Timed Petri Net model (PTPN's lowering target).
//!
//! `TimedNet` is [`Net`] instantiated with PTPN's place/transition kind
//! payloads and no arc kind. Time is an *annotation* on transitions; the
//! state-class (DBM) firing semantics is reserved — see `crate::analysis`
//! for the untimed reachability and PTPN itself for the timed analysis.

use serde::{Deserialize, Serialize};

use crate::net::Net;

/// Sentinel for "no upper bound" (+∞).
pub const INF: i32 = i32::MAX;

/// A time interval with optional open endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeInterval {
    pub earliest: i32,
    pub latest: i32,
    pub left_open: bool,
    pub right_open: bool,
}

impl TimeInterval {
    pub fn closed(earliest: i32, latest: i32) -> Self {
        Self {
            earliest,
            latest,
            left_open: false,
            right_open: false,
        }
    }

    pub fn effective_earliest(&self) -> i32 {
        if self.left_open {
            self.earliest + 1
        } else {
            self.earliest
        }
    }

    pub fn effective_latest(&self) -> i32 {
        if self.latest == INF {
            return INF;
        }
        if self.right_open {
            self.latest - 1
        } else {
            self.latest
        }
    }

    pub fn is_valid(&self) -> bool {
        self.earliest >= 0
            && (self.latest == INF || self.latest >= self.earliest)
            && (self.effective_latest() == INF || self.effective_earliest() <= self.effective_latest())
    }
}

/// PTPN place attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimedPlaceKind {
    pub capacity: usize,
    /// Saturating places clamp overflow instead of blocking the transition.
    pub saturate: bool,
}

/// PTPN transition attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimedTransitionKind {
    pub interval: TimeInterval,
    pub priority: i32,
    pub core: i32,
    pub suspendable: bool,
}

/// The priority timed Petri net (no arc payload).
pub type TimedNet = Net<TimedPlaceKind, TimedTransitionKind, ()>;
