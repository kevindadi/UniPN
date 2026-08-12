use std::collections::VecDeque;
use std::fmt;

use crate::ids::TransitionId;
use crate::runtime::RuntimeError;
use crate::semantics::Semantics;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchStrategy {
    BreadthFirst,
    DepthFirst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExploreConfig {
    pub strategy: SearchStrategy,
    pub max_states: usize,
}

impl Default for ExploreConfig {
    fn default() -> Self {
        Self {
            strategy: SearchStrategy::BreadthFirst,
            max_states: 10_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StateId(pub usize);

#[derive(Clone)]
pub struct Edge<B> {
    pub source: StateId,
    pub target: StateId,
    pub transition: TransitionId,
    pub binding: B,
}

#[derive(Clone)]
pub struct ReachabilityGraph<S, B> {
    pub states: Vec<S>,
    pub edges: Vec<Edge<B>>,
    pub blocked: Vec<StateId>,
    pub truncated: bool,
}

impl<S, B> ReachabilityGraph<S, B> {
    pub fn state(&self, id: StateId) -> Option<&S> {
        self.states.get(id.0)
    }

    pub fn outgoing(&self, source: StateId) -> impl Iterator<Item = &Edge<B>> {
        self.edges.iter().filter(move |edge| edge.source == source)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnalysisError {
    InvalidConfiguration {
        max_states: usize,
    },
    Initial {
        source: RuntimeError,
    },
    Enabled {
        state: StateId,
        source: RuntimeError,
    },
    Fire {
        state: StateId,
        transition: TransitionId,
        source: RuntimeError,
    },
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration { max_states } => {
                write!(f, "max_states must be greater than zero, got {max_states}")
            }
            Self::Initial { source } => write!(f, "failed to create initial state: {source}"),
            Self::Enabled { state, source } => {
                write!(
                    f,
                    "failed to enumerate transitions from state {state:?}: {source}"
                )
            }
            Self::Fire {
                state,
                transition,
                source,
            } => write!(
                f,
                "failed to fire transition {transition} from state {state:?}: {source}"
            ),
        }
    }
}

impl std::error::Error for AnalysisError {}

struct Worklist {
    pending: VecDeque<StateId>,
    strategy: SearchStrategy,
}

impl Worklist {
    fn new(strategy: SearchStrategy, root: StateId) -> Self {
        let mut pending = VecDeque::new();
        pending.push_back(root);
        Self { pending, strategy }
    }

    fn pop(&mut self) -> Option<StateId> {
        match self.strategy {
            SearchStrategy::BreadthFirst => self.pending.pop_front(),
            SearchStrategy::DepthFirst => self.pending.pop_back(),
        }
    }

    fn push(&mut self, state: StateId) {
        self.pending.push_back(state);
    }
}

pub fn explore<Sema>(
    semantics: &Sema,
    model: &Sema::Model,
    config: ExploreConfig,
) -> Result<ReachabilityGraph<Sema::State, Sema::Binding>, AnalysisError>
where
    Sema: Semantics,
{
    if config.max_states == 0 {
        return Err(AnalysisError::InvalidConfiguration {
            max_states: config.max_states,
        });
    }

    let initial = semantics
        .initial_state(model)
        .map_err(|source| AnalysisError::Initial { source })?;
    let mut graph = ReachabilityGraph {
        states: vec![initial],
        edges: Vec::new(),
        blocked: Vec::new(),
        truncated: false,
    };
    let mut worklist = Worklist::new(config.strategy, StateId(0));

    while let Some(source) = worklist.pop() {
        let enabled = semantics
            .enabled(model, &graph.states[source.0])
            .map_err(|error| AnalysisError::Enabled {
                state: source,
                source: error,
            })?;

        if enabled.is_empty() {
            graph.blocked.push(source);
            continue;
        }

        for (transition, binding) in enabled {
            let execution = semantics
                .fire(model, &graph.states[source.0], transition, &binding)
                .map_err(|error| AnalysisError::Fire {
                    state: source,
                    transition,
                    source: error,
                })?;
            let target_state = execution.state;
            let target = graph
                .states
                .iter()
                .position(|state| *state == target_state)
                .map(StateId);

            let target = match target {
                Some(target) => target,
                None if graph.states.len() >= config.max_states => {
                    graph.truncated = true;
                    continue;
                }
                None => {
                    let target = StateId(graph.states.len());
                    graph.states.push(target_state);
                    worklist.push(target);
                    target
                }
            };

            graph.edges.push(Edge {
                source,
                target,
                transition: execution.fired,
                binding,
            });
        }
    }

    Ok(graph)
}

pub fn find_deadlocks<S, B>(
    graph: &ReachabilityGraph<S, B>,
    is_normal_termination: impl Fn(&S) -> bool,
) -> Vec<StateId> {
    graph
        .blocked
        .iter()
        .copied()
        .filter(|state| !is_normal_termination(&graph.states[state.0]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worklist_supports_both_search_orders() {
        let mut bfs = Worklist::new(SearchStrategy::BreadthFirst, StateId(0));
        bfs.push(StateId(1));
        bfs.push(StateId(2));
        assert_eq!(bfs.pop(), Some(StateId(0)));
        assert_eq!(bfs.pop(), Some(StateId(1)));

        let mut dfs = Worklist::new(SearchStrategy::DepthFirst, StateId(0));
        dfs.push(StateId(1));
        dfs.push(StateId(2));
        assert_eq!(dfs.pop(), Some(StateId(2)));
        assert_eq!(dfs.pop(), Some(StateId(1)));
    }
}
