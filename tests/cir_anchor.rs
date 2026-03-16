#![cfg(feature = "cir-anchor")]

//! Tests for the `cir-anchor` feature flag.

use cvn::analysis::{explore, AnalysisConfig, SearchStrategy};
use cvn::builder::CvnNetBuilder;
use cvn::error::ErrorCode;
use cvn::model::*;

#[test]
fn anchor_sids_accessible_after_build() {
    let net = CvnNetBuilder::new()
        .add_control_place("p0", "main", "s0")
        .add_control_place("p1", "main", "s1")
        .set_return("p1")
        .add_transition_with_anchor("t0", TransitionKind::Sequential, &["sid_a", "sid_b"])
        .add_input_arc("p0", "t0", 1, BoolExpr::True)
        .add_output_arc("t0", "p1", 1, None)
        .set_initial_tokens("p0", 1)
        .build()
        .expect("valid net");

    let t = net.transition(&TransitionId::new("t0")).unwrap();
    let sids: Vec<&str> = t.anchor_sids().iter().map(|s| s.as_str()).collect();
    assert_eq!(sids, vec!["sid_a", "sid_b"]);
}

#[test]
fn v105_empty_anchor_with_anchor_check() {
    let result = CvnNetBuilder::new()
        .add_control_place("p0", "main", "s0")
        .add_control_place("p1", "main", "s1")
        .set_return("p1")
        .add_transition("t0", TransitionKind::Sequential)
        .add_input_arc("p0", "t0", 1, BoolExpr::True)
        .add_output_arc("t0", "p1", 1, None)
        .set_initial_tokens("p0", 1)
        .build_with_anchor_check();

    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.code == ErrorCode::V105),
        "should report V105 when anchor is empty, got: {:?}",
        errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
}

#[test]
fn no_v105_when_anchor_present() {
    let result = CvnNetBuilder::new()
        .add_control_place("p0", "main", "s0")
        .add_control_place("p1", "main", "s1")
        .set_return("p1")
        .add_transition_with_anchor("t0", TransitionKind::Sequential, &["sid_a"])
        .add_input_arc("p0", "t0", 1, BoolExpr::True)
        .add_output_arc("t0", "p1", 1, None)
        .set_initial_tokens("p0", 1)
        .build_with_anchor_check();

    assert!(result.is_ok(), "should pass with anchor present");
}

#[test]
fn counterexample_contains_anchor_sids() {
    let net = CvnNetBuilder::new()
        .add_resource_place("mtx_a", "mtx_a", ResourceType::Mutex)
        .add_resource_place("mtx_b", "mtx_b", ResourceType::Mutex)
        .add_control_place("w1_start", "w1", "s0")
        .add_control_place("w1_locked_a", "w1", "s1")
        .add_control_place("w1_locked_ab", "w1", "s2")
        .add_control_place("w1_done", "w1", "s3")
        .set_return("w1_done")
        .add_control_place("w2_start", "w2", "s0")
        .add_control_place("w2_locked_b", "w2", "s1")
        .add_control_place("w2_locked_ba", "w2", "s2")
        .add_control_place("w2_done", "w2", "s3")
        .set_return("w2_done")
        .add_transition_with_anchor("t_w1_lock_a", TransitionKind::Lock, &["w1_lock_a_sid"])
        .add_input_arc("w1_start", "t_w1_lock_a", 1, BoolExpr::True)
        .add_input_arc("mtx_a", "t_w1_lock_a", 1, BoolExpr::True)
        .add_output_arc("t_w1_lock_a", "w1_locked_a", 1, None)
        .add_transition_with_anchor("t_w1_lock_b", TransitionKind::Lock, &["w1_lock_b_sid"])
        .add_input_arc("w1_locked_a", "t_w1_lock_b", 1, BoolExpr::True)
        .add_input_arc("mtx_b", "t_w1_lock_b", 1, BoolExpr::True)
        .add_output_arc("t_w1_lock_b", "w1_locked_ab", 1, None)
        .add_transition_with_anchor("t_w1_done", TransitionKind::Unlock, &["w1_done_sid"])
        .add_input_arc("w1_locked_ab", "t_w1_done", 1, BoolExpr::True)
        .add_output_arc("t_w1_done", "w1_done", 1, None)
        .add_output_arc("t_w1_done", "mtx_a", 1, None)
        .add_output_arc("t_w1_done", "mtx_b", 1, None)
        .add_transition_with_anchor("t_w2_lock_b", TransitionKind::Lock, &["w2_lock_b_sid"])
        .add_input_arc("w2_start", "t_w2_lock_b", 1, BoolExpr::True)
        .add_input_arc("mtx_b", "t_w2_lock_b", 1, BoolExpr::True)
        .add_output_arc("t_w2_lock_b", "w2_locked_b", 1, None)
        .add_transition_with_anchor("t_w2_lock_a", TransitionKind::Lock, &["w2_lock_a_sid"])
        .add_input_arc("w2_locked_b", "t_w2_lock_a", 1, BoolExpr::True)
        .add_input_arc("mtx_a", "t_w2_lock_a", 1, BoolExpr::True)
        .add_output_arc("t_w2_lock_a", "w2_locked_ba", 1, None)
        .add_transition_with_anchor("t_w2_done", TransitionKind::Unlock, &["w2_done_sid"])
        .add_input_arc("w2_locked_ba", "t_w2_done", 1, BoolExpr::True)
        .add_output_arc("t_w2_done", "w2_done", 1, None)
        .add_output_arc("t_w2_done", "mtx_a", 1, None)
        .add_output_arc("t_w2_done", "mtx_b", 1, None)
        .set_initial_tokens("w1_start", 1)
        .set_initial_tokens("w2_start", 1)
        .set_initial_tokens("mtx_a", 1)
        .set_initial_tokens("mtx_b", 1)
        .build_with_anchor_check()
        .expect("valid net");

    let config = AnalysisConfig {
        strategy: SearchStrategy::Bfs,
        max_states: 10_000,
    };
    let result = explore(&net, &config).unwrap();
    assert!(!result.deadlocks.is_empty(), "should find deadlock");

    let dl = &result.deadlocks[0];
    let has_anchor = dl.trace.iter().any(|step| !step.anchor_sids.is_empty());
    assert!(has_anchor, "counterexample steps should carry anchor SIDs");
}

#[test]
fn json_roundtrip_with_anchors() {
    let net = CvnNetBuilder::new()
        .add_control_place("p0", "main", "s0")
        .add_control_place("p1", "main", "s1")
        .set_return("p1")
        .add_transition_with_anchor("t0", TransitionKind::Sequential, &["sid_x"])
        .add_input_arc("p0", "t0", 1, BoolExpr::True)
        .add_output_arc("t0", "p1", 1, None)
        .set_initial_tokens("p0", 1)
        .build()
        .expect("valid net");

    let json = serde_json::to_string(&net).unwrap();
    assert!(json.contains("sid_x"), "JSON should contain anchor SID");

    let deserialized: cvn::net::CvnNet = serde_json::from_str(&json).unwrap();
    let t = deserialized.transition(&TransitionId::new("t0")).unwrap();
    assert_eq!(t.anchor_sids()[0], "sid_x");
}

#[test]
fn json_without_anchors_deserializes() {
    let net = CvnNetBuilder::new()
        .add_control_place("p0", "main", "s0")
        .add_control_place("p1", "main", "s1")
        .set_return("p1")
        .add_transition("t0", TransitionKind::Sequential)
        .add_input_arc("p0", "t0", 1, BoolExpr::True)
        .add_output_arc("t0", "p1", 1, None)
        .set_initial_tokens("p0", 1)
        .build()
        .expect("valid net");

    let json = serde_json::to_string(&net).unwrap();
    assert!(
        !json.contains("anchor_sids"),
        "JSON should not contain anchor_sids when empty"
    );

    let deserialized: cvn::net::CvnNet = serde_json::from_str(&json).unwrap();
    let t = deserialized.transition(&TransitionId::new("t0")).unwrap();
    assert!(t.anchor_sids().is_empty());
}
