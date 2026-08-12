//! `NetLike`: the unified net contract (object-safe).
//!
//! Any net (CVN, ConcBugDect MIR→PN, test/timed nets) that implements this
//! trait is consumed by the shared algorithms in [`crate::analysis`]. A **pure
//! P/T net** only fills the structural predicates
//! (`pre_arcs`/`post_arcs`/`initial_state`/`num_*`); `enabled_transitions` and
//! `fire` use the default implementations. Frontends with guards/updates/
//! capacities/timing override them as needed.
//!
//! `place_kind`/`transition_kind` are pure annotations; they do not participate
//! in the default firing semantics. Domain-specific notions such as "resource",
//! "thread terminal" and "wait point" default to `false` here — a frontend that
//! models them (e.g. [`crate::net::Net`]) overrides the corresponding
//! predicates. The shared explorers never decide what a deadlock is; they only
//! report blocked states, and the caller classifies them.

use thiserror::Error;

use crate::ids::{PlaceId, TransitionId, Weight};
use crate::model::{PlaceKind, TransitionKind};
use crate::state::State;

/// Firing error.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FireError {
    #[error("transition {0} is out of bounds")]
    OutOfBounds(TransitionId),
    #[error("transition {0} is not enabled under the supplied state")]
    NotEnabled(TransitionId),
    #[error("place {place} capacity exceeded: {after} > {capacity}")]
    Capacity {
        place: PlaceId,
        after: Weight,
        capacity: Weight,
    },
}

/// The unified net contract.
pub trait NetLike {
    // ── Structure ──

    fn num_places(&self) -> usize;
    fn num_transitions(&self) -> usize;

    fn place_ids(&self) -> Vec<PlaceId> {
        (0..self.num_places()).map(PlaceId).collect()
    }

    fn transition_ids(&self) -> Vec<TransitionId> {
        (0..self.num_transitions()).map(TransitionId).collect()
    }

    fn place_label(&self, _p: PlaceId) -> String {
        String::new()
    }

    fn place_kind(&self, _p: PlaceId) -> Option<PlaceKind> {
        None
    }

    fn transition_label(&self, _t: TransitionId) -> String {
        String::new()
    }

    fn transition_kind(&self, _t: TransitionId) -> Option<TransitionKind> {
        None
    }

    /// Anchoring back to the source (ConcIR sid / source line); empty by default.
    fn transition_anchors(&self, _t: TransitionId) -> Vec<String> {
        Vec::new()
    }

    /// Disjunctive OR family; none by default.
    fn transition_family(&self, _t: TransitionId) -> Option<&str> {
        None
    }

    /// Source scope (function/def) that produced this transition; none by
    /// default.
    fn transition_scope(&self, _t: TransitionId) -> Option<&str> {
        None
    }

    /// The preset of transition `t`: `(place, weight)`.
    fn pre_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)>;

    /// The postset of transition `t`: `(place, weight)`.
    fn post_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)>;

    // ── Optional domain predicates ──
    //
    // These default to `false`: the generic layer makes no assumption about
    // what a resource / terminal / wait point is. Frontends that need these
    // distinctions override them (see [`crate::net::Net`]).

    /// Whether this place is a thread-scope terminal.
    fn is_thread_terminal(&self, _p: PlaceId) -> bool {
        false
    }

    /// Whether this place is a condvar wait point.
    fn is_wait_point(&self, _p: PlaceId) -> bool {
        false
    }

    /// Whether this place is a resource place.
    fn is_resource(&self, _p: PlaceId) -> bool {
        false
    }

    // ── Runtime ──

    fn initial_state(&self) -> State;

    /// The set of enabled transitions in a given state. The default
    /// implementation is the pure P/T semantics.
    fn enabled_transitions(&self, s: &State) -> Vec<TransitionId> {
        let mut out = Vec::new();
        for t in self.transition_ids() {
            let mut ok = true;
            for (p, w) in self.pre_arcs(t) {
                if s.marking.tokens(p) < w {
                    ok = false;
                    break;
                }
            }
            if ok {
                out.push(t);
            }
        }
        out
    }

    /// Fire a transition. The default implementation is the pure P/T semantics
    /// (consumes the preset, produces the postset).
    fn fire(&self, t: TransitionId, s: &State) -> Result<State, FireError> {
        if t.index() >= self.num_transitions() {
            return Err(FireError::OutOfBounds(t));
        }
        let mut next = s.clone();
        for (p, w) in self.pre_arcs(t) {
            let tokens = next.marking.tokens(p);
            if tokens < w {
                return Err(FireError::NotEnabled(t));
            }
            next.marking.set(p, tokens - w);
        }
        for (p, w) in self.post_arcs(t) {
            next.marking.set(p, next.marking.tokens(p) + w);
        }
        Ok(next)
    }
}
