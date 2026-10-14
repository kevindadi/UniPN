//! Construction for [`LabeledNet`].
//!
//! [`LabeledBuilder`] is [`NetBuilder`] over [`Tag`] kinds; places and
//! transitions go in through the generic `add_place` / `add_marked_place` /
//! `add_transition` with a `Tag(label)`, and this adds only weighted arc
//! shorthands and the `build` that names the result.

use crate::net::{ArcDir, Marking, NetBuilder, PlaceId, TransitionId};

use super::kinds::{LabeledNet, Tag};

/// Builder for a [`LabeledNet`]: the net plus its initial marking.
pub type LabeledBuilder<PL, TL> = NetBuilder<Tag<PL>, Tag<TL>, ()>;

impl<PL, TL> LabeledBuilder<PL, TL> {
    /// A `place → transition` arc of `weight`. Parallel arcs are kept as
    /// separate arcs; enabling sums them.
    pub fn add_input_arc(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.add_arc(place, transition, ArcDir::Input, weight, ());
    }

    /// A `transition → place` arc of `weight`.
    pub fn add_output_arc(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.add_arc(place, transition, ArcDir::Output, weight, ());
    }

    /// The finished net and its initial marking.
    pub fn build(self) -> (LabeledNet<PL, TL>, Marking) {
        let (net, marking, ()) = self.into_parts();
        (net, marking)
    }
}
