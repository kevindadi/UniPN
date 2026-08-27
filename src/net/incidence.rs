//! Adjacency view and ordinary incidence matrix of a [`Net`](crate::net::Net).
//!
//! The view is a derived snapshot: it aggregates parallel arcs of the same
//! direction and indexes endpoints by place/transition id. It does **not**
//! live inside [`Net`](crate::net::Net), because the net's `arcs` field is
//! public and can change after construction.
//!
//! # What the matrix captures
//!
//! [`IncidenceMatrix`] is the *ordinary token-flow skeleton*
//!
//! ```text
//! C[p, t] = w_post(p, t) − w_pre(p, t)
//! ```
//!
//! using only [`ArcDir::Input`](crate::net::ArcDir::Input) and
//! [`ArcDir::Output`](crate::net::ArcDir::Output) weights. Read, inhibitor,
//! and reset arcs are indexed on [`Incidence`] but do not enter `C`.
//!
//! This is enough for P-invariants, T-invariants, and the marking equation
//! `Δm = C · σ` on the **token counts**. It is intentionally kind-erased:
//! guards, updates, colors, clocks, and capacity policies are invisible.
//!
//! # CVN and future colored nets
//!
//! - **Current CVN.** Tokens are still uncolored `usize` counts. Guards and
//!   variable updates constrain *whether* a transition may fire and how
//!   `State.extra` changes; they do not change the token-flow weights. `C`
//!   is therefore the exact incidence matrix of the marking component.
//!   `C · σ = Δm` is a *necessary* condition on reachable markings, not a
//!   sufficient one (a guard, a bounded Int domain, or a capacity reject
//!   can forbid a sequence that the equation allows).
//! - **True colored Petri nets.** A scalar integer `C` is either the
//!   incidence of an *unfolding* (one place per color, one transition per
//!   binding) or an *aggregation* that forgets colors. Color-respecting
//!   incidence is function-valued (`C(p, t)` maps a binding to a multiset
//!   of colors) and would be a separate type. Do not encode guards or arc
//!   inscriptions into this matrix.

use serde::{Deserialize, Serialize};

use crate::net::{ArcDir, Net, PlaceId, TransitionId};

/// Aggregated adjacency of a net, keyed by contiguous place/transition ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incidence {
    pre: Vec<Vec<(PlaceId, usize)>>,
    post: Vec<Vec<(PlaceId, usize)>>,
    read: Vec<Vec<(PlaceId, usize)>>,
    inhibitor: Vec<Vec<(PlaceId, usize)>>,
    reset: Vec<Vec<(PlaceId, usize)>>,
    consumers: Vec<Vec<(TransitionId, usize)>>,
    producers: Vec<Vec<(TransitionId, usize)>>,
    readers: Vec<Vec<(TransitionId, usize)>>,
    inhibitors_of: Vec<Vec<(TransitionId, usize)>>,
    resets_of: Vec<Vec<(TransitionId, usize)>>,
}

impl Incidence {
    /// Build a snapshot of `net`. Arcs whose endpoints are out of range are
    /// skipped; parallel arcs of the same direction are weight-summed.
    pub fn of<PK, TK, AK>(net: &Net<PK, TK, AK>) -> Self {
        let n_p = net.num_places();
        let n_t = net.num_transitions();
        let mut inc = Self {
            pre: vec![Vec::new(); n_t],
            post: vec![Vec::new(); n_t],
            read: vec![Vec::new(); n_t],
            inhibitor: vec![Vec::new(); n_t],
            reset: vec![Vec::new(); n_t],
            consumers: vec![Vec::new(); n_p],
            producers: vec![Vec::new(); n_p],
            readers: vec![Vec::new(); n_p],
            inhibitors_of: vec![Vec::new(); n_p],
            resets_of: vec![Vec::new(); n_p],
        };

        for arc in &net.arcs {
            let p = arc.place.index();
            let t = arc.transition.index();
            if p >= n_p || t >= n_t {
                continue;
            }
            match arc.direction {
                ArcDir::Input => {
                    push_place(&mut inc.pre[t], arc.place, arc.weight);
                    push_transition(&mut inc.consumers[p], arc.transition, arc.weight);
                }
                ArcDir::Output => {
                    push_place(&mut inc.post[t], arc.place, arc.weight);
                    push_transition(&mut inc.producers[p], arc.transition, arc.weight);
                }
                ArcDir::Read => {
                    push_place(&mut inc.read[t], arc.place, arc.weight);
                    push_transition(&mut inc.readers[p], arc.transition, arc.weight);
                }
                ArcDir::Inhibitor => {
                    push_place(&mut inc.inhibitor[t], arc.place, arc.weight);
                    push_transition(&mut inc.inhibitors_of[p], arc.transition, arc.weight);
                }
                ArcDir::Reset => {
                    push_place(&mut inc.reset[t], arc.place, arc.weight);
                    push_transition(&mut inc.resets_of[p], arc.transition, arc.weight);
                }
            }
        }

        inc
    }

    pub fn num_places(&self) -> usize {
        self.consumers.len()
    }

    pub fn num_transitions(&self) -> usize {
        self.pre.len()
    }

    /// Aggregated input weights of `transition` (`•t` with multiplicity).
    pub fn pre(&self, transition: TransitionId) -> &[(PlaceId, usize)] {
        self.pre.get(transition.index()).map_or(&[], Vec::as_slice)
    }

    /// Aggregated output weights of `transition` (`t•` with multiplicity).
    pub fn post(&self, transition: TransitionId) -> &[(PlaceId, usize)] {
        self.post.get(transition.index()).map_or(&[], Vec::as_slice)
    }

    pub fn read(&self, transition: TransitionId) -> &[(PlaceId, usize)] {
        self.read.get(transition.index()).map_or(&[], Vec::as_slice)
    }

    pub fn inhibitor(&self, transition: TransitionId) -> &[(PlaceId, usize)] {
        self.inhibitor
            .get(transition.index())
            .map_or(&[], Vec::as_slice)
    }

    pub fn reset(&self, transition: TransitionId) -> &[(PlaceId, usize)] {
        self.reset
            .get(transition.index())
            .map_or(&[], Vec::as_slice)
    }

    /// Transitions that consume tokens from `place` (Input arcs).
    pub fn consumers(&self, place: PlaceId) -> &[(TransitionId, usize)] {
        self.consumers.get(place.index()).map_or(&[], Vec::as_slice)
    }

    /// Transitions that produce tokens into `place` (Output arcs).
    pub fn producers(&self, place: PlaceId) -> &[(TransitionId, usize)] {
        self.producers.get(place.index()).map_or(&[], Vec::as_slice)
    }

    pub fn readers(&self, place: PlaceId) -> &[(TransitionId, usize)] {
        self.readers.get(place.index()).map_or(&[], Vec::as_slice)
    }

    pub fn inhibitors_of(&self, place: PlaceId) -> &[(TransitionId, usize)] {
        self.inhibitors_of
            .get(place.index())
            .map_or(&[], Vec::as_slice)
    }

    pub fn resets_of(&self, place: PlaceId) -> &[(TransitionId, usize)] {
        self.resets_of.get(place.index()).map_or(&[], Vec::as_slice)
    }

    /// Input weight on `place → transition`, or 0 if none.
    pub fn pre_weight(&self, place: PlaceId, transition: TransitionId) -> usize {
        weight_of(self.pre(transition), place)
    }

    /// Output weight on `transition → place`, or 0 if none.
    pub fn post_weight(&self, place: PlaceId, transition: TransitionId) -> usize {
        weight_of(self.post(transition), place)
    }

    /// Ordinary incidence matrix of the token-flow skeleton.
    pub fn matrix(&self) -> IncidenceMatrix {
        IncidenceMatrix::from_incidence(self)
    }
}

/// Dense place × transition matrix `C[p, t] = w_post − w_pre` (`i64`).
///
/// Row-major: index `p * num_transitions + t`. Entries that would not fit
/// in `i64` saturate at `i64::MAX` / `i64::MIN`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IncidenceMatrix {
    rows: usize,
    cols: usize,
    entries: Vec<i64>,
}

impl IncidenceMatrix {
    fn from_incidence(inc: &Incidence) -> Self {
        let rows = inc.num_places();
        let cols = inc.num_transitions();
        let mut entries = vec![0i64; rows.saturating_mul(cols)];
        if cols == 0 {
            return Self {
                rows,
                cols,
                entries,
            };
        }

        for t in 0..cols {
            let tid = TransitionId(t);
            for &(place, weight) in inc.pre(tid) {
                let idx = place.index() * cols + t;
                entries[idx] = entries[idx].saturating_sub(weight_i64(weight));
            }
            for &(place, weight) in inc.post(tid) {
                let idx = place.index() * cols + t;
                entries[idx] = entries[idx].saturating_add(weight_i64(weight));
            }
        }

        Self {
            rows,
            cols,
            entries,
        }
    }

    pub fn num_places(&self) -> usize {
        self.rows
    }

    pub fn num_transitions(&self) -> usize {
        self.cols
    }

    /// `C[place, transition]`, or 0 if either id is out of range.
    pub fn get(&self, place: PlaceId, transition: TransitionId) -> i64 {
        let p = place.index();
        let t = transition.index();
        if p >= self.rows || t >= self.cols {
            return 0;
        }
        self.entries[p * self.cols + t]
    }

    /// One matrix row (all transitions of a place), or `None` if out of range.
    pub fn row(&self, place: PlaceId) -> Option<&[i64]> {
        let p = place.index();
        if p >= self.rows {
            return None;
        }
        if self.cols == 0 {
            return Some(&[]);
        }
        let start = p * self.cols;
        Some(&self.entries[start..start + self.cols])
    }

    /// Non-zero entries as `(place, transition, C[p, t])`.
    pub fn nonzero(&self) -> impl Iterator<Item = (PlaceId, TransitionId, i64)> + '_ {
        let cols = self.cols;
        self.entries
            .iter()
            .enumerate()
            .filter(move |&(_, &v)| cols != 0 && v != 0)
            .map(move |(i, &v)| (PlaceId(i / cols), TransitionId(i % cols), v))
    }

    /// The marking equation `Δm = C · σ`.
    ///
    /// `sigma[t]` is the firing count of transition `t`. Returns `None` if
    /// `sigma.len()` is not `num_transitions`.
    pub fn apply(&self, sigma: &[usize]) -> Option<Vec<i64>> {
        if sigma.len() != self.cols {
            return None;
        }
        let mut delta = vec![0i64; self.rows];
        if self.cols == 0 {
            return Some(delta);
        }
        for (t, &count) in sigma.iter().enumerate() {
            if count == 0 {
                continue;
            }
            let n = weight_i64(count);
            for (p, slot) in delta.iter_mut().enumerate() {
                *slot = slot.saturating_add(self.entries[p * self.cols + t].saturating_mul(n));
            }
        }
        Some(delta)
    }

    pub fn as_slice(&self) -> &[i64] {
        &self.entries
    }
}

impl std::ops::Index<(PlaceId, TransitionId)> for IncidenceMatrix {
    type Output = i64;

    fn index(&self, (place, transition): (PlaceId, TransitionId)) -> &i64 {
        let p = place.index();
        let t = transition.index();
        assert!(
            p < self.rows && t < self.cols,
            "incidence index ({p}, {t}) out of range {}×{}",
            self.rows,
            self.cols
        );
        &self.entries[p * self.cols + t]
    }
}

fn push_place(list: &mut Vec<(PlaceId, usize)>, place: PlaceId, weight: usize) {
    if let Some((_, total)) = list.iter_mut().find(|(p, _)| *p == place) {
        *total = total.saturating_add(weight);
    } else {
        list.push((place, weight));
    }
}

fn push_transition(list: &mut Vec<(TransitionId, usize)>, transition: TransitionId, weight: usize) {
    if let Some((_, total)) = list.iter_mut().find(|(t, _)| *t == transition) {
        *total = total.saturating_add(weight);
    } else {
        list.push((transition, weight));
    }
}

fn weight_of(list: &[(PlaceId, usize)], place: PlaceId) -> usize {
    list.iter()
        .find_map(|(p, w)| (*p == place).then_some(*w))
        .unwrap_or(0)
}

fn weight_i64(weight: usize) -> i64 {
    i64::try_from(weight).unwrap_or(i64::MAX)
}
