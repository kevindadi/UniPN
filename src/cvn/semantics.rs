//! CVN firing semantics.
//!
//! On top of structural enabling the CVN adds two predicates of its own:
//! input-arc guards must not evaluate to false, and an output-arc update must
//! not leave its variable's bounded Int domain (which is what keeps the state
//! space finite). Firing applies the updates and, unlike the other two nets,
//! *rejects* a firing that would push a resource place past its capacity.

use crate::analysis::Semantics;
use crate::net::{ArcDir, PlaceCapacity, PlaceId, TransitionId};

use super::expr::{ConcreteVal, Val, eval_expr, eval_guard};
use super::kinds::{ControlSub, CvnArcKind, CvnNet, CvnState, PlaceKind, ResourceType};

/// Resource places carry the capacity (Mutex=1, RwLock=max_readers,
/// Semaphore=count); control places and channels are unbounded.
impl PlaceCapacity for PlaceKind {
    fn capacity(&self) -> Option<usize> {
        match self {
            Self::Resource(ResourceType::Mutex) => Some(1),
            Self::Resource(ResourceType::RwLock { max_readers }) => Some(*max_readers),
            Self::Resource(ResourceType::Semaphore { count }) => Some(*count),
            _ => None,
        }
    }
}

impl CvnNet {
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

    /// Whether no input-arc guard evaluates to false. An `Unknown` guard counts
    /// as satisfied (over-approximation).
    fn guards_hold(&self, state: &CvnState, transition: TransitionId) -> bool {
        self.arcs_of(transition, ArcDir::Input)
            .all(|arc| match &arc.kind {
                CvnArcKind::Guard(guard) => eval_guard(guard, &state.extra.vars).is_not_false(),
                CvnArcKind::Plain | CvnArcKind::Update(_) | CvnArcKind::DropVars(_) => true,
            })
    }

    /// Whether every output-arc update lands inside its variable's declared
    /// domain. An update leaving the domain disables the transition, which is
    /// what bounds the state space.
    fn updates_stay_in_domain(&self, state: &CvnState, transition: TransitionId) -> bool {
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

    /// Apply the output-arc updates, evaluating every expression against the
    /// pre-firing variable store.
    fn apply_updates(&self, next: &mut CvnState, before: &CvnState, transition: TransitionId) {
        for arc in self.arcs_of(transition, ArcDir::Output) {
            if let CvnArcKind::Update(update) = &arc.kind {
                for (var, expr) in update {
                    next.extra
                        .vars
                        .insert(var.clone(), eval_expr(expr, &before.extra.vars));
                }
            }
        }
    }

    /// Drop the variables whose scope ends with this transition. Runs after the
    /// updates, so a variable can be read on the way out and still be dropped.
    fn drop_scoped_vars(&self, next: &mut CvnState, transition: TransitionId) {
        for arc in self.arcs_of(transition, ArcDir::Output) {
            if let CvnArcKind::DropVars(vars) = &arc.kind {
                for var in vars {
                    next.extra.vars.remove(var);
                }
            }
        }
    }
}

impl Semantics for CvnNet {
    type State = CvnState;

    fn can_fire(&self, state: &Self::State, transition: TransitionId) -> bool {
        self.structurally_enabled(&state.marking, transition)
            && self.guards_hold(state, transition)
            && self.updates_stay_in_domain(state, transition)
    }

    fn fire_enabled(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        let mut next = state.clone();
        self.consume_inputs(&mut next.marking, transition);
        self.produce_outputs_bounded(&mut next.marking, transition)?;
        self.apply_updates(&mut next, state, transition);
        self.drop_scoped_vars(&mut next, transition);
        self.apply_resets(&mut next.marking, transition);
        Some(next)
    }
}
