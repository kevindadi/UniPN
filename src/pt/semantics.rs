//! P/T firing semantics (ConcBugDect's).
//!
//! Enabling aggregates input-arc weights per place and honours read and
//! inhibitor arcs; firing consumes inputs, produces outputs clamped to each
//! place's capacity, and empties reset arcs' places.

use crate::analysis::NetLike;
use crate::net::{ArcDir, Marking, PlaceId, TransitionId};

use super::kinds::PtNet;

impl NetLike for PtNet {
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
        let mut next = state.clone();

        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = next.tokens(arc.place);
            next.set(arc.place, current.checked_sub(arc.weight)?);
        }

        for arc in self.arcs_of(transition, ArcDir::Output) {
            let current = next.tokens(arc.place);
            let produced = current.checked_add(arc.weight)?;
            // Saturating capacity clamp (ConcBugDect's firing semantics).
            let clamped = self
                .place(arc.place)
                .and_then(|p| p.kind.capacity)
                .map_or(produced, |cap| produced.min(cap));
            next.set(arc.place, clamped);
        }

        for arc in self.arcs_of(transition, ArcDir::Reset) {
            next.set(arc.place, 0);
        }

        Some(next)
    }
}

impl PtNet {
    /// The aggregate input weight on `place → transition` (0 if no arc).
    pub fn input_weight(&self, place: PlaceId, transition: TransitionId) -> usize {
        self.arcs
            .iter()
            .filter(|a| {
                a.place == place && a.transition == transition && a.direction == ArcDir::Input
            })
            .map(|a| a.weight)
            .sum()
    }

    /// The aggregate output weight on `transition → place` (0 if no arc).
    pub fn output_weight(&self, place: PlaceId, transition: TransitionId) -> usize {
        self.arcs
            .iter()
            .filter(|a| {
                a.place == place && a.transition == transition && a.direction == ArcDir::Output
            })
            .map(|a| a.weight)
            .sum()
    }

    /// `Result`-shaped firing (mirrors ConcBugDect's `fire_transition`).
    // The `()` error type is part of the mirrored ConcBugDect signature.
    #[allow(clippy::result_unit_err)]
    pub fn fire_transition(
        &self,
        marking: &Marking,
        transition: TransitionId,
    ) -> Result<Marking, ()> {
        NetLike::fire(self, marking, transition).ok_or(())
    }

    /// The enabled transitions under a marking (mirrors ConcBugDect's API).
    pub fn enabled_transitions(&self, marking: &Marking) -> Vec<TransitionId> {
        NetLike::enabled(self, marking)
    }

    fn is_enabled(&self, state: &Marking, transition: TransitionId) -> bool {
        // Aggregate input-arc weights per place.
        let mut required: Vec<(PlaceId, usize)> = Vec::new();
        for arc in self.arcs_for(transition) {
            match arc.direction {
                ArcDir::Input => {
                    if let Some((_, total)) = required.iter_mut().find(|(p, _)| *p == arc.place) {
                        *total = total.checked_add(arc.weight).unwrap_or(usize::MAX);
                    } else {
                        required.push((arc.place, arc.weight));
                    }
                }
                ArcDir::Read => {
                    if state.tokens(arc.place) < arc.weight {
                        return false;
                    }
                }
                ArcDir::Inhibitor => {
                    if state.tokens(arc.place) >= arc.weight {
                        return false;
                    }
                }
                ArcDir::Output | ArcDir::Reset => {}
            }
        }
        required
            .into_iter()
            .all(|(place, count)| state.tokens(place) >= count)
    }
}
