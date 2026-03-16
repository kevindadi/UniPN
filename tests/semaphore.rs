//! Test 6: Semaphore rate limiting.
//!
//! Semaphore with count=2, three workers.
//! At most 2 workers hold permits simultaneously. No deadlock.

use cvn::analysis::{explore, AnalysisConfig};
use cvn::builder::CvnNetBuilder;
use cvn::model::*;

fn build_semaphore_net() -> cvn::net::CvnNet {
    CvnNetBuilder::new()
        // Resource: Semaphore with count=2
        .add_resource_place("sem", "sem", ResourceType::Semaphore { count: 2 })
        // main
        .add_control_place("main_start", "main", "ms0")
        .add_control_place("main_s1", "main", "ms1")
        .add_control_place("main_s2", "main", "ms2")
        .add_control_place("main_s3", "main", "ms3")
        .add_control_place("main_s4", "main", "ms4")
        .add_control_place("main_done", "main", "ms5")
        .set_return("main_done")
        // w1
        .add_control_place("w1_start", "w1", "w1s0")
        .add_control_place("w1_acquired", "w1", "w1s1")
        .add_control_place("w1_done", "w1", "w1s2")
        .set_return("w1_done")
        // w2
        .add_control_place("w2_start", "w2", "w2s0")
        .add_control_place("w2_acquired", "w2", "w2s1")
        .add_control_place("w2_done", "w2", "w2s2")
        .set_return("w2_done")
        // w3
        .add_control_place("w3_start", "w3", "w3s0")
        .add_control_place("w3_acquired", "w3", "w3s1")
        .add_control_place("w3_done", "w3", "w3s2")
        .set_return("w3_done")
        // main: spawn w1, w2, w3 then join all
        .add_transition("t_spawn_w1", TransitionKind::Spawn, &["ms0"])
        .add_input_arc("main_start", "t_spawn_w1", 1, BoolExpr::True)
        .add_output_arc("t_spawn_w1", "main_s1", 1, None)
        .add_output_arc("t_spawn_w1", "w1_start", 1, None)
        .add_transition("t_spawn_w2", TransitionKind::Spawn, &["ms1"])
        .add_input_arc("main_s1", "t_spawn_w2", 1, BoolExpr::True)
        .add_output_arc("t_spawn_w2", "main_s2", 1, None)
        .add_output_arc("t_spawn_w2", "w2_start", 1, None)
        .add_transition("t_spawn_w3", TransitionKind::Spawn, &["ms2"])
        .add_input_arc("main_s2", "t_spawn_w3", 1, BoolExpr::True)
        .add_output_arc("t_spawn_w3", "main_s3", 1, None)
        .add_output_arc("t_spawn_w3", "w3_start", 1, None)
        .add_transition("t_join_w1", TransitionKind::Join, &["ms3"])
        .add_input_arc("main_s3", "t_join_w1", 1, BoolExpr::True)
        .add_input_arc("w1_done", "t_join_w1", 1, BoolExpr::True)
        .add_output_arc("t_join_w1", "main_s4", 1, None)
        .add_transition("t_join_w2", TransitionKind::Join, &["ms4"])
        .add_input_arc("main_s4", "t_join_w2", 1, BoolExpr::True)
        .add_input_arc("w2_done", "t_join_w2", 1, BoolExpr::True)
        .add_output_arc("t_join_w2", "main_done", 1, None)
        // Note: w3 isn't explicitly joined — it completes independently.
        // Its return place is terminal, so is_terminal handles it.
        // w1: acquire(sem) → release(sem)
        .add_transition("t_w1_acq", TransitionKind::Lock, &["w1s0"])
        .add_input_arc("w1_start", "t_w1_acq", 1, BoolExpr::True)
        .add_input_arc("sem", "t_w1_acq", 1, BoolExpr::True)
        .add_output_arc("t_w1_acq", "w1_acquired", 1, None)
        .add_transition("t_w1_rel", TransitionKind::Unlock, &["w1s1"])
        .add_input_arc("w1_acquired", "t_w1_rel", 1, BoolExpr::True)
        .add_output_arc("t_w1_rel", "w1_done", 1, None)
        .add_output_arc("t_w1_rel", "sem", 1, None)
        // w2: acquire(sem) → release(sem)
        .add_transition("t_w2_acq", TransitionKind::Lock, &["w2s0"])
        .add_input_arc("w2_start", "t_w2_acq", 1, BoolExpr::True)
        .add_input_arc("sem", "t_w2_acq", 1, BoolExpr::True)
        .add_output_arc("t_w2_acq", "w2_acquired", 1, None)
        .add_transition("t_w2_rel", TransitionKind::Unlock, &["w2s1"])
        .add_input_arc("w2_acquired", "t_w2_rel", 1, BoolExpr::True)
        .add_output_arc("t_w2_rel", "w2_done", 1, None)
        .add_output_arc("t_w2_rel", "sem", 1, None)
        // w3: acquire(sem) → release(sem)
        .add_transition("t_w3_acq", TransitionKind::Lock, &["w3s0"])
        .add_input_arc("w3_start", "t_w3_acq", 1, BoolExpr::True)
        .add_input_arc("sem", "t_w3_acq", 1, BoolExpr::True)
        .add_output_arc("t_w3_acq", "w3_acquired", 1, None)
        .add_transition("t_w3_rel", TransitionKind::Unlock, &["w3s1"])
        .add_input_arc("w3_acquired", "t_w3_rel", 1, BoolExpr::True)
        .add_output_arc("t_w3_rel", "w3_done", 1, None)
        .add_output_arc("t_w3_rel", "sem", 1, None)
        // Initial tokens
        .set_initial_tokens("main_start", 1)
        .set_initial_tokens("sem", 2)
        .build()
        .expect("valid semaphore net")
}

#[test]
fn semaphore_no_deadlock() {
    let net = build_semaphore_net();
    let result = explore(&net, &AnalysisConfig::default()).unwrap();
    assert!(
        result.deadlocks.is_empty(),
        "semaphore scenario should not deadlock"
    );
}

#[test]
fn at_most_two_workers_hold_permits() {
    let net = build_semaphore_net();
    let result = explore(&net, &AnalysisConfig::default()).unwrap();

    for idx in result.reachability_graph.node_indices() {
        let state = &result.reachability_graph[idx];
        let acquired_count = state.tokens(&PlaceId::new("w1_acquired"))
            + state.tokens(&PlaceId::new("w2_acquired"))
            + state.tokens(&PlaceId::new("w3_acquired"));
        assert!(
            acquired_count <= 2,
            "at most 2 workers should hold permits at once, found {}",
            acquired_count
        );
    }
}
