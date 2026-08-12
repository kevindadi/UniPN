use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{PlaceId, TransitionId};
use crate::runtime::{PtMarking, RuntimeError};

pub type Weight = u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapacityMode {
    #[default]
    Reject,
    Saturate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaceRole {
    Control,
    Resource,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtPlace {
    pub id: PlaceId,
    pub name: String,
    pub initial: Weight,
    pub capacity: Option<Weight>,
    pub capacity_mode: CapacityMode,
    pub role: Option<PlaceRole>,
    pub span: Option<String>,
}

impl PtPlace {
    pub fn new(name: impl Into<String>, initial: Weight) -> Self {
        Self {
            id: PlaceId(0),
            name: name.into(),
            initial,
            capacity: None,
            capacity_mode: CapacityMode::Reject,
            role: None,
            span: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtTransition {
    pub id: TransitionId,
    pub name: String,
    pub priority: Option<u32>,
    pub span: Option<String>,
}

impl PtTransition {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: TransitionId(0),
            name: name.into(),
            priority: None,
            span: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PtArcKind {
    Input,
    Output,
    Read,
    Inhibitor,
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtArc {
    pub place: PlaceId,
    pub transition: TransitionId,
    pub kind: PtArcKind,
    pub weight: Weight,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PtModelError {
    #[error("place id {0} is not contiguous")]
    InvalidPlaceId(PlaceId),
    #[error("transition id {0} is not contiguous")]
    InvalidTransitionId(TransitionId),
    #[error("arc references missing place {0}")]
    MissingPlace(PlaceId),
    #[error("arc references missing transition {0}")]
    MissingTransition(TransitionId),
    #[error("{kind:?} arc on {transition} at {place} has zero weight")]
    ZeroWeight {
        place: PlaceId,
        transition: TransitionId,
        kind: PtArcKind,
    },
    #[error("place {place} has an invalid capacity of zero")]
    ZeroCapacity { place: PlaceId },
    #[error("{kind:?} arcs on {transition} at {place} have overflowing total weight")]
    ArcWeightOverflow {
        place: PlaceId,
        transition: TransitionId,
        kind: PtArcKind,
    },
    #[error("place {place} starts with {tokens} tokens, exceeding capacity {capacity}")]
    InitialCapacityExceeded {
        place: PlaceId,
        tokens: Weight,
        capacity: Weight,
    },
    #[error("metadata does not match the number of places or transitions")]
    MetadataLengthMismatch,
    #[error("invalid exploration configuration: max_states must be greater than zero")]
    InvalidConfiguration,
    #[error("marking has {actual} places, expected {expected}")]
    InvalidMarkingLength { expected: usize, actual: usize },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum PtExecutionError {
    #[error("invalid P/T model: {0}")]
    Model(#[from] PtModelError),
    #[error("unknown transition {0}")]
    UnknownTransition(TransitionId),
    #[error("transition is not enabled")]
    NotEnabled,
    #[error("token count overflow at place {0}")]
    ArithmeticOverflow(PlaceId),
    #[error("token count underflow at place {0}")]
    ArithmeticUnderflow(PlaceId),
    #[error("capacity exceeded at place {place}: {tokens} > {capacity}")]
    CapacityExceeded {
        place: PlaceId,
        tokens: Weight,
        capacity: Weight,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtNet {
    pub places: Vec<PtPlace>,
    pub transitions: Vec<PtTransition>,
    pub arcs: Vec<PtArc>,
}

impl PtNet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_place(&mut self, mut place: PtPlace) -> PlaceId {
        let id = PlaceId(self.places.len());
        place.id = id;
        self.places.push(place);
        id
    }

    pub fn add_transition(&mut self, mut transition: PtTransition) -> TransitionId {
        let id = TransitionId(self.transitions.len());
        transition.id = id;
        self.transitions.push(transition);
        id
    }

    pub fn add_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        kind: PtArcKind,
        weight: Weight,
    ) {
        self.arcs.push(PtArc {
            place,
            transition,
            kind,
            weight,
        });
    }

    pub fn add_input_arc(&mut self, place: PlaceId, transition: TransitionId, weight: Weight) {
        self.add_arc(place, transition, PtArcKind::Input, weight);
    }

    pub fn add_output_arc(&mut self, place: PlaceId, transition: TransitionId, weight: Weight) {
        self.add_arc(place, transition, PtArcKind::Output, weight);
    }

    pub fn add_read_arc(&mut self, place: PlaceId, transition: TransitionId, weight: Weight) {
        self.add_arc(place, transition, PtArcKind::Read, weight);
    }

    pub fn add_inhibitor_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        threshold: Weight,
    ) {
        self.add_arc(place, transition, PtArcKind::Inhibitor, threshold);
    }

    pub fn add_reset_arc(&mut self, place: PlaceId, transition: TransitionId) {
        self.add_arc(place, transition, PtArcKind::Reset, 1);
    }

    pub fn place(&self, id: PlaceId) -> Option<&PtPlace> {
        self.places.get(id.index())
    }

    pub fn transition(&self, id: TransitionId) -> Option<&PtTransition> {
        self.transitions.get(id.index())
    }

    pub fn arcs_for(&self, transition: TransitionId) -> impl Iterator<Item = &PtArc> {
        self.arcs
            .iter()
            .filter(move |arc| arc.transition == transition)
    }

    pub fn initial_marking(&self) -> PtMarking {
        PtMarking::from_tokens(self.places.iter().map(|place| place.initial))
    }

    pub fn validate(&self) -> Result<(), PtModelError> {
        for (index, place) in self.places.iter().enumerate() {
            if place.id.index() != index {
                return Err(PtModelError::InvalidPlaceId(place.id));
            }
            if place.capacity == Some(0) {
                return Err(PtModelError::ZeroCapacity { place: place.id });
            }
            if let Some(capacity) = place.capacity
                && place.initial > capacity
            {
                return Err(PtModelError::InitialCapacityExceeded {
                    place: place.id,
                    tokens: place.initial,
                    capacity,
                });
            }
        }
        for (index, transition) in self.transitions.iter().enumerate() {
            if transition.id.index() != index {
                return Err(PtModelError::InvalidTransitionId(transition.id));
            }
        }
        for arc in &self.arcs {
            if self.place(arc.place).is_none() {
                return Err(PtModelError::MissingPlace(arc.place));
            }
            if self.transition(arc.transition).is_none() {
                return Err(PtModelError::MissingTransition(arc.transition));
            }
            if arc.weight == 0 {
                return Err(PtModelError::ZeroWeight {
                    place: arc.place,
                    transition: arc.transition,
                    kind: arc.kind,
                });
            }
        }
        for transition in &self.transitions {
            for kind in [PtArcKind::Input, PtArcKind::Output] {
                let mut totals = Vec::new();
                for arc in self.arcs_for(transition.id).filter(|arc| arc.kind == kind) {
                    add_total(&mut totals, arc.place, arc.weight).map_err(|_| {
                        PtModelError::ArcWeightOverflow {
                            place: arc.place,
                            transition: transition.id,
                            kind,
                        }
                    })?;
                }
            }
        }
        Ok(())
    }

    pub fn enabled(&self, marking: &PtMarking) -> Result<Vec<TransitionId>, PtExecutionError> {
        self.validate()?;
        self.validate_marking(marking)?;
        Ok(self
            .transitions
            .iter()
            .filter(|transition| self.is_enabled(marking, transition.id))
            .map(|transition| transition.id)
            .collect())
    }

    pub fn fire(
        &self,
        marking: &PtMarking,
        transition: TransitionId,
    ) -> Result<PtMarking, PtExecutionError> {
        self.validate()?;
        self.validate_marking(marking)?;
        if self.transition(transition).is_none() {
            return Err(PtExecutionError::UnknownTransition(transition));
        }
        if !self.is_enabled(marking, transition) {
            return Err(PtExecutionError::NotEnabled);
        }

        let mut next = marking.clone();
        for arc in self.arcs_for(transition) {
            if arc.kind == PtArcKind::Input {
                let current = next.tokens(arc.place);
                next.set(
                    arc.place,
                    current
                        .checked_sub(arc.weight)
                        .ok_or(PtExecutionError::ArithmeticUnderflow(arc.place))?,
                );
            }
        }
        let mut outputs = Vec::new();
        for arc in self.arcs_for(transition) {
            if arc.kind == PtArcKind::Output {
                add_total(&mut outputs, arc.place, arc.weight)
                    .map_err(|_| PtExecutionError::ArithmeticOverflow(arc.place))?;
            }
        }
        for (place, weight) in outputs {
            let current = next.tokens(place);
            let produced = current
                .checked_add(weight)
                .ok_or(PtExecutionError::ArithmeticOverflow(place))?;
            next.set(place, self.apply_capacity(place, produced)?);
        }
        for arc in self.arcs_for(transition) {
            if arc.kind == PtArcKind::Reset {
                next.set(arc.place, 0);
            }
        }
        Ok(next)
    }

    pub fn as_runtime_error(error: PtExecutionError) -> RuntimeError {
        match error {
            PtExecutionError::Model(error) => RuntimeError::InvalidPtModel(error.to_string()),
            PtExecutionError::UnknownTransition(transition) => {
                RuntimeError::UnknownTransition(transition)
            }
            PtExecutionError::NotEnabled => RuntimeError::NotEnabled,
            PtExecutionError::ArithmeticOverflow(place) => {
                RuntimeError::ArithmeticOverflow { place }
            }
            PtExecutionError::ArithmeticUnderflow(place) => {
                RuntimeError::ArithmeticUnderflow { place }
            }
            PtExecutionError::CapacityExceeded { place, .. } => {
                RuntimeError::CapacityExceeded { place }
            }
        }
    }

    fn is_enabled(&self, marking: &PtMarking, transition: TransitionId) -> bool {
        let mut input_totals = Vec::new();
        for arc in self.arcs_for(transition) {
            match arc.kind {
                PtArcKind::Input => {
                    if add_total(&mut input_totals, arc.place, arc.weight).is_err() {
                        return false;
                    }
                }
                PtArcKind::Read => {
                    if marking.tokens(arc.place) < arc.weight {
                        return false;
                    }
                }
                PtArcKind::Inhibitor => {
                    if marking.tokens(arc.place) >= arc.weight {
                        return false;
                    }
                }
                PtArcKind::Output | PtArcKind::Reset => {}
            }
        }
        input_totals
            .into_iter()
            .all(|(place, required)| marking.tokens(place) >= required)
    }

    fn validate_marking(&self, marking: &PtMarking) -> Result<(), PtExecutionError> {
        if marking.len() != self.places.len() {
            return Err(PtExecutionError::Model(PtModelError::InvalidPlaceId(
                PlaceId(marking.len()),
            )));
        }
        Ok(())
    }

    fn apply_capacity(&self, place: PlaceId, tokens: Weight) -> Result<Weight, PtExecutionError> {
        let declaration = self.place(place).expect("validated place");
        let Some(capacity) = declaration.capacity else {
            return Ok(tokens);
        };
        if tokens <= capacity {
            return Ok(tokens);
        }
        match declaration.capacity_mode {
            CapacityMode::Reject => Err(PtExecutionError::CapacityExceeded {
                place,
                tokens,
                capacity,
            }),
            CapacityMode::Saturate => Ok(capacity),
        }
    }
}

fn add_total(
    totals: &mut Vec<(PlaceId, Weight)>,
    place: PlaceId,
    weight: Weight,
) -> Result<(), ()> {
    if let Some((_, total)) = totals.iter_mut().find(|candidate| candidate.0 == place) {
        *total = total.checked_add(weight).ok_or(())?;
    } else {
        totals.push((place, weight));
    }
    Ok(())
}

impl From<PtExecutionError> for RuntimeError {
    fn from(error: PtExecutionError) -> Self {
        PtNet::as_runtime_error(error)
    }
}
