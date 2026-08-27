//! Chain-style CVN construction.

use std::collections::BTreeMap;

use crate::net::{ArcDir, Marking, PlaceId, State, TransitionId};

use super::expr::{BoolExpr, Val, VarUpdate};
use super::kinds::{
    CvnArcKind, CvnExtra, CvnNet, CvnState, CvnTransition, PlaceKind, TransitionKind, VarStore,
};

/// Chain-style CVN builder: produces the net plus its initial state.
#[derive(Default)]
pub struct CvnBuilder {
    net: CvnNet,
    marking: Vec<usize>,
    vars: VarStore,
    domains: BTreeMap<String, (i64, i64)>,
}

impl CvnBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_place(&mut self, name: impl Into<String>, kind: PlaceKind) -> PlaceId {
        let id = self.net.add_place(name, kind);
        self.marking.push(0);
        id
    }

    pub fn add_transition(
        &mut self,
        name: impl Into<String>,
        kind: TransitionKind,
    ) -> TransitionId {
        self.net.add_transition(
            name,
            CvnTransition {
                kind,
                scope: None,
                anchors: Vec::new(),
                family: None,
            },
        )
    }

    pub fn set_anchor(&mut self, transition: TransitionId, anchor: impl Into<String>) -> &mut Self {
        if let Some(t) = self.net.transitions.get_mut(transition.index()) {
            t.kind.anchors.push(anchor.into());
        }
        self
    }

    pub fn set_scope(&mut self, transition: TransitionId, scope: impl Into<String>) -> &mut Self {
        if let Some(t) = self.net.transitions.get_mut(transition.index()) {
            t.kind.scope = Some(scope.into());
        }
        self
    }

    pub fn set_family(&mut self, transition: TransitionId, family: impl Into<String>) -> &mut Self {
        if let Some(t) = self.net.transitions.get_mut(transition.index()) {
            t.kind.family = Some(family.into());
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
        self.net
            .add_arc(place, transition, ArcDir::Input, weight, kind);
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
        self.net
            .add_arc(place, transition, ArcDir::Output, weight, kind);
        self
    }

    pub fn set_initial_tokens(&mut self, place: PlaceId, count: usize) -> &mut Self {
        if let Some(slot) = self.marking.get_mut(place.index()) {
            *slot = count;
        }
        self
    }

    pub fn add_variable(&mut self, name: impl Into<String>, initial: Val) -> &mut Self {
        self.vars.insert(name.into(), initial);
        self
    }

    /// Declare a bounded Int domain (an update leaving the domain disables the
    /// transition, keeping the state space finite).
    pub fn set_variable_domain(&mut self, name: impl Into<String>, lo: i64, hi: i64) -> &mut Self {
        self.domains.insert(name.into(), (lo, hi));
        self
    }

    pub fn build(self) -> (CvnNet, CvnState) {
        (
            self.net,
            State::new(
                Marking::new(self.marking),
                CvnExtra {
                    vars: self.vars,
                    domains: self.domains,
                },
            ),
        )
    }
}
