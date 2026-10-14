//! CVN property checks over an explored [`ReachabilityGraph`].
//!
//! What is left here is only the part that is CVN knowledge. The deadlock
//! *definition* now lives on [`Net`](crate::net::Net) behind
//! [`PlaceRole`](crate::net::PlaceRole), and "never fired" in
//! [`unfired_labels`]; this module adds the CVN's source attribution
//! (anchors) and its disjunctive families.

use crate::analysis::labels::unfired_labels;
use crate::analysis::{Counterexample, FiringStep, PropertyViolation, ReachabilityGraph};
use crate::cvn::{CvnNet, CvnState};
use crate::net::{PlaceId, TransitionId};

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

/// What a dead transition is reported as: its disjunctive family if it has
/// one, otherwise itself.
#[derive(Clone, PartialEq, Eq, Hash)]
enum DeadGroup {
    Family(String),
    Alone(TransitionId),
}

/// Find transitions that never fire behaviorally, reporting a whole disjunctive
/// family once when every one of its members is dead.
///
/// This is [`unfired_labels`] under one particular labeling — a transition's
/// family, or the transition itself when it has none — so "one dead branch of
/// an OR is not a defect, a dead OR is" is just "a label is dead when none of
/// its transitions fired". A dead family is reported at its lowest-id member;
/// what the CVN adds on top is the ConcIR anchors that let the repair layer
/// point at source.
pub fn find_dead_transitions(
    net: &CvnNet,
    graph: &ReachabilityGraph<CvnState>,
) -> Vec<Counterexample<CvnState>> {
    let initial = graph.states[graph.initial].clone();
    unfired_labels(net, graph, |t| {
        Some(match &t.kind.family {
            Some(family) => DeadGroup::Family(family.clone()),
            None => DeadGroup::Alone(t.id),
        })
    })
    .into_iter()
    .map(|(_, members)| {
        let t = members[0];
        Counterexample {
            kind: PropertyViolation::DeadTransition {
                transition: t,
                anchors: net
                    .transition(t)
                    .map_or_else(Vec::new, |tr| tr.kind.anchors.clone()),
            },
            trace: Vec::new(),
            final_state: initial.clone(),
        }
    })
    .collect()
}
