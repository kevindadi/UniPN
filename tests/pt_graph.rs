use unipn::analysis::{PtSearchStrategy, PtStateGraph, PtStateGraphConfig};
use unipn::bug::{
    AliasId, AtomicOrdering, BugNet, PlaceMetadata, PlaceType, ResourceId, SourceLocation,
    ThreadId, TransitionMetadata, TransitionType, UnsafeOp,
};
use unipn::{PtPlace, PtTransition, TransitionId};

fn bug_net() -> (BugNet, unipn::PlaceId, unipn::TransitionId) {
    let mut net = BugNet::new();
    let place = net.add_place(
        PtPlace::new("ready", 1),
        PlaceMetadata {
            place_type: PlaceType::BasicBlock,
            span: Some(SourceLocation::new("worker.rs", 10, 4)),
        },
    );
    let transition = net.add_transition(
        PtTransition::new("lock"),
        TransitionMetadata {
            transition_type: TransitionType::Lock {
                resource: ResourceId(7),
            },
            span: Some(SourceLocation::new("worker.rs", 11, 8)),
        },
    );
    net.net.add_input_arc(place, transition, 1);
    (net, place, transition)
}

#[test]
fn explores_bug_net_with_metadata_snapshots_and_path() {
    let (net, place, transition) = bug_net();
    let graph = PtStateGraph::explore_bug_net(
        &net,
        PtStateGraphConfig {
            strategy: PtSearchStrategy::BreadthFirst,
            max_states: 10,
        },
    )
    .unwrap();

    assert_eq!(graph.states.len(), 2);
    assert_eq!(graph.initial.0, 0);
    assert_eq!(graph.states[0].enabled, vec![transition]);
    assert_eq!(graph.states[0].places[place.index()].tokens, 1);
    assert_eq!(
        graph.states[0].places[place.index()].metadata,
        net.place_metadata(place).cloned()
    );

    let edge = graph.outgoing(graph.initial).next().unwrap();
    assert_eq!(edge.transition, transition);
    assert_eq!(
        edge.transition_metadata,
        net.transition_metadata(transition).cloned()
    );
    assert_eq!(edge.changes.len(), 1);
    assert_eq!(edge.changes[0].before, 1);
    assert_eq!(edge.changes[0].after, 0);
    assert_eq!(edge.arcs.len(), 1);
    assert_eq!(graph.path_to(edge.target).unwrap().len(), 1);
}

#[test]
fn state_graph_deduplicates_markings_and_records_blocked_states() {
    let mut net = BugNet::new();
    let source = net.add_place(
        PtPlace::new("source", 1),
        PlaceMetadata {
            place_type: PlaceType::FunctionStart,
            span: None,
        },
    );
    let target = net.add_place(
        PtPlace::new("target", 0),
        PlaceMetadata {
            place_type: PlaceType::FunctionEnd,
            span: None,
        },
    );
    let first = net.add_transition(
        PtTransition::new("first"),
        TransitionMetadata {
            transition_type: TransitionType::Goto,
            span: None,
        },
    );
    let second = net.add_transition(
        PtTransition::new("second"),
        TransitionMetadata {
            transition_type: TransitionType::Switch,
            span: None,
        },
    );
    net.net.add_input_arc(source, first, 1);
    net.net.add_output_arc(target, first, 1);
    net.net.add_input_arc(source, second, 1);
    net.net.add_output_arc(target, second, 1);

    let graph = PtStateGraph::explore_bug_net(&net, PtStateGraphConfig::default()).unwrap();
    assert_eq!(graph.states.len(), 2);
    assert_eq!(graph.edges.len(), 2);
    assert_eq!(graph.blocked, vec![unipn::analysis::PtStateId(1)]);
    assert_eq!(
        graph.path_to(unipn::analysis::PtStateId(1)).unwrap().len(),
        1
    );
}

#[test]
fn state_graph_supports_depth_first_search_and_truncation() {
    let (net, _, _) = bug_net();
    let graph = PtStateGraph::explore_bug_net(
        &net,
        PtStateGraphConfig {
            strategy: PtSearchStrategy::DepthFirst,
            max_states: 1,
        },
    )
    .unwrap();
    assert_eq!(graph.states.len(), 1);
    assert!(graph.truncated);
    assert!(graph.edges.is_empty());
}

#[test]
fn invalid_metadata_and_configuration_are_rejected() {
    let (net, _, _) = bug_net();
    let invalid_places = [net.places[0].clone(), net.places[0].clone()];
    assert_eq!(
        PtStateGraph::explore(
            &net.net,
            Some((&invalid_places, &net.transitions)),
            PtStateGraphConfig::default(),
        ),
        Err(unipn::PtExecutionError::Model(
            unipn::PtModelError::MetadataLengthMismatch
        ))
    );
    assert_eq!(
        PtStateGraph::explore(
            &net.net,
            None,
            PtStateGraphConfig {
                strategy: PtSearchStrategy::BreadthFirst,
                max_states: 0,
            },
        ),
        Err(unipn::PtExecutionError::Model(
            unipn::PtModelError::InvalidConfiguration
        ))
    );
}

#[test]
fn fire_failures_are_recorded_without_aborting_exploration() {
    let mut net = BugNet::new();
    let place = net.add_place(
        PtPlace {
            capacity: Some(1),
            ..PtPlace::new("p", 1)
        },
        PlaceMetadata {
            place_type: PlaceType::Resource,
            span: None,
        },
    );
    let transition = net.add_transition(
        PtTransition::new("store"),
        TransitionMetadata {
            transition_type: TransitionType::AtomicStore {
                alias: AliasId(1),
                ordering: AtomicOrdering::SeqCst,
                thread: ThreadId(2),
            },
            span: None,
        },
    );
    net.net.add_output_arc(place, transition, 1);

    let graph = PtStateGraph::explore_bug_net(&net, PtStateGraphConfig::default()).unwrap();
    assert_eq!(graph.states.len(), 1);
    assert_eq!(graph.failures.len(), 1);
    assert_eq!(graph.failures[0].source, graph.initial);
    assert_eq!(graph.failures[0].transition, transition);
}

#[test]
fn metadata_supports_unsafe_and_atomic_events() {
    let unsafe_op = UnsafeOp {
        alias: AliasId(3),
        is_write: true,
        span: Some(SourceLocation::new("memory.rs", 2, 3)),
        basic_block: 4,
        ty: "u64".into(),
    };
    let unsafe_metadata = TransitionMetadata {
        transition_type: TransitionType::UnsafeWrite(unsafe_op.clone()),
        span: None,
    };
    assert_eq!(
        unsafe_metadata.transition_type,
        TransitionType::UnsafeWrite(unsafe_op)
    );
    assert_eq!(TransitionId(0).index(), 0);
}
