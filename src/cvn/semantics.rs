//! CVN firing semantics.
//!
//! Enabling is gated by input-arc guards and by the bounded-Int domains of the
//! variable store; firing shuffles tokens, applies output-arc updates, and
//! rejects a firing that would exceed a resource place's capacity.

use crate::analysis::NetLike;
use crate::net::{ArcDir, PlaceId, TransitionId};

use super::expr::{ConcreteVal, Val, eval_expr, eval_guard};
use super::kinds::{ControlSub, CvnArcKind, CvnNet, CvnState, PlaceKind, ResourceType};

impl CvnNet {
    /// Resource capacity derived from a place kind (Mutex=1, RwLock=max_readers,
    /// Semaphore=count; control places and channels are unbounded).
    pub fn capacity_of(&self, place: PlaceId) -> Option<usize> {
        match &self.place(place)?.kind {
            PlaceKind::Resource(ResourceType::Mutex) => Some(1),
            PlaceKind::Resource(ResourceType::RwLock { max_readers }) => Some(*max_readers),
            PlaceKind::Resource(ResourceType::Semaphore { count }) => Some(*count),
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

    /// Whether `place` is a resource place.
    pub fn is_resource(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Resource(_))
        )
    }

    /// Whether `place` is a thread terminal (used by deadlock classification).
    pub fn is_thread_terminal(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Control(
                ControlSub::ThreadEnd | ControlSub::FunctionEnd
            ))
        )
    }

    /// Whether `place` is a condvar wait point (signal-loss classification).
    pub fn is_wait_point(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Control(ControlSub::WaitPoint))
        )
    }

    pub fn place_label(&self, place: PlaceId) -> String {
        self.place(place)
            .map_or_else(|| format!("p{}", place.index()), |p| p.name.clone())
    }

    pub fn transition_label(&self, transition: TransitionId) -> String {
        self.transition(transition)
            .map_or_else(|| format!("t{}", transition.index()), |t| t.name.clone())
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
                        && !eval_guard(guard, &state.extra.vars).is_not_false()
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
        if required
            .iter()
            .any(|(place, count)| state.marking.tokens(*place) < *count)
        {
            return false;
        }

        // Bounded Int domains: an update leaving the domain disables the
        // transition (decidability).
        for arc in self.arcs_of(transition, ArcDir::Output) {
            if let CvnArcKind::Update(update) = &arc.kind {
                for (var, expr) in update {
                    if let Some((lo, hi)) = state.extra.domains.get(var)
                        && let Val::Concrete(ConcreteVal::Int(v)) =
                            eval_expr(expr, &state.extra.vars)
                        && (v < *lo || v > *hi)
                    {
                        return false;
                    }
                }
            }
        }
        true
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
            next.marking
                .set(arc.place, current.checked_sub(arc.weight)?);
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
                    next.extra
                        .vars
                        .insert(var.clone(), eval_expr(expr, &state.extra.vars));
                }
            }
        }

        for arc in self.arcs_of(transition, ArcDir::Reset) {
            next.marking.set(arc.place, 0);
        }

        Some(next)
    }
}
