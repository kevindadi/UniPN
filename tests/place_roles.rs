//! Place roles added so a subnet can be read back off the net.

use unipn::net::Marking;
use unipn::pt::{PtPlaceKind, ResourceKind};
use unipn::{ControlSub, PlaceClass, PtNet};

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
