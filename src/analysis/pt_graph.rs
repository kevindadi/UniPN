use std::collections::{HashMap, VecDeque};

use crate::bug::{BugNet, PlaceMetadata, TransitionMetadata};
use crate::ids::{PlaceId, TransitionId};
use crate::pt::{PtArc, PtExecutionError, PtNet};
use crate::runtime::PtMarking;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PtStateId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PtSearchStrategy {
    BreadthFirst,
    DepthFirst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PtStateGraphConfig {
    pub strategy: PtSearchStrategy,
    pub max_states: usize,
}

impl Default for PtStateGraphConfig {
    fn default() -> Self {
        Self {
            strategy: PtSearchStrategy::BreadthFirst,
            max_states: 10_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PtPlaceSnapshot {
    pub id: PlaceId,
    pub name: String,
    pub tokens: u64,
    pub metadata: Option<PlaceMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenChange {
    pub place: PlaceId,
    pub before: u64,
    pub after: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PtEdge {
    pub source: PtStateId,
    pub target: PtStateId,
    pub transition: TransitionId,
    pub transition_metadata: Option<TransitionMetadata>,
    pub changes: Vec<TokenChange>,
    pub arcs: Vec<PtArc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PtState {
    pub id: PtStateId,
    pub marking: PtMarking,
    pub enabled: Vec<TransitionId>,
    pub places: Vec<PtPlaceSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PtTransitionFailure {
    pub source: PtStateId,
    pub transition: TransitionId,
    pub error: PtExecutionError,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PtStateGraph {
    pub states: Vec<PtState>,
    pub edges: Vec<PtEdge>,
    pub blocked: Vec<PtStateId>,
    pub failures: Vec<PtTransitionFailure>,
    pub truncated: bool,
    pub initial: PtStateId,
}

impl PtStateGraph {
    pub fn explore(
        net: &PtNet,
        metadata: Option<(&[PlaceMetadata], &[TransitionMetadata])>,
        config: PtStateGraphConfig,
    ) -> Result<Self, PtExecutionError> {
        if config.max_states == 0 {
            return Err(PtExecutionError::Model(
                crate::pt::PtModelError::InvalidConfiguration,
            ));
        }
        net.validate()?;
        let (place_metadata, transition_metadata) = metadata.unwrap_or((&[], &[]));
        if (!place_metadata.is_empty() && place_metadata.len() != net.places.len())
            || (!transition_metadata.is_empty()
                && transition_metadata.len() != net.transitions.len())
        {
            return Err(PtExecutionError::Model(
                crate::pt::PtModelError::MetadataLengthMismatch,
            ));
        }

        let initial_marking = net.initial_marking();
        let initial = PtStateId(0);
        let mut graph = Self {
            states: vec![snapshot(net, initial, initial_marking, &[], place_metadata)],
            edges: Vec::new(),
            blocked: Vec::new(),
            failures: Vec::new(),
            truncated: false,
            initial,
        };
        let mut pending = VecDeque::from([initial]);
        let mut known = HashMap::from([(graph.states[0].marking.clone(), initial)]);

        while let Some(source) = match config.strategy {
            PtSearchStrategy::BreadthFirst => pending.pop_front(),
            PtSearchStrategy::DepthFirst => pending.pop_back(),
        } {
            let enabled = net.enabled(&graph.states[source.0].marking)?;
            graph.states[source.0].enabled = enabled.clone();
            if enabled.is_empty() {
                graph.blocked.push(source);
                continue;
            }

            for transition in enabled {
                let target_marking = match net.fire(&graph.states[source.0].marking, transition) {
                    Ok(marking) => marking,
                    Err(error) => {
                        graph.failures.push(PtTransitionFailure {
                            source,
                            transition,
                            error,
                        });
                        continue;
                    }
                };
                let target = if let Some(target) = known.get(&target_marking).copied() {
                    target
                } else if graph.states.len() >= config.max_states {
                    graph.truncated = true;
                    continue;
                } else {
                    let target = PtStateId(graph.states.len());
                    known.insert(target_marking.clone(), target);
                    graph.states.push(snapshot(
                        net,
                        target,
                        target_marking.clone(),
                        &[],
                        place_metadata,
                    ));
                    pending.push_back(target);
                    target
                };
                let changes = graph.states[source.0]
                    .marking
                    .iter()
                    .map(|(place, before)| TokenChange {
                        place,
                        before: *before,
                        after: graph.states[target.0].marking.tokens(place),
                    })
                    .filter(|change| change.before != change.after)
                    .collect();
                graph.edges.push(PtEdge {
                    source,
                    target,
                    transition,
                    transition_metadata: transition_metadata.get(transition.index()).cloned(),
                    changes,
                    arcs: net.arcs_for(transition).copied().collect(),
                });
            }
        }

        Ok(graph)
    }

    pub fn explore_bug_net(
        bug_net: &BugNet,
        config: PtStateGraphConfig,
    ) -> Result<Self, PtExecutionError> {
        bug_net.validate()?;
        Self::explore(
            &bug_net.net,
            Some((&bug_net.places, &bug_net.transitions)),
            config,
        )
    }

    pub fn state(&self, id: PtStateId) -> Option<&PtState> {
        self.states.get(id.0)
    }

    pub fn outgoing(&self, source: PtStateId) -> impl Iterator<Item = &PtEdge> {
        self.edges.iter().filter(move |edge| edge.source == source)
    }

    pub fn incoming(&self, target: PtStateId) -> impl Iterator<Item = &PtEdge> {
        self.edges.iter().filter(move |edge| edge.target == target)
    }

    pub fn path_to(&self, target: PtStateId) -> Option<Vec<&PtEdge>> {
        self.state(target)?;
        let mut path = Vec::new();
        let mut current = target;
        while current != self.initial {
            let edge = self.incoming(current).next()?;
            path.push(edge);
            current = edge.source;
        }
        path.reverse();
        Some(path)
    }
}

fn snapshot(
    net: &PtNet,
    id: PtStateId,
    marking: PtMarking,
    enabled: &[TransitionId],
    metadata: &[PlaceMetadata],
) -> PtState {
    PtState {
        id,
        places: net
            .places
            .iter()
            .map(|place| PtPlaceSnapshot {
                id: place.id,
                name: place.name.clone(),
                tokens: marking.tokens(place.id),
                metadata: metadata.get(place.id.index()).cloned(),
            })
            .collect(),
        marking,
        enabled: enabled.to_vec(),
    }
}
