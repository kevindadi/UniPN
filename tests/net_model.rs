use unipn::analysis::{AnalysisConfig, NetLike, SearchStrategy, explore, find_deadlocks};
use unipn::cvn::expr::{BoolExpr, CmpOp, Expr, Val, VarUpdate};
use unipn::cvn::kinds::{ControlSub, PlaceKind, ResourceType, TransitionKind};
use unipn::net::{ArcDir, Marking};
use unipn::pt::{PlaceType, PtPlaceKind, PtTransitionKind, TransitionType};
use unipn::{
    CvnBuilder, CvnNet, PlaceId, PtNet, TimeInterval, TimedNet, TimedPlaceKind, TimedState,
    TimedTransitionKind, TransitionId,
};

// ── P/T net ──────────────────────────────────────────────────────────────

fn pt_relay() -> (PtNet, Marking) {
    let mut net = PtNet::new();
    let p0 = net.add_place("p0", PtPlaceKind::new(PlaceType::BasicBlock));
    let p1 = net.add_place("p1", PtPlaceKind::new(PlaceType::BasicBlock));
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p0, t, ArcDir::Input, 1, ());
    net.add_arc(p1, t, ArcDir::Output, 1, ());
    (net, Marking::new(vec![1, 0]))
}

#[test]
fn pt_net_fires_and_reports_deadlock_via_caller_predicate() {
    let (net, initial) = pt_relay();
    let enabled = net.enabled(&initial);
    assert_eq!(enabled, vec![TransitionId(0)]);

    let fired = net.fire(&initial, TransitionId(0)).unwrap();
    assert_eq!(fired.tokens(PlaceId(0)), 0);
    assert_eq!(fired.tokens(PlaceId(1)), 1);

    let graph = explore(&net, initial, &AnalysisConfig::default());
    assert_eq!(graph.state_count(), 2);
    assert_eq!(graph.blocked, vec![1]);
    // Caller decides what a deadlock is.
    let deadlocks = find_deadlocks(&graph, |s| s.iter_nonzero().next().is_some());
    assert_eq!(deadlocks, vec![1]);
}

#[test]
fn pt_capacity_saturates() {
    let mut net = PtNet::new();
    let p = net.add_place("p", PtPlaceKind::new(PlaceType::BasicBlock));
    net.places[0].kind.capacity = Some(1);
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p, t, ArcDir::Output, 1, ());

    // Producing into a full place clamps to capacity (ConcBugDect semantics).
    let marking = Marking::new(vec![1]);
    let fired = net.fire(&marking, t).unwrap();
    assert_eq!(fired.tokens(p), 1);
}

#[test]
fn pt_read_inhibitor_reset_arcs() {
    let mut net = PtNet::new();
    let p = net.add_place("p", PtPlaceKind::new(PlaceType::BasicBlock));
    let q = net.add_place("q", PtPlaceKind::new(PlaceType::BasicBlock));
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p, t, ArcDir::Read, 1, ());
    net.add_arc(q, t, ArcDir::Reset, 1, ());

    // Read arc requires p >= 1 without consuming; reset empties q.
    let marking = Marking::new(vec![1, 5]);
    assert!(net.enabled(&marking).contains(&t));
    let fired = net.fire(&marking, t).unwrap();
    assert_eq!(fired.tokens(p), 1); // read is non-destructive
    assert_eq!(fired.tokens(q), 0); // reset

    // Inhibitor arc blocks when the place holds enough tokens.
    let mut net2 = PtNet::new();
    let p2 = net2.add_place("p", PtPlaceKind::new(PlaceType::BasicBlock));
    let t2 = net2.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net2.add_arc(p2, t2, ArcDir::Inhibitor, 1, ());
    assert!(net2.enabled(&Marking::new(vec![0])).contains(&t2));
    assert!(!net2.enabled(&Marking::new(vec![1])).contains(&t2));
}

// ── CVN net ──────────────────────────────────────────────────────────────

fn cvn_counter() -> (CvnNet, unipn::CvnState) {
    let mut b = CvnBuilder::new();
    let p = b.add_place("p", PlaceKind::Control(ControlSub::Statement));
    let t = b.add_transition("inc", TransitionKind::Sequential);
    let guard = BoolExpr::Cmp {
        op: CmpOp::Lt,
        lhs: Box::new(Expr::Ref("x".into())),
        rhs: Box::new(Expr::Lit(Val::int(3))),
    };
    let mut update = VarUpdate::new();
    update.insert(
        "x".into(),
        Expr::BinOp {
            op: unipn::Op::Add,
            lhs: Box::new(Expr::Ref("x".into())),
            rhs: Box::new(Expr::Lit(Val::int(1))),
        },
    );
    b.add_input_arc(p, t, 1, guard);
    b.add_output_arc(t, p, 1, Some(update));
    b.set_initial_tokens(p, 1);
    b.add_variable("x", Val::int(0));
    b.build()
}

#[test]
fn cvn_guard_and_update_drive_firing() {
    let (net, initial) = cvn_counter();
    assert_eq!(initial.extra.vars.get("x"), Some(&Val::int(0)));

    let graph = explore(&net, initial, &AnalysisConfig::default());
    // x increments 0→3 while x<3, so 3 firings, 4 states.
    assert_eq!(graph.state_count(), 4);
    assert!(!graph.truncated);

    let final_state = &graph.states[*graph.blocked.last().unwrap()];
    assert_eq!(final_state.extra.vars.get("x"), Some(&Val::int(3)));
}

#[test]
fn cvn_resource_capacity_blocks_second_lock() {
    let mut b = CvnBuilder::new();
    let lock = b.add_place("m", PlaceKind::Resource(ResourceType::Mutex));
    let t = b.add_transition("lock", TransitionKind::Lock);
    b.add_input_arc(lock, t, 1, BoolExpr::True);
    b.set_initial_tokens(lock, 1);
    let (net, initial) = b.build();

    let fired = net.fire(&initial, t).unwrap();
    assert_eq!(fired.marking.tokens(lock), 0);
    // Mutex capacity 1 → producing a second token is blocked, and the second
    // lock is not enabled (no token).
    assert!(net.enabled(&fired).is_empty());
}

// ── Timed net (discrete NetLike) ─────────────────────────────────────────

fn timed_place(capacity: Option<usize>, saturate: bool) -> TimedPlaceKind {
    TimedPlaceKind { capacity, saturate }
}

fn timed_transition(earliest: i32, latest: i32) -> TimedTransitionKind {
    TimedTransitionKind {
        interval: TimeInterval::closed(earliest, latest),
        priority: 0,
        core: 0,
        suspendable: false,
    }
}

fn timed_relay() -> (TimedNet, TimedState) {
    let mut net = TimedNet::new();
    let p0 = net.add_place("p0", timed_place(None, false));
    let p1 = net.add_place("p1", timed_place(None, false));
    let t = net.add_transition("t", timed_transition(1, 5));
    net.add_arc(p0, t, ArcDir::Input, 1, ());
    net.add_arc(p1, t, ArcDir::Output, 1, ());
    (net, TimedState::from(Marking::new(vec![1, 0])))
}

#[test]
fn timed_net_fires_through_netlike_and_explore() {
    let (net, initial) = timed_relay();
    assert_eq!(net.enabled(&initial), vec![TransitionId(0)]);

    let fired = NetLike::fire(&net, &initial, TransitionId(0)).unwrap();
    assert_eq!(fired.marking.tokens(PlaceId(0)), 0);
    assert_eq!(fired.marking.tokens(PlaceId(1)), 1);
    assert!(NetLike::fire(&net, &fired, TransitionId(0)).is_none());

    let graph = explore(&net, initial, &AnalysisConfig::default());
    assert_eq!(graph.state_count(), 2);
    assert_eq!(graph.blocked, vec![1]);
}

#[test]
fn timed_net_clamps_capacity_and_stays_enabled() {
    let mut net = TimedNet::new();
    let src = net.add_place("src", timed_place(None, false));
    let dst = net.add_place("dst", timed_place(Some(1), true));
    let t = net.add_transition("produce", timed_transition(0, 0));
    net.add_arc(src, t, ArcDir::Input, 1, ());
    net.add_arc(dst, t, ArcDir::Output, 1, ());

    // Successor capacity does not gate enabling; overflow is clamped.
    let state = TimedState::from(Marking::new(vec![1, 1]));
    assert_eq!(net.enabled(&state), vec![t]);
    let fired = NetLike::fire(&net, &state, t).unwrap();
    assert_eq!(fired.marking.tokens(src), 0);
    assert_eq!(fired.marking.tokens(dst), 1);
}

#[test]
fn bf_vs_dfs_agree_on_reachable_state_count() {
    let (net, initial) = cvn_counter();
    let bfs = explore(&net, initial.clone(), &AnalysisConfig::default());
    let dfs = explore(
        &net,
        initial,
        &AnalysisConfig {
            strategy: SearchStrategy::Dfs,
            ..AnalysisConfig::default()
        },
    );
    assert_eq!(bfs.state_count(), dfs.state_count());
}

// ── Incidence / adjacency ────────────────────────────────────────────────

#[test]
fn incidence_matrix_of_a_relay_is_minus_one_plus_one() {
    let (net, _) = pt_relay();
    let inc = net.incidence();
    let t = TransitionId(0);
    let p0 = PlaceId(0);
    let p1 = PlaceId(1);

    assert_eq!(inc.pre(t), &[(p0, 1)]);
    assert_eq!(inc.post(t), &[(p1, 1)]);
    assert_eq!(inc.consumers(p0), &[(t, 1)]);
    assert_eq!(inc.producers(p1), &[(t, 1)]);

    let c = inc.matrix();
    assert_eq!(c.get(p0, t), -1);
    assert_eq!(c.get(p1, t), 1);
    assert_eq!(c.apply(&[1]), Some(vec![-1, 1]));
}

#[test]
fn incidence_aggregates_parallel_arcs_and_excludes_test_arcs() {
    let mut net = PtNet::new();
    let p = net.add_place("p", PtPlaceKind::new(PlaceType::BasicBlock));
    let q = net.add_place("q", PtPlaceKind::new(PlaceType::BasicBlock));
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p, t, ArcDir::Input, 2, ());
    net.add_arc(p, t, ArcDir::Input, 3, ());
    net.add_arc(q, t, ArcDir::Output, 1, ());
    net.add_arc(p, t, ArcDir::Read, 1, ());
    net.add_arc(q, t, ArcDir::Inhibitor, 4, ());
    net.add_arc(q, t, ArcDir::Reset, 1, ());

    let inc = net.incidence();
    assert_eq!(inc.pre_weight(p, t), 5);
    assert_eq!(inc.post_weight(q, t), 1);
    assert_eq!(inc.read(t), &[(p, 1)]);
    assert_eq!(inc.inhibitor(t), &[(q, 4)]);
    assert_eq!(inc.reset(t), &[(q, 1)]);

    let c = net.incidence_matrix();
    // Read / inhibitor / reset do not enter C.
    assert_eq!(c.get(p, t), -5);
    assert_eq!(c.get(q, t), 1);
}

#[test]
fn incidence_self_loop_cancels_in_c_but_stays_in_adjacency() {
    let mut net = PtNet::new();
    let p = net.add_place("p", PtPlaceKind::new(PlaceType::BasicBlock));
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p, t, ArcDir::Input, 1, ());
    net.add_arc(p, t, ArcDir::Output, 1, ());

    let inc = net.incidence();
    assert_eq!(inc.pre(t), &[(p, 1)]);
    assert_eq!(inc.post(t), &[(p, 1)]);
    assert_eq!(inc.matrix().get(p, t), 0);
}

#[test]
fn cvn_incidence_is_the_token_skeleton_guards_do_not_enter_c() {
    let (net, initial) = cvn_counter();
    let p = PlaceId(0);
    let t = TransitionId(0);
    let c = net.incidence_matrix();

    // Self-loop of weight 1: marking is conserved, C = [0].
    assert_eq!(c.get(p, t), 0);
    // The marking equation allows any firing count…
    assert_eq!(c.apply(&[3]), Some(vec![0]));
    assert_eq!(c.apply(&[100]), Some(vec![0]));
    // …but the guard `x < 3` only permits three firings.
    let graph = explore(&net, initial, &AnalysisConfig::default());
    assert_eq!(graph.edge_count(), 3);
    assert_eq!(graph.states.last().unwrap().marking.tokens(p), 1);
}
