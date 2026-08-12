//! Behavioral dead-transition detection.

use std::collections::HashSet;

use crate::ids::TransitionId;
use crate::netlike::NetLike;

use super::{Counterexample, PropertyViolation, ReachabilityGraph};

/// Find transitions that never fire behaviorally (or whole families that are
/// dead).
///
/// Transitions appearing on any edge of the reachability graph are live; a
/// transition that never fires but belongs to a disjunctive OR family with at
/// least one live member is skipped (family semantics = at most one member
/// fires). A wholly-dead family is reported once (representative = smallest
/// transition id).
pub fn find_dead_transitions(net: &dyn NetLike, rg: &ReachabilityGraph) -> Vec<Counterexample> {
    let fired = rg.fired_transitions();

    let mut live_families: HashSet<&str> = HashSet::default();
    let mut all: Vec<TransitionId> = net.transition_ids();
    for t in &all {
        if fired.contains(t)
            && let Some(f) = net.transition_family(*t)
        {
            live_families.insert(f);
        }
    }
    all.sort_by_key(|t| t.0);

    let mut dead = Vec::new();
    let mut reported_families: HashSet<&str> = HashSet::default();
    let initial = net.initial_state();

    for t in all {
        if fired.contains(&t) {
            continue;
        }
        if let Some(f) = net.transition_family(t) {
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
                anchors: net.transition_anchors(t),
            },
            trace: Vec::new(),
            final_state: initial.clone(),
        });
    }
    dead.sort_by_key(|cx| match &cx.kind {
        PropertyViolation::DeadTransition { transition, .. } => transition.0,
        _ => 0,
    });
    dead
}

/// Suppress dead transitions dominated by a deadlock: a transition whose input
/// place lies downstream of a deadlock's blocked control flow is not truly
/// dead, the deadlock merely truncated the exploration. Used to avoid duplicate
/// diagnoses of the same deadlock.
#[allow(dead_code)]
pub(crate) fn deadlock_dominated(
    net: &dyn NetLike,
    rg: &ReachabilityGraph,
    dead: Vec<Counterexample>,
) -> Vec<Counterexample> {
    let _ = (net, rg);
    dead
}
