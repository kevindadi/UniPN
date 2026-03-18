//! Well-formedness validation for CVN networks.
//!
//! Checks conditions W2–W9 (W1 is guaranteed by the type system) and
//! structural invariants V0xx. Returns a list of all detected errors.

use crate::error::{CvnError, ErrorCode, ErrorLocation};
use crate::model::*;
use crate::net::CvnNet;
use rustc_hash::{FxHashMap, FxHashSet};

/// Validate a CVN network, returning all detected errors.
///
/// Called by [`CvnNetBuilder::build()`](crate::builder::CvnNetBuilder::build).
pub fn validate(
    net: &CvnNet,
    input_arcs: &[InputArcData],
    output_arcs: &[OutputArcData],
) -> Vec<CvnError> {
    let mut errors = Vec::new();

    check_arc_references(net, input_arcs, output_arcs, &mut errors);
    check_arc_weights(input_arcs, output_arcs, &mut errors);
    check_control_input_arcs(net, input_arcs, &mut errors);
    check_control_output_arcs(net, output_arcs, &mut errors);
    check_update_conflicts(net, output_arcs, &mut errors);
    check_branch_pairs(net, input_arcs, &mut errors);
    check_resource_initial_tokens(net, &mut errors);

    errors
}

/// V003/V004: arcs reference non-existent places or transitions.
fn check_arc_references(
    net: &CvnNet,
    input_arcs: &[InputArcData],
    output_arcs: &[OutputArcData],
    errors: &mut Vec<CvnError>,
) {
    for arc in input_arcs {
        if net.place(&arc.place).is_none() {
            errors.push(CvnError::new(
                ErrorCode::V003,
                format!("input arc references non-existent place '{}'", arc.place),
                ErrorLocation::Arc {
                    place: arc.place.clone(),
                    transition: arc.transition.clone(),
                },
            ));
        }
        if net.transition(&arc.transition).is_none() {
            errors.push(CvnError::new(
                ErrorCode::V004,
                format!(
                    "input arc references non-existent transition '{}'",
                    arc.transition
                ),
                ErrorLocation::Arc {
                    place: arc.place.clone(),
                    transition: arc.transition.clone(),
                },
            ));
        }
    }
    for arc in output_arcs {
        if net.transition(&arc.transition).is_none() {
            errors.push(CvnError::new(
                ErrorCode::V004,
                format!(
                    "output arc references non-existent transition '{}'",
                    arc.transition
                ),
                ErrorLocation::Transition(arc.transition.clone()),
            ));
        }
        if net.place(&arc.place).is_none() {
            errors.push(CvnError::new(
                ErrorCode::V003,
                format!("output arc references non-existent place '{}'", arc.place),
                ErrorLocation::Place(arc.place.clone()),
            ));
        }
    }
}

/// V005/V006: arc weights must be >= 1.
fn check_arc_weights(
    input_arcs: &[InputArcData],
    output_arcs: &[OutputArcData],
    errors: &mut Vec<CvnError>,
) {
    for arc in input_arcs {
        if arc.weight == 0 {
            errors.push(CvnError::new(
                ErrorCode::V005,
                format!(
                    "input arc from '{}' to '{}' has weight 0",
                    arc.place, arc.transition
                ),
                ErrorLocation::Arc {
                    place: arc.place.clone(),
                    transition: arc.transition.clone(),
                },
            ));
        }
    }
    for arc in output_arcs {
        if arc.weight == 0 {
            errors.push(CvnError::new(
                ErrorCode::V006,
                format!(
                    "output arc from '{}' to '{}' has weight 0",
                    arc.transition, arc.place
                ),
                ErrorLocation::Arc {
                    place: arc.place.clone(),
                    transition: arc.transition.clone(),
                },
            ));
        }
    }
}

/// W2 (V101/V102): each transition must have exactly one control input arc.
fn check_control_input_arcs(
    net: &CvnNet,
    input_arcs: &[InputArcData],
    errors: &mut Vec<CvnError>,
) {
    let mut control_input_count: FxHashMap<TransitionId, usize> = FxHashMap::default();

    for arc in input_arcs {
        if let Some(place) = net.place(&arc.place) {
            if place.is_control_flow() {
                *control_input_count
                    .entry(arc.transition.clone())
                    .or_insert(0) += 1;
            }
        }
    }

    for t in net.transitions() {
        let count = control_input_count.get(&t.id).copied().unwrap_or(0);
        // Join transitions require 2 control inputs (current thread + joined thread).
        let max_allowed = match &t.kind {
            TransitionKind::Join => 2,
            _ => 1,
        };
        if count == 0 {
            errors.push(CvnError::new(
                ErrorCode::V101,
                format!("transition '{}' has no control input arc", t.id),
                ErrorLocation::Transition(t.id.clone()),
            ));
        } else if count > max_allowed {
            errors.push(CvnError::new(
                ErrorCode::V102,
                format!(
                    "transition '{}' has {} control input arcs (max allowed {})",
                    t.id, count, max_allowed
                ),
                ErrorLocation::Transition(t.id.clone()),
            ));
        }
    }
}

/// W3 (V103): non-return transitions must have at least one control output arc.
fn check_control_output_arcs(
    net: &CvnNet,
    output_arcs: &[OutputArcData],
    errors: &mut Vec<CvnError>,
) {
    let mut has_control_output: FxHashSet<TransitionId> = FxHashSet::default();

    for arc in output_arcs {
        if let Some(place) = net.place(&arc.place) {
            if place.is_control_flow() {
                has_control_output.insert(arc.transition.clone());
            }
        }
    }

    for t in net.transitions() {
        if !t.is_return() && !has_control_output.contains(&t.id) {
            errors.push(CvnError::new(
                ErrorCode::V103,
                format!(
                    "non-return transition '{}' has no control output arc",
                    t.id
                ),
                ErrorLocation::Transition(t.id.clone()),
            ));
        }
    }
}

/// W4 (V104): output arcs on the same transition must not update the same variable.
fn check_update_conflicts(
    net: &CvnNet,
    output_arcs: &[OutputArcData],
    errors: &mut Vec<CvnError>,
) {
    let mut updates_per_transition: FxHashMap<TransitionId, FxHashSet<String>> =
        FxHashMap::default();

    for arc in output_arcs {
        if let Some(update) = &arc.update {
            let vars = updates_per_transition
                .entry(arc.transition.clone())
                .or_default();
            for var_name in update.keys() {
                if !vars.insert(var_name.clone()) {
                    // Check if transition exists before reporting
                    if net.transition(&arc.transition).is_some() {
                        errors.push(CvnError::new(
                            ErrorCode::V104,
                            format!(
                                "transition '{}' has conflicting updates for variable '{}'",
                                arc.transition, var_name
                            ),
                            ErrorLocation::Transition(arc.transition.clone()),
                        ));
                    }
                }
            }
        }
    }
}

/// W7 (V105): every transition must have at least one anchor SID.
///
/// Only available with the `cir-anchor` feature. Called separately from
/// [`validate()`] via [`CvnNetBuilder::build_with_anchor_check()`](crate::builder::CvnNetBuilder::build_with_anchor_check).
#[cfg(feature = "cir-anchor")]
pub(crate) fn check_anchor_sids(net: &CvnNet) -> Vec<CvnError> {
    let mut errors = Vec::new();
    for t in net.transitions() {
        if t.anchor_sids.is_empty() {
            errors.push(CvnError::new(
                ErrorCode::V105,
                format!("transition '{}' has no anchor SID", t.id),
                ErrorLocation::Transition(t.id.clone()),
            ));
        }
    }
    errors
}

/// W8 (V201/V202): branch transitions must appear in complementary pairs.
fn check_branch_pairs(
    net: &CvnNet,
    input_arcs: &[InputArcData],
    errors: &mut Vec<CvnError>,
) {
    // Group branch transitions by their control input place
    let mut branch_true_by_source: FxHashMap<PlaceId, Vec<TransitionId>> = FxHashMap::default();
    let mut branch_false_by_source: FxHashMap<PlaceId, Vec<TransitionId>> = FxHashMap::default();

    for t in net.transitions() {
        let is_true = matches!(t.kind, TransitionKind::BranchTrue);
        let is_false = matches!(t.kind, TransitionKind::BranchFalse);
        if !is_true && !is_false {
            continue;
        }

        // Find the control input place for this transition
        for arc in input_arcs {
            if arc.transition != t.id {
                continue;
            }
            if let Some(place) = net.place(&arc.place) {
                if place.is_control_flow() {
                    if is_true {
                        branch_true_by_source
                            .entry(arc.place.clone())
                            .or_default()
                            .push(t.id.clone());
                    } else {
                        branch_false_by_source
                            .entry(arc.place.clone())
                            .or_default()
                            .push(t.id.clone());
                    }
                }
            }
        }
    }

    // Check pairing
    let all_sources: FxHashSet<&PlaceId> = branch_true_by_source
        .keys()
        .chain(branch_false_by_source.keys())
        .collect();

    for source in all_sources {
        let trues = branch_true_by_source.get(source);
        let falses = branch_false_by_source.get(source);
        match (trues, falses) {
            (Some(_), None) => {
                if let Some(tids) = trues {
                    for tid in tids {
                        errors.push(CvnError::new(
                            ErrorCode::V201,
                            format!(
                                "BranchTrue transition '{}' from place '{}' has no BranchFalse pair",
                                tid, source
                            ),
                            ErrorLocation::Transition(tid.clone()),
                        ));
                    }
                }
            }
            (None, Some(_)) => {
                if let Some(tids) = falses {
                    for tid in tids {
                        errors.push(CvnError::new(
                            ErrorCode::V201,
                            format!(
                                "BranchFalse transition '{}' from place '{}' has no BranchTrue pair",
                                tid, source
                            ),
                            ErrorLocation::Transition(tid.clone()),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
}

/// V401/V402: resource place initial tokens must match their declared type.
fn check_resource_initial_tokens(net: &CvnNet, errors: &mut Vec<CvnError>) {
    for place in net.places() {
        if let PlaceKind::Resource { resource_type, .. } = &place.kind {
            let tokens = net.initial_marking().get(&place.id).copied().unwrap_or(0);
            match resource_type {
                ResourceType::Mutex => {
                    if tokens != 1 {
                        errors.push(CvnError::new(
                            ErrorCode::V401,
                            format!(
                                "Mutex place '{}' should have 1 initial token, has {}",
                                place.id, tokens
                            ),
                            ErrorLocation::Place(place.id.clone()),
                        ));
                    }
                }
                ResourceType::RwLock { max_readers } => {
                    if *max_readers < 1 {
                        errors.push(CvnError::new(
                            ErrorCode::V402,
                            format!(
                                "RwLock place '{}' has max_readers = 0",
                                place.id
                            ),
                            ErrorLocation::Place(place.id.clone()),
                        ));
                    }
                    if tokens != *max_readers {
                        errors.push(CvnError::new(
                            ErrorCode::V401,
                            format!(
                                "RwLock place '{}' should have {} initial tokens, has {}",
                                place.id, max_readers, tokens
                            ),
                            ErrorLocation::Place(place.id.clone()),
                        ));
                    }
                }
                ResourceType::Semaphore { count } => {
                    if tokens != *count {
                        errors.push(CvnError::new(
                            ErrorCode::V401,
                            format!(
                                "Semaphore place '{}' should have {} initial tokens, has {}",
                                place.id, count, tokens
                            ),
                            ErrorLocation::Place(place.id.clone()),
                        ));
                    }
                }
                ResourceType::Channel => {
                    if tokens != 0 {
                        errors.push(CvnError::new(
                            ErrorCode::V401,
                            format!(
                                "Channel place '{}' should have 0 initial tokens, has {}",
                                place.id, tokens
                            ),
                            ErrorLocation::Place(place.id.clone()),
                        ));
                    }
                }
                ResourceType::Condvar => {}
            }
        }
    }
}
