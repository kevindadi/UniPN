//! Test 3: Well-formedness violations.

use cvn::builder::CvnNetBuilder;
use cvn::error::ErrorCode;
use cvn::model::*;

#[test]
fn v101_missing_control_input_arc() {
    let result = CvnNetBuilder::new()
        .add_control_place("p0", "main", "s0")
        .add_control_place("p1", "main", "s1")
        .set_return("p1")
        // Transition with NO input arcs at all
        .add_transition("t0", TransitionKind::Sequential)
        .add_output_arc("t0", "p1", 1, None)
        .set_initial_tokens("p0", 1)
        .build();

    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.code == ErrorCode::V101),
        "should report V101 (missing control input arc), got: {:?}",
        errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
}

#[test]
fn v201_unpaired_branch() {
    let result = CvnNetBuilder::new()
        .add_control_place("p0", "main", "s0")
        .add_control_place("p_true", "main", "s1")
        .set_return("p_true")
        .add_variable("x", Val::int(0))
        // Only BranchTrue, no BranchFalse
        .add_transition("t_true", TransitionKind::BranchTrue)
        .add_input_arc("p0", "t_true", 1, gt(var("x"), lit_int(0)))
        .add_output_arc("t_true", "p_true", 1, None)
        .set_initial_tokens("p0", 1)
        .build();

    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.code == ErrorCode::V201),
        "should report V201 (unpaired branch), got: {:?}",
        errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
}

#[test]
fn v104_conflicting_updates() {
    let result = CvnNetBuilder::new()
        .add_control_place("p0", "main", "s0")
        .add_control_place("p1", "main", "s1")
        .add_control_place("p2", "main", "s2")
        .set_return("p1")
        .set_return("p2")
        .add_variable("x", Val::int(0))
        .add_transition("t0", TransitionKind::Sequential)
        .add_input_arc("p0", "t0", 1, BoolExpr::True)
        // Two output arcs that both update variable "x"
        .add_output_arc(
            "t0",
            "p1",
            1,
            Some({
                let mut u = VarUpdate::new();
                u.insert("x".to_string(), lit_int(1));
                u
            }),
        )
        .add_output_arc(
            "t0",
            "p2",
            1,
            Some({
                let mut u = VarUpdate::new();
                u.insert("x".to_string(), lit_int(2));
                u
            }),
        )
        .set_initial_tokens("p0", 1)
        .build();

    let errors = result.unwrap_err();
    assert!(
        errors.iter().any(|e| e.code == ErrorCode::V104),
        "should report V104 (conflicting updates), got: {:?}",
        errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
}
