//! CVN property checks over an explored [`ReachabilityGraph`].
//!
//! What is left here is only the part that is CVN knowledge. The deadlock
//! *definition* now lives on [`Net`](crate::net::Net) behind
//! [`PlaceRole`](crate::net::PlaceRole), and "never fired" in
//! [`unfired_transitions`]; this module adds the CVN's source attribution
//! (anchors) and its disjunctive families.

use std::collections::HashSet;

use crate::analysis::{
    Counterexample, FiringStep, PropertyViolation, ReachabilityGraph, unfired_transitions,
};
use crate::cvn::{CvnNet, CvnState};
use crate::net::PlaceId;

/// Decide whether a state with no enabled transitions is a deadlock: at least
/// one control-flow token sits on a non-terminal, non-resource place.
pub fn is_deadlock(net: &CvnNet, state: &CvnState) -> bool {
    net.is_deadlock(&state.marking)
}

/// The blocked (control-flow, non-terminal) places in a deadlock state.
pub fn blocked_places(net: &CvnNet, state: &CvnState) -> Vec<PlaceId> {
    net.blocked_places(&state.marking)
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

/// Find transitions that never fire behaviorally, reporting a whole disjunctive
/// family once when every one of its members is dead.
///
/// The "never fired" part is [`unfired_transitions`]; what the CVN adds is the
/// family folding — one dead branch of an OR is not a defect, a dead OR is —
/// and the ConcIR anchors that let the repair layer point at source.
pub fn find_dead_transitions(
    net: &CvnNet,
    graph: &ReachabilityGraph<CvnState>,
) -> Vec<Counterexample<CvnState>> {
    let fired = graph.fired_transitions();
    let family_of = |t| net.transition(t).and_then(|tr| tr.kind.family.as_deref());

    let live_families: HashSet<&str> = fired.iter().filter_map(|&t| family_of(t)).collect();

    let initial = graph.states[graph.initial].clone();
    let mut reported_families: HashSet<&str> = HashSet::new();
    let mut dead = Vec::new();

    for t in unfired_transitions(net, graph) {
        if let Some(family) = family_of(t)
            && (live_families.contains(family) || !reported_families.insert(family))
        {
            continue;
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

    dead
}
