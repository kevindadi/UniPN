use unipn::analysis::pt::{
    BoundnessResult, StateGraph, StateGraphConfig, check_boundness, check_place_boundness,
};
use unipn::pt::{PlaceType, PtNet, PtPlaceKind, PtTransitionKind, TransitionType};
use unipn::{ArcDir, Marking, PlaceId, TransitionId};

fn relay_net() -> (PtNet, Marking) {
    let mut net = PtNet::new();
    let p0 = net.add_place("p0", PtPlaceKind::new(PlaceType::BasicBlock));
    let p1 = net.add_place("p1", PtPlaceKind::new(PlaceType::BasicBlock));
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p0, t, ArcDir::Input, 1, ());
    net.add_arc(p1, t, ArcDir::Output, 1, ());
    (net, Marking::new(vec![1, 0]))
}

#[test]
fn reachability_graph_builds_and_reports_deadlock() {
    let (net, marking) = relay_net();
    let graph = StateGraph::from_net(&net, marking);

    assert_eq!(graph.states.len(), 2);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.blocked.len(), 1);

    // The blocked state is the one with a token on p1.
    let blocked = *graph.blocked.iter().next().unwrap();
    let node = graph.node(blocked);
    assert_eq!(node.marking.tokens(PlaceId(1)), 1);

    // p1 is a BasicBlock, so the token stranded there is a real deadlock.
    assert_eq!(graph.deadlock_states(&net), vec![blocked]);

    let stats = graph.stats();
    assert_eq!(stats.state_count, 2);
    assert_eq!(stats.edge_count, 1);
    assert_eq!(stats.blocked_count, 1);
    assert!(!stats.truncated);
}

#[test]
fn a_finished_thread_is_blocked_but_not_deadlocked() {
    let mut net = PtNet::new();
    let start = net.add_place("start", PtPlaceKind::new(PlaceType::FunctionStart));
    let lock = net.add_place("mutex", PtPlaceKind::new(PlaceType::Resources));
    let end = net.add_place("end", PtPlaceKind::new(PlaceType::FunctionEnd));
    let t = net.add_transition("run", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(start, t, ArcDir::Input, 1, ());
    net.add_arc(end, t, ArcDir::Output, 1, ());

    // The mutex starts free and is never taken, so its token stays put.
    let graph = StateGraph::from_net(&net, Marking::new(vec![1, 1, 0]));

    let terminal = *graph.blocked.iter().next().unwrap();
    assert_eq!(graph.node(terminal).marking.tokens(end), 1);
    assert_eq!(graph.node(terminal).marking.tokens(lock), 1);

    // Nothing left to fire, yet neither the finished thread nor the free mutex
    // is evidence of a deadlock.
    assert_eq!(graph.stats().blocked_count, 1);
    assert!(graph.deadlock_states(&net).is_empty());
}

#[test]
fn state_graph_carries_snapshots() {
    let (net, marking) = relay_net();
    let graph = StateGraph::from_net(&net, marking);

    let initial = graph.node(graph.initial);
    assert_eq!(initial.enabled.len(), 1);
    assert_eq!(initial.enabled[0].name, "t");
    assert_eq!(initial.places.len(), 1); // only non-zero places by default

    let edge = &graph.edges[0].2;
    assert_eq!(edge.transition.transition_type, TransitionType::Normal);
    assert_eq!(edge.changes.len(), 2);
}

#[test]
fn por_produces_same_reachable_states() {
    let (net, marking) = relay_net();
    let std = StateGraph::with_config(&net, marking.clone(), StateGraphConfig::default());
    let por = StateGraph::with_config(
        &net,
        marking,
        StateGraphConfig {
            use_por: true,
            ..StateGraphConfig::default()
        },
    );
    assert_eq!(std.states.len(), por.states.len());
    assert_eq!(std.blocked.len(), por.blocked.len());
}

fn unbounded_net() -> (PtNet, Marking) {
    let mut net = PtNet::new();
    let p = net.add_place("p", PtPlaceKind::new(PlaceType::BasicBlock));
    let q = net.add_place("q", PtPlaceKind::new(PlaceType::BasicBlock));
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p, t, ArcDir::Input, 1, ());
    net.add_arc(p, t, ArcDir::Output, 1, ());
    net.add_arc(q, t, ArcDir::Output, 1, ());
    (net, Marking::new(vec![1, 0]))
}

#[test]
fn boundness_detects_bounded_and_unbounded_nets() {
    let (bounded, marking) = relay_net();
    assert_eq!(
        check_boundness(&bounded, &marking),
        BoundnessResult::Bounded
    );

    let (unbounded, marking) = unbounded_net();
    match check_boundness(&unbounded, &marking) {
        BoundnessResult::Unbounded { .. } => {}
        other => panic!("expected Unbounded, got {other:?}"),
    }

    assert_eq!(
        check_place_boundness(&unbounded, &marking, PlaceId(1)),
        BoundnessResult::Unbounded {
            unbounded_places: vec![PlaceId(1)],
            witness_sequence: Some(vec![TransitionId(0)]),
        }
    );
}

#[test]
fn state_limit_truncates_graph() {
    let (net, marking) = relay_net();
    let graph = StateGraph::with_config(
        &net,
        marking,
        StateGraphConfig {
            state_limit: Some(1),
            ..StateGraphConfig::default()
        },
    );
    assert!(graph.truncated);
    assert_eq!(graph.states.len(), 1);
}
