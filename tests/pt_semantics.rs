use unipn::core::expr::{ActionExpr, GuardExpr, Pattern, Term};
use unipn::core::model::{
    ArcDecl, InhibitorArc, InputArc, ModelError, Multiplicity, NetModel, OutputArc, PlaceDecl,
    ReadArc, ResetArc, RoleTag, TimingSpec, TransitionDecl,
};
use unipn::core::sort::Sort;
use unipn::core::value::{Token, Value};
use unipn::{
    Execution, PlaceId, PtMarking, PtSemantics, PtState, RuntimeError, Semantics, TransitionId,
};

fn place(id: usize) -> PlaceDecl {
    PlaceDecl {
        id: PlaceId(id),
        name: format!("p{id}"),
        sort: 0,
        multiplicity: Multiplicity::Many,
        role: Some(RoleTag::Other("test".into())),
    }
}

fn transition(id: usize) -> TransitionDecl {
    TransitionDecl {
        id: TransitionId(id),
        name: format!("t{id}"),
        guard: None,
        action: None,
        priority: None,
        timing: None,
        role: None,
    }
}

fn input(transition: usize, place: usize, weight: u32) -> ArcDecl {
    ArcDecl::Input {
        transition: TransitionId(transition),
        arc: InputArc {
            place: PlaceId(place),
            pattern: Pattern::Wildcard,
            weight,
        },
    }
}

fn output(transition: usize, place: usize, weight: u32) -> ArcDecl {
    ArcDecl::Output {
        transition: TransitionId(transition),
        arc: OutputArc {
            place: PlaceId(place),
            term: Term::Const(Value::Unit),
            weight,
        },
    }
}

fn model(
    place_count: usize,
    transitions: Vec<TransitionDecl>,
    arcs: Vec<ArcDecl>,
    initial_marking: Vec<(PlaceId, usize)>,
) -> NetModel {
    NetModel {
        places: (0..place_count).map(place).collect(),
        transitions,
        arcs,
        sorts: vec![Sort::Unit],
        initial_marking: initial_marking
            .into_iter()
            .map(|(place, count)| {
                (
                    place,
                    (0..count).map(|_| Token::new(0, Value::Unit)).collect(),
                )
            })
            .collect(),
    }
}

fn state(tokens: Vec<u32>) -> PtState {
    PtState::new(PtMarking(tokens), (), ())
}

#[test]
fn initial_state_aggregates_unit_tokens_and_empty_places() {
    let net = model(
        2,
        vec![transition(0)],
        Vec::new(),
        vec![(PlaceId(0), 2), (PlaceId(0), 3)],
    );

    let result = PtSemantics.initial_state(&net).unwrap();
    assert_eq!(result.marking.0, vec![5, 0]);
}

#[test]
fn enabled_preserves_transition_declaration_order_and_allows_source_transitions() {
    let net = model(
        1,
        vec![transition(0), transition(1)],
        vec![input(1, 0, 1)],
        vec![(PlaceId(0), 1)],
    );
    let current = PtSemantics.initial_state(&net).unwrap();

    let enabled = PtSemantics.enabled(&net, &current).unwrap();
    assert_eq!(
        enabled.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        vec![TransitionId(0), TransitionId(1)]
    );
}

#[test]
fn repeated_arcs_are_aggregated_and_same_place_input_output_is_atomic() {
    let net = model(
        1,
        vec![transition(0)],
        vec![
            input(0, 0, 1),
            input(0, 0, 2),
            output(0, 0, 2),
            output(0, 0, 1),
        ],
        vec![(PlaceId(0), 3)],
    );
    let before = PtSemantics.initial_state(&net).unwrap();

    let execution = PtSemantics
        .fire(&net, &before, TransitionId(0), &())
        .unwrap();
    assert_eq!(execution.fired, TransitionId(0));
    assert_eq!(execution.state.marking.0, vec![3]);
    assert_eq!(before.marking.0, vec![3]);
}

#[test]
fn fire_reports_not_enabled_without_mutating_the_input_state() {
    let net = model(
        1,
        vec![transition(0)],
        vec![input(0, 0, 2)],
        vec![(PlaceId(0), 1)],
    );
    let before = PtSemantics.initial_state(&net).unwrap();

    assert_eq!(
        PtSemantics.fire(&net, &before, TransitionId(0), &()),
        Err(RuntimeError::NotEnabled)
    );
    assert_eq!(before.marking.0, vec![1]);
}

#[test]
fn fire_reports_unknown_transition_and_invalid_marking_length() {
    let net = model(1, vec![transition(0)], Vec::new(), Vec::new());
    let current = state(vec![0]);
    assert_eq!(
        PtSemantics.fire(&net, &current, TransitionId(4), &()),
        Err(RuntimeError::UnknownTransition(TransitionId(4)))
    );

    let malformed = state(Vec::new());
    assert_eq!(
        PtSemantics.enabled(&net, &malformed),
        Err(RuntimeError::InvalidMarkingLength {
            expected: 1,
            actual: 0,
        })
    );
}

#[test]
fn fire_reports_arithmetic_overflow_without_mutating_the_input_state() {
    let net = model(1, vec![transition(0)], vec![output(0, 0, 1)], Vec::new());
    let before = state(vec![u32::MAX]);

    assert_eq!(
        PtSemantics.fire(&net, &before, TransitionId(0), &()),
        Err(RuntimeError::ArithmeticOverflow { place: PlaceId(0) })
    );
    assert_eq!(before.marking.0, vec![u32::MAX]);
}

#[test]
fn model_rejects_zero_weight_arcs() {
    let input_net = model(1, vec![transition(0)], vec![input(0, 0, 0)], Vec::new());
    assert_eq!(
        input_net.validate(),
        Err(ModelError::ZeroInputWeight {
            transition: TransitionId(0),
            place: PlaceId(0),
        })
    );

    let output_net = model(1, vec![transition(0)], vec![output(0, 0, 0)], Vec::new());
    assert_eq!(
        output_net.validate(),
        Err(ModelError::ZeroOutputWeight {
            transition: TransitionId(0),
            place: PlaceId(0),
        })
    );
}

#[test]
fn pt_semantics_rejects_non_unit_tokens_and_extended_features() {
    let mut non_unit = model(1, vec![transition(0)], Vec::new(), Vec::new());
    non_unit.initial_marking = vec![(PlaceId(0), vec![Token::new(0, Value::Int(1))])];
    assert_eq!(
        PtSemantics.initial_state(&non_unit),
        Err(RuntimeError::TypeMismatch)
    );

    let mut guard = model(1, vec![transition(0)], Vec::new(), Vec::new());
    guard.transitions[0].guard = Some(GuardExpr::True);
    assert_eq!(
        PtSemantics.initial_state(&guard),
        Err(RuntimeError::Unsupported)
    );

    let mut priority = model(1, vec![transition(0)], Vec::new(), Vec::new());
    priority.transitions[0].priority = Some(1);
    assert_eq!(
        PtSemantics.initial_state(&priority),
        Err(RuntimeError::Unsupported)
    );

    let mut timed = model(1, vec![transition(0)], Vec::new(), Vec::new());
    timed.transitions[0].timing = Some(TimingSpec {
        earliest: Some(0),
        latest: None,
    });
    assert_eq!(
        PtSemantics.initial_state(&timed),
        Err(RuntimeError::Unsupported)
    );
}

#[test]
fn pt_semantics_rejects_read_inhibitor_and_reset_arcs() {
    for arc in [
        ArcDecl::Read {
            transition: TransitionId(0),
            arc: ReadArc {
                place: PlaceId(0),
                pattern: Pattern::Wildcard,
            },
        },
        ArcDecl::Inhibitor {
            transition: TransitionId(0),
            arc: InhibitorArc {
                place: PlaceId(0),
                pattern: Pattern::Wildcard,
            },
        },
        ArcDecl::Reset {
            transition: TransitionId(0),
            arc: ResetArc { place: PlaceId(0) },
        },
    ] {
        let net = model(1, vec![transition(0)], vec![arc], Vec::new());
        assert_eq!(
            PtSemantics.initial_state(&net),
            Err(RuntimeError::Unsupported)
        );
    }
}

#[test]
fn pt_semantics_rejects_non_unit_place_sorts() {
    let mut net = model(1, vec![transition(0)], Vec::new(), Vec::new());
    net.sorts = vec![Sort::Bool];
    assert_eq!(
        PtSemantics.initial_state(&net),
        Err(RuntimeError::Unsupported)
    );
}

#[test]
fn pt_semantics_does_not_execute_arc_expressions() {
    let mut net = model(
        1,
        vec![transition(0)],
        vec![input(0, 0, 1)],
        vec![(PlaceId(0), 1)],
    );
    if let ArcDecl::Input { arc, .. } = &mut net.arcs[0] {
        arc.pattern = Pattern::Const(Value::Bool(true));
    }

    let state = PtSemantics.initial_state(&net).unwrap();
    assert_eq!(PtSemantics.enabled(&net, &state).unwrap().len(), 1);
}

#[allow(dead_code)]
fn _keep_imports(_: ActionExpr, _: Execution<PtState>, _: RoleTag) {}
