//! Test 7: DOT export snapshot test.

use cvn::builder::CvnNetBuilder;
use cvn::export::to_dot;
use cvn::model::*;

fn build_simple_mutex_net() -> cvn::net::CvnNet {
    CvnNetBuilder::new()
        .add_resource_place("mtx_a", "mtx_a", ResourceType::Mutex)
        .add_resource_place("mtx_b", "mtx_b", ResourceType::Mutex)
        .add_control_place("w1_start", "w1", "s0")
        .add_control_place("w1_locked_a", "w1", "s1")
        .add_control_place("w1_done", "w1", "s2")
        .set_return("w1_done")
        .add_transition("t_w1_lock_a", TransitionKind::Lock, &["s0"])
        .add_input_arc("w1_start", "t_w1_lock_a", 1, BoolExpr::True)
        .add_input_arc("mtx_a", "t_w1_lock_a", 1, BoolExpr::True)
        .add_output_arc("t_w1_lock_a", "w1_locked_a", 1, None)
        .add_transition("t_w1_drop_a", TransitionKind::Unlock, &["s1"])
        .add_input_arc("w1_locked_a", "t_w1_drop_a", 1, BoolExpr::True)
        .add_output_arc("t_w1_drop_a", "w1_done", 1, None)
        .add_output_arc("t_w1_drop_a", "mtx_a", 1, None)
        .set_initial_tokens("w1_start", 1)
        .set_initial_tokens("mtx_a", 1)
        .set_initial_tokens("mtx_b", 1)
        .build()
        .expect("valid simple mutex net")
}

#[test]
fn dot_export_contains_all_nodes() {
    let net = build_simple_mutex_net();
    let dot = to_dot(&net);

    // Verify all places appear
    assert!(dot.contains("mtx_a"), "DOT should contain mtx_a");
    assert!(dot.contains("mtx_b"), "DOT should contain mtx_b");
    assert!(dot.contains("w1_start"), "DOT should contain w1_start");
    assert!(
        dot.contains("w1_locked_a"),
        "DOT should contain w1_locked_a"
    );
    assert!(dot.contains("w1_done"), "DOT should contain w1_done");

    // Verify transitions appear
    assert!(
        dot.contains("t_w1_lock_a"),
        "DOT should contain t_w1_lock_a"
    );
    assert!(
        dot.contains("t_w1_drop_a"),
        "DOT should contain t_w1_drop_a"
    );

    // Verify basic DOT structure
    assert!(dot.starts_with("digraph CVN {"));
    assert!(dot.trim_end().ends_with('}'));
}

#[test]
fn dot_export_snapshot() {
    let net = build_simple_mutex_net();
    let dot = to_dot(&net);
    insta::assert_snapshot!(dot);
}
