//! Place roles added so a subnet can be read back off the net.

use unipn::analysis::pt::StateGraph;
use unipn::net::{ArcDir, Marking};
use unipn::pt::{PtPlaceKind, PtTransitionKind, ResourceKind, TransitionType};
use unipn::{ControlSub, PlaceClass, PlaceRole, PtNet};

#[test]
fn resource_kind_round_trips_from_construction() {
    let mut net = PtNet::new();
    let mutex = net.add_place("m", PtPlaceKind::with_resource(ResourceKind::Mutex));
    let condvar = net.add_place("cv", PtPlaceKind::with_resource(ResourceKind::Condvar));
    let unknown = net.add_place("old", PtPlaceKind::resource());
    let block = net.add_place("bb", PtPlaceKind::control(ControlSub::BasicBlock));

    assert_eq!(net.resource_kind(mutex), Some(ResourceKind::Mutex));
    assert_eq!(net.resource_kind(condvar), Some(ResourceKind::Condvar));
    assert_eq!(net.resource_kind(unknown), Some(ResourceKind::Other));
    assert_eq!(net.resource_kind(block), None);
    assert!(net.is_resource(mutex));
    assert!(!net.is_resource(block));
}

#[test]
fn resource_kind_round_trips_through_serde() {
    let kind = PtPlaceKind::with_resource(ResourceKind::RwLock);
    let written = serde_json::to_string(&kind).unwrap();
    assert!(
        written.contains(r#""place_type":{"Resource":"RwLock"}"#),
        "{written}"
    );
    let loaded: PtPlaceKind = serde_json::from_str(&written).unwrap();
    assert_eq!(loaded.place_type, PlaceClass::Resource(ResourceKind::RwLock));

    let mut net = PtNet::new();
    let place = net.add_place("rw", loaded);
    assert_eq!(net.resource_kind(place), Some(ResourceKind::RwLock));
    assert!(!net.is_deadlock(&Marking::new(vec![1])));
}

#[test]
fn thread_place_token_is_not_a_deadlock_candidate() {
    let mut net = PtNet::new();
    let thread = net.add_place("parent_thread", PtPlaceKind::control(ControlSub::ThreadPlace));
    let child_end = net.add_place("child_end", PtPlaceKind::control(ControlSub::FunctionEnd));
    let join = net.add_transition(
        "join",
        PtTransitionKind::new(TransitionType::Join("child".into())),
    );
    // The join consumes the thread token and the child end, so the thread
    // place has a consumer and is not a sink. Only the thread token is marked.
    net.add_arc(thread, join, ArcDir::Input, 1, ());
    net.add_arc(child_end, join, ArcDir::Input, 1, ());

    let marking = Marking::new(vec![1, 0]);
    assert!(net.is_thread_place(thread));
    assert!(!net.is_sink(thread));
    assert!(!net.place(thread).unwrap().kind.is_terminal());
    assert!(net.place(child_end).unwrap().kind.is_terminal());
    assert!(!net.is_deadlock(&marking));
    assert!(net.blocked_places(&marking).is_empty());

    let graph = StateGraph::from_net(&net, marking);
    assert!(
        graph.deadlock_states(&net).is_empty(),
        "a token on ThreadPlace must not be reported as a stuck thread"
    );
}

#[test]
fn function_end_is_still_the_only_annotated_thread_end() {
    let mut net = PtNet::new();
    let end = net.add_place("end", PtPlaceKind::control(ControlSub::FunctionEnd));
    let stuck = net.add_place("bb", PtPlaceKind::control(ControlSub::BasicBlock));
    let go = net.add_transition("go", PtTransitionKind::new(TransitionType::Goto));
    net.add_arc(stuck, go, ArcDir::Input, 1, ());
    net.add_arc(end, go, ArcDir::Input, 1, ());

    assert!(net.place(end).unwrap().kind.is_terminal());
    assert!(!net.is_thread_place(end));
    assert!(!net.place(stuck).unwrap().kind.is_terminal());
    assert!(!net.is_deadlock(&Marking::new(vec![1, 0])));
    assert!(net.is_deadlock(&Marking::new(vec![0, 1])));
}
