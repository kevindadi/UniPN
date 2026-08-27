//! CVN property checks over an explored [`ReachabilityGraph`]: deadlock
//! classification, dead-transition (and dead disjunctive family) detection, and
//! structural conflict sets.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::analysis::{Counterexample, FiringStep, PropertyViolation, ReachabilityGraph};
use crate::cvn::{CvnNet, CvnState};
use crate::net::{ArcDir, PlaceId, TransitionId};

/// Decide whether a state with no enabled transitions is a deadlock: at least
/// one control-flow token sits on a non-terminal, non-resource place.
pub fn is_deadlock(net: &CvnNet, state: &CvnState) -> bool {
    state
        .marking
        .iter_nonzero()
        .any(|(p, _)| !net.is_resource(p) && !net.is_thread_terminal(p))
}

/// The blocked (control-flow, non-terminal) places in a deadlock state.
pub fn blocked_places(net: &CvnNet, state: &CvnState) -> Vec<PlaceId> {
    state
        .marking
        .iter_nonzero()
        .filter(|(p, _)| !net.is_resource(*p) && !net.is_thread_terminal(*p))
        .map(|(p, _)| p)
        .collect()
}

fn trace_of(net: &CvnNet, graph: &ReachabilityGraph<CvnState>, target: usize) -> Vec<FiringStep> {
    graph
        .trace_to(target)
        .into_iter()
        .map(|t| FiringStep {
            transition: t,
            anchors: net
                .transition(t)
                .map_or_else(Vec::new, |tr| tr.kind.anchors.clone()),
        })
        .collect()
}

/// Classify the graph's blocked states into deadlock counterexamples.
pub fn find_deadlocks(
    net: &CvnNet,
    graph: &ReachabilityGraph<CvnState>,
) -> Vec<Counterexample<CvnState>> {
    graph
        .blocked
        .iter()
        .filter(|&&i| is_deadlock(net, &graph.states[i]))
        .map(|&i| Counterexample {
            kind: PropertyViolation::Deadlock,
            trace: trace_of(net, graph, i),
            final_state: graph.states[i].clone(),
        })
        .collect()
}

/// Find transitions that never fire behaviorally (or whole disjunctive families
/// that are dead).
pub fn find_dead_transitions(
    net: &CvnNet,
    graph: &ReachabilityGraph<CvnState>,
) -> Vec<Counterexample<CvnState>> {
    let fired = graph.fired_transitions();

    let mut live_families: HashSet<&str> = HashSet::new();
    let mut all: Vec<TransitionId> = net.transition_ids().collect();
    for t in &all {
        if fired.contains(t)
            && let Some(f) = net.transition(*t).and_then(|tr| tr.kind.family.as_deref())
        {
            live_families.insert(f);
        }
    }
    all.sort_by_key(|t| t.index());

    let initial = graph.states[graph.initial].clone();
    let mut reported_families: HashSet<&str> = HashSet::new();
    let mut dead = Vec::new();

    for t in all {
        if fired.contains(&t) {
            continue;
        }
        if let Some(f) = net.transition(t).and_then(|tr| tr.kind.family.as_deref()) {
            if live_families.contains(f) {
                continue;
            }
            if !reported_families.insert(f) {
                continue;
            }
        }
        dead.push(Counterexample {
            kind: PropertyViolation::DeadTransition {
                transition: t,
                anchors: net
                    .transition(t)
                    .map_or_else(Vec::new, |tr| tr.kind.anchors.clone()),
            },
            trace: Vec::new(),
            final_state: initial.clone(),
        });
    }

    dead.sort_by_key(|cx| match &cx.kind {
        PropertyViolation::DeadTransition { transition, .. } => transition.index(),
        _ => 0,
    });
    dead
}

/// Transition pairs sharing an input place (potential races/conflicts).
pub fn conflict_sets(net: &CvnNet) -> Vec<(TransitionId, TransitionId)> {
    let mut by_place: HashMap<PlaceId, Vec<TransitionId>> = HashMap::new();
    for t in net.transition_ids() {
        for arc in net.arcs_of(t, ArcDir::Input) {
            by_place.entry(arc.place).or_default().push(t);
        }
    }

    let mut pairs: BTreeSet<(TransitionId, TransitionId)> = BTreeSet::new();
    for consumers in by_place.values() {
        for i in 0..consumers.len() {
            for j in (i + 1)..consumers.len() {
                let a = consumers[i].min(consumers[j]);
                let b = consumers[i].max(consumers[j]);
                pairs.insert((a, b));
            }
        }
    }
    pairs.into_iter().collect()
}
