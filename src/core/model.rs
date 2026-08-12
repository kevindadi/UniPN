use serde::{Deserialize, Serialize};

use crate::ids::{PlaceId, TransitionId};

use super::expr::{ActionExpr, GuardExpr, Pattern, Term};
use super::sort::{Sort, SortId, Symbol};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Multiplicity {
    One,
    Many,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleTag {
    Control,
    Resource,
    Terminal,
    Wait,
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
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputArc {
    pub place: PlaceId,
    pub term: Term,
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
}
