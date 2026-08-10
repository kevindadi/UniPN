//! Using Petri nets to guide concurrent test-case generation.
//!
//! Core insight: **every path of the reachability graph is a thread-interleaving
//! schedule**; a test case = path (schedule) + the data constraints along the way
//! (variable store). This module is a pure consumer above the reachability
//! graph and only reads [`crate::netlike::NetLike`].

use crate::analysis::{FiringStep, ReachabilityGraph};
use crate::state::{State, VarStore};

/// Coverage criteria.
#[derive(Clone, Debug, Default)]
pub enum CoverageCriteria {
    /// Path coverage: one test case per executable path (greedy dedup).
    #[default]
    Path,
    /// Conflict-pair coverage: generate an interleaving-pair test for every
    /// transition pair sharing an input place.
    ConflictPair,
    /// Boundary-state coverage: cover all guard-critical states (`x==k` /
    /// `x>=k`).
    BoundaryState,
    /// Deadlock regression: directly use deadlock counterexample traces.
    DeadlockRegression,
}

/// A generated test case.
#[derive(Clone, Debug)]
pub struct TestCase {
    /// The schedule: (transition, anchoring) in execution order.
    pub schedule: Vec<FiringStep>,
    /// The state before each step (for assertions/replay).
    pub states: Vec<State>,
    /// Initial variable bindings (data constraints).
    pub input_bindings: VarStore,
    /// Expected assertions (target states / invariants).
    pub expectations: Vec<String>,
}

/// Extract "longest terminating paths" from the reachability graph as a base
/// test-case set.
pub fn extract_schedules(rg: &ReachabilityGraph) -> Vec<Vec<FiringStep>> {
    // Simple heuristic: DFS from the initial state to a terminal (no enabled
    // transitions or a deadlock), taking one depth-first path at a time.
    let mut out = Vec::new();
    let mut visited = vec![false; rg.states.len()];
    dfs_paths(rg, rg.initial, &mut visited, &mut Vec::new(), &mut out);
    out
}

fn dfs_paths(
    rg: &ReachabilityGraph,
    idx: usize,
    visited: &mut Vec<bool>,
    path: &mut Vec<FiringStep>,
    out: &mut Vec<Vec<FiringStep>>,
) {
    visited[idx] = true;
    let outgoing: Vec<(usize, usize, crate::ids::TransitionId)> = rg
        .edges
        .iter()
        .filter(|(s, _, _)| *s == idx)
        .copied()
        .collect();

    if outgoing.is_empty() {
        if !path.is_empty() {
            out.push(path.clone());
        }
        visited[idx] = false;
        return;
    }

    for (_, dst, t) in outgoing {
        let anchors = Vec::new();
        path.push(FiringStep {
            transition: t,
            anchors,
        });
        if !visited[dst] {
            dfs_paths(rg, dst, visited, path, out);
        }
        path.pop();
    }
    visited[idx] = false;
}

/// Generate test cases (reserved).
pub fn generate_tests(
    _net: &dyn crate::netlike::NetLike,
    _rg: &ReachabilityGraph,
    _criteria: CoverageCriteria,
) -> Vec<TestCase> {
    // TODO: derive test cases from extract_schedules per criteria + variable
    // bindings + assertions.
    Vec::new()
}
