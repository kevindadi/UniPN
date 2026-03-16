//! State space search engine (BFS/DFS) for CVN analysis.

use crate::analysis::counterexample::{Counterexample, FiringStep, PropertyViolation};
use crate::analysis::deadlock;
use crate::error::{CvnError, ErrorCode, ErrorLocation};
use crate::model::{State, TransitionId};
use crate::net::CvnNet;
use petgraph::graph::{DiGraph, NodeIndex};
use rustc_hash::FxHashMap;
use std::collections::VecDeque;

/// Search strategy for state space exploration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchStrategy {
    /// Breadth-first search (finds shortest counterexamples).
    #[default]
    Bfs,
    /// Depth-first search (lower memory usage).
    Dfs,
}

/// Configuration for the analysis engine.
#[derive(Clone, Debug)]
pub struct AnalysisConfig {
    /// The search strategy to use.
    pub strategy: SearchStrategy,
    /// Maximum number of states to explore before aborting.
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

/// Result of a state space analysis.
#[derive(Clone, Debug)]
pub struct AnalysisResult {
    /// The reachability graph: nodes are states, edges are transition IDs.
    pub reachability_graph: DiGraph<State, TransitionId>,
    /// All detected deadlock counterexamples.
    pub deadlocks: Vec<Counterexample>,
    /// Total number of states explored.
    pub state_count: usize,
}

/// Explore the full reachable state space of a CVN network.
///
/// Returns the reachability graph and any deadlocks found.
pub fn explore(net: &CvnNet, config: &AnalysisConfig) -> Result<AnalysisResult, CvnError> {
    match config.strategy {
        SearchStrategy::Bfs => explore_bfs(net, config),
        SearchStrategy::Dfs => explore_dfs(net, config),
    }
}

fn explore_bfs(net: &CvnNet, config: &AnalysisConfig) -> Result<AnalysisResult, CvnError> {
    let mut graph = DiGraph::<State, TransitionId>::new();
    let mut state_to_node: FxHashMap<u64, NodeIndex> = FxHashMap::default();
    let mut deadlocks = Vec::new();
    let mut queue = VecDeque::new();

    // Predecessor tracking: node_index -> (parent_node_index, transition_id)
    let mut predecessors: FxHashMap<NodeIndex, (NodeIndex, TransitionId)> = FxHashMap::default();

    let initial = net.initial_state();
    let initial_hash = hash_state(&initial);
    let initial_node = graph.add_node(initial.clone());
    state_to_node.insert(initial_hash, initial_node);
    queue.push_back(initial_node);

    while let Some(current_node) = queue.pop_front() {
        if graph.node_count() > config.max_states {
            return Err(CvnError::new(
                ErrorCode::V302,
                format!(
                    "state space explosion: exceeded {} states",
                    config.max_states
                ),
                ErrorLocation::None,
            ));
        }

        let current_state = graph[current_node].clone();
        let enabled = net.enabled_transitions(&current_state);

        if enabled.is_empty() && deadlock::is_deadlock(net, &current_state) {
            let trace = reconstruct_trace(&graph, &predecessors, current_node, net);
            deadlocks.push(Counterexample {
                kind: PropertyViolation::Deadlock,
                trace,
                final_state: current_state.clone(),
            });
            continue;
        }

        for tid in &enabled {
            let new_state = net.fire(tid, &current_state).expect("enabled => can fire");
            let new_hash = hash_state(&new_state);

            let target_node = if let Some(&existing) = state_to_node.get(&new_hash) {
                existing
            } else {
                let node = graph.add_node(new_state);
                state_to_node.insert(new_hash, node);
                predecessors.insert(node, (current_node, tid.clone()));
                queue.push_back(node);
                node
            };

            graph.add_edge(current_node, target_node, tid.clone());
        }
    }

    Ok(AnalysisResult {
        state_count: graph.node_count(),
        reachability_graph: graph,
        deadlocks,
    })
}

fn explore_dfs(net: &CvnNet, config: &AnalysisConfig) -> Result<AnalysisResult, CvnError> {
    let mut graph = DiGraph::<State, TransitionId>::new();
    let mut state_to_node: FxHashMap<u64, NodeIndex> = FxHashMap::default();
    let mut deadlocks = Vec::new();
    let mut stack = Vec::new();

    let mut predecessors: FxHashMap<NodeIndex, (NodeIndex, TransitionId)> = FxHashMap::default();

    let initial = net.initial_state();
    let initial_hash = hash_state(&initial);
    let initial_node = graph.add_node(initial.clone());
    state_to_node.insert(initial_hash, initial_node);
    stack.push(initial_node);

    while let Some(current_node) = stack.pop() {
        if graph.node_count() > config.max_states {
            return Err(CvnError::new(
                ErrorCode::V302,
                format!(
                    "state space explosion: exceeded {} states",
                    config.max_states
                ),
                ErrorLocation::None,
            ));
        }

        let current_state = graph[current_node].clone();
        let enabled = net.enabled_transitions(&current_state);

        if enabled.is_empty() && deadlock::is_deadlock(net, &current_state) {
            let trace = reconstruct_trace(&graph, &predecessors, current_node, net);
            deadlocks.push(Counterexample {
                kind: PropertyViolation::Deadlock,
                trace,
                final_state: current_state.clone(),
            });
            continue;
        }

        for tid in &enabled {
            let new_state = net.fire(tid, &current_state).expect("enabled => can fire");
            let new_hash = hash_state(&new_state);

            let target_node = if let Some(&existing) = state_to_node.get(&new_hash) {
                existing
            } else {
                let node = graph.add_node(new_state);
                state_to_node.insert(new_hash, node);
                predecessors.insert(node, (current_node, tid.clone()));
                stack.push(node);
                node
            };

            graph.add_edge(current_node, target_node, tid.clone());
        }
    }

    Ok(AnalysisResult {
        state_count: graph.node_count(),
        reachability_graph: graph,
        deadlocks,
    })
}

fn hash_state(state: &State) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    state.hash(&mut hasher);
    hasher.finish()
}

fn reconstruct_trace(
    _graph: &DiGraph<State, TransitionId>,
    predecessors: &FxHashMap<NodeIndex, (NodeIndex, TransitionId)>,
    target: NodeIndex,
    net: &CvnNet,
) -> Vec<FiringStep> {
    let mut path = Vec::new();
    let mut current = target;

    while let Some((parent, tid)) = predecessors.get(&current) {
        let anchor_sids = net
            .transition(tid)
            .map(|t| t.anchor_sids.clone())
            .unwrap_or_default();
        path.push(FiringStep {
            transition_id: tid.clone(),
            anchor_sids,
        });
        current = *parent;
    }

    path.reverse();
    path
}

/// Check whether any path exists where a specific condition holds.
///
/// Returns `true` if any reachable state satisfies the predicate.
pub fn exists_path(
    net: &CvnNet,
    config: &AnalysisConfig,
    predicate: impl Fn(&State) -> bool,
) -> Result<bool, CvnError> {
    let result = explore(net, config)?;
    for node_idx in result.reachability_graph.node_indices() {
        if predicate(&result.reachability_graph[node_idx]) {
            return Ok(true);
        }
    }
    Ok(false)
}
