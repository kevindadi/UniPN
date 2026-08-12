use unipn::core::model::{ArcDecl, InputArc, Multiplicity, NetModel, OutputArc, PlaceDecl, TransitionDecl};
use unipn::core::expr::{Pattern, Term};
use unipn::core::sort::Sort;
use unipn::domain::concrete::ConcreteDomain;
use unipn::engine::interp::InterpEngine;
use unipn::semantics::colored::ColoredSemantics;
use unipn::semantics::Semantics;
use unipn::{PlaceId, TransitionId};

#[test]
fn new_core_skeleton_builds_basic_model() {
    let model = NetModel {
        places: vec![
            PlaceDecl {
                id: PlaceId(0),
                name: "p0".into(),
                sort: 0,
                multiplicity: Multiplicity::Many,
                role: None,
            },
            PlaceDecl {
                id: PlaceId(1),
                name: "p1".into(),
                sort: 0,
                multiplicity: Multiplicity::Many,
                role: None,
            },
        ],
        transitions: vec![TransitionDecl {
            id: TransitionId(0),
            name: "t0".into(),
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
                    pattern: Pattern::Wildcard,
                },
            },
            ArcDecl::Output {
                transition: TransitionId(0),
                arc: OutputArc {
                    place: PlaceId(1),
                    term: Term::Const(unipn::Value::Unit),
                },
            },
        ],
        sorts: vec![Sort::Unit],
    };

    assert_eq!(model.input_arcs(TransitionId(0)).len(), 1);
    assert_eq!(model.output_arcs(TransitionId(0)).len(), 1);

    let semantics = ColoredSemantics;
    let engine = InterpEngine::new(semantics);
    let state = engine.semantics.initial_state(&model);

    assert!(engine.semantics.enabled(&model, &state).is_empty());
    assert!(state.marking.0.is_empty());

    let _domain = ConcreteDomain;
}
