//! Petri net reduction: loop removal, sequence merge, and intermediate-place
//! elimination (port of ConcBugDect's `net/reduce`).

use std::fmt;
use std::sync::Arc;

use crate::net::{Marking, PlaceId, TransitionId};
use crate::pt::{PtNet, TransitionType};

mod graph;
mod intermediate_place;
mod loop_removal;
mod sequence_merge;

use graph::{MaterializedNet, ReductionGraph};

/// Detector-visible transition types that must survive reduction as distinct
/// nodes.
pub(crate) fn preserves_transition_type(t: &TransitionType) -> bool {
    matches!(
        t,
        TransitionType::UnsafeAccess(_)
            | TransitionType::UnsafeRead(..)
            | TransitionType::UnsafeWrite(..)
    )
}

pub type ReductionValidator = dyn Fn(&PtNet, &Marking) -> Result<(), ReductionError> + Send + Sync;

#[derive(Default, Clone)]
pub struct ReductionOptions {
    /// Run invariant checks after each reduction step.
    pub invariant_checker: Option<Arc<ReductionValidator>>,
    /// todo: add property checkers (deadlock-related).
    pub property_checker: Option<Arc<ReductionValidator>>,
}

#[derive(Debug)]
pub struct ReductionResult {
    pub net: PtNet,
    pub marking: Marking,
    pub trace: ReductionTrace,
    pub steps: Vec<ReductionStep>,
    pub stage_nets: ReductionStageNets,
}

#[derive(Debug, Clone)]
pub struct ReductionTrace {
    pub place_mapping: Vec<Vec<PlaceId>>,
    pub transition_mapping: Vec<Vec<TransitionId>>,
}

#[derive(Debug, Clone)]
pub enum ReductionStep {
    LoopRemoved {
        removed_places: Vec<PlaceId>,
        removed_transitions: Vec<TransitionId>,
    },
    SequenceMerged {
        head_places: Vec<PlaceId>,
        tail_places: Vec<PlaceId>,
        merged_transitions: Vec<TransitionId>,
        removed_places: Vec<PlaceId>,
    },
    IntermediatePlaceEliminated {
        places: Vec<PlaceId>,
        merged_transitions: Vec<TransitionId>,
    },
}

#[derive(Debug, Clone)]
pub struct ReductionStageNets {
    pub after_loop: PtNet,
    pub after_sequence: PtNet,
    pub after_intermediate: PtNet,
}

#[derive(Debug)]
pub enum ReductionError {
    ValidationFailed(String),
}

impl fmt::Display for ReductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReductionError::ValidationFailed(msg) => {
                write!(f, "validator rejected reduced net: {msg}")
            }
        }
    }
}

impl std::error::Error for ReductionError {}

pub struct Reducer {
    options: ReductionOptions,
}

impl Reducer {
    pub fn new(options: ReductionOptions) -> Self {
        Self { options }
    }

    pub fn reduce(
        &self,
        net: &PtNet,
        marking: &Marking,
    ) -> Result<ReductionResult, ReductionError> {
        let mut graph = ReductionGraph::from_net(net, marking);
        let mut steps = Vec::new();

        let loop_steps = graph.remove_simple_loops();
        if !loop_steps.is_empty() {
            steps.extend(loop_steps);
            self.validate(&graph)?;
        }
        let after_loop = graph.materialize().net;

        let sequence_steps = graph.merge_linear_sequences();
        if !sequence_steps.is_empty() {
            steps.extend(sequence_steps);
            self.validate(&graph)?;
        }
        let after_sequence = graph.materialize().net;

        let intermediate_steps = graph.eliminate_intermediate_places();
        if !intermediate_steps.is_empty() {
            steps.extend(intermediate_steps);
            self.validate(&graph)?;
        }
        let after_intermediate = graph.materialize().net;

        let MaterializedNet {
            net: reduced_net,
            marking: reduced_marking,
            trace,
        } = graph.materialize();

        Ok(ReductionResult {
            net: reduced_net,
            marking: reduced_marking,
            trace,
            steps,
            stage_nets: ReductionStageNets {
                after_loop,
                after_sequence,
                after_intermediate,
            },
        })
    }

    fn validate(&self, graph: &ReductionGraph) -> Result<(), ReductionError> {
        if self.options.invariant_checker.is_none() && self.options.property_checker.is_none() {
            return Ok(());
        }

        let materialized = graph.materialize();
        if let Some(checker) = &self.options.invariant_checker {
            checker(&materialized.net, &materialized.marking)
                .map_err(|err| ReductionError::ValidationFailed(err.to_string()))?;
        }
        if let Some(checker) = &self.options.property_checker {
            checker(&materialized.net, &materialized.marking)
                .map_err(|err| ReductionError::ValidationFailed(err.to_string()))?;
        }
        Ok(())
    }
}

pub fn reduce(
    net: &PtNet,
    marking: &Marking,
    options: ReductionOptions,
) -> Result<ReductionResult, ReductionError> {
    Reducer::new(options).reduce(net, marking)
}

/// Convenience alias for the ConcBugDect-facing name.
pub fn reduce_in_place(
    net: &PtNet,
    marking: &Marking,
    options: ReductionOptions,
) -> Result<ReductionResult, ReductionError> {
    reduce(net, marking, options)
}
