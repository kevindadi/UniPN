//! Chain-style timed construction.
//!
//! [`TimedBuilder`] is [`NetBuilder`] instantiated for PTPN's kinds. Its whole
//! job is the one thing that is easy to get wrong by hand: keeping the initial
//! marking index-aligned with the places as they are added.

use crate::net::NetBuilder;

use super::kinds::{TimedExtra, TimedNet, TimedPlaceKind, TimedState, TimedTransitionKind};

/// Chain-style timed builder: the net plus its initial state.
pub type TimedBuilder = NetBuilder<TimedPlaceKind, TimedTransitionKind, (), TimedExtra>;

impl TimedBuilder {
    /// The finished net and its initial state.
    pub fn build(self) -> (TimedNet, TimedState) {
        self.into_net_and_state()
    }
}
