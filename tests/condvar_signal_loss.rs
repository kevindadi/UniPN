//! Test 2: Condvar signal loss with the new translation scheme.
//!
//! Models a condvar scenario where signal can be lost if notifier runs before
//! the waiter reaches its wait point. The waiter waits unconditionally (no
//! ready-flag check), so if notify fires before the waiter enters wait, the
//! signal is lost and the waiter deadlocks.
//!
//! New translation uses global variables nw_cv (waiter count) and na_ws_wait
//! (notify-all flag), plus resource place rp_cv (notify token pool).

use cvn::analysis::{explore, AnalysisConfig, PropertyViolation, SearchStrategy};
use cvn::builder::CvnNetBuilder;
use cvn::model::*;

fn build_condvar_net() -> cvn::net::CvnNet {
    CvnNetBuilder::new()
        // Resources
        .add_resource_place("mtx", "mtx", ResourceType::Mutex)
        .add_resource_place("rp_cv", "cv", ResourceType::Condvar)
        // Variables
        .add_variable("nw_cv", Val::int(0))
        .add_variable("na_ws_wait", Val::bool(false))
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
        .add_control_place("w_reacquired", "waiter", "ws2")
        .add_control_place("w_done", "waiter", "ws3")
        .set_return("w_done")
        // waiter wait place & reacquire place
        .add_wait_place("w_waiting", "cv", "waiter", "ws_wait")
        .add_control_place("w_ra", "waiter", "ws_ra")
        // notifier control places
        .add_control_place("n_start", "notifier", "ns0")
        .add_control_place("n_locked", "notifier", "ns1")
        .add_control_place("n_notified", "notifier", "ns2")
        .add_control_place("n_done", "notifier", "ns3")
        .set_return("n_done")
        // --- Transitions ---
        // main: spawn waiter
        .add_transition("t_spawn_waiter", TransitionKind::Spawn)
        .add_input_arc("main_start", "t_spawn_waiter", 1, BoolExpr::True)
        .add_output_arc("t_spawn_waiter", "main_spawned_waiter", 1, None)
        .add_output_arc("t_spawn_waiter", "w_start", 1, None)
        // main: spawn notifier
        .add_transition("t_spawn_notifier", TransitionKind::Spawn)
        .add_input_arc("main_spawned_waiter", "t_spawn_notifier", 1, BoolExpr::True)
        .add_output_arc("t_spawn_notifier", "main_spawned_notifier", 1, None)
        .add_output_arc("t_spawn_notifier", "n_start", 1, None)
        // main: join waiter
        .add_transition("t_join_waiter", TransitionKind::Join)
        .add_input_arc("main_spawned_notifier", "t_join_waiter", 1, BoolExpr::True)
        .add_input_arc("w_done", "t_join_waiter", 1, BoolExpr::True)
        .add_output_arc("t_join_waiter", "main_joined_waiter", 1, None)
        // main: join notifier
        .add_transition("t_join_notifier", TransitionKind::Join)
        .add_input_arc("main_joined_waiter", "t_join_notifier", 1, BoolExpr::True)
        .add_input_arc("n_done", "t_join_notifier", 1, BoolExpr::True)
        .add_output_arc("t_join_notifier", "main_done", 1, None)
        // waiter: lock(mtx)
        .add_transition("t_w_lock", TransitionKind::Lock)
        .add_input_arc("w_start", "t_w_lock", 1, BoolExpr::True)
        .add_input_arc("mtx", "t_w_lock", 1, BoolExpr::True)
        .add_output_arc("t_w_lock", "w_locked", 1, None)
        // waiter: t_enter — unconditionally enters wait
        .add_transition("t_w_enter", TransitionKind::CondvarWaitEnter)
        .add_input_arc("w_locked", "t_w_enter", 1, BoolExpr::True)
        .add_output_arc("t_w_enter", "w_waiting", 1, None)
        .add_output_arc(
            "t_w_enter",
            "mtx",
            1,
            Some({
                let mut u = VarUpdate::new();
                u.insert("nw_cv".to_string(), add(var("nw_cv"), lit_int(1)));
                u.insert("na_ws_wait".to_string(), lit_bool(false));
                u
            }),
        )
        // waiter: t_wake1 — consume rp_cv token
        .add_transition("t_w_wake1", TransitionKind::CondvarWakeByNotify)
        .add_input_arc("w_waiting", "t_w_wake1", 1, BoolExpr::True)
        .add_input_arc("rp_cv", "t_w_wake1", 1, BoolExpr::True)
        .add_output_arc(
            "t_w_wake1",
            "w_ra",
            1,
            Some({
                let mut u = VarUpdate::new();
                u.insert("nw_cv".to_string(), sub(var("nw_cv"), lit_int(1)));
                u
            }),
        )
        // waiter: t_wakeA — guarded by na flag
        .add_transition("t_w_wakeA", TransitionKind::CondvarWakeByNotifyAll)
        .add_input_arc(
            "w_waiting",
            "t_w_wakeA",
            1,
            eq(var("na_ws_wait"), lit_bool(true)),
        )
        .add_output_arc(
            "t_w_wakeA",
            "w_ra",
            1,
            Some({
                let mut u = VarUpdate::new();
                u.insert("nw_cv".to_string(), sub(var("nw_cv"), lit_int(1)));
                u.insert("na_ws_wait".to_string(), lit_bool(false));
                u
            }),
        )
        // waiter: t_reacq — reacquire mutex
        .add_transition("t_w_reacquire", TransitionKind::CondvarReacquire)
        .add_input_arc("w_ra", "t_w_reacquire", 1, BoolExpr::True)
        .add_input_arc("mtx", "t_w_reacquire", 1, BoolExpr::True)
        .add_output_arc("t_w_reacquire", "w_reacquired", 1, None)
        // waiter: drop(mtx)
        .add_transition("t_w_drop", TransitionKind::Unlock)
        .add_input_arc("w_reacquired", "t_w_drop", 1, BoolExpr::True)
        .add_output_arc("t_w_drop", "w_done", 1, None)
        .add_output_arc("t_w_drop", "mtx", 1, None)
        // notifier: lock(mtx)
        .add_transition("t_n_lock", TransitionKind::Lock)
        .add_input_arc("n_start", "t_n_lock", 1, BoolExpr::True)
        .add_input_arc("mtx", "t_n_lock", 1, BoolExpr::True)
        .add_output_arc("t_n_lock", "n_locked", 1, None)
        // notifier: notify — produces rp_cv token when nw_cv > 0
        .add_transition("t_n_notify", TransitionKind::CondvarNotify)
        .add_input_arc(
            "n_locked",
            "t_n_notify",
            1,
            gt(var("nw_cv"), lit_int(0)),
        )
        .add_output_arc("t_n_notify", "n_notified", 1, None)
        .add_output_arc("t_n_notify", "rp_cv", 1, None)
        // notifier: notify lost — fires when nw_cv == 0 (signal loss)
        .add_transition("t_n_notify_lost", TransitionKind::CondvarNotifyLost)
        .add_input_arc(
            "n_locked",
            "t_n_notify_lost",
            1,
            eq(var("nw_cv"), lit_int(0)),
        )
        .add_output_arc("t_n_notify_lost", "n_notified", 1, None)
        // notifier: drop(mtx)
        .add_transition("t_n_drop", TransitionKind::Unlock)
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

    // Should find a deadlock: waiter stuck at w_waiting because notify_lost was
    // taken (nw_cv == 0 at notify time, no rp_cv token deposited).
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

    // When the waiter enters wait before the notifier notifies, the notify
    // produces a rp_cv token and the waiter wakes up successfully.
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
