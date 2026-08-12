use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{PlaceId, SortId, Symbol, TransitionId};

use super::expr::{ActionExpr, GuardExpr, Pattern, Term};
use super::sort::Sort;
use super::value::Token;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Multiplicity {
    One,
    Many,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleTag {
    Control,
    Resource,
    Other(Symbol),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceDecl {
    pub id: PlaceId,
    pub name: String,
    pub sort: SortId,
    pub multiplicity: Multiplicity,
    pub role: Option<RoleTag>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingSpec {
    pub earliest: Option<u64>,
    pub latest: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionDecl {
    pub id: TransitionId,
    pub name: String,
    pub guard: Option<GuardExpr>,
    pub action: Option<ActionExpr>,
    pub priority: Option<u32>,
    pub timing: Option<TimingSpec>,
    pub role: Option<RoleTag>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputArc {
    pub place: PlaceId,
    pub pattern: Pattern,
    pub weight: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputArc {
    pub place: PlaceId,
    pub term: Term,
    pub weight: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadArc {
    pub place: PlaceId,
    pub pattern: Pattern,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InhibitorArc {
    pub place: PlaceId,
    pub pattern: Pattern,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResetArc {
    pub place: PlaceId,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArcDecl {
    Input {
        transition: TransitionId,
        arc: InputArc,
    },
    Output {
        transition: TransitionId,
        arc: OutputArc,
    },
    Read {
        transition: TransitionId,
        arc: ReadArc,
    },
    Inhibitor {
        transition: TransitionId,
        arc: InhibitorArc,
    },
    Reset {
        transition: TransitionId,
        arc: ResetArc,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct NetModel {
    pub places: Vec<PlaceDecl>,
    pub transitions: Vec<TransitionDecl>,
    pub arcs: Vec<ArcDecl>,
    pub sorts: Vec<Sort>,
    pub initial_marking: Vec<(PlaceId, Vec<Token>)>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    #[error("place id {0} is not contiguous or does not match its declaration index")]
    InvalidPlaceId(PlaceId),
    #[error("transition id {0} is not contiguous or does not match its declaration index")]
    InvalidTransitionId(TransitionId),
    #[error("place {place} references missing sort {sort}")]
    MissingPlaceSort { place: PlaceId, sort: SortId },
    #[error("arc references missing place {0}")]
    MissingArcPlace(PlaceId),
    #[error("arc references missing transition {0}")]
    MissingArcTransition(TransitionId),
    #[error("initial marking references missing place {0}")]
    MissingInitialPlace(PlaceId),
    #[error("input arc on {transition} has zero weight")]
    ZeroInputWeight {
        transition: TransitionId,
        place: PlaceId,
    },
    #[error("output arc on {transition} has zero weight")]
    ZeroOutputWeight {
        transition: TransitionId,
        place: PlaceId,
    },
}

impl NetModel {
    pub fn place(&self, id: PlaceId) -> Option<&PlaceDecl> {
        self.places.get(id.index())
    }

    pub fn transition(&self, id: TransitionId) -> Option<&TransitionDecl> {
        self.transitions.get(id.index())
    }

    pub fn input_arcs(&self, t: TransitionId) -> Vec<&InputArc> {
        self.arcs
            .iter()
            .filter_map(|arc| match arc {
                ArcDecl::Input { transition, arc } if *transition == t => Some(arc),
                _ => None,
            })
            .collect()
    }

    pub fn output_arcs(&self, t: TransitionId) -> Vec<&OutputArc> {
        self.arcs
            .iter()
            .filter_map(|arc| match arc {
                ArcDecl::Output { transition, arc } if *transition == t => Some(arc),
                _ => None,
            })
            .collect()
    }

    pub fn read_arcs(&self, t: TransitionId) -> Vec<&ReadArc> {
        self.arcs
            .iter()
            .filter_map(|arc| match arc {
                ArcDecl::Read { transition, arc } if *transition == t => Some(arc),
                _ => None,
            })
            .collect()
    }

    pub fn inhibitor_arcs(&self, t: TransitionId) -> Vec<&InhibitorArc> {
        self.arcs
            .iter()
            .filter_map(|arc| match arc {
                ArcDecl::Inhibitor { transition, arc } if *transition == t => Some(arc),
                _ => None,
            })
            .collect()
    }

    pub fn reset_arcs(&self, t: TransitionId) -> Vec<&ResetArc> {
        self.arcs
            .iter()
            .filter_map(|arc| match arc {
                ArcDecl::Reset { transition, arc } if *transition == t => Some(arc),
                _ => None,
            })
            .collect()
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        for (index, place) in self.places.iter().enumerate() {
            if place.id.index() != index {
                return Err(ModelError::InvalidPlaceId(place.id));
            }
            if place.sort >= self.sorts.len() {
                return Err(ModelError::MissingPlaceSort {
                    place: place.id,
                    sort: place.sort,
                });
            }
        }
        for (index, transition) in self.transitions.iter().enumerate() {
            if transition.id.index() != index {
                return Err(ModelError::InvalidTransitionId(transition.id));
            }
        }
        for arc in &self.arcs {
            let (transition, place) = match arc {
                ArcDecl::Input { transition, arc } => {
                    if arc.weight == 0 {
                        return Err(ModelError::ZeroInputWeight {
                            transition: *transition,
                            place: arc.place,
                        });
                    }
                    (*transition, arc.place)
                }
                ArcDecl::Output { transition, arc } => {
                    if arc.weight == 0 {
                        return Err(ModelError::ZeroOutputWeight {
                            transition: *transition,
                            place: arc.place,
                        });
                    }
                    (*transition, arc.place)
                }
                ArcDecl::Read { transition, arc } => (*transition, arc.place),
                ArcDecl::Inhibitor { transition, arc } => (*transition, arc.place),
                ArcDecl::Reset { transition, arc } => (*transition, arc.place),
            };
            if self.transition(transition).is_none() {
                return Err(ModelError::MissingArcTransition(transition));
            }
            if self.place(place).is_none() {
                return Err(ModelError::MissingArcPlace(place));
            }
        }
        for (place, _) in &self.initial_marking {
            if self.place(*place).is_none() {
                return Err(ModelError::MissingInitialPlace(*place));
            }
        }
        Ok(())
    }
}
