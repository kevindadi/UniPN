//! Conflict sets (the basis for selecting contention points in test
//! generation).

use unipn::analysis::conflict_sets;
use unipn::expr::BoolExpr;
use unipn::model::{ControlSub, PlaceKind, TransitionKind};
use unipn::NetBuilder;

fn two_conflicting() -> unipn::Net {
    let mut b = NetBuilder::new();
    let p0 = b.add_place("p0", PlaceKind::Control(ControlSub::Statement));
    let p1 = b.add_place("p1", PlaceKind::Control(ControlSub::ThreadEnd));
    let p2 = b.add_place("p2", PlaceKind::Control(ControlSub::ThreadEnd));
    let t1 = b.add_transition("t1", TransitionKind::Sequential);
    let t2 = b.add_transition("t2", TransitionKind::Sequential);
    b.add_input_arc(p0, t1, 1, BoolExpr::True);
    b.add_output_arc(t1, p1, 1, None);
    b.add_input_arc(p0, t2, 1, BoolExpr::True);
    b.add_output_arc(t2, p2, 1, None);
    b.set_initial_tokens(p0, 1);
    b.build()
}

#[test]
fn detects_shared_input_pair() {
    let net = two_conflicting();
    let pairs = conflict_sets(&net);
    assert_eq!(pairs.len(), 1, "t1/t2 share input p0");
    assert_eq!(pairs[0], (unipn::TransitionId(0), unipn::TransitionId(1)));
}
