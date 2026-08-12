use unipn::core::expr::{Pattern, Term};
use unipn::core::model::{
    ArcDecl, InputArc, Multiplicity, NetModel, OutputArc, PlaceDecl, TransitionDecl,
};
use unipn::core::sort::Sort;
use unipn::core::value::{Token, Value};
use unipn::{ColoredSemantics, PlaceId, Semantics, TransitionId};

fn int_relay_net() -> NetModel {
    NetModel {
        places: vec![
            PlaceDecl {
                id: PlaceId(0),
                name: "src".into(),
                sort: 0,
                multiplicity: Multiplicity::Many,
                role: None,
            },
            PlaceDecl {
                id: PlaceId(1),
                name: "dst".into(),
                sort: 0,
                multiplicity: Multiplicity::Many,
                role: None,
            },
        ],
        transitions: vec![TransitionDecl {
            id: TransitionId(0),
            name: "relay".into(),
            guard: None,
            action: None,
            priority: None,
            timing: None,
            role: None,
        }],
        arcs: vec![
            ArcDecl::Input {
                transition: TransitionId(0),
                arc: InputArc {
                    place: PlaceId(0),
                    pattern: Pattern::Var("x".into()),
                    weight: 1,
                },
            },
            ArcDecl::Output {
                transition: TransitionId(0),
                arc: OutputArc {
                    place: PlaceId(1),
                    term: Term::Var("x".into()),
                    weight: 1,
                },
            },
        ],
        sorts: vec![Sort::Int { lo: None, hi: None }],
        initial_marking: vec![(PlaceId(0), vec![Token::new(0, Value::Int(42))])],
    }
}

#[test]
fn colored_semantics_binds_and_fires_a_single_token() {
    let semantics = ColoredSemantics;
    let model = int_relay_net();
    let state = semantics.initial_state(&model).unwrap();

    assert_eq!(state.marking.len(), 1);

    let enabled = semantics.enabled(&model, &state).unwrap();
    assert_eq!(enabled.len(), 1);
    let (transition, binding) = &enabled[0];
    assert_eq!(*transition, TransitionId(0));
    assert_eq!(binding.get("x"), Some(&Value::Int(42)));

    let execution = semantics
        .fire(&model, &state, *transition, binding)
        .unwrap();
    assert_eq!(execution.state.marking.place(PlaceId(0)).unwrap().len(), 0);
    assert_eq!(execution.state.marking.place(PlaceId(1)).unwrap().len(), 1);
    assert_eq!(
        execution.state.marking.place(PlaceId(1)).unwrap().items()[0].value,
        Value::Int(42)
    );
}

#[test]
fn colored_semantics_reports_no_binding_when_place_is_empty() {
    let semantics = ColoredSemantics;
    let mut model = int_relay_net();
    model.initial_marking.clear();

    let state = semantics.initial_state(&model).unwrap();
    assert!(semantics.enabled(&model, &state).unwrap().is_empty());
}

#[test]
fn colored_semantics_guards_filter_bindings() {
    use unipn::core::expr::{GuardExpr, Term as _Term};

    let semantics = ColoredSemantics;
    let mut model = int_relay_net();
    // Guard: x == 0 — the single token is 42, so nothing is enabled.
    model.transitions[0].guard = Some(GuardExpr::Eq(
        _Term::Var("x".into()),
        _Term::Const(Value::Int(0)),
    ));

    let state = semantics.initial_state(&model).unwrap();
    assert!(semantics.enabled(&model, &state).unwrap().is_empty());
}
