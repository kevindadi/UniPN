//! The CVN network structure backed by a petgraph directed graph.
//!
//! The net is a bipartite graph where nodes are either [`Place`]s or [`Transition`]s,
//! and edges are either input arcs (Place → Transition) or output arcs (Transition → Place).

use crate::error::{CvnError, ErrorCode, ErrorLocation};
use crate::model::*;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

/// A node in the CVN bipartite graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NetNode {
    /// A place node.
    Place(Place),
    /// A transition node.
    Transition(Transition),
}

impl NetNode {
    /// Returns the place if this is a place node.
    pub fn as_place(&self) -> Option<&Place> {
        match self {
            Self::Place(p) => Some(p),
            Self::Transition(_) => None,
        }
    }

    /// Returns the transition if this is a transition node.
    pub fn as_transition(&self) -> Option<&Transition> {
        match self {
            Self::Place(_) => None,
            Self::Transition(t) => Some(t),
        }
    }
}

/// An edge in the CVN bipartite graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NetEdge {
    /// Input arc: Place → Transition (weight + guard).
    Input(InputArcData),
    /// Output arc: Transition → Place (weight + optional update).
    Output(OutputArcData),
}

/// The CVN (Concurrency Verification Net) — a weighted P/T Petri net with global variable guards.
///
/// Internally represented as a petgraph [`DiGraph`] with fast ID-to-index lookup maps.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CvnNet {
    /// The underlying petgraph directed graph.
    graph: DiGraph<NetNode, NetEdge>,
    /// Place ID → NodeIndex lookup.
    place_index: FxHashMap<PlaceId, NodeIndex>,
    /// Transition ID → NodeIndex lookup.
    transition_index: FxHashMap<TransitionId, NodeIndex>,
    /// Initial marking (token distribution).
    initial_marking: Marking,
    /// Initial variable store.
    initial_vars: VarStore,
}

impl CvnNet {
    /// Create a new CvnNet from pre-built components (used by the builder).
    pub(crate) fn from_parts(
        graph: DiGraph<NetNode, NetEdge>,
        place_index: FxHashMap<PlaceId, NodeIndex>,
        transition_index: FxHashMap<TransitionId, NodeIndex>,
        initial_marking: Marking,
        initial_vars: VarStore,
    ) -> Self {
        Self {
            graph,
            place_index,
            transition_index,
            initial_marking,
            initial_vars,
        }
    }

    /// Access the underlying petgraph directed graph.
    pub fn petgraph(&self) -> &DiGraph<NetNode, NetEdge> {
        &self.graph
    }

    /// Get the initial state (marking + variable store).
    pub fn initial_state(&self) -> State {
        State::new(self.initial_marking.clone(), self.initial_vars.clone())
    }

    /// Get the initial marking.
    pub fn initial_marking(&self) -> &Marking {
        &self.initial_marking
    }

    /// Get the initial variable store.
    pub fn initial_vars(&self) -> &VarStore {
        &self.initial_vars
    }

    /// Look up the graph node index for a place.
    pub fn place_node(&self, id: &PlaceId) -> Option<NodeIndex> {
        self.place_index.get(id).copied()
    }

    /// Look up the graph node index for a transition.
    pub fn transition_node(&self, id: &TransitionId) -> Option<NodeIndex> {
        self.transition_index.get(id).copied()
    }

    /// Get a place by its ID.
    pub fn place(&self, id: &PlaceId) -> Option<&Place> {
        self.place_node(id)
            .and_then(|idx| self.graph[idx].as_place())
    }

    /// Get a transition by its ID.
    pub fn transition(&self, id: &TransitionId) -> Option<&Transition> {
        self.transition_node(id)
            .and_then(|idx| self.graph[idx].as_transition())
    }

    /// Iterate over all place IDs.
    pub fn place_ids(&self) -> impl Iterator<Item = &PlaceId> {
        self.place_index.keys()
    }

    /// Iterate over all transition IDs.
    pub fn transition_ids(&self) -> impl Iterator<Item = &TransitionId> {
        self.transition_index.keys()
    }

    /// Iterate over all places.
    pub fn places(&self) -> impl Iterator<Item = &Place> {
        self.place_index.values().map(|&idx| {
            self.graph[idx]
                .as_place()
                .expect("place_index points to Place node")
        })
    }

    /// Iterate over all transitions.
    pub fn transitions(&self) -> impl Iterator<Item = &Transition> {
        self.transition_index.values().map(|&idx| {
            self.graph[idx]
                .as_transition()
                .expect("transition_index points to Transition node")
        })
    }

    /// Get the input arcs for a transition (Place → Transition edges).
    pub fn input_arcs(&self, tid: &TransitionId) -> Vec<&InputArcData> {
        let Some(&t_idx) = self.transition_index.get(tid) else {
            return Vec::new();
        };
        self.graph
            .edges_directed(t_idx, Direction::Incoming)
            .filter_map(|e| match e.weight() {
                NetEdge::Input(data) => Some(data),
                NetEdge::Output(_) => None,
            })
            .collect()
    }

    /// Get the output arcs for a transition (Transition → Place edges).
    pub fn output_arcs(&self, tid: &TransitionId) -> Vec<&OutputArcData> {
        let Some(&t_idx) = self.transition_index.get(tid) else {
            return Vec::new();
        };
        self.graph
            .edges_directed(t_idx, Direction::Outgoing)
            .filter_map(|e| match e.weight() {
                NetEdge::Output(data) => Some(data),
                NetEdge::Input(_) => None,
            })
            .collect()
    }

    /// Check whether a transition is enabled in the given state.
    ///
    /// A transition is enabled iff:
    /// - All input arcs have sufficient tokens (`M(p) >= weight`)
    /// - All input arc guards do not evaluate to `false`
    pub fn is_enabled(&self, tid: &TransitionId, state: &State) -> bool {
        let input_arcs = self.input_arcs(tid);
        for arc in &input_arcs {
            if state.tokens(&arc.place) < arc.weight {
                return false;
            }
            let guard_result = eval_guard(&arc.guard, &state.vars);
            if guard_result == GuardResult::False {
                return false;
            }
        }
        true
    }

    /// Get all enabled transition IDs in the given state.
    pub fn enabled_transitions(&self, state: &State) -> Vec<TransitionId> {
        self.transition_index
            .keys()
            .filter(|tid| self.is_enabled(tid, state))
            .cloned()
            .collect()
    }

    /// Fire a transition, producing a new state.
    ///
    /// Returns an error if the transition is not enabled (insufficient tokens).
    pub fn fire(
        &self,
        tid: &TransitionId,
        state: &State,
    ) -> Result<State, CvnError> {
        let mut new_marking = state.marking.clone();
        let mut new_vars = state.vars.clone();

        // Consume tokens from input arcs
        for arc in self.input_arcs(tid) {
            let current = new_marking.get(&arc.place).copied().unwrap_or(0);
            if current < arc.weight {
                return Err(CvnError::new(
                    ErrorCode::V301,
                    format!(
                        "insufficient tokens at place '{}': have {}, need {}",
                        arc.place, current, arc.weight
                    ),
                    ErrorLocation::Arc {
                        place: arc.place.clone(),
                        transition: tid.clone(),
                    },
                ));
            }
            let remaining = current - arc.weight;
            if remaining == 0 {
                new_marking.remove(&arc.place);
            } else {
                new_marking.insert(arc.place.clone(), remaining);
            }
        }

        // Produce tokens on output arcs and apply variable updates
        for arc in self.output_arcs(tid) {
            let current = new_marking.get(&arc.place).copied().unwrap_or(0);
            new_marking.insert(arc.place.clone(), current + arc.weight);

            if let Some(update) = &arc.update {
                for (var_name, expr) in update {
                    let val = eval_expr(expr, &state.vars);
                    new_vars.insert(var_name.clone(), val);
                }
            }
        }

        Ok(State::new(new_marking, new_vars))
    }

    /// Get the number of places in the net.
    pub fn place_count(&self) -> usize {
        self.place_index.len()
    }

    /// Get the number of transitions in the net.
    pub fn transition_count(&self) -> usize {
        self.transition_index.len()
    }
}
