//! Priority Timed Petri Net model (PTPN's lowering target).
//!
//! `TimedNet` is [`Net`] instantiated with PTPN's place/transition kind
//! payloads and no arc kind. Time is an *annotation* on transitions; the
//! discrete (untimed) firing lives here and is exposed through [`NetLike`],
//! while the state-class (DBM) reachability analysis lives in
//! [`crate::analysis::timed`].

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fmt;

use crate::analysis::NetLike;
use crate::ids::TransitionId;
use crate::net::{ArcDir, Marking, Net};

/// Sentinel for "no upper bound" (+∞) in time intervals and DBM matrices.
pub const INF: i32 = i32::MAX;

/// The "control core" (negative core id) for zero-time control transitions.
pub const CONTROL_TRANSITION_CORE: i32 = -1;

// Overflow recording: `fire` clamps every overflowing place to capacity, but a
// NON-saturating place being clamped is an invalid behavior and is recorded so
// the metrics layer can report it. Reset at the start of each build.
thread_local! {
    static OVERFLOW: RefCell<std::collections::BTreeSet<usize>> =
        const { RefCell::new(std::collections::BTreeSet::new()) };
}

pub fn reset_overflow_recording() {
    OVERFLOW.with(|o| o.borrow_mut().clear());
}

pub fn overflowed_places() -> Vec<usize> {
    OVERFLOW.with(|o| o.borrow().iter().copied().collect())
}

/// A time interval with optional open endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeInterval {
    pub earliest: i32,
    pub latest: i32,
    pub left_open: bool,
    pub right_open: bool,
}

impl TimeInterval {
    pub fn new(
        earliest: i32,
        latest: i32,
        left_open: bool,
        right_open: bool,
    ) -> Result<Self, String> {
        if earliest < 0 {
            return Err("earliest time must be non-negative".to_string());
        }
        if latest != INF && latest < earliest {
            return Err("latest time must be >= earliest time".to_string());
        }
        let interval = Self {
            earliest,
            latest,
            left_open,
            right_open,
        };
        if !interval.has_non_empty_integer_domain() {
            return Err("time interval has an empty integer domain".to_string());
        }
        Ok(interval)
    }

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

    pub fn has_non_empty_integer_domain(&self) -> bool {
        self.effective_latest() == INF || self.effective_earliest() <= self.effective_latest()
    }

    pub fn is_valid(&self) -> bool {
        self.earliest >= 0
            && (self.latest == INF || self.latest >= self.earliest)
            && self.has_non_empty_integer_domain()
    }

    pub fn contains(&self, time: i32) -> bool {
        time >= self.effective_earliest()
            && (self.effective_latest() == INF || time <= self.effective_latest())
    }
}

impl fmt::Display for TimeInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}, {}{}",
            if self.left_open { "(" } else { "[" },
            self.earliest,
            if self.latest == INF {
                "∞".to_string()
            } else {
                self.latest.to_string()
            },
            if self.right_open { ")" } else { "]" }
        )
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
    ///
    /// This inherent method does not re-check enabling (the timed state-class
    /// explorer already has). [`NetLike::fire`] returns `None` when the
    /// transition is not structurally enabled.
    pub fn fire(&self, marking: &Marking, transition: TransitionId) -> Marking {
        self.fire_marking(marking, transition)
    }

    fn fire_marking(&self, marking: &Marking, transition: TransitionId) -> Marking {
        let mut next = marking.clone();

        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = next.tokens(arc.place);
            next.set(arc.place, current.saturating_sub(arc.weight));
        }

        for arc in self.arcs_of(transition, ArcDir::Output) {
            let current = next.tokens(arc.place);
            let produced = current.saturating_add(arc.weight);
            let place = self.place(arc.place);
            let clamped = match place.and_then(|p| p.kind.capacity) {
                Some(cap) if produced > cap => {
                    // Firing always happens (enabling is input-driven). Overflow
                    // is clamped; a non-saturating place being clamped is an
                    // invalid behavior recorded for the metrics layer.
                    if place.is_some_and(|p| !p.kind.saturate) {
                        OVERFLOW.with(|o| o.borrow_mut().insert(arc.place.index()));
                    }
                    cap
                }
                _ => produced,
            };
            next.set(arc.place, clamped);
        }

        next
    }
}

impl NetLike for TimedNet {
    type State = Marking;

    fn num_places(&self) -> usize {
        self.places.len()
    }

    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    fn enabled(&self, state: &Self::State) -> Vec<TransitionId> {
        self.transitions
            .iter()
            .filter(|t| self.is_enabled(state, t.id))
            .map(|t| t.id)
            .collect()
    }

    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        if !self.is_enabled(state, transition) {
            return None;
        }
        Some(self.fire_marking(state, transition))
    }
}
