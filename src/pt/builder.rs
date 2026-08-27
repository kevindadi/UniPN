//! Chain-style P/T construction.
//!
//! [`PtBuilder`] is [`NetBuilder`] instantiated for the P/T kinds; the marking
//! bookkeeping lives there. What is specific to ConcBugDect's construction API
//! is the arc handling: `add_*_arc` *accumulates* onto an existing parallel arc
//! instead of pushing a second one, and `set_*_weight` overwrites it.

use crate::net::{ArcDir, Marking, NetBuilder, PlaceId, TransitionId};

use super::dot::DiagnosticReport;
use super::kinds::{PtNet, PtPlaceKind, PtTransitionKind};

/// Chain-style P/T builder: the net plus its initial marking.
pub type PtBuilder = NetBuilder<PtPlaceKind, PtTransitionKind, ()>;

impl PtBuilder {
    pub fn empty() -> Self {
        Self::new()
    }

    /// Add `weight` to the `place → transition` input arc, creating it if absent.
    pub fn add_input_arc(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.accumulate_arc(place, transition, ArcDir::Input, weight);
    }

    /// Add `weight` to the `transition → place` output arc, creating it if absent.
    pub fn add_output_arc(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.accumulate_arc(place, transition, ArcDir::Output, weight);
    }

    pub fn set_input_weight(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.overwrite_arc(place, transition, ArcDir::Input, weight);
    }

    pub fn set_output_weight(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.overwrite_arc(place, transition, ArcDir::Output, weight);
    }

    fn accumulate_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
    ) {
        if weight == 0 {
            return;
        }
        match self.arc_index(place, transition, direction) {
            Some(i) => {
                let arc = &mut self.net_mut().arcs[i];
                arc.weight = arc.weight.saturating_add(weight);
            }
            None => self.add_arc(place, transition, direction, weight, ()),
        }
    }

    fn overwrite_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
    ) {
        match self.arc_index(place, transition, direction) {
            Some(i) => self.net_mut().arcs[i].weight = weight,
            None => self.add_arc(place, transition, direction, weight, ()),
        }
    }

    fn arc_index(
        &self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
    ) -> Option<usize> {
        self.net()
            .arcs
            .iter()
            .position(|a| a.place == place && a.transition == transition && a.direction == direction)
    }

    /// A cloned snapshot of the built net + initial marking (without consuming
    /// the builder, so construction can continue).
    pub fn snapshot(&self) -> (PtNet, Marking) {
        (self.net().clone(), self.initial_marking())
    }

    pub fn to_dot(&self) -> String {
        self.net().to_dot()
    }

    pub fn write_dot<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        self.net().write_dot(path)
    }

    pub fn diagnose_connectivity(&self) -> DiagnosticReport {
        self.net().diagnose_connectivity()
    }

    pub fn log_diagnostics(&self) {
        self.net().log_diagnostics()
    }

    /// The finished net and its initial marking.
    pub fn build(self) -> (PtNet, Marking) {
        let (net, marking, ()) = self.into_parts();
        (net, marking)
    }
}

/// Convenience: build a marking directly from a slice of counts.
pub fn marking(counts: impl IntoIterator<Item = usize>) -> Marking {
    Marking::new(counts.into_iter().collect())
}
