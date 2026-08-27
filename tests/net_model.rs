use unipn::analysis::{
    AnalysisConfig, NetLike, SearchStrategy, conflict_sets, explore, find_deadlocks,
};
use unipn::cvn::expr::{BoolExpr, CmpOp, Expr, Val, VarUpdate};
use unipn::cvn::kinds::{
    ControlSub, CvnArcKind, CvnTransition, PlaceKind, ResourceType, TransitionKind,
};
use unipn::net::{ArcDir, Marking, TransitionRole};
use unipn::pt::{
    AliasId, AtomicOrdering, PlaceType, PtPlaceKind, PtTransitionKind, TransitionType,
};
use unipn::{
    CvnBuilder, CvnNet, PlaceId, PtNet, TimeInterval, TimedBuilder, TimedNet, TimedPlaceKind,
    TimedState, TimedTransitionKind, TransitionId,
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
    let p = b.add_place("p", PlaceKind::Control(ControlSub::BasicBlock));
    let t = b.add_transition("inc", CvnTransition::new(TransitionKind::Sequential));
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
    let t = b.add_transition("lock", CvnTransition::new(TransitionKind::Lock));
    b.add_input_arc(lock, t, 1, BoolExpr::True);
    b.set_initial_tokens(lock, 1);
    let (net, initial) = b.build();

    let fired = net.fire(&initial, t).unwrap();
    assert_eq!(fired.marking.tokens(lock), 0);
    // Mutex capacity 1 → producing a second token is blocked, and the second
    // lock is not enabled (no token).
    assert!(net.enabled(&fired).is_empty());
}

#[test]
fn a_zero_capacity_channel_refuses_a_lone_send() {
    // A rendezvous channel has nowhere to park a message, so a send that is not
    // matched by a receive must be rejected outright rather than clamped.
    let build = |capacity: usize| {
        let mut b = CvnBuilder::new();
        let ready = b.add_place("ready", PlaceKind::Control(ControlSub::BasicBlock));
        let sent = b.add_place("sent", PlaceKind::Control(ControlSub::FunctionEnd));
        let chan = b.add_place(
            "ch",
            PlaceKind::Resource(ResourceType::Channel { capacity }),
        );
        let send = b.add_transition("send", CvnTransition::new(TransitionKind::Send));
        b.add_input_arc(ready, send, 1, BoolExpr::True);
        b.add_output_arc(send, sent, 1, None);
        b.add_output_arc(send, chan, 1, None);
        b.set_initial_tokens(ready, 1);
        let (net, initial) = b.build();
        (net, initial, send, chan)
    };

    // Capacity 0: structurally enabled, but the firing itself is refused.
    let (net, initial, send, _) = build(0);
    assert_eq!(net.enabled(&initial), vec![send]);
    assert!(net.fire(&initial, send).is_none());

    // Capacity 1: the same send goes through and the slot is taken.
    let (net, initial, send, chan) = build(1);
    let fired = net.fire(&initial, send).unwrap();
    assert_eq!(fired.marking.tokens(chan), 1);
}

#[test]
fn dropping_a_dead_variable_merges_equivalent_states() {
    // Two branches write different values to `x` and then converge. Once `x`
    // is out of scope the two final states are the same behavior, but a store
    // that still holds `x` keeps them apart.
    let build = |scope_end: bool| {
        let mut b = CvnBuilder::new();
        let p0 = b.add_place("p0", PlaceKind::Control(ControlSub::BasicBlock));
        let p1 = b.add_place("p1", PlaceKind::Control(ControlSub::BasicBlock));
        let p2 = b.add_place("p2", PlaceKind::Control(ControlSub::FunctionEnd));
        let ta = b.add_transition("a", CvnTransition::new(TransitionKind::BranchTrue));
        let tb = b.add_transition("b", CvnTransition::new(TransitionKind::BranchFalse));
        let end = b.add_transition("end", CvnTransition::new(TransitionKind::Return));

        for (t, value) in [(ta, 1), (tb, 2)] {
            let mut update = VarUpdate::new();
            update.insert("x".into(), Expr::Lit(Val::int(value)));
            b.add_input_arc(p0, t, 1, BoolExpr::True);
            b.add_output_arc(t, p1, 1, Some(update));
        }

        b.add_input_arc(p1, end, 1, BoolExpr::True);
        if scope_end {
            b.add_scope_end_arc(end, p2, 1, ["x".to_string()]);
        } else {
            b.add_output_arc(end, p2, 1, None);
        }

        b.set_initial_tokens(p0, 1);
        b.add_variable("x", Val::int(0));
        b.build()
    };

    let (net, initial) = build(false);
    let kept = explore(&net, initial, &AnalysisConfig::default());

    let (net, initial) = build(true);
    let dropped = explore(&net, initial, &AnalysisConfig::default());

    // The dead `x` splits the two final states; dropping it merges them.
    assert_eq!(kept.state_count(), 5);
    assert_eq!(dropped.state_count(), 4);

    let final_state = &dropped.states[*dropped.blocked.last().unwrap()];
    assert!(!final_state.extra.vars.contains_key("x"));
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
    let mut b = TimedBuilder::new();
    let p0 = b.add_marked_place("p0", timed_place(None, false), 1);
    let p1 = b.add_place("p1", timed_place(None, false));
    let t = b.add_transition("t", timed_transition(1, 5));
    b.add_arc(p0, t, ArcDir::Input, 1, ());
    b.add_arc(p1, t, ArcDir::Output, 1, ());
    b.build()
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
    let mut b = TimedBuilder::new();
    let src = b.add_marked_place("src", timed_place(None, false), 1);
    let dst = b.add_marked_place("dst", timed_place(Some(1), true), 1);
    let t = b.add_transition("produce", timed_transition(0, 0));
    b.add_arc(src, t, ArcDir::Input, 1, ());
    b.add_arc(dst, t, ArcDir::Output, 1, ());
    let (net, state) = b.build();

    // Successor capacity does not gate enabling; overflow is clamped.
    assert_eq!(net.enabled(&state), vec![t]);
    let fired = NetLike::fire(&net, &state, t).unwrap();
    assert_eq!(fired.marking.tokens(src), 0);
    assert_eq!(fired.marking.tokens(dst), 1);
}

#[test]
fn overflow_is_reported_only_for_non_saturating_places() {
    // Same clamp on two places, but only the non-saturating one is a fault:
    // a saturating place is *meant* to absorb the overflow.
    let build = |saturate: bool| {
        let mut b = TimedBuilder::new();
        let src = b.add_marked_place("src", timed_place(None, false), 1);
        let dst = b.add_marked_place("dst", timed_place(Some(1), saturate), 1);
        let t = b.add_transition("produce", timed_transition(0, 0));
        b.add_arc(src, t, ArcDir::Input, 1, ());
        b.add_arc(dst, t, ArcDir::Output, 1, ());
        let (net, state) = b.build();
        (net, state, dst, t)
    };

    let (net, state, dst, t) = build(true);
    let (marking, overflowed) = net.fire_reporting_overflow(&state.marking, t);
    assert_eq!(marking.tokens(dst), 1);
    assert!(overflowed.is_empty());

    let (net, state, dst, t) = build(false);
    let (marking, overflowed) = net.fire_reporting_overflow(&state.marking, t);
    assert_eq!(marking.tokens(dst), 1);
    assert_eq!(overflowed, vec![dst]);
}

#[test]
fn timed_net_aggregates_parallel_inputs_and_honours_inhibitor() {
    let mut net = TimedNet::new();
    let p = net.add_place("p", timed_place(None, false));
    let block = net.add_place("block", timed_place(None, false));
    let t = net.add_transition("t", timed_transition(0, 0));
    // Two parallel input arcs on the same place: the demand is their sum.
    net.add_arc(p, t, ArcDir::Input, 2, ());
    net.add_arc(p, t, ArcDir::Input, 3, ());
    net.add_arc(block, t, ArcDir::Inhibitor, 1, ());

    let state = |p_tokens: usize, block_tokens: usize| {
        TimedState::from(Marking::new(vec![p_tokens, block_tokens]))
    };

    // 4 tokens do not cover the aggregated demand of 5.
    assert!(net.enabled(&state(4, 0)).is_empty());
    assert_eq!(net.enabled(&state(5, 0)), vec![t]);
    // The inhibitor arc blocks once `block` holds a token.
    assert!(net.enabled(&state(5, 1)).is_empty());

    // Firing consumes both parallel weights.
    let fired = net.fire(&Marking::new(vec![5, 0]), t);
    assert_eq!(fired.tokens(p), 0);
}

#[test]
fn place_capacity_is_uniform_across_frontends() {
    let (pt, _) = pt_relay();
    // P/T and timed places report their capacity field; CVN derives it.
    assert_eq!(pt.capacity_of(PlaceId(0)), None);

    let mut b = CvnBuilder::new();
    let mutex = b.add_place("m", PlaceKind::Resource(ResourceType::Mutex));
    let sem = b.add_place(
        "s",
        PlaceKind::Resource(ResourceType::Semaphore { count: 3 }),
    );
    let ctrl = b.add_place("c", PlaceKind::Control(ControlSub::BasicBlock));
    let (cvn, _) = b.build();
    assert_eq!(cvn.capacity_of(mutex), Some(1));
    assert_eq!(cvn.capacity_of(sem), Some(3));
    assert_eq!(cvn.capacity_of(ctrl), None);
}

#[test]
fn place_roles_and_the_deadlock_definition_are_shared() {
    // The same shape twice: a control point that takes the mutex and runs to a
    // function end. The control point needs its outgoing arc, or it would read
    // as a structural sink and therefore as an ending.
    let mut pt = PtNet::new();
    let pt_ctrl = pt.add_place("bb", PtPlaceKind::new(PlaceType::BasicBlock));
    let pt_res = pt.add_place("m", PtPlaceKind::new(PlaceType::Resources));
    let pt_end = pt.add_place("end", PtPlaceKind::new(PlaceType::FunctionEnd));
    let pt_t = pt.add_transition("lock", PtTransitionKind::new(TransitionType::Lock(0)));
    pt.add_arc(pt_ctrl, pt_t, ArcDir::Input, 1, ());
    pt.add_arc(pt_res, pt_t, ArcDir::Input, 1, ());
    pt.add_arc(pt_end, pt_t, ArcDir::Output, 1, ());

    let mut b = CvnBuilder::new();
    let cvn_ctrl = b.add_place("bb", PlaceKind::Control(ControlSub::BasicBlock));
    let cvn_res = b.add_place("m", PlaceKind::Resource(ResourceType::Mutex));
    let cvn_end = b.add_place("end", PlaceKind::Control(ControlSub::FunctionEnd));
    let cvn_t = b.add_transition("lock", CvnTransition::new(TransitionKind::Lock));
    b.add_arc(cvn_ctrl, cvn_t, ArcDir::Input, 1, CvnArcKind::Plain);
    b.add_arc(cvn_res, cvn_t, ArcDir::Input, 1, CvnArcKind::Plain);
    b.add_arc(cvn_end, cvn_t, ArcDir::Output, 1, CvnArcKind::Plain);
    let (cvn, _) = b.build();

    assert!(pt.is_resource(pt_res) && cvn.is_resource(cvn_res));
    assert!(pt.is_terminal(pt_end) && cvn.is_terminal(cvn_end));
    assert!(!pt.is_resource(pt_ctrl) && !pt.is_terminal(pt_ctrl));
    assert!(!cvn.is_resource(cvn_ctrl) && !cvn.is_terminal(cvn_ctrl));

    // A finished thread next to a free mutex is not a deadlock; a token
    // stranded on the control point is. Both frontends now say so.
    let done = Marking::new(vec![0, 1, 1]);
    let stuck = Marking::new(vec![1, 0, 0]);
    assert!(!pt.is_deadlock(&done) && !cvn.is_deadlock(&done));
    assert!(pt.is_deadlock(&stuck) && cvn.is_deadlock(&stuck));

    let all = Marking::new(vec![1, 1, 1]);
    assert_eq!(pt.blocked_places(&all), vec![pt_ctrl]);
    assert_eq!(cvn.blocked_places(&all), vec![cvn_ctrl]);
}

#[test]
fn an_unannotated_exit_is_terminal_because_it_is_a_sink() {
    // A detached thread's last place: the lowering never labelled it, but no arc
    // can take the token anywhere, so it is an ending rather than a deadlock.
    let mut net = PtNet::new();
    let start = net.add_place("bb0", PtPlaceKind::new(PlaceType::BasicBlock));
    let stranded = net.add_place("bb1", PtPlaceKind::new(PlaceType::BasicBlock));
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(start, t, ArcDir::Input, 1, ());
    net.add_arc(stranded, t, ArcDir::Output, 1, ());

    assert!(!net.is_sink(start) && net.is_sink(stranded));
    assert!(net.is_terminal(stranded));
    assert!(!net.is_deadlock(&Marking::new(vec![0, 1])));

    // A read arc does not consume, so it does not stop a place being a sink.
    let observer = net.add_transition("obs", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(stranded, observer, ArcDir::Read, 1, ());
    assert!(net.is_sink(stranded));
}

#[test]
fn transition_roles_give_both_frontends_one_vocabulary() {
    // Same operation, two lowerings: P/T carries the alias id its pointer
    // analysis inferred, the CVN carries a bare tag.
    let pt_lock = PtTransitionKind::new(TransitionType::Lock(7));
    let cvn_lock = CvnTransition::new(TransitionKind::Lock);
    assert!(pt_lock.is_acquire() && cvn_lock.is_acquire());
    assert!(!pt_lock.is_release() && !cvn_lock.is_release());

    // Releasing a Rust lock is dropping its guard.
    assert!(PtTransitionKind::new(TransitionType::DropWrite(7)).is_release());
    assert!(CvnTransition::new(TransitionKind::Unlock).is_release());

    assert!(PtTransitionKind::new(TransitionType::Spawn("w".into())).is_thread_spawn());
    assert!(CvnTransition::new(TransitionKind::Spawn).is_thread_spawn());

    let alias = AliasId {
        instance_id: 0,
        local: 1,
        array_index: None,
        field: None,
    };
    let pt_atomic = TransitionType::AtomicLoad(alias, AtomicOrdering::SeqCst, "x".into(), 0);
    assert!(PtTransitionKind::new(pt_atomic).is_atomic());
    assert!(CvnTransition::new(TransitionKind::AtomicLoad).is_atomic());

    assert!(PtTransitionKind::new(TransitionType::UnsafeAccess(Vec::new())).is_unsafe_access());
    assert!(CvnTransition::new(TransitionKind::UnsafeAccess).is_unsafe_access());

    // Where the two lowerings genuinely differ: P/T's single `Wait` both drops
    // and retakes the lock, so it is neither; the CVN splits it in two and
    // classifies both halves.
    let pt_wait = PtTransitionKind::new(TransitionType::Wait);
    assert!(!pt_wait.is_acquire() && !pt_wait.is_release());
    assert!(CvnTransition::new(TransitionKind::CondvarWaitEnter).is_release());
    assert!(CvnTransition::new(TransitionKind::CondvarReacquire).is_acquire());

    // Waiting for an *event* is a separate question from taking a resource, and
    // both frontends draw the line in the same place: once the notification has
    // arrived, the thread is merely queueing for the lock. A semaphore permit
    // blocks too, but somebody is holding it, so that is the acquire side.
    assert!(pt_wait.is_blocking_wait());
    assert!(CvnTransition::new(TransitionKind::CondvarWakeByNotify).is_blocking_wait());
    assert!(!CvnTransition::new(TransitionKind::CondvarReacquire).is_blocking_wait());
    assert!(!CvnTransition::new(TransitionKind::Acquire).is_blocking_wait());
}

#[test]
fn a_wait_point_is_derived_from_the_way_out_not_from_the_place_kind() {
    // The condvar-wait shape, as the ConcIR lowering builds it: `waiting` can
    // only be left by a wake, `reacquire` leads into the lock. Both are plain
    // `BasicBlock`s — which of them a stranded token means trouble on is read
    // off the transitions, not off the place.
    let mut b = CvnBuilder::new();
    let waiting = b.add_place("s:waiting", PlaceKind::Control(ControlSub::BasicBlock));
    let holding = b.add_place("s:reacquire", PlaceKind::Control(ControlSub::BasicBlock));
    let next = b.add_place("s:next", PlaceKind::Control(ControlSub::BasicBlock));
    let signals = b.add_place("cv.signals", PlaceKind::Resource(ResourceType::Condvar));
    let lock = b.add_place("m", PlaceKind::Resource(ResourceType::Mutex));

    let wake = b.add_transition(
        "s#wake",
        CvnTransition::new(TransitionKind::CondvarWakeByNotify),
    );
    b.add_arc(waiting, wake, ArcDir::Input, 1, CvnArcKind::Plain);
    b.add_arc(signals, wake, ArcDir::Input, 1, CvnArcKind::Plain);
    b.add_arc(holding, wake, ArcDir::Output, 1, CvnArcKind::Plain);

    let reacquire = b.add_transition(
        "s#reacquire",
        CvnTransition::new(TransitionKind::CondvarReacquire),
    );
    b.add_arc(holding, reacquire, ArcDir::Input, 1, CvnArcKind::Plain);
    b.add_arc(lock, reacquire, ArcDir::Input, 1, CvnArcKind::Plain);
    b.add_arc(next, reacquire, ArcDir::Output, 1, CvnArcKind::Plain);
    let (cvn, _) = b.build();

    // A token here means a notification that never came.
    assert!(cvn.is_wait_point(waiting));
    // A token here means contention for the lock, which is a different bug.
    assert!(!cvn.is_wait_point(holding));
    // The signal count is only left by the wake as well, but a resource resting
    // on its own place is not waiting for anything.
    assert!(!cvn.is_wait_point(signals));
    // And a place nothing consumes from is an ending, not a wait.
    assert!(!cvn.is_wait_point(next));

    // P/T parks the same thread in front of its single `Wait` transition, so the
    // question has to be asked of the way *out* to work for both frontends.
    let mut pt = PtNet::new();
    let before = pt.add_place("bb0", PtPlaceKind::new(PlaceType::BasicBlock));
    let after = pt.add_place("bb1", PtPlaceKind::new(PlaceType::BasicBlock));
    let w = pt.add_transition("wait", PtTransitionKind::new(TransitionType::Wait));
    pt.add_arc(before, w, ArcDir::Input, 1, ());
    pt.add_arc(after, w, ArcDir::Output, 1, ());
    assert!(pt.is_wait_point(before) && !pt.is_wait_point(after));

    // One ordinary way out is enough to say the token is not waiting on an
    // event: it could have gone the other way.
    let escape = pt.add_transition("goto", PtTransitionKind::new(TransitionType::Goto));
    pt.add_arc(before, escape, ArcDir::Input, 1, ());
    assert!(!pt.is_wait_point(before));
}

#[test]
fn conflict_sets_are_structural_and_serve_both_frontends() {
    let mut pt = PtNet::new();
    let shared = pt.add_place("m", PtPlaceKind::new(PlaceType::Resources));
    let a = pt.add_transition("a", PtTransitionKind::new(TransitionType::Lock(0)));
    let b = pt.add_transition("b", PtTransitionKind::new(TransitionType::Lock(0)));
    pt.add_arc(shared, a, ArcDir::Input, 1, ());
    pt.add_arc(shared, b, ArcDir::Input, 1, ());
    assert_eq!(conflict_sets(&pt), vec![(a, b)]);

    // A relay has nothing to compete over.
    let (relay, _) = pt_relay();
    assert!(conflict_sets(&relay).is_empty());

    // The same function, no CVN knowledge involved.
    let mut cb = CvnBuilder::new();
    let cvn_shared = cb.add_place("m", PlaceKind::Resource(ResourceType::Mutex));
    let ca = cb.add_transition("a", CvnTransition::new(TransitionKind::Lock));
    let cbt = cb.add_transition("b", CvnTransition::new(TransitionKind::Lock));
    cb.add_arc(cvn_shared, ca, ArcDir::Input, 1, CvnArcKind::Plain);
    cb.add_arc(cvn_shared, cbt, ArcDir::Input, 1, CvnArcKind::Plain);
    let (cvn, _) = cb.build();
    assert_eq!(conflict_sets(&cvn), vec![(ca, cbt)]);
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
