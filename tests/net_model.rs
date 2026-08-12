use unipn::analysis::{AnalysisConfig, NetLike, SearchStrategy, explore, find_deadlocks};
use unipn::expr::{BoolExpr, CmpOp, Expr, Val, VarUpdate};
use unipn::model::{ControlSub, PlaceKind, ResourceType, TransitionKind};
use unipn::net::{ArcDir, Marking};
use unipn::pt::{CapacityMode, PlaceType, PtPlaceKind, PtTransitionKind, TransitionType};
use unipn::{
    CvnBuilder, CvnNet, PlaceId, PtNet, TimeInterval, TimedNet, TimedPlaceKind,
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
fn pt_capacity_reject_and_saturate() {
    let mut net = PtNet::new();
    let p = net.add_place("p", PtPlaceKind::new(PlaceType::BasicBlock));
    let kind = PtPlaceKind {
        place_type: PlaceType::BasicBlock,
        span: None,
        capacity: Some(1),
        capacity_mode: CapacityMode::Reject,
    };
    net.places[0].kind = kind;
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p, t, ArcDir::Output, 1, ());

    // A transition producing into a full Reject-capacity place is not fireable.
    let marking = Marking::new(vec![1]);
    assert!(net.fire(&marking, t).is_none());

    net.places[0].kind.capacity_mode = CapacityMode::Saturate;
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
    assert_eq!(initial.extra.get("x"), Some(&Val::int(0)));

    let graph = explore(&net, initial, &AnalysisConfig::default());
    // x increments 0→3 while x<3, so 3 firings, 4 states.
    assert_eq!(graph.state_count(), 4);
    assert!(!graph.truncated);

    let final_state = &graph.states[*graph.blocked.last().unwrap()];
    assert_eq!(final_state.extra.get("x"), Some(&Val::int(3)));
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

// ── Timed net (model only) ───────────────────────────────────────────────

#[test]
fn timed_net_model_builds() {
    let mut net = TimedNet::new();
    let p = net.add_place(
        "cpu",
        TimedPlaceKind {
            capacity: 1,
            saturate: false,
        },
    );
    let t = net.add_transition(
        "exec",
        TimedTransitionKind {
            interval: TimeInterval::closed(1, 5),
            priority: 0,
            core: 0,
            suspendable: false,
        },
    );
    net.add_arc(p, t, ArcDir::Input, 1, ());
    assert_eq!(net.num_places(), 1);
    assert_eq!(net.num_transitions(), 1);
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
