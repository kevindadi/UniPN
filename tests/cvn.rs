use unipn::analysis::{
    AnalysisConfig, PropertyViolation, SearchStrategy, blocked_places, check_boundness,
    conflict_sets, explore, find_dead_transitions, find_deadlocks, is_deadlock,
};
use unipn::model::{ControlSub, PlaceKind, ResourceType, TransitionKind};
use unipn::{BoolExpr, CmpOp, Expr, NetBuilder, NetLike, PlaceId, Val, VarUpdate};

fn mutex_net() -> unipn::Net {
    let mut b = NetBuilder::new();
    let start = b.add_place("start", PlaceKind::Control(ControlSub::Statement));
    let held = b.add_place("held", PlaceKind::Control(ControlSub::Statement));
    let end = b.add_place("end", PlaceKind::Control(ControlSub::ThreadEnd));
    let lock = b.add_place("m", PlaceKind::Resource(ResourceType::Mutex));
    let t_lock = b.add_transition("lock", TransitionKind::Lock);
    let t_unlock = b.add_transition("unlock", TransitionKind::Unlock);

    b.add_input_arc(start, t_lock, 1, BoolExpr::True);
    b.add_input_arc(lock, t_lock, 1, BoolExpr::True);
    b.add_output_arc(t_lock, held, 1, None);

    b.add_input_arc(held, t_unlock, 1, BoolExpr::True);
    b.add_output_arc(t_unlock, end, 1, None);
    b.add_output_arc(t_unlock, lock, 1, None);

    b.set_initial_tokens(start, 1);
    b.set_initial_tokens(lock, 1);
    b.build()
}

fn guarded_net() -> unipn::Net {
    let mut b = NetBuilder::new();
    let p = b.add_place("p", PlaceKind::Control(ControlSub::Statement));
    let q = b.add_place("q", PlaceKind::Control(ControlSub::Statement));
    let t = b.add_transition("t", TransitionKind::Sequential);

    let guard = BoolExpr::Cmp {
        op: CmpOp::Eq,
        lhs: Box::new(Expr::Ref("x".into())),
        rhs: Box::new(Expr::Lit(Val::int(0))),
    };
    let mut update = VarUpdate::new();
    update.insert("x".into(), Expr::Lit(Val::int(1)));

    b.add_input_arc(p, t, 1, guard);
    b.add_output_arc(t, q, 1, Some(update));
    b.set_initial_tokens(p, 1);
    b.add_variable("x", Val::int(0));
    b.set_variable_domain("x", 0, 1);
    b.build()
}

#[test]
fn mutex_net_fires_to_completion_without_deadlock() {
    let net = mutex_net();
    let config = AnalysisConfig::default();
    let rg = explore(&net, &config);
    assert!(!rg.truncated);
    assert!(
        find_deadlocks(&net, &rg).is_empty(),
        "mutex net should not deadlock"
    );
    assert_eq!(rg.state_count(), 3);
}

#[test]
fn guarded_transition_is_disabled_by_guard_and_updates_vars() {
    let net = guarded_net();
    let state = net.initial_state();

    // x = 0 → guard satisfied.
    let enabled = net.enabled_transitions(&state);
    assert_eq!(enabled.len(), 1);

    let next = net.fire(enabled[0], &state).unwrap();
    assert_eq!(next.marking.tokens(PlaceId(1)), 1);
    assert_eq!(
        next.vars().get("x"),
        Some(&Val::int(1)),
        "firing must apply the variable update"
    );

    // x = 1 → guard (x == 0) fails, transition disabled.
    assert!(net.enabled_transitions(&next).is_empty());
}

#[test]
fn dead_transition_detection_and_conflict_sets() {
    let mut b = NetBuilder::new();
    let p = b.add_place("p", PlaceKind::Control(ControlSub::Statement));
    let live = b.add_transition("live", TransitionKind::Sequential);
    let dead = b.add_transition("dead", TransitionKind::Sequential);
    let q = b.add_place("q", PlaceKind::Control(ControlSub::Statement));
    // A place that never receives a token, so `dead` never fires.
    let unreachable = b.add_place("unreachable", PlaceKind::Control(ControlSub::Statement));

    b.add_input_arc(p, live, 1, BoolExpr::True);
    b.add_output_arc(live, q, 1, None);
    b.add_input_arc(unreachable, dead, 1, BoolExpr::True);
    b.add_output_arc(dead, q, 1, None);
    b.set_initial_tokens(p, 1);

    let net = b.build();
    let rg = explore(&net, &AnalysisConfig::default());

    let dead_transitions = find_dead_transitions(&net, &rg);
    assert_eq!(dead_transitions.len(), 1);
    match &dead_transitions[0].kind {
        PropertyViolation::DeadTransition { transition, .. } => {
            assert_eq!(*transition, dead);
        }
        other => panic!("expected DeadTransition, got {other:?}"),
    }

    let conflicts = conflict_sets(&net);
    assert!(conflicts.is_empty(), "no shared input place here");
}

#[test]
fn deadlock_and_blocked_places_are_reported() {
    // Two transitions that each need a token from the other's output → deadlock.
    let mut b = NetBuilder::new();
    let pa = b.add_place("pa", PlaceKind::Control(ControlSub::Statement));
    let pb = b.add_place("pb", PlaceKind::Control(ControlSub::Statement));
    let ta = b.add_transition("ta", TransitionKind::Sequential);
    let tb = b.add_transition("tb", TransitionKind::Sequential);

    b.add_input_arc(pa, ta, 1, BoolExpr::True);
    b.add_output_arc(ta, pb, 1, None);
    b.add_input_arc(pb, tb, 1, BoolExpr::True);
    b.add_output_arc(tb, pa, 1, None);
    b.set_initial_tokens(pa, 1);

    let net = b.build();
    let rg = explore(&net, &AnalysisConfig::default());

    // pa → ta → pb → tb → pa is actually a cycle, so this net is live, not
    // deadlocked. Build a genuinely deadlocked net instead.
    assert!(find_deadlocks(&net, &rg).is_empty());

    // A net where both transitions compete for the same single token but the
    // graph still progresses is not a deadlock; verify `is_deadlock` on a
    // state with no enabled transitions.
    let mut dead_net = NetBuilder::new();
    let p0 = dead_net.add_place("p0", PlaceKind::Control(ControlSub::Statement));
    let t0 = dead_net.add_transition("t0", TransitionKind::Sequential);
    dead_net.add_input_arc(p0, t0, 2, BoolExpr::True);
    dead_net.set_initial_tokens(p0, 1);
    let dead_net = dead_net.build();

    let initial = dead_net.initial_state();
    assert!(dead_net.enabled_transitions(&initial).is_empty());
    assert!(is_deadlock(&dead_net, &initial));
    assert_eq!(blocked_places(&dead_net, &initial), vec![p0]);
}

#[test]
fn boundness_detects_bounded_and_unbounded_nets() {
    let bounded = mutex_net();
    assert!(matches!(
        check_boundness(&bounded),
        unipn::analysis::BoundnessResult::Bounded
    ));

    let mut b = NetBuilder::new();
    let p = b.add_place("p", PlaceKind::Control(ControlSub::Statement));
    let q = b.add_place("q", PlaceKind::Control(ControlSub::Statement));
    let t = b.add_transition("t", TransitionKind::Sequential);
    b.add_input_arc(p, t, 1, BoolExpr::True);
    b.add_output_arc(t, p, 1, None); // self-loop keeps p live
    b.add_output_arc(t, q, 1, None); // generator → unbounded q
    b.set_initial_tokens(p, 1);
    let unbounded = b.build();

    match check_boundness(&unbounded) {
        unipn::analysis::BoundnessResult::Unbounded { .. } => {}
        other => panic!("expected Unbounded, got {other:?}"),
    }
}

#[test]
fn dot_export_mentions_places_and_transitions() {
    let net = mutex_net();
    let dot = unipn::export::to_dot(&net);
    assert!(dot.contains("digraph PetriNet"));
    assert!(dot.contains("p0"));
    assert!(dot.contains("t0"));
}

#[cfg(feature = "invariants")]
#[test]
fn invariants_find_the_mutex_conservation_law() {
    let net = mutex_net();
    let place_invariants = unipn::analysis::invariants::place_invariants(&net);
    assert!(!place_invariants.is_empty());
}

#[test]
fn strategy_selection_affects_exploration_order_only() {
    let net = mutex_net();
    let bfs = explore(
        &net,
        &AnalysisConfig {
            strategy: SearchStrategy::Bfs,
            ..AnalysisConfig::default()
        },
    );
    let dfs = explore(
        &net,
        &AnalysisConfig {
            strategy: SearchStrategy::Dfs,
            ..AnalysisConfig::default()
        },
    );
    assert_eq!(bfs.state_count(), dfs.state_count());
}
