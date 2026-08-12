//! The CVN (Concurrency Verification Net) backend — ConcPlanVerify's lowering
//! target. Guards live on input arcs, variable updates on output arcs, and the
//! variable store is the net's `State` extra payload.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::analysis::NetLike;
use crate::expr::{BoolExpr, Val, VarUpdate, eval_expr, eval_guard};
use crate::ids::{PlaceId, TransitionId};
use crate::model::{ControlSub, PlaceKind, ResourceType, TransitionKind};
use crate::net::{ArcDir, Marking, Net, State};

/// The per-arc payload: guards (input arcs) and updates (output arcs).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CvnArcKind {
    Plain,
    Guard(BoolExpr),
    Update(VarUpdate),
}

/// Ordered variable store.
pub type VarStore = BTreeMap<String, Val>;

/// The CVN net.
pub type CvnNet = Net<PlaceKind, TransitionKind, CvnArcKind>;

/// The CVN state: marking + variable store.
pub type CvnState = State<VarStore>;

impl CvnNet {
    /// Resource capacity derived from a place kind (Mutex=1, RwLock=max_readers,
    /// Semaphore=count; control places and channels are unbounded).
    pub fn capacity_of(&self, place: PlaceId) -> Option<usize> {
        match &self.place(place)?.kind {
            PlaceKind::Resource(ResourceType::Mutex) => Some(1),
            PlaceKind::Resource(ResourceType::RwLock { max_readers }) => Some(*max_readers as usize),
            PlaceKind::Resource(ResourceType::Semaphore { count }) => Some(*count as usize),
            _ => None,
        }
    }

    /// Whether `place` is a control-flow place.
    pub fn is_control_flow(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Control(_))
        )
    }

    /// Whether `place` is a thread terminal (used by deadlock classification).
    pub fn is_thread_terminal(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Control(ControlSub::ThreadEnd | ControlSub::FunctionEnd))
        )
    }

    fn is_enabled(&self, state: &CvnState, transition: TransitionId) -> bool {
        let mut required: Vec<(PlaceId, usize)> = Vec::new();
        for arc in self.arcs_for(transition) {
            match arc.direction {
                ArcDir::Input => {
                    if let Some((_, total)) = required.iter_mut().find(|(p, _)| *p == arc.place) {
                        *total = total.checked_add(arc.weight).unwrap_or(usize::MAX);
                    } else {
                        required.push((arc.place, arc.weight));
                    }
                    if let CvnArcKind::Guard(guard) = &arc.kind
                        && !eval_guard(guard, &state.extra).is_not_false()
                    {
                        return false;
                    }
                }
                ArcDir::Read => {
                    if state.marking.tokens(arc.place) < arc.weight {
                        return false;
                    }
                }
                ArcDir::Inhibitor => {
                    if state.marking.tokens(arc.place) >= arc.weight {
                        return false;
                    }
                }
                ArcDir::Output | ArcDir::Reset => {}
            }
        }
        required
            .into_iter()
            .all(|(place, count)| state.marking.tokens(place) >= count)
    }
}

impl NetLike for CvnNet {
    type State = CvnState;

    fn num_places(&self) -> usize {
        self.places.len()
    }

    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    fn enabled(&self, state: &Self::State) -> Vec<TransitionId> {
        self.transitions
            .iter()
            .filter(|t| self.is_enabled(state, t.id))
            .map(|t| t.id)
            .collect()
    }

    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        if !self.is_enabled(state, transition) {
            return None;
        }

        let mut next = state.clone();

        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = next.marking.tokens(arc.place);
            next.marking.set(arc.place, current.checked_sub(arc.weight)?);
        }

        for arc in self.arcs_of(transition, ArcDir::Output) {
            let current = next.marking.tokens(arc.place);
            let produced = current.checked_add(arc.weight)?;
            if let Some(cap) = self.capacity_of(arc.place)
                && produced > cap
            {
                return None;
            }
            next.marking.set(arc.place, produced);

            if let CvnArcKind::Update(update) = &arc.kind {
                for (var, expr) in update {
                    next.extra.insert(var.clone(), eval_expr(expr, &state.extra));
                }
            }
        }

        for arc in self.arcs_of(transition, ArcDir::Reset) {
            next.marking.set(arc.place, 0);
        }

        Some(next)
    }
}

/// Chain-style CVN builder: produces the net plus its initial state.
pub struct CvnBuilder {
    net: CvnNet,
    marking: Vec<usize>,
    vars: VarStore,
}

impl Default for CvnBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CvnBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_place(&mut self, name: impl Into<String>, kind: PlaceKind) -> PlaceId {
        let id = self.net.add_place(name, kind);
        self.marking.push(0);
        id
    }

    pub fn add_transition(
        &mut self,
        name: impl Into<String>,
        kind: TransitionKind,
    ) -> TransitionId {
        self.net.add_transition(name, kind)
    }

    pub fn add_input_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        weight: usize,
        guard: BoolExpr,
    ) -> &mut Self {
        let kind = if guard == BoolExpr::True {
            CvnArcKind::Plain
        } else {
            CvnArcKind::Guard(guard)
        };
        self.net.add_arc(place, transition, ArcDir::Input, weight, kind);
        self
    }

    pub fn add_output_arc(
        &mut self,
        transition: TransitionId,
        place: PlaceId,
        weight: usize,
        update: Option<VarUpdate>,
    ) -> &mut Self {
        let kind = update.map_or(CvnArcKind::Plain, CvnArcKind::Update);
        self.net.add_arc(place, transition, ArcDir::Output, weight, kind);
        self
    }

    pub fn set_initial_tokens(&mut self, place: PlaceId, count: usize) -> &mut Self {
        if let Some(slot) = self.marking.get_mut(place.index()) {
            *slot = count;
        }
        self
    }

    pub fn add_variable(&mut self, name: impl Into<String>, initial: Val) -> &mut Self {
        self.vars.insert(name.into(), initial);
        self
    }

    pub fn build(self) -> (CvnNet, CvnState) {
        (self.net, State::new(Marking::new(self.marking), self.vars))
    }
}
