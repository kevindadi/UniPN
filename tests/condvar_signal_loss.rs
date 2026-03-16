//! Test 2: Condvar signal loss.
//!
//! Models a condvar scenario where signal can be lost if notifier runs before
//! the waiter reaches its wait point.
//!
//! The notify is modeled as two competing transitions:
//! - `t_n_notify_wake`: fires when a waiter IS present in w_waiting,
//!   moves the waiter out of waiting state
//! - `t_n_notify_noop`: fires unconditionally (over-approximation),
//!   does NOT wake any waiter → signal loss if waiter later enters wait

use cvn::analysis::{explore, AnalysisConfig, PropertyViolation, SearchStrategy};
use cvn::builder::CvnNetBuilder;
use cvn::model::*;

fn build_condvar_net() -> cvn::net::CvnNet {
    CvnNetBuilder::new()
        // Resources
        .add_resource_place("mtx", "mtx", ResourceType::Mutex)
        // Variable: ready (Bool, initially false)
        .add_variable("ready", Val::bool(false))
        // main control places
        .add_control_place("main_start", "main", "ms0")
        .add_control_place("main_spawned_waiter", "main", "ms1")
        .add_control_place("main_spawned_notifier", "main", "ms2")
        .add_control_place("main_joined_waiter", "main", "ms3")
        .add_control_place("main_done", "main", "ms4")
        .set_return("main_done")
        // waiter control places
        .add_control_place("w_start", "waiter", "ws0")
        .add_control_place("w_locked", "waiter", "ws1")
        .add_control_place("w_check_ready", "waiter", "ws2")
        .add_control_place("w_reacquired", "waiter", "ws3")
        .add_control_place("w_done", "waiter", "ws4")
        .set_return("w_done")
        // waiter wait place (condvar)
        .add_wait_place("w_waiting", "cv", "waiter", "ws_wait")
        // waiter woken place (after notify_wake moves waiter out of waiting)
        .add_control_place("w_woken", "waiter", "ws_woken")
        // notifier control places
        .add_control_place("n_start", "notifier", "ns0")
        .add_control_place("n_locked", "notifier", "ns1")
        .add_control_place("n_written", "notifier", "ns2")
        .add_control_place("n_notified", "notifier", "ns3")
        .add_control_place("n_done", "notifier", "ns4")
        .set_return("n_done")
        // --- Transitions ---
        // main: spawn waiter
        .add_transition("t_spawn_waiter", TransitionKind::Spawn, &["ms0"])
        .add_input_arc("main_start", "t_spawn_waiter", 1, BoolExpr::True)
        .add_output_arc("t_spawn_waiter", "main_spawned_waiter", 1, None)
        .add_output_arc("t_spawn_waiter", "w_start", 1, None)
        // main: spawn notifier
        .add_transition("t_spawn_notifier", TransitionKind::Spawn, &["ms1"])
        .add_input_arc("main_spawned_waiter", "t_spawn_notifier", 1, BoolExpr::True)
        .add_output_arc("t_spawn_notifier", "main_spawned_notifier", 1, None)
        .add_output_arc("t_spawn_notifier", "n_start", 1, None)
        // main: join waiter
        .add_transition("t_join_waiter", TransitionKind::Join, &["ms2"])
        .add_input_arc("main_spawned_notifier", "t_join_waiter", 1, BoolExpr::True)
        .add_input_arc("w_done", "t_join_waiter", 1, BoolExpr::True)
        .add_output_arc("t_join_waiter", "main_joined_waiter", 1, None)
        // main: join notifier
        .add_transition("t_join_notifier", TransitionKind::Join, &["ms3"])
        .add_input_arc("main_joined_waiter", "t_join_notifier", 1, BoolExpr::True)
        .add_input_arc("n_done", "t_join_notifier", 1, BoolExpr::True)
        .add_output_arc("t_join_notifier", "main_done", 1, None)
        // waiter: lock(mtx)
        .add_transition("t_w_lock", TransitionKind::Lock, &["ws0"])
        .add_input_arc("w_start", "t_w_lock", 1, BoolExpr::True)
        .add_input_arc("mtx", "t_w_lock", 1, BoolExpr::True)
        .add_output_arc("t_w_lock", "w_locked", 1, None)
        // waiter: sequential step to branch point
        .add_transition("t_w_seq", TransitionKind::Sequential, &["ws1"])
        .add_input_arc("w_locked", "t_w_seq", 1, BoolExpr::True)
        .add_output_arc("t_w_seq", "w_check_ready", 1, None)
        // waiter: branch ready == true → skip to reacquired
        .add_transition("t_w_branch_true", TransitionKind::BranchTrue, &["ws2"])
        .add_input_arc(
            "w_check_ready",
            "t_w_branch_true",
            1,
            eq(var("ready"), lit_bool(true)),
        )
        .add_output_arc("t_w_branch_true", "w_reacquired", 1, None)
        // waiter: branch ready == false → wait(cv, release mtx)
        .add_transition("t_w_branch_false", TransitionKind::BranchFalse, &["ws2"])
        .add_input_arc(
            "w_check_ready",
            "t_w_branch_false",
            1,
            eq(var("ready"), lit_bool(false)),
        )
        .add_output_arc("t_w_branch_false", "w_waiting", 1, None)
        .add_output_arc("t_w_branch_false", "mtx", 1, None) // release mutex
        // waiter: condvar wakeup → reacquire mutex (fired by notify_wake)
        .add_transition("t_w_reacquire", TransitionKind::CondvarWait, &["ws_woken"])
        .add_input_arc("w_woken", "t_w_reacquire", 1, BoolExpr::True)
        .add_input_arc("mtx", "t_w_reacquire", 1, BoolExpr::True)
        .add_output_arc("t_w_reacquire", "w_reacquired", 1, None)
        // waiter: drop(mtx)
        .add_transition("t_w_drop", TransitionKind::Unlock, &["ws3"])
        .add_input_arc("w_reacquired", "t_w_drop", 1, BoolExpr::True)
        .add_output_arc("t_w_drop", "w_done", 1, None)
        .add_output_arc("t_w_drop", "mtx", 1, None)
        // notifier: lock(mtx)
        .add_transition("t_n_lock", TransitionKind::Lock, &["ns0"])
        .add_input_arc("n_start", "t_n_lock", 1, BoolExpr::True)
        .add_input_arc("mtx", "t_n_lock", 1, BoolExpr::True)
        .add_output_arc("t_n_lock", "n_locked", 1, None)
        // notifier: write(ready, true)
        .add_transition("t_n_write", TransitionKind::VarWrite, &["ns1"])
        .add_input_arc("n_locked", "t_n_write", 1, BoolExpr::True)
        .add_output_arc(
            "t_n_write",
            "n_written",
            1,
            Some({
                let mut u = VarUpdate::new();
                u.insert("ready".to_string(), lit_bool(true));
                u
            }),
        )
        // notifier: notify variant 1 — wake the waiter (requires waiter in w_waiting)
        .add_transition(
            "t_n_notify_wake",
            TransitionKind::CondvarNotify {
                target_wait_place: "w_waiting".to_string(),
            },
            &["ns2"],
        )
        .add_input_arc("n_written", "t_n_notify_wake", 1, BoolExpr::True)
        .add_input_arc("w_waiting", "t_n_notify_wake", 1, BoolExpr::True)
        .add_output_arc("t_n_notify_wake", "n_notified", 1, None)
        .add_output_arc("t_n_notify_wake", "w_woken", 1, None) // move waiter to woken
        // notifier: notify variant 2 — noop (no waiter present, signal lost)
        .add_transition(
            "t_n_notify_noop",
            TransitionKind::CondvarNotify {
                target_wait_place: "w_waiting".to_string(),
            },
            &["ns2"],
        )
        .add_input_arc("n_written", "t_n_notify_noop", 1, BoolExpr::True)
        .add_output_arc("t_n_notify_noop", "n_notified", 1, None)
        // notifier: drop(mtx)
        .add_transition("t_n_drop", TransitionKind::Unlock, &["ns3"])
        .add_input_arc("n_notified", "t_n_drop", 1, BoolExpr::True)
        .add_output_arc("t_n_drop", "n_done", 1, None)
        .add_output_arc("t_n_drop", "mtx", 1, None)
        // Initial tokens
        .set_initial_tokens("main_start", 1)
        .set_initial_tokens("mtx", 1)
        .build()
        .expect("valid condvar net")
}

#[test]
fn condvar_signal_loss_detectable() {
    let net = build_condvar_net();
    let config = AnalysisConfig {
        strategy: SearchStrategy::Bfs,
        max_states: 100_000,
    };

    let result = explore(&net, &config).unwrap();

    // Should find a deadlock: waiter stuck at w_waiting because notify_noop was taken
    let has_signal_loss_deadlock = result.deadlocks.iter().any(|dl| {
        dl.kind == PropertyViolation::Deadlock
            && dl
                .final_state
                .marking
                .contains_key(&PlaceId::new("w_waiting"))
    });

    assert!(
        has_signal_loss_deadlock,
        "should find a deadlock where waiter is stuck at w_waiting (signal loss). \
         Found {} deadlocks total.",
        result.deadlocks.len()
    );
}

#[test]
fn condvar_has_successful_path_too() {
    let net = build_condvar_net();
    let config = AnalysisConfig::default();
    let result = explore(&net, &config).unwrap();

    // There should also be states where everyone completes successfully
    // (when notify_wake fires, or when waiter takes the true branch)
    let has_terminal = result
        .reachability_graph
        .node_indices()
        .any(|idx| {
            let state = &result.reachability_graph[idx];
            state.tokens(&PlaceId::new("main_done")) == 1
        });

    assert!(
        has_terminal,
        "there should also be a successful completion path"
    );
}
