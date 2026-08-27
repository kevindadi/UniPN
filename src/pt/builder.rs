//! Chain-style P/T construction (mirrors ConcBugDect's `Net` construction API).

use serde::{Deserialize, Serialize};

use crate::net::{ArcDir, Marking, PlaceId, TransitionId};

use super::dot::DiagnosticReport;
use super::kinds::{PlaceType, PtNet, PtPlaceKind, PtTransitionKind, TransitionType};

/// A place under construction (mirrors ConcBugDect's `Place`); `tokens` is the
/// initial marking and `usize::MAX` capacity means unbounded.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtPlace {
    pub name: String,
    pub tokens: usize,
    pub capacity: usize,
    pub place_type: PlaceType,
    pub span: String,
}

impl PtPlace {
    pub fn new(
        name: impl Into<String>,
        tokens: usize,
        capacity: usize,
        place_type: PlaceType,
        span: String,
    ) -> Self {
        Self {
            name: name.into(),
            tokens,
            capacity,
            place_type,
            span,
        }
    }
}

/// A transition under construction (mirrors ConcBugDect's `Transition`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtTransition {
    pub name: String,
    pub transition_type: TransitionType,
}

impl PtTransition {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transition_type: TransitionType::Normal,
        }
    }

    pub fn new_with_transition_type(
        name: impl Into<String>,
        transition_type: TransitionType,
    ) -> Self {
        Self {
            name: name.into(),
            transition_type,
        }
    }
}

/// Chain-style P/T builder (mirrors ConcBugDect's `Net` construction API):
/// accumulates the net plus its initial marking.
#[derive(Clone, Debug, Default)]
pub struct PtBuilder {
    net: PtNet,
    marking: Vec<usize>,
}

impl PtBuilder {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn add_place(&mut self, place: PtPlace) -> PlaceId {
        let id = self.net.add_place(
            place.name,
            PtPlaceKind {
                place_type: place.place_type,
                span: place.span,
                capacity: (place.capacity != usize::MAX).then_some(place.capacity),
            },
        );
        self.marking.push(place.tokens);
        id
    }

    pub fn add_transition(&mut self, transition: PtTransition) -> TransitionId {
        self.net.add_transition(
            transition.name,
            PtTransitionKind {
                transition_type: transition.transition_type,
            },
        )
    }

    pub fn add_input_arc(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.add_weighted_arc(place, transition, ArcDir::Input, weight);
    }

    pub fn add_output_arc(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.add_weighted_arc(place, transition, ArcDir::Output, weight);
    }

    pub fn set_input_weight(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.set_weighted_arc(place, transition, ArcDir::Input, weight);
    }

    pub fn set_output_weight(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.set_weighted_arc(place, transition, ArcDir::Output, weight);
    }

    fn add_weighted_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
    ) {
        if weight == 0 {
            return;
        }
        if let Some(arc) =
            self.net.arcs.iter_mut().find(|a| {
                a.place == place && a.transition == transition && a.direction == direction
            })
        {
            arc.weight = arc.weight.saturating_add(weight);
        } else {
            self.net.add_arc(place, transition, direction, weight, ());
        }
    }

    fn set_weighted_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
    ) {
        if let Some(arc) =
            self.net.arcs.iter_mut().find(|a| {
                a.place == place && a.transition == transition && a.direction == direction
            })
        {
            arc.weight = weight;
        } else {
            self.net.add_arc(place, transition, direction, weight, ());
        }
    }

    pub fn places_len(&self) -> usize {
        self.net.num_places()
    }

    pub fn transitions_len(&self) -> usize {
        self.net.num_transitions()
    }

    pub fn initial_marking(&self) -> Marking {
        Marking::new(self.marking.clone())
    }

    /// Mutable access to a transition's kind (for post-creation mutation).
    pub fn transition_mut(&mut self, transition: TransitionId) -> Option<&mut PtTransitionKind> {
        self.net
            .transitions
            .get_mut(transition.index())
            .map(|t| &mut t.kind)
    }

    /// Mutable access to a place's kind (for post-creation capacity mutation).
    pub fn place_mut(&mut self, place: PlaceId) -> Option<&mut PtPlaceKind> {
        self.net.places.get_mut(place.index()).map(|p| &mut p.kind)
    }

    pub fn set_place_tokens(&mut self, place: PlaceId, tokens: usize) {
        if let Some(slot) = self.marking.get_mut(place.index()) {
            *slot = tokens;
        }
    }

    /// The underlying net (read-only view while building).
    pub fn net(&self) -> &PtNet {
        &self.net
    }

    /// A cloned snapshot of the built net + initial marking (without consuming
    /// the builder, so construction can continue).
    pub fn snapshot(&self) -> (PtNet, Marking) {
        (self.net.clone(), self.initial_marking())
    }

    pub fn to_dot(&self) -> String {
        self.net.to_dot()
    }

    pub fn write_dot<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        self.net.write_dot(path)
    }

    pub fn diagnose_connectivity(&self) -> DiagnosticReport {
        self.net.diagnose_connectivity()
    }

    pub fn log_diagnostics(&self) {
        self.net.log_diagnostics()
    }

    pub fn build(self) -> (PtNet, Marking) {
        (self.net, Marking::new(self.marking))
    }
}

/// Convenience: build a marking directly from a slice of counts.
pub fn marking(counts: impl IntoIterator<Item = usize>) -> Marking {
    Marking::new(counts.into_iter().collect())
}
