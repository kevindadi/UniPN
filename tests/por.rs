//! Partial-order reduction (POR, sleep-set) properties:
//! - **Deadlock-freedom equivalence**: the POR graph has a deadlock ⟺ the full
//!   graph has one (sound & complete).
//! - POR does not preserve the state/deadlock count; it only drops redundant
//!   interleavings, so its state count is ≤ the naive one.

mod common;

use unipn::analysis::{AnalysisConfig, explore};

#[test]
fn por_deadlock_freedom_is_equivalent() {
    let net = common::mutex_deadlock();
    let plain = explore(&net, &AnalysisConfig::default());
    let por = explore(
        &net,
        &AnalysisConfig {
            por: true,
            ..Default::default()
        },
    );
    // Full graph has a deadlock ⟺ POR graph has one.
    assert_eq!(
        plain.deadlocks.is_empty(),
        por.deadlocks.is_empty(),
        "POR must preserve deadlock-freedom"
    );
    assert!(!por.deadlocks.is_empty(), "mutex net does deadlock");
    // POR state count ≤ naive.
    assert!(por.state_count() <= plain.state_count());
}

#[test]
fn por_no_false_deadlock_on_safe_net() {
    let net = common::simple_chain();
    let plain = explore(&net, &AnalysisConfig::default());
    let por = explore(
        &net,
        &AnalysisConfig {
            por: true,
            ..Default::default()
        },
    );
    assert!(plain.deadlocks.is_empty());
    assert!(por.deadlocks.is_empty(), "POR must not invent deadlocks");
}

#[test]
fn por_on_independent_transitions_cuts_interleavings() {
    use unipn::expr::BoolExpr;
    use unipn::model::{ControlSub, PlaceKind, TransitionKind};
    use unipn::NetBuilder;

    let mut b = NetBuilder::new();
    let a0 = b.add_place("a0", PlaceKind::Control(ControlSub::Statement));
    let a1 = b.add_place("a1", PlaceKind::Control(ControlSub::ThreadEnd));
    let c0 = b.add_place("c0", PlaceKind::Control(ControlSub::Statement));
    let c1 = b.add_place("c1", PlaceKind::Control(ControlSub::ThreadEnd));
    let ta = b.add_transition("ta", TransitionKind::Sequential);
    let tc = b.add_transition("tc", TransitionKind::Sequential);
    b.add_input_arc(a0, ta, 1, BoolExpr::True);
    b.add_output_arc(ta, a1, 1, None);
    b.add_input_arc(c0, tc, 1, BoolExpr::True);
    b.add_output_arc(tc, c1, 1, None);
    b.set_initial_tokens(a0, 1);
    b.set_initial_tokens(c0, 1);
    let net = b.build();

    let plain = explore(&net, &AnalysisConfig::default());
    let por = explore(
        &net,
        &AnalysisConfig {
            por: true,
            ..Default::default()
        },
    );
    // Two independent transitions have 2! = 2 interleavings; POR keeps only
    // the 1 representative order.
    assert!(
        por.edge_count() < plain.edge_count(),
        "POR must drop redundant interleavings"
    );
    assert!(por.state_count() <= plain.state_count());
    // Deadlock-freedom is unaffected.
    assert!(plain.deadlocks.is_empty() && por.deadlocks.is_empty());
}
