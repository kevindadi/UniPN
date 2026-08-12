//! State-space exploration: BFS / DFS / partial-order reduction (sleep-set).
//!
//! Uses a standalone reachability-graph storage (no petgraph dependency):
//! `states: Vec<State>` + `edges: Vec<(src, dst, transition)>`.
//!
//! The explorer is domain-neutral: it reports which states are *blocked* (no
//! enabled transitions) and leaves deadlock classification to the caller
//! (see [`super::find_deadlocks`]).

use std::collections::{HashMap, HashSet, VecDeque};

use crate::ids::TransitionId;
use crate::netlike::NetLike;
use crate::state::State;

use super::{AnalysisConfig, FiringStep, SearchStrategy};

/// Reachability graph.
#[derive(Clone, Debug)]
pub struct ReachabilityGraph {
    pub states: Vec<State>,
    pub edges: Vec<(usize, usize, TransitionId)>,
    pub initial: usize,
    /// Indices of states with no enabled transitions (candidate deadlocks).
    /// Whether each one is a *real* deadlock is decided by the caller.
    pub blocked: Vec<usize>,
    pub truncated: bool,
    /// Predecessor link for trace reconstruction: `target → (source, transition,
    /// anchors)`.
    preds: HashMap<usize, (usize, TransitionId, Vec<String>)>,
}

impl ReachabilityGraph {
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
    pub fn trace_to(&self, target: usize) -> Vec<FiringStep> {
        let mut path = Vec::new();
        let mut current = target;
        while let Some(&(parent, t, ref anchors)) = self.preds.get(&current) {
            path.push(FiringStep {
                transition: t,
                anchors: anchors.clone(),
            });
            current = parent;
        }
        path.reverse();
        path
    }
}

/// Explore the whole reachable state space.
pub fn explore(net: &dyn NetLike, config: &AnalysisConfig) -> ReachabilityGraph {
    if config.por {
        return explore_por(net, config.max_states);
    }
    match config.strategy {
        SearchStrategy::Bfs => explore_bfs(net, config.max_states),
        SearchStrategy::Dfs => explore_dfs(net, config.max_states),
    }
}

// ── BFS ──

fn explore_bfs(net: &dyn NetLike, max_states: usize) -> ReachabilityGraph {
    let mut e = Explorer::new(net, max_states);
    let initial = net.initial_state();
    let (init_idx, _) = e.insert_state(initial).unwrap();
    let mut queue = VecDeque::new();
    queue.push_back(init_idx);

    while let Some(idx) = queue.pop_front() {
        let state = e.states[idx].clone();
        let enabled = net.enabled_transitions(&state);
        if enabled.is_empty() {
            e.blocked.push(idx);
            continue;
        }
        for t in enabled {
            if let Ok(next) = net.fire(t, &state)
                && let Some((target, is_new)) = e.insert_state(next)
            {
                e.record_edge(idx, target, t);
                if is_new {
                    queue.push_back(target);
                }
            }
        }
    }
    e.finish()
}

// ── DFS ──

fn explore_dfs(net: &dyn NetLike, max_states: usize) -> ReachabilityGraph {
    let mut e = Explorer::new(net, max_states);
    let initial = net.initial_state();
    let (init_idx, _) = e.insert_state(initial).unwrap();
    let mut stack = vec![init_idx];

    while let Some(idx) = stack.pop() {
        let state = e.states[idx].clone();
        let enabled = net.enabled_transitions(&state);
        if enabled.is_empty() {
            e.blocked.push(idx);
            continue;
        }
        for t in enabled {
            if let Ok(next) = net.fire(t, &state)
                && let Some((target, is_new)) = e.insert_state(next)
            {
                e.record_edge(idx, target, t);
                if is_new {
                    stack.push(target);
                }
            }
        }
    }
    e.finish()
}

// ── POR (sleep-set) ──

fn explore_por(net: &dyn NetLike, max_states: usize) -> ReachabilityGraph {
    let mut e = Explorer::new(net, max_states);
    let initial = net.initial_state();
    let (init_idx, _) = e.insert_state(initial).unwrap();

    let mut queue: VecDeque<(usize, HashSet<TransitionId>)> = VecDeque::new();
    let mut sleep_sets: HashMap<usize, HashSet<TransitionId>> = HashMap::default();
    queue.push_back((init_idx, HashSet::default()));

    while let Some((idx, sleep)) = queue.pop_front() {
        if e.states.len() > max_states {
            break;
        }
        let state = e.states[idx].clone();
        let enabled: HashSet<TransitionId> = net.enabled_transitions(&state).into_iter().collect();

        if enabled.is_empty() {
            e.blocked.push(idx);
            continue;
        }

        let to_fire: Vec<TransitionId> = enabled.difference(&sleep).copied().collect();
        for t in to_fire {
            let Ok(next) = net.fire(t, &state) else {
                continue;
            };
            let enabled_next: HashSet<TransitionId> =
                net.enabled_transitions(&next).into_iter().collect();

            // Add enabled transitions independent of `t` to the sleep set:
            // their interleavings commute, so only one representative order is
            // expanded.
            let mut new_sleep = sleep.clone();
            for &tt in &enabled {
                if tt != t && transitions_are_independent(net, t, tt) {
                    new_sleep.insert(tt);
                }
            }
            new_sleep = new_sleep.intersection(&enabled_next).copied().collect();

            let Some((target, is_new)) = e.insert_state(next) else {
                continue;
            };
            e.record_edge(idx, target, t);

            if is_new {
                // New state: enqueue with the current sleep set.
                sleep_sets.insert(target, new_sleep.clone());
                queue.push_back((target, new_sleep));
            } else {
                // Same marking reached with a different sleep set: merge by
                // intersection so no deadlock is lost.
                let old_sleep = sleep_sets.get(&target).cloned().unwrap_or_default();
                let merged: HashSet<TransitionId> =
                    old_sleep.intersection(&new_sleep).copied().collect();
                if merged != old_sleep {
                    sleep_sets.insert(target, merged.clone());
                    queue.push_back((target, merged));
                }
            }
        }
    }
    e.finish()
}

/// Whether two transitions are independent: they share no place (neither in
/// their pre nor post). Independent transitions commute.
fn transitions_are_independent(net: &dyn NetLike, t1: TransitionId, t2: TransitionId) -> bool {
    if t1 == t2 {
        return false;
    }
    let mut places: HashSet<usize> = HashSet::default();
    for (p, _) in net.pre_arcs(t1).into_iter().chain(net.post_arcs(t1)) {
        places.insert(p.index());
    }
    for (p, _) in net.pre_arcs(t2).into_iter().chain(net.post_arcs(t2)) {
        if places.contains(&p.index()) {
            return false;
        }
    }
    true
}

// ── Shared explorer ──

struct Explorer<'a> {
    net: &'a dyn NetLike,
    max_states: usize,
    states: Vec<State>,
    seen: HashMap<State, usize>,
    edges: Vec<(usize, usize, TransitionId)>,
    preds: HashMap<usize, (usize, TransitionId, Vec<String>)>,
    blocked: Vec<usize>,
    truncated: bool,
}

impl<'a> Explorer<'a> {
    fn new(net: &'a dyn NetLike, max_states: usize) -> Self {
        Self {
            net,
            max_states,
            states: Vec::new(),
            seen: HashMap::default(),
            edges: Vec::new(),
            preds: HashMap::default(),
            blocked: Vec::new(),
            truncated: false,
        }
    }

    fn insert_state(&mut self, state: State) -> Option<(usize, bool)> {
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
        self.preds
            .entry(dst)
            .or_insert_with(|| (src, t, self.net.transition_anchors(t)));
    }

    fn finish(self) -> ReachabilityGraph {
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
