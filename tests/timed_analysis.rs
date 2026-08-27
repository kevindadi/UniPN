use std::collections::HashMap;

use unipn::analysis::timed::{
    CanonicalizationMode, Scheduling, StateClassReachabilityGraph, TimedReachabilityConfig,
    out_edge_transitions, reachable_markings,
};
use unipn::net::{ArcDir, Marking, PlaceId};
use unipn::{
    TimeInterval, TimedBuilder, TimedNet, TimedPlaceKind, TimedTransitionKind, TransitionId,
};

fn single_transition_net() -> (TimedNet, Marking) {
    let mut net = TimedNet::new();
    let p0 = net.add_place(
        "p0",
        TimedPlaceKind {
            capacity: None,
            saturate: false,
        },
    );
    let p1 = net.add_place(
        "p1",
        TimedPlaceKind {
            capacity: None,
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
    net.add_input_arc(p0, t, ());
    net.add_output_arc(t, p1, ());
    (net, Marking::new(vec![1, 0]))
}

#[test]
fn builds_basic_state_class_graph() {
    let (net, marking) = single_transition_net();
    let mut graph = StateClassReachabilityGraph::new(&net, marking);
    let states = graph.build(100);

    assert_eq!(states, 2);
    assert_eq!(graph.get_graph().stats.total_transitions, 1);
    assert!(!graph.get_graph().stats.truncated);

    // Initial marking {p0=1} and fired marking {p1=1}.
    let markings = reachable_markings(graph.get_graph());
    assert_eq!(markings.len(), 2);
    assert!(markings.contains(&Marking::new(vec![1, 0])));
    assert!(markings.contains(&Marking::new(vec![0, 1])));
}

#[test]
fn respects_state_limit() {
    let (net, marking) = single_transition_net();
    let mut graph = StateClassReachabilityGraph::new(&net, marking);
    let states = graph.build(1);
    assert_eq!(states, 1);
    assert!(graph.get_graph().stats.truncated);
}

#[test]
fn canonicalization_and_extrapolation_are_configurable() {
    let (net, marking) = single_transition_net();
    let config = TimedReachabilityConfig {
        canonicalization: CanonicalizationMode::MaxLowerBound,
        extrapolation: true,
        core_parallelism: HashMap::new(),
    };
    let mut graph = StateClassReachabilityGraph::with_config(&net, marking, config);
    let states = graph.build(100);
    assert_eq!(states, 2);
}

#[test]
fn overflow_is_reported_on_the_graph_not_globally() {
    // `feed` produces into a capacity-1 non-saturating place that already holds
    // a token, so every firing clamps. The fault belongs to this build's
    // result, so it is read off the graph.
    let mut b = TimedBuilder::new();
    let src = b.add_marked_place(
        "src",
        TimedPlaceKind {
            capacity: None,
            saturate: false,
        },
        1,
    );
    let dst = b.add_marked_place(
        "dst",
        TimedPlaceKind {
            capacity: Some(1),
            saturate: false,
        },
        1,
    );
    let t = b.add_transition(
        "feed",
        TimedTransitionKind {
            interval: TimeInterval::closed(0, 0),
            priority: 0,
            core: 0,
            suspendable: false,
        },
    );
    b.add_arc(src, t, ArcDir::Input, 1, ());
    b.add_arc(dst, t, ArcDir::Output, 1, ());
    let (net, initial) = b.build();

    let mut graph = StateClassReachabilityGraph::new(&net, initial.marking.clone());
    graph.build(100);
    assert!(
        graph
            .get_graph()
            .stats
            .overflowed_places
            .contains(&PlaceId(dst.index()))
    );

    // A fresh build starts from a clean report — no global state to reset.
    let mut again = StateClassReachabilityGraph::new(&net, initial.marking);
    assert!(again.get_graph().stats.overflowed_places.is_empty());
    again.build(100);
    assert_eq!(graph.get_graph().stats.overflowed_places.len(), 1);
}

fn same_core_priority_net() -> (TimedNet, Marking) {
    let mut net = TimedNet::new();
    let input = net.add_place(
        "input",
        TimedPlaceKind {
            capacity: Some(2),
            saturate: false,
        },
    );
    net.add_transition(
        "low",
        TimedTransitionKind {
            interval: TimeInterval::closed(0, 5),
            priority: 1,
            core: 0,
            suspendable: true,
        },
    );
    net.add_transition(
        "high",
        TimedTransitionKind {
            interval: TimeInterval::closed(3, 3),
            priority: 9,
            core: 0,
            suspendable: false,
        },
    );
    net.add_arc(input, TransitionId(0), ArcDir::Input, 1, ());
    net.add_arc(input, TransitionId(0), ArcDir::Output, 1, ());
    net.add_arc(input, TransitionId(1), ArcDir::Input, 1, ());
    net.add_arc(input, TransitionId(1), ArcDir::Output, 1, ());
    (net, Marking::new(vec![1]))
}

#[test]
fn priority_filter_fires_high_priority_not_earliest() {
    let (net, marking) = same_core_priority_net();
    let (struct_enabled, priority_enabled, suspended) =
        Scheduling::compute_sets(&net, &marking, &HashMap::new());

    assert_eq!(struct_enabled, vec![0, 1]);
    assert_eq!(priority_enabled, vec![1]);
    assert_eq!(suspended, vec![0]);

    let mut graph = StateClassReachabilityGraph::new(&net, marking);
    graph.build(64);
    let transitions = out_edge_transitions(graph.get_graph(), graph.get_graph().initial);
    assert_eq!(transitions, vec![1]);
}
