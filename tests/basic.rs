//! Basic semantics: enabled / fire / explore.

mod common;

use unipn::analysis::{AnalysisConfig, explore};
use unipn::expr::BoolExpr;
use unipn::model::{ControlSub, PlaceKind, TransitionKind};
use unipn::{NetBuilder, NetLike};

#[test]
fn simple_chain_fires_and_reaches_terminal() {
    let net = common::simple_chain();
    let s0 = net.initial_state();
    assert_eq!(net.enabled_transitions(&s0).len(), 1);

    let s1 = net.fire(unipn::TransitionId(0), &s0).unwrap();
    assert_eq!(s1.marking.tokens(unipn::PlaceId(1)), 1);
    assert!(net.enabled_transitions(&s1).is_empty());
}

#[test]
fn explore_chain_no_deadlock() {
    let net = common::simple_chain();
    let rg = explore(&net, &AnalysisConfig::default());
    assert!(rg.deadlocks.is_empty(), "chain must not deadlock");
    assert_eq!(rg.state_count(), 2);
}

#[test]
fn fire_rejects_not_enabled() {
    let net = common::simple_chain();
    let s1 = net.fire(unipn::TransitionId(0), &net.initial_state()).unwrap();
    assert!(
        net.fire(unipn::TransitionId(0), &s1).is_err(),
        "t0 must not be re-fireable"
    );
}

#[test]
fn transition_ids_sequential() {
    let mut b = NetBuilder::new();
    let p0 = b.add_place("p0", PlaceKind::Control(ControlSub::Statement));
    let p1 = b.add_place("p1", PlaceKind::Control(ControlSub::ThreadEnd));
    let t0 = b.add_transition("t0", TransitionKind::Sequential);
    let t1 = b.add_transition("t1", TransitionKind::Sequential);
    assert_eq!(t0.index(), 0);
    assert_eq!(t1.index(), 1);
    b.add_input_arc(p0, t0, 1, BoolExpr::True);
    b.add_output_arc(t0, p1, 1, None);
    b.add_input_arc(p1, t1, 1, BoolExpr::True);
    b.add_output_arc(t1, p0, 1, None);
    b.set_initial_tokens(p0, 1);
    let net = b.build();
    assert_eq!(net.num_places(), 2);
    assert_eq!(net.num_transitions(), 2);
}
