use unipn::analysis::generic::{
    AnalysisError, ExploreConfig, SearchStrategy, StateId, explore, find_deadlocks,
};
use unipn::{Execution, RuntimeError, Semantics, TransitionId};

#[derive(Clone)]
struct Binding(u8);

struct BranchingSemantics;

impl Semantics for BranchingSemantics {
    type Model = ();
    type State = u8;
    type Binding = Binding;
    type Domain = ();

    fn initial_state(&self, _model: &Self::Model) -> Result<Self::State, RuntimeError> {
        Ok(0)
    }

    fn enabled(
        &self,
        _model: &Self::Model,
        state: &Self::State,
    ) -> Result<Vec<(TransitionId, Self::Binding)>, RuntimeError> {
        Ok(match state {
            0 => vec![(TransitionId(0), Binding(1)), (TransitionId(1), Binding(2))],
            1 | 2 => vec![(TransitionId(2), Binding(3))],
            _ => Vec::new(),
        })
    }

    fn fire(
        &self,
        _model: &Self::Model,
        _state: &Self::State,
        transition: TransitionId,
        binding: &Self::Binding,
    ) -> Result<Execution<Self::State>, RuntimeError> {
        if transition == TransitionId(2) {
            assert_eq!(binding.0, 3);
        }
        Ok(Execution {
            state: binding.0,
            fired: transition,
        })
    }
}

struct FailingSemantics {
    fail_in_enabled: bool,
}

impl Semantics for FailingSemantics {
    type Model = ();
    type State = u8;
    type Binding = ();
    type Domain = ();

    fn initial_state(&self, _model: &Self::Model) -> Result<Self::State, RuntimeError> {
        Ok(0)
    }

    fn enabled(
        &self,
        _model: &Self::Model,
        _state: &Self::State,
    ) -> Result<Vec<(TransitionId, Self::Binding)>, RuntimeError> {
        if self.fail_in_enabled {
            Err(RuntimeError::Unsupported)
        } else {
            Ok(vec![(TransitionId(0), ())])
        }
    }

    fn fire(
        &self,
        _model: &Self::Model,
        _state: &Self::State,
        _transition: TransitionId,
        _binding: &Self::Binding,
    ) -> Result<Execution<Self::State>, RuntimeError> {
        Err(RuntimeError::NotEnabled)
    }
}

fn config(strategy: SearchStrategy, max_states: usize) -> ExploreConfig {
    ExploreConfig {
        strategy,
        max_states,
    }
}

#[test]
fn bfs_and_dfs_deduplicate_states_and_preserve_edges() {
    for strategy in [SearchStrategy::BreadthFirst, SearchStrategy::DepthFirst] {
        let graph = explore(&BranchingSemantics, &(), config(strategy, 10)).unwrap();
        assert_eq!(graph.states, vec![0, 1, 2, 3]);
        assert_eq!(graph.edges.len(), 4);
        assert_eq!(graph.blocked, vec![StateId(3)]);
        assert!(!graph.truncated);
        assert_eq!(graph.state(StateId(2)), Some(&2));
        assert_eq!(graph.outgoing(StateId(0)).count(), 2);
    }
}

#[test]
fn deadlock_filter_excludes_normal_termination() {
    let graph = explore(
        &BranchingSemantics,
        &(),
        config(SearchStrategy::BreadthFirst, 10),
    )
    .unwrap();
    assert!(find_deadlocks(&graph, |state| *state == 3).is_empty());
    assert_eq!(
        find_deadlocks(&graph, |state| *state == 99),
        vec![StateId(3)]
    );
}

#[test]
fn state_limit_counts_root_and_marks_graph_truncated() {
    let graph = explore(
        &BranchingSemantics,
        &(),
        config(SearchStrategy::BreadthFirst, 2),
    )
    .unwrap();
    assert_eq!(graph.states, vec![0, 1]);
    assert!(graph.truncated);
}

#[test]
fn zero_state_limit_is_rejected() {
    let result = explore(
        &BranchingSemantics,
        &(),
        config(SearchStrategy::BreadthFirst, 0),
    );
    assert!(matches!(
        result,
        Err(AnalysisError::InvalidConfiguration { max_states: 0 })
    ));
}

#[test]
fn semantic_errors_are_not_silently_skipped() {
    let enabled_result = explore(
        &FailingSemantics {
            fail_in_enabled: true,
        },
        &(),
        config(SearchStrategy::BreadthFirst, 10),
    );
    assert!(matches!(
        enabled_result,
        Err(AnalysisError::Enabled {
            state: StateId(0),
            source: RuntimeError::Unsupported,
        })
    ));

    let fire_result = explore(
        &FailingSemantics {
            fail_in_enabled: false,
        },
        &(),
        config(SearchStrategy::BreadthFirst, 10),
    );
    assert!(matches!(
        fire_result,
        Err(AnalysisError::Fire {
            state: StateId(0),
            transition: TransitionId(0),
            source: RuntimeError::NotEnabled,
        })
    ));
}

#[test]
fn graph_does_not_require_hashable_or_eq_bindings() {
    let graph = explore(
        &BranchingSemantics,
        &(),
        config(SearchStrategy::BreadthFirst, 10),
    )
    .unwrap();
    assert_eq!(graph.edges[0].binding.0, 1);
}

#[test]
fn pt_semantics_can_feed_the_generic_explorer() {
    let model = unipn::NetModel {
        places: vec![],
        transitions: vec![],
        arcs: vec![],
        sorts: vec![],
        initial_marking: vec![],
    };
    let graph = explore(&unipn::PtSemantics, &model, ExploreConfig::default()).unwrap();
    assert_eq!(graph.states.len(), 1);
    assert_eq!(graph.blocked, vec![StateId(0)]);
}

#[test]
fn analysis_error_display_contains_context() {
    let error = AnalysisError::Fire {
        state: StateId(4),
        transition: TransitionId(2),
        source: RuntimeError::NotEnabled,
    };
    let rendered = error.to_string();
    assert!(rendered.contains("t2"));
    assert!(rendered.contains("state"));
}

#[test]
fn strategy_is_part_of_configuration_but_not_state_identity() {
    let bfs = explore(
        &BranchingSemantics,
        &(),
        config(SearchStrategy::BreadthFirst, 10),
    )
    .unwrap();
    let dfs = explore(
        &BranchingSemantics,
        &(),
        config(SearchStrategy::DepthFirst, 10),
    )
    .unwrap();
    assert_eq!(bfs.states.len(), dfs.states.len());
    assert_eq!(bfs.edges.len(), dfs.edges.len());
}

#[test]
fn blocked_states_are_only_states_with_no_enabled_transitions() {
    let graph = explore(
        &BranchingSemantics,
        &(),
        config(SearchStrategy::BreadthFirst, 10),
    )
    .unwrap();
    assert_eq!(graph.blocked, vec![StateId(3)]);
    assert!(graph.outgoing(StateId(3)).next().is_none());
}

#[test]
fn self_loops_are_recorded_without_reexploring_the_state() {
    struct Loop;

    impl Semantics for Loop {
        type Model = ();
        type State = u8;
        type Binding = ();
        type Domain = ();

        fn initial_state(&self, _model: &Self::Model) -> Result<Self::State, RuntimeError> {
            Ok(0)
        }

        fn enabled(
            &self,
            _model: &Self::Model,
            _state: &Self::State,
        ) -> Result<Vec<(TransitionId, Self::Binding)>, RuntimeError> {
            Ok(vec![(TransitionId(0), ())])
        }

        fn fire(
            &self,
            _model: &Self::Model,
            _state: &Self::State,
            transition: TransitionId,
            _binding: &Self::Binding,
        ) -> Result<Execution<Self::State>, RuntimeError> {
            Ok(Execution {
                state: 0,
                fired: transition,
            })
        }
    }

    let graph = explore(&Loop, &(), config(SearchStrategy::BreadthFirst, 1)).unwrap();
    assert_eq!(graph.states, vec![0]);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.edges[0].target, StateId(0));
    assert!(!graph.truncated);
}
