//! Chain-style builder: `NetBuilder`.

use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::expr::{BoolExpr, Val, VarUpdate};
use crate::ids::{PlaceId, TransitionId, Weight};
use crate::model::{Place, PlaceKind, Transition, TransitionKind};
use crate::net::Net;
use crate::state::{Marking, VarStore};
use crate::storage::Incidence;

/// The unified builder.
#[derive(Default)]
pub struct NetBuilder {
    places: Vec<Place>,
    transitions: Vec<Transition>,
    pre: Incidence,
    post: Incidence,
    pre_guards: FxHashMap<(TransitionId, PlaceId), BoolExpr>,
    post_updates: FxHashMap<(TransitionId, PlaceId), VarUpdate>,
    initial_marking: Vec<u32>,
    initial_vars: Option<VarStore>,
    var_domains: FxHashMap<String, (i64, i64)>,
}

impl NetBuilder {
    pub fn new() -> Self {
        Self {
            pre: Incidence::with_transitions(0),
            post: Incidence::with_transitions(0),
            ..Default::default()
        }
    }

    // ── Nodes ──

    pub fn add_place(&mut self, name: impl Into<String>, kind: PlaceKind) -> PlaceId {
        let id = PlaceId(self.places.len());
        self.places.push(Place {
            id,
            name: name.into(),
            kind,
            capacity: None,
        });
        self.initial_marking.push(0);
        id
    }

    pub fn add_transition(&mut self, name: impl Into<String>, kind: TransitionKind) -> TransitionId {
        let id = TransitionId(self.transitions.len());
        self.transitions.push(Transition {
            id,
            name: name.into(),
            kind,
            scope: None,
            anchors: Vec::new(),
            family: None,
            #[cfg(feature = "timed")]
            timing: None,
            #[cfg(feature = "timed")]
            priority: None,
        });
        self.pre = Incidence::with_transitions(self.transitions.len());
        self.post = Incidence::with_transitions(self.transitions.len());
        id
    }

    // ── Arcs ──

    pub fn add_input_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        weight: Weight,
        guard: BoolExpr,
    ) -> &mut Self {
        self.pre.add(transition, place, weight);
        if guard != BoolExpr::True {
            self.pre_guards.insert((transition, place), guard);
        }
        self
    }

    pub fn add_output_arc(
        &mut self,
        transition: TransitionId,
        place: PlaceId,
        weight: Weight,
        update: Option<VarUpdate>,
    ) -> &mut Self {
        self.post.add(transition, place, weight);
        if let Some(u) = update {
            self.post_updates.insert((transition, place), u);
        }
        self
    }

    // ── Attributes ──

    pub fn set_capacity(&mut self, place: PlaceId, capacity: Weight) -> &mut Self {
        if let Some(p) = self.places.get_mut(place.index()) {
            p.capacity = Some(capacity);
        }
        self
    }

    pub fn set_anchor(&mut self, transition: TransitionId, anchor: impl Into<String>) -> &mut Self {
        if let Some(t) = self.transitions.get_mut(transition.index()) {
            t.anchors.push(anchor.into());
        }
        self
    }

    pub fn set_scope(&mut self, transition: TransitionId, scope: impl Into<String>) -> &mut Self {
        if let Some(t) = self.transitions.get_mut(transition.index()) {
            t.scope = Some(scope.into());
        }
        self
    }

    pub fn set_family(&mut self, transition: TransitionId, family: impl Into<String>) -> &mut Self {
        if let Some(t) = self.transitions.get_mut(transition.index()) {
            t.family = Some(family.into());
        }
        self
    }

    // ── Initial state ──

    pub fn set_initial_tokens(&mut self, place: PlaceId, count: u32) -> &mut Self {
        if let Some(c) = self.initial_marking.get_mut(place.index()) {
            *c = count;
        }
        self
    }

    pub fn add_variable(&mut self, name: impl Into<String>, initial: Val) -> &mut Self {
        let vars = self.initial_vars.get_or_insert_with(IndexMap::new);
        vars.insert(name.into(), initial);
        self
    }

    /// Declare a bounded Int domain (an update leaving the domain disables the
    /// transition, keeping the state space finite).
    pub fn set_variable_domain(&mut self, name: impl Into<String>, lo: i64, hi: i64) -> &mut Self {
        self.var_domains.insert(name.into(), (lo, hi));
        self
    }

    pub fn build(self) -> Net {
        Net::from_parts(
            self.places,
            self.transitions,
            self.pre,
            self.post,
            self.pre_guards,
            self.post_updates,
            Marking::new(self.initial_marking),
            self.initial_vars,
            self.var_domains,
        )
    }
}
