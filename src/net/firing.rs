//! Structural firing primitives shared by every frontend.
//!
//! These are the rules that follow from the *arc structure* alone and are
//! therefore the same for P/T, timed, and colored nets: which places a
//! transition demands (with parallel arcs summed), whether read and inhibitor
//! arcs let it through, and how tokens move on firing.
//!
//! What is **not** here is the part each frontend decides for itself — guards,
//! variable updates, clock zones, and what a capacity violation means. That
//! choice is expressed by picking [`Net::produce_outputs_clamped`] or
//! [`Net::produce_outputs_bounded`], and by the frontend's own
//! [`Semantics`](crate::analysis::Semantics) impl.

use crate::net::{ArcDir, Marking, Net, PlaceId, TransitionId, accumulate};

/// A place kind that carries a token capacity.
///
/// The three frontends express capacity differently — a plain field, a field
/// plus a saturation flag, or a value derived from the resource type — so the
/// capacity lookup is a trait on the place kind rather than a fixed field on
/// [`Place`](crate::net::Place).
pub trait PlaceCapacity {
    /// The place's token capacity; `None` = unbounded.
    fn capacity(&self) -> Option<usize>;
}

/// Kind-less nets are unbounded.
impl PlaceCapacity for () {
    fn capacity(&self) -> Option<usize> {
        None
    }
}

impl<PK, TK, AK> Net<PK, TK, AK> {
    /// The aggregate input weight on `place → transition` (0 if no arc).
    pub fn input_weight(&self, place: PlaceId, transition: TransitionId) -> usize {
        self.directed_weight(place, transition, ArcDir::Input)
    }

    /// The aggregate output weight on `transition → place` (0 if no arc).
    pub fn output_weight(&self, place: PlaceId, transition: TransitionId) -> usize {
        self.directed_weight(place, transition, ArcDir::Output)
    }

    fn directed_weight(
        &self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
    ) -> usize {
        self.arcs
            .iter()
            .filter(|a| a.place == place && a.transition == transition && a.direction == direction)
            .map(|a| a.weight)
            .sum()
    }

    /// Structural enabling: every read arc is satisfied, no inhibitor arc
    /// blocks, and the marking covers the aggregated input demand.
    ///
    /// This is the *whole* enabling condition for a plain P/T or timed net. A
    /// net with guards (CVN) checks this first and then its own predicates.
    pub fn structurally_enabled(&self, marking: &Marking, transition: TransitionId) -> bool {
        let mut demand: Vec<(PlaceId, usize)> = Vec::new();
        for arc in self.arcs_for(transition) {
            match arc.direction {
                ArcDir::Input => accumulate(&mut demand, arc.place, arc.weight),
                ArcDir::Read => {
                    if marking.tokens(arc.place) < arc.weight {
                        return false;
                    }
                }
                ArcDir::Inhibitor => {
                    if marking.tokens(arc.place) >= arc.weight {
                        return false;
                    }
                }
                ArcDir::Output | ArcDir::Reset => {}
            }
        }
        demand
            .into_iter()
            .all(|(place, count)| marking.tokens(place) >= count)
    }

    /// Consume the input tokens of `transition`.
    ///
    /// Saturating: a place that does not hold enough tokens drops to zero
    /// instead of erroring, so callers must establish enabling first (see
    /// [`Net::structurally_enabled`]).
    pub fn consume_inputs(&self, marking: &mut Marking, transition: TransitionId) {
        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = marking.tokens(arc.place);
            marking.set(arc.place, current.saturating_sub(arc.weight));
        }
    }

    /// Empty every place reached by a reset arc of `transition`.
    pub fn apply_resets(&self, marking: &mut Marking, transition: TransitionId) {
        for arc in self.arcs_of(transition, ArcDir::Reset) {
            marking.set(arc.place, 0);
        }
    }
}

impl<PK: PlaceCapacity, TK, AK> Net<PK, TK, AK> {
    /// The capacity of `place`; `None` = unbounded or unknown place.
    pub fn capacity_of(&self, place: PlaceId) -> Option<usize> {
        self.place(place).and_then(|p| p.kind.capacity())
    }

    /// Produce the output tokens of `transition`, clamping any place that would
    /// exceed its capacity.
    ///
    /// Returns the places that were clamped, so a caller that treats clamping
    /// as an anomaly (the timed net's overflow metric) can report them. Firing
    /// itself always succeeds.
    pub fn produce_outputs_clamped(
        &self,
        marking: &mut Marking,
        transition: TransitionId,
    ) -> Vec<PlaceId> {
        let mut clamped = Vec::new();
        for arc in self.arcs_of(transition, ArcDir::Output) {
            let produced = marking.tokens(arc.place).saturating_add(arc.weight);
            match self.capacity_of(arc.place) {
                Some(cap) if produced > cap => {
                    clamped.push(arc.place);
                    marking.set(arc.place, cap);
                }
                _ => {
                    marking.set(arc.place, produced);
                }
            }
        }
        clamped
    }

    /// Produce the output tokens of `transition`, rejecting the firing if any
    /// place would exceed its capacity.
    ///
    /// On `None` the marking may already be partially updated, so the caller
    /// must discard it (which is why the frontends produce into a clone).
    pub fn produce_outputs_bounded(
        &self,
        marking: &mut Marking,
        transition: TransitionId,
    ) -> Option<()> {
        for arc in self.arcs_of(transition, ArcDir::Output) {
            let produced = marking.tokens(arc.place).checked_add(arc.weight)?;
            if self
                .capacity_of(arc.place)
                .is_some_and(|cap| produced > cap)
            {
                return None;
            }
            marking.set(arc.place, produced);
        }
        Some(())
    }
}
