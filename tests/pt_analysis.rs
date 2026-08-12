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
    assert_eq!(graph.deadlocks.len(), 1);

    // The blocked state is the one with a token on p1.
    let blocked = *graph.deadlocks.iter().next().unwrap();
    let node = graph.node(blocked);
    assert_eq!(node.marking.tokens(PlaceId(1)), 1);

    let stats = graph.stats();
    assert_eq!(stats.state_count, 2);
    assert_eq!(stats.edge_count, 1);
    assert_eq!(stats.deadlock_count, 1);
    assert!(!stats.truncated);
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
    assert_eq!(std.deadlocks.len(), por.deadlocks.len());
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
