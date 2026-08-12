use crate::runtime::RuntimeError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Execution<S> {
    pub state: S,
    pub fired: crate::ids::TransitionId,
}

pub trait Semantics {
    type Model;
    type State: Clone + Eq;
    type Binding: Clone;
    type Domain;

    fn initial_state(&self, model: &Self::Model) -> Result<Self::State, RuntimeError>;
    fn enabled(
        &self,
        model: &Self::Model,
        state: &Self::State,
    ) -> Result<Vec<(crate::ids::TransitionId, Self::Binding)>, RuntimeError>;
    fn fire(
        &self,
        model: &Self::Model,
        state: &Self::State,
        transition: crate::ids::TransitionId,
        binding: &Self::Binding,
    ) -> Result<Execution<Self::State>, RuntimeError>;
}

pub trait TimedSemantics: Semantics {
    type Time;

    fn time_successors(
        &self,
        model: &Self::Model,
        state: &Self::State,
    ) -> Result<Vec<Self::State>, RuntimeError>;
}

pub trait PrioritySemantics: Semantics {
    fn maximal_enabled(
        &self,
        model: &Self::Model,
        state: &Self::State,
    ) -> Result<Vec<(crate::ids::TransitionId, Self::Binding)>, RuntimeError>;
}

pub trait PartialOrderSemantics: Semantics {
    fn independent(
        &self,
        model: &Self::Model,
        left: crate::ids::TransitionId,
        right: crate::ids::TransitionId,
    ) -> Result<bool, RuntimeError>;
}
