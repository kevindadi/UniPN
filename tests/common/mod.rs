//! Shared test helpers: build various nets.

#![allow(dead_code)]

use unipn::expr::BoolExpr;
use unipn::model::{ControlSub, PlaceKind, ResourceType, TransitionKind};
use unipn::{Net, NetBuilder};

/// Simple chain: p0 → t0 → p1 (p1 is the thread terminal).
pub fn simple_chain() -> Net {
    let mut b = NetBuilder::new();
    let p0 = b.add_place("p0", PlaceKind::Control(ControlSub::Statement));
    let p1 = b.add_place("p1", PlaceKind::Control(ControlSub::ThreadEnd));
    let t0 = b.add_transition("t0", TransitionKind::Sequential);
    b.add_input_arc(p0, t0, 1, BoolExpr::True);
    b.add_output_arc(t0, p1, 1, None);
    b.set_initial_tokens(p0, 1);
    b.build()
}

/// Two-mutex deadlock: thread 1 holds A and waits for B, thread 2 holds B and
/// waits for A.
pub fn mutex_deadlock() -> Net {
    let mut b = NetBuilder::new();
    let a = b.add_place("A", PlaceKind::Resource(ResourceType::Mutex));
    let b_ = b.add_place("B", PlaceKind::Resource(ResourceType::Mutex));

    let t1_s1 = b.add_place("t1_s1", PlaceKind::Control(ControlSub::Statement));
    let t1_s2 = b.add_place("t1_s2", PlaceKind::Control(ControlSub::Statement));
    let t1_done = b.add_place("t1_done", PlaceKind::Control(ControlSub::ThreadEnd));
    let t2_s1 = b.add_place("t2_s1", PlaceKind::Control(ControlSub::Statement));
    let t2_s2 = b.add_place("t2_s2", PlaceKind::Control(ControlSub::Statement));
    let t2_done = b.add_place("t2_done", PlaceKind::Control(ControlSub::ThreadEnd));

    let t1_lock_a = b.add_transition("t1_lock_a", TransitionKind::Lock);
    let t1_lock_b = b.add_transition("t1_lock_b", TransitionKind::Lock);
    let t2_lock_b = b.add_transition("t2_lock_b", TransitionKind::Lock);
    let t2_lock_a = b.add_transition("t2_lock_a", TransitionKind::Lock);

    // t1: lock A then B
    b.add_input_arc(t1_s1, t1_lock_a, 1, BoolExpr::True);
    b.add_input_arc(a, t1_lock_a, 1, BoolExpr::True);
    b.add_output_arc(t1_lock_a, t1_s2, 1, None);
    b.add_input_arc(t1_s2, t1_lock_b, 1, BoolExpr::True);
    b.add_input_arc(b_, t1_lock_b, 1, BoolExpr::True);
    b.add_output_arc(t1_lock_b, t1_done, 1, None);

    // t2: lock B then A (reverse order)
    b.add_input_arc(t2_s1, t2_lock_b, 1, BoolExpr::True);
    b.add_input_arc(b_, t2_lock_b, 1, BoolExpr::True);
    b.add_output_arc(t2_lock_b, t2_s2, 1, None);
    b.add_input_arc(t2_s2, t2_lock_a, 1, BoolExpr::True);
    b.add_input_arc(a, t2_lock_a, 1, BoolExpr::True);
    b.add_output_arc(t2_lock_a, t2_done, 1, None);

    b.set_initial_tokens(t1_s1, 1);
    b.set_initial_tokens(t2_s1, 1);
    b.set_initial_tokens(a, 1);
    b.set_initial_tokens(b_, 1);
    b.build()
}

/// Cycle: p0 → t0 → p1 → t1 → p0 (for invariant/boundary tests).
pub fn cycle() -> Net {
    let mut b = NetBuilder::new();
    let p0 = b.add_place("p0", PlaceKind::Control(ControlSub::Statement));
    let p1 = b.add_place("p1", PlaceKind::Control(ControlSub::Statement));
    let t0 = b.add_transition("t0", TransitionKind::Sequential);
    let t1 = b.add_transition("t1", TransitionKind::Sequential);
    b.add_input_arc(p0, t0, 1, BoolExpr::True);
    b.add_output_arc(t0, p1, 1, None);
    b.add_input_arc(p1, t1, 1, BoolExpr::True);
    b.add_output_arc(t1, p0, 1, None);
    b.set_initial_tokens(p0, 1);
    b.build()
}
