//! The adjacency index [`Net`](crate::net::Net) keeps over its own arcs.
//!
//! Firing a transition needs that transition's arcs, and nothing else. Without
//! an index every lookup is a scan of the whole arc list, so computing one
//! state's enabled set costs `|T| · |A|` instead of the sum of the transitions'
//! own arc counts — the difference between a net that explores in seconds and
//! one that does not explore at all.
//!
//! The index stores arc *positions*, not copies, and is owned by the net. That
//! is the whole reason `Net::arcs` is private: the index is correct exactly
//! when every change to the arc list goes through a method that keeps it so.
//! A weight change does not move an arc, so only the aggregated demand of the
//! one transition it belongs to has to be recomputed.

use crate::net::{Arc, ArcDir, PlaceId, TransitionId, accumulate};

/// Arc positions by endpoint, plus each transition's aggregated input demand.
///
/// Built from the arc list alone, so two nets with the same arcs have the same
/// index; that keeps the derived `PartialEq` on `Net` meaningful.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ArcIndex {
    by_transition: Vec<Vec<usize>>,
    by_place: Vec<Vec<usize>>,
    /// Input weights per transition with parallel arcs summed — the one part of
    /// structural enabling that is not a per-arc check.
    demand: Vec<Vec<(PlaceId, usize)>>,
}

impl ArcIndex {
    pub(crate) fn build<AK>(arcs: &[Arc<AK>]) -> Self {
        let mut index = Self::default();
        for position in 0..arcs.len() {
            index.insert(arcs, position);
        }
        index
    }

    /// Index the arc at `position`, which must be the last one pushed.
    pub(crate) fn insert<AK>(&mut self, arcs: &[Arc<AK>], position: usize) {
        let arc = &arcs[position];
        let t = arc.transition.index();
        let p = arc.place.index();
        if self.by_transition.len() <= t {
            self.by_transition.resize_with(t + 1, Vec::new);
            self.demand.resize_with(t + 1, Vec::new);
        }
        if self.by_place.len() <= p {
            self.by_place.resize_with(p + 1, Vec::new);
        }
        self.by_transition[t].push(position);
        self.by_place[p].push(position);
        if arc.direction == ArcDir::Input {
            accumulate(&mut self.demand[t], arc.place, arc.weight);
        }
    }

    /// Recompute the aggregated demand of `transition` after one of its arcs
    /// changed weight.
    pub(crate) fn refresh_demand<AK>(&mut self, arcs: &[Arc<AK>], transition: TransitionId) {
        let Some(positions) = self.by_transition.get(transition.index()) else {
            return;
        };
        let mut demand = Vec::new();
        for &position in positions {
            let arc = &arcs[position];
            if arc.direction == ArcDir::Input {
                accumulate(&mut demand, arc.place, arc.weight);
            }
        }
        self.demand[transition.index()] = demand;
    }

    pub(crate) fn of_transition(&self, transition: TransitionId) -> &[usize] {
        self.by_transition
            .get(transition.index())
            .map_or(&[], Vec::as_slice)
    }

    pub(crate) fn of_place(&self, place: PlaceId) -> &[usize] {
        self.by_place.get(place.index()).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn demand(&self, transition: TransitionId) -> &[(PlaceId, usize)] {
        self.demand
            .get(transition.index())
            .map_or(&[], Vec::as_slice)
    }
}
