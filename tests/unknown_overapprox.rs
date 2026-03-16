//! Test 4: Unknown over-approximation.
//!
//! Variable `count` starts as Unknown. A branch on `count > 0` evaluates to
//! Unknown, so both BranchTrue and BranchFalse transitions are enabled,
//! causing the state space to split into two paths.

use cvn::analysis::{explore, AnalysisConfig};
use cvn::builder::CvnNetBuilder;
use cvn::model::*;

#[test]
fn unknown_enables_both_branches() {
    let net = CvnNetBuilder::new()
        .add_variable("count", Val::Unknown)
        .add_control_place("p0", "f", "s0")
        .add_control_place("p_true", "f", "s_true")
        .add_control_place("p_false", "f", "s_false")
        .set_return("p_true")
        .set_return("p_false")
        // BranchTrue: count > 0
        .add_transition("t_true", TransitionKind::BranchTrue)
        .add_input_arc("p0", "t_true", 1, gt(var("count"), lit_int(0)))
        .add_output_arc("t_true", "p_true", 1, None)
        // BranchFalse: !(count > 0)
        .add_transition("t_false", TransitionKind::BranchFalse)
        .add_input_arc("p0", "t_false", 1, not(gt(var("count"), lit_int(0))))
        .add_output_arc("t_false", "p_false", 1, None)
        .set_initial_tokens("p0", 1)
        .build()
        .expect("valid net with Unknown variable");

    // Both branches should be enabled from the initial state
    let initial_state = net.initial_state();
    let enabled = net.enabled_transitions(&initial_state);
    assert_eq!(
        enabled.len(),
        2,
        "both branch transitions should be enabled when guard is Unknown"
    );

    // Full exploration should produce two terminal states (one per branch)
    let result = explore(&net, &AnalysisConfig::default()).unwrap();
    assert!(
        result.deadlocks.is_empty(),
        "no deadlocks expected"
    );
    // 3 states: initial + true branch + false branch
    assert_eq!(
        result.state_count, 3,
        "should have 3 reachable states"
    );
}
