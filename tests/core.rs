use unipn::core::expr::{GuardExpr, Pattern, Term};
use unipn::core::model::{
    ArcDecl, InputArc, ModelError, Multiplicity, NetModel, PlaceDecl, TransitionDecl,
};
use unipn::core::sort::Sort;
use unipn::core::value::{Token, Value};
use unipn::domain::concrete::ConcreteDomain;
use unipn::{Domain, DomainError, PlaceId, RoleTag, TransitionId};

fn model() -> NetModel {
    NetModel {
        places: vec![PlaceDecl {
            id: PlaceId(0),
            name: "ready".into(),
            sort: 0,
            multiplicity: Multiplicity::Many,
            role: Some(RoleTag::Control),
        }],
        transitions: vec![TransitionDecl {
            id: TransitionId(0),
            name: "advance".into(),
            guard: Some(GuardExpr::Eq(
                Term::Var("state".into()),
                Term::Const(Value::Int(1)),
            )),
            action: None,
            priority: None,
            timing: None,
            role: None,
        }],
        arcs: vec![ArcDecl::Input {
            transition: TransitionId(0),
            arc: InputArc {
                place: PlaceId(0),
                pattern: Pattern::Var("state".into()),
                weight: 1,
            },
        }],
        sorts: vec![Sort::Int {
            lo: Some(0),
            hi: Some(2),
        }],
        initial_marking: vec![(PlaceId(0), vec![Token::new(0, Value::Int(1))])],
    }
}

#[test]
fn validates_model_references() {
    assert!(model().validate().is_ok());

    let mut invalid = model();
    invalid.arcs.push(ArcDecl::Input {
        transition: TransitionId(9),
        arc: InputArc {
            place: PlaceId(0),
            pattern: Pattern::Wildcard,
            weight: 1,
        },
    });
    assert_eq!(
        invalid.validate(),
        Err(ModelError::MissingArcTransition(TransitionId(9)))
    );
}

#[test]
fn concrete_domain_matches_and_evaluates() {
    let domain = ConcreteDomain;
    let mut env = unipn::BindingEnv::default();
    let matched = domain
        .match_pattern(
            &mut env,
            &Pattern::Tuple(vec![Pattern::Var("x".into()), Pattern::Wildcard]),
            &Value::Tuple(vec![Value::Int(7), Value::Bool(true)]),
        )
        .unwrap();
    assert!(matched);
    assert_eq!(env.get("x"), Some(&Value::Int(7)));

    let result = domain
        .eval_guard(
            &env,
            &GuardExpr::Eq(Term::Var("x".into()), Term::Const(Value::Int(7))),
        )
        .unwrap();
    assert_eq!(result, unipn::TruthValue::True);
}

#[test]
fn concrete_domain_reports_unsupported_or_invalid_inputs() {
    let domain = ConcreteDomain;
    let env = unipn::BindingEnv::default();

    assert_eq!(
        domain.eval_term(&env, &Term::Var("missing".into())),
        Err(DomainError::UnknownVariable("missing".into()))
    );
    assert_eq!(
        domain.eval_term(&env, &Term::Call(0, vec![])),
        Err(DomainError::UnknownFunction(0))
    );
    assert_eq!(
        domain.eval_guard(&env, &GuardExpr::Pred(1, vec![])),
        Err(DomainError::UnknownFunction(1))
    );

    let mut env = unipn::BindingEnv::default();
    assert!(
        domain
            .match_pattern(&mut env, &Pattern::Var("x".into()), &Value::Int(1))
            .unwrap()
    );
    assert_eq!(
        domain.match_pattern(&mut env, &Pattern::Var("x".into()), &Value::Int(2)),
        Err(DomainError::BindingConflict("x".into()))
    );
}
#[test]
fn colored_marking_supports_typed_tokens() {
    let mut marking = unipn::ColoredMarking::new(1);
    let token = Token::new(0, Value::Int(1));
    assert!(marking.insert(PlaceId(0), token.clone()));
    assert_eq!(marking.len(), 1);
    assert!(marking.remove_one(PlaceId(0), &token));
    assert!(marking.is_empty());
}

#[test]
fn runtime_state_is_generic_over_time() {
    let state = unipn::RuntimeState::new(unipn::PtMarking::new(1), (), 0_u64);
    assert_eq!(state.time, 0);
}

#[test]
fn sort_ids_are_explicit() {
    assert_eq!(model().places[0].sort, 0);
}
