//! Construction for [`TpnNet`].
//!
//! [`TpnBuilder`] is [`NetBuilder`] over [`Tag`] places and
//! [`TpnTransition`] transitions; places and transitions go in through the
//! generic `add_place` / `add_marked_place` / `add_transition`, and this adds
//! only weighted arc shorthands and the `build` that names the result.

use crate::net::{ArcDir, Marking, NetBuilder, PlaceId, TransitionId};

use super::kinds::{TpnNet, TpnTransition};
use crate::labeled::Tag;

/// Builder for a [`TpnNet`]: the net plus its initial marking.
pub type TpnBuilder<PL, TL> = NetBuilder<Tag<PL>, TpnTransition<TL>, ()>;

impl<PL, TL> TpnBuilder<PL, TL> {
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
    pub fn build(self) -> (TpnNet<PL, TL>, Marking) {
        let (net, marking, ()) = self.into_parts();
        (net, marking)
    }
}
