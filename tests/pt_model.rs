use unipn::{
    CapacityMode, PlaceId, PtArcKind, PtExecutionError, PtMarking, PtModelError, PtNet, PtPlace,
    PtTransition, TransitionId,
};

fn net_with_transition(place: PtPlace) -> (PtNet, PlaceId, TransitionId) {
    let mut net = PtNet::new();
    let place = net.add_place(place);
    let transition = net.add_transition(PtTransition::new("t"));
    (net, place, transition)
}

#[test]
fn executes_extended_arcs_and_preserves_u64_tokens() {
    let (mut net, place, transition) = net_with_transition(PtPlace::new("p", u64::MAX - 2));
    net.add_input_arc(place, transition, 1);
    net.add_input_arc(place, transition, 1);
    net.add_read_arc(place, transition, u64::MAX - 2);
    net.add_output_arc(place, transition, 2);

    let marking = net.initial_marking();
    assert_eq!(net.enabled(&marking).unwrap(), vec![transition]);
    assert_eq!(
        net.fire(&marking, transition).unwrap(),
        PtMarking::from_tokens([u64::MAX - 2])
    );
}

#[test]
fn inhibitor_and_reset_arcs_control_firing() {
    let (mut net, place, transition) = net_with_transition(PtPlace::new("p", 3));
    net.add_inhibitor_arc(place, transition, 3);
    assert!(net.enabled(&net.initial_marking()).unwrap().is_empty());

    net.arcs.clear();
    net.add_reset_arc(place, transition);
    let next = net.fire(&net.initial_marking(), transition).unwrap();
    assert_eq!(next.tokens(place), 0);
}

#[test]
fn capacity_modes_apply_after_aggregating_output_arcs() {
    let (mut rejecting, place, transition) = net_with_transition(PtPlace::new("p", 1));
    rejecting.places[place.index()].capacity = Some(3);
    rejecting.add_output_arc(place, transition, 1);
    rejecting.add_output_arc(place, transition, 1);
    rejecting.add_output_arc(place, transition, 1);
    assert_eq!(
        rejecting.fire(&rejecting.initial_marking(), transition),
        Err(PtExecutionError::CapacityExceeded {
            place,
            tokens: 4,
            capacity: 3,
        })
    );

    let (mut saturating, place, transition) = net_with_transition(PtPlace::new("p", 1));
    saturating.places[place.index()].capacity = Some(3);
    saturating.places[place.index()].capacity_mode = CapacityMode::Saturate;
    saturating.add_output_arc(place, transition, 1);
    saturating.add_output_arc(place, transition, 1);
    saturating.add_output_arc(place, transition, 1);
    assert_eq!(
        saturating
            .fire(&saturating.initial_marking(), transition)
            .unwrap()
            .tokens(place),
        3
    );
}

#[test]
fn validates_initial_capacity_and_repeated_arc_overflow() {
    let mut capacity = PtNet::new();
    let place = capacity.add_place(PtPlace {
        capacity: Some(2),
        ..PtPlace::new("p", 3)
    });
    assert_eq!(
        capacity.validate(),
        Err(PtModelError::InitialCapacityExceeded {
            place,
            tokens: 3,
            capacity: 2,
        })
    );

    let (mut overflow, place, transition) = net_with_transition(PtPlace::new("p", 0));
    overflow.add_input_arc(place, transition, u64::MAX);
    overflow.add_input_arc(place, transition, 1);
    assert_eq!(
        overflow.validate(),
        Err(PtModelError::ArcWeightOverflow {
            place,
            transition,
            kind: PtArcKind::Input,
        })
    );
}

#[test]
fn rejects_invalid_marking_length_and_unknown_transition() {
    let (net, _, _) = net_with_transition(PtPlace::new("p", 0));
    assert_eq!(
        net.enabled(&PtMarking::new(0)),
        Err(PtExecutionError::Model(PtModelError::InvalidPlaceId(
            PlaceId(0)
        )))
    );
    assert_eq!(
        net.fire(&net.initial_marking(), TransitionId(4)),
        Err(PtExecutionError::UnknownTransition(TransitionId(4)))
    );
}

#[test]
fn reset_wins_over_output_on_the_same_place() {
    let (mut net, place, transition) = net_with_transition(PtPlace::new("p", 1));
    net.add_output_arc(place, transition, 2);
    net.add_reset_arc(place, transition);
    assert_eq!(
        net.fire(&net.initial_marking(), transition)
            .unwrap()
            .tokens(place),
        0
    );
}

#[test]
fn zero_weight_arcs_are_rejected() {
    let (mut net, place, transition) = net_with_transition(PtPlace::new("p", 0));
    net.add_arc(place, transition, PtArcKind::Read, 0);
    assert_eq!(
        net.validate(),
        Err(PtModelError::ZeroWeight {
            place,
            transition,
            kind: PtArcKind::Read,
        })
    );
}

#[test]
fn place_and_transition_ids_are_assigned_by_the_net() {
    let mut net = PtNet::new();
    let place = net.add_place(PtPlace {
        id: PlaceId(99),
        ..PtPlace::new("p", 0)
    });
    let transition = net.add_transition(PtTransition {
        id: TransitionId(99),
        ..PtTransition::new("t")
    });
    assert_eq!(place, PlaceId(0));
    assert_eq!(transition, TransitionId(0));
    assert_eq!(net.places[0].id, place);
    assert_eq!(net.transitions[0].id, transition);
}

#[test]
fn arc_kind_is_preserved_in_snapshots() {
    let (mut net, place, transition) = net_with_transition(PtPlace::new("p", 1));
    net.add_read_arc(place, transition, 1);
    assert_eq!(
        net.arcs_for(transition).next().unwrap().kind,
        PtArcKind::Read
    );
}
