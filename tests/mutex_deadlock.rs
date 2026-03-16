//! Test 1: Two-thread Mutex deadlock detection.
//!
//! w1: lock(mtx_a) → lock(mtx_b) → drop(mtx_b) → drop(mtx_a)
//! w2: lock(mtx_b) → lock(mtx_a) → drop(mtx_a) → drop(mtx_b)
//!
//! Expected: deadlock found where w1 holds mtx_a waiting for mtx_b,
//! and w2 holds mtx_b waiting for mtx_a.

use cvn::analysis::{explore, AnalysisConfig, PropertyViolation, SearchStrategy};
use cvn::builder::CvnNetBuilder;
use cvn::model::*;

fn build_mutex_deadlock_net() -> cvn::net::CvnNet {
    CvnNetBuilder::new()
        // Resource places
        .add_resource_place("mtx_a", "mtx_a", ResourceType::Mutex)
        .add_resource_place("mtx_b", "mtx_b", ResourceType::Mutex)
        // main thread control places
        .add_control_place("main_start", "main", "s0")
        .add_control_place("main_spawned_w1", "main", "s1")
        .add_control_place("main_spawned_w2", "main", "s2")
        .add_control_place("main_joined_w1", "main", "s3")
        .add_control_place("main_done", "main", "s4")
        .set_return("main_done")
        // w1 control places
        .add_control_place("w1_start", "w1", "w1s0")
        .add_control_place("w1_locked_a", "w1", "w1s1")
        .add_control_place("w1_locked_ab", "w1", "w1s2")
        .add_control_place("w1_dropped_b", "w1", "w1s3")
        .add_control_place("w1_done", "w1", "w1s4")
        .set_return("w1_done")
        // w2 control places
        .add_control_place("w2_start", "w2", "w2s0")
        .add_control_place("w2_locked_b", "w2", "w2s1")
        .add_control_place("w2_locked_ba", "w2", "w2s2")
        .add_control_place("w2_dropped_a", "w2", "w2s3")
        .add_control_place("w2_done", "w2", "w2s4")
        .set_return("w2_done")
        // --- Transitions ---
        // main: spawn w1
        .add_transition("t_spawn_w1", TransitionKind::Spawn)
        .add_input_arc("main_start", "t_spawn_w1", 1, BoolExpr::True)
        .add_output_arc("t_spawn_w1", "main_spawned_w1", 1, None)
        .add_output_arc("t_spawn_w1", "w1_start", 1, None)
        // main: spawn w2
        .add_transition("t_spawn_w2", TransitionKind::Spawn)
        .add_input_arc("main_spawned_w1", "t_spawn_w2", 1, BoolExpr::True)
        .add_output_arc("t_spawn_w2", "main_spawned_w2", 1, None)
        .add_output_arc("t_spawn_w2", "w2_start", 1, None)
        // main: join w1
        .add_transition("t_join_w1", TransitionKind::Join)
        .add_input_arc("main_spawned_w2", "t_join_w1", 1, BoolExpr::True)
        .add_input_arc("w1_done", "t_join_w1", 1, BoolExpr::True)
        .add_output_arc("t_join_w1", "main_joined_w1", 1, None)
        // main: join w2
        .add_transition("t_join_w2", TransitionKind::Join)
        .add_input_arc("main_joined_w1", "t_join_w2", 1, BoolExpr::True)
        .add_input_arc("w2_done", "t_join_w2", 1, BoolExpr::True)
        .add_output_arc("t_join_w2", "main_done", 1, None)
        // w1: lock(mtx_a)
        .add_transition("t_w1_lock_a", TransitionKind::Lock)
        .add_input_arc("w1_start", "t_w1_lock_a", 1, BoolExpr::True)
        .add_input_arc("mtx_a", "t_w1_lock_a", 1, BoolExpr::True)
        .add_output_arc("t_w1_lock_a", "w1_locked_a", 1, None)
        // w1: lock(mtx_b)
        .add_transition("t_w1_lock_b", TransitionKind::Lock)
        .add_input_arc("w1_locked_a", "t_w1_lock_b", 1, BoolExpr::True)
        .add_input_arc("mtx_b", "t_w1_lock_b", 1, BoolExpr::True)
        .add_output_arc("t_w1_lock_b", "w1_locked_ab", 1, None)
        // w1: drop(mtx_b)
        .add_transition("t_w1_drop_b", TransitionKind::Unlock)
        .add_input_arc("w1_locked_ab", "t_w1_drop_b", 1, BoolExpr::True)
        .add_output_arc("t_w1_drop_b", "w1_dropped_b", 1, None)
        .add_output_arc("t_w1_drop_b", "mtx_b", 1, None)
        // w1: drop(mtx_a)
        .add_transition("t_w1_drop_a", TransitionKind::Unlock)
        .add_input_arc("w1_dropped_b", "t_w1_drop_a", 1, BoolExpr::True)
        .add_output_arc("t_w1_drop_a", "w1_done", 1, None)
        .add_output_arc("t_w1_drop_a", "mtx_a", 1, None)
        // w2: lock(mtx_b)
        .add_transition("t_w2_lock_b", TransitionKind::Lock)
        .add_input_arc("w2_start", "t_w2_lock_b", 1, BoolExpr::True)
        .add_input_arc("mtx_b", "t_w2_lock_b", 1, BoolExpr::True)
        .add_output_arc("t_w2_lock_b", "w2_locked_b", 1, None)
        // w2: lock(mtx_a)
        .add_transition("t_w2_lock_a", TransitionKind::Lock)
        .add_input_arc("w2_locked_b", "t_w2_lock_a", 1, BoolExpr::True)
        .add_input_arc("mtx_a", "t_w2_lock_a", 1, BoolExpr::True)
        .add_output_arc("t_w2_lock_a", "w2_locked_ba", 1, None)
        // w2: drop(mtx_a)
        .add_transition("t_w2_drop_a", TransitionKind::Unlock)
        .add_input_arc("w2_locked_ba", "t_w2_drop_a", 1, BoolExpr::True)
        .add_output_arc("t_w2_drop_a", "w2_dropped_a", 1, None)
        .add_output_arc("t_w2_drop_a", "mtx_a", 1, None)
        // w2: drop(mtx_b)
        .add_transition("t_w2_drop_b", TransitionKind::Unlock)
        .add_input_arc("w2_dropped_a", "t_w2_drop_b", 1, BoolExpr::True)
        .add_output_arc("t_w2_drop_b", "w2_done", 1, None)
        .add_output_arc("t_w2_drop_b", "mtx_b", 1, None)
        // Initial tokens
        .set_initial_tokens("main_start", 1)
        .set_initial_tokens("mtx_a", 1)
        .set_initial_tokens("mtx_b", 1)
        .build()
        .expect("valid mutex deadlock net")
}

#[test]
fn detects_deadlock_bfs() {
    let net = build_mutex_deadlock_net();
    let config = AnalysisConfig {
        strategy: SearchStrategy::Bfs,
        max_states: 100_000,
    };

    let result = explore(&net, &config).unwrap();
    assert!(
        !result.deadlocks.is_empty(),
        "should detect at least one deadlock"
    );

    for dl in &result.deadlocks {
        assert_eq!(dl.kind, PropertyViolation::Deadlock);
        assert!(!dl.trace.is_empty(), "deadlock trace should not be empty");
    }
}

#[test]
fn detects_deadlock_dfs() {
    let net = build_mutex_deadlock_net();
    let config = AnalysisConfig {
        strategy: SearchStrategy::Dfs,
        max_states: 100_000,
    };

    let result = explore(&net, &config).unwrap();
    assert!(
        !result.deadlocks.is_empty(),
        "should detect at least one deadlock"
    );
}

#[test]
fn deadlock_trace_contains_lock_steps() {
    let net = build_mutex_deadlock_net();
    let config = AnalysisConfig::default();
    let result = explore(&net, &config).unwrap();

    let dl = &result.deadlocks[0];
    let transition_ids: Vec<_> = dl
        .trace
        .iter()
        .map(|s| s.transition_id.0.as_str())
        .collect();

    assert!(
        transition_ids.contains(&"t_spawn_w1"),
        "trace should contain spawn w1"
    );
    assert!(
        transition_ids.contains(&"t_spawn_w2"),
        "trace should contain spawn w2"
    );
}
