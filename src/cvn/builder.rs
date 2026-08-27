//! Chain-style CVN construction.
//!
//! [`CvnBuilder`] is [`NetBuilder`] instantiated with [`CvnExtra`] as the
//! accumulated state, so the variable store and the declared domains end up in
//! the built [`CvnState`] without this module tracking the marking itself.

use crate::net::{ArcDir, NetBuilder, PlaceId, TransitionId};

use super::expr::{BoolExpr, Val, VarUpdate};
use super::kinds::{CvnArcKind, CvnExtra, CvnNet, CvnState, CvnTransition, PlaceKind};

/// Chain-style CVN builder: produces the net plus its initial state.
///
/// Transitions go in through the generic
/// [`add_transition`](NetBuilder::add_transition) with a [`CvnTransition`]
/// payload — [`CvnTransition::new`] covers the common case of a kind with no
/// source attribution yet.
pub type CvnBuilder = NetBuilder<PlaceKind, CvnTransition, CvnArcKind, CvnExtra>;

impl CvnBuilder {
    pub fn set_anchor(&mut self, transition: TransitionId, anchor: impl Into<String>) -> &mut Self {
        if let Some(kind) = self.transition_kind_mut(transition) {
            kind.anchors.push(anchor.into());
        }
        self
    }

    pub fn set_scope(&mut self, transition: TransitionId, scope: impl Into<String>) -> &mut Self {
        if let Some(kind) = self.transition_kind_mut(transition) {
            kind.scope = Some(scope.into());
        }
        self
    }

    pub fn set_family(&mut self, transition: TransitionId, family: impl Into<String>) -> &mut Self {
        if let Some(kind) = self.transition_kind_mut(transition) {
            kind.family = Some(family.into());
        }
        self
    }

    pub fn add_input_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        weight: usize,
        guard: BoolExpr,
    ) -> &mut Self {
        let kind = if guard == BoolExpr::True {
            CvnArcKind::Plain
        } else {
            CvnArcKind::Guard(guard)
        };
        self.add_arc(place, transition, ArcDir::Input, weight, kind);
        self
    }

    pub fn add_output_arc(
        &mut self,
        transition: TransitionId,
        place: PlaceId,
        weight: usize,
        update: Option<VarUpdate>,
    ) -> &mut Self {
        let kind = update.map_or(CvnArcKind::Plain, CvnArcKind::Update);
        self.add_arc(place, transition, ArcDir::Output, weight, kind);
        self
    }

    /// An output arc that drops `vars` from the store when the transition fires
    /// — a scope ending. Dead variables left in the store would split otherwise
    /// equal states, so a frontend should drop a local when its scope ends.
    pub fn add_scope_end_arc(
        &mut self,
        transition: TransitionId,
        place: PlaceId,
        weight: usize,
        vars: impl IntoIterator<Item = String>,
    ) -> &mut Self {
        let kind = CvnArcKind::DropVars(vars.into_iter().collect());
        self.add_arc(place, transition, ArcDir::Output, weight, kind);
        self
    }

    pub fn add_variable(&mut self, name: impl Into<String>, initial: Val) -> &mut Self {
        self.extra_mut().vars.insert(name.into(), initial);
        self
    }

    /// Declare a bounded Int domain (an update leaving the domain disables the
    /// transition, keeping the state space finite).
    pub fn set_variable_domain(&mut self, name: impl Into<String>, lo: i64, hi: i64) -> &mut Self {
        self.extra_mut().domains.insert(name.into(), (lo, hi));
        self
    }

    /// The finished net and its initial state.
    pub fn build(self) -> (CvnNet, CvnState) {
        self.into_net_and_state()
    }
}
