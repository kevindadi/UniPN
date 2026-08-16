//! Net-level analysis: reachability exploration and deadlock classification.
//!
//! The algorithms are domain-neutral and depend only on the [`NetLike`]
//! firing contract. The explorer reports which states are *blocked* (no
//! enabled transitions); whether a blocked state is a *deadlock* is decided by
//! the caller via [`find_deadlocks`].
//!
//! P/T analysis lives in [`pt`]. Timed (DBM/state-class) analysis is
//! `analysis::timed` when the `timed` feature is enabled.

pub mod pt;
#[cfg(feature = "timed")]
pub mod timed;

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;

use crate::ids::TransitionId;

/// The firing contract a net must satisfy to be analyzed.
///
/// Each net (P/T, timed, CVN) supplies its own `enabled`/`fire` and its own
/// `State` (marking + per-net extra data). The model itself is not abstracted
/// here — only this minimal execution surface.
pub trait NetLike {
    type State: Clone + Eq + Hash;

    fn num_places(&self) -> usize;
    fn num_transitions(&self) -> usize;

    /// Transitions enabled under `state`.
    fn enabled(&self, state: &Self::State) -> Vec<TransitionId>;

    /// Fire `transition` from `state`; `None` if it is not enabled.
    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State>;
}

/// Search strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchStrategy {
    #[default]
    Bfs,
    Dfs,
}

/// Exploration configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalysisConfig {
    pub strategy: SearchStrategy,
    pub max_states: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            strategy: SearchStrategy::Bfs,
            max_states: 100_000,
        }
    }
}

/// A reachability graph over a net's states.
#[derive(Clone, Debug)]
pub struct ReachabilityGraph<S> {
    pub states: Vec<S>,
    pub edges: Vec<(usize, usize, TransitionId)>,
    pub initial: usize,
    /// Indices of states with no enabled transitions (candidate deadlocks).
    pub blocked: Vec<usize>,
    pub truncated: bool,
    preds: HashMap<usize, (usize, TransitionId)>,
}

impl<S> ReachabilityGraph<S> {
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// The set of transitions that fired on any edge of the graph (used for
    /// dead-transition detection).
    pub fn fired_transitions(&self) -> HashSet<TransitionId> {
        self.edges.iter().map(|(_, _, t)| *t).collect()
    }

    /// Reconstruct the firing sequence that reaches `target` from the initial
    /// state.
    pub fn trace_to(&self, target: usize) -> Vec<TransitionId> {
        let mut path = Vec::new();
        let mut current = target;
        while let Some(&(parent, t)) = self.preds.get(&current) {
            path.push(t);
            current = parent;
        }
        path.reverse();
        path
    }
}

/// Explore the whole reachable state space from `initial`.
pub fn explore<N: NetLike>(
    net: &N,
    initial: N::State,
    config: &AnalysisConfig,
) -> ReachabilityGraph<N::State> {
    let mut explorer: Explorer<N> = Explorer::new(config.max_states);
    let (init_idx, _) = explorer.insert(initial).unwrap();
    let mut pending = VecDeque::from([init_idx]);

    while let Some(idx) = match config.strategy {
        SearchStrategy::Bfs => pending.pop_front(),
        SearchStrategy::Dfs => pending.pop_back(),
    } {
        let state = explorer.states[idx].clone();
        let enabled = net.enabled(&state);
        if enabled.is_empty() {
            explorer.blocked.push(idx);
            continue;
        }
        for transition in enabled {
            let Some(next) = net.fire(&state, transition) else {
                continue;
            };
            let Some((target, is_new)) = explorer.insert(next) else {
                continue;
            };
            explorer.record_edge(idx, target, transition);
            if is_new {
                pending.push_back(target);
            }
        }
    }

    explorer.finish()
}

/// Classify the blocked states of a graph into deadlock state indices using the
/// caller-supplied predicate.
pub fn find_deadlocks<S>(
    graph: &ReachabilityGraph<S>,
    is_deadlock: impl Fn(&S) -> bool,
) -> Vec<usize> {
    graph
        .blocked
        .iter()
        .copied()
        .filter(|&i| is_deadlock(&graph.states[i]))
        .collect()
}

/// A single firing step in a counterexample trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiringStep {
    pub transition: TransitionId,
    /// Anchoring back to the source (ConcIR sid / source line).
    pub anchors: Vec<String>,
}

/// The kind of property violation a counterexample demonstrates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyViolation {
    Deadlock,
    DeadTransition {
        transition: TransitionId,
        anchors: Vec<String>,
    },
    GoalUnmet {
        goal: String,
    },
}

/// A counterexample: a firing sequence + final state + violation kind.
#[derive(Clone, Debug)]
pub struct Counterexample<S> {
    pub kind: PropertyViolation,
    pub trace: Vec<FiringStep>,
    pub final_state: S,
}

struct Explorer<N: NetLike> {
    max_states: usize,
    states: Vec<N::State>,
    seen: HashMap<N::State, usize>,
    edges: Vec<(usize, usize, TransitionId)>,
    preds: HashMap<usize, (usize, TransitionId)>,
    blocked: Vec<usize>,
    truncated: bool,
}

impl<N: NetLike> Explorer<N> {
    fn new(max_states: usize) -> Self {
        Self {
            max_states,
            states: Vec::new(),
            seen: HashMap::new(),
            edges: Vec::new(),
            preds: HashMap::new(),
            blocked: Vec::new(),
            truncated: false,
        }
    }

    fn insert(&mut self, state: N::State) -> Option<(usize, bool)> {
        match self.seen.entry(state.clone()) {
            std::collections::hash_map::Entry::Occupied(e) => Some((*e.get(), false)),
            std::collections::hash_map::Entry::Vacant(v) => {
                if self.states.len() >= self.max_states {
                    self.truncated = true;
                    return None;
                }
                let idx = self.states.len();
                self.states.push(state);
                v.insert(idx);
                Some((idx, true))
            }
        }
    }

    fn record_edge(&mut self, src: usize, dst: usize, t: TransitionId) {
        self.edges.push((src, dst, t));
        self.preds.entry(dst).or_insert((src, t));
    }

    fn finish(self) -> ReachabilityGraph<N::State> {
        ReachabilityGraph {
            states: self.states,
            edges: self.edges,
            initial: 0,
            blocked: self.blocked,
            truncated: self.truncated,
            preds: self.preds,
        }
    }
}
