//! Priority Timed Petri Net model (PTPN's lowering target).
//!
//! `TimedNet` is [`Net`] instantiated with PTPN's place/transition kind
//! payloads and no arc kind. Time is an *annotation* on transitions; the
//! discrete (untimed) firing lives here, while the state-class (DBM)
//! reachability analysis lives in [`crate::analysis::timed`].

use serde::{Deserialize, Serialize};

use crate::ids::TransitionId;
use crate::net::{ArcDir, Marking, Net};

/// Sentinel for "no upper bound" (+∞) in time intervals and DBM matrices.
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
            && (self.effective_latest() == INF
                || self.effective_earliest() <= self.effective_latest())
    }
}

/// PTPN place attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimedPlaceKind {
    /// `None` = unbounded.
    pub capacity: Option<usize>,
    /// Saturating places absorb overflow (a transition producing into a full
    /// saturating place stays enabled and the count is clamped on firing).
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

impl TimedNet {
    /// Structural (input-driven) enabling: every input place holds enough
    /// tokens. Successor capacity never gates a transition (overflow on
    /// non-saturating places is clamped on firing).
    pub fn is_enabled(&self, marking: &Marking, transition: TransitionId) -> bool {
        self.arcs_of(transition, ArcDir::Input)
            .all(|arc| marking.tokens(arc.place) >= arc.weight)
    }

    /// Fires a transition: consumes input tokens and produces output tokens,
    /// clamping each output place to its capacity.
    pub fn fire(&self, marking: &Marking, transition: TransitionId) -> Marking {
        let mut next = marking.clone();

        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = next.tokens(arc.place);
            next.set(arc.place, current.saturating_sub(arc.weight));
        }

        for arc in self.arcs_of(transition, ArcDir::Output) {
            let current = next.tokens(arc.place);
            let produced = current.saturating_add(arc.weight);
            let clamped = self
                .place(arc.place)
                .and_then(|p| p.kind.capacity)
                .map_or(produced, |cap| produced.min(cap));
            next.set(arc.place, clamped);
        }

        next
    }
}
