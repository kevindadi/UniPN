use crate::runtime::error::RuntimeError;

pub struct Execution<S> {
    pub state: S,
    pub fired: Option<crate::ids::TransitionId>,
}

pub trait Semantics {
    type Model;
    type State: Clone + Eq;
    type Binding: Clone;
    type Domain;

    fn initial_state(&self, model: &Self::Model) -> Self::State;
    fn enabled(&self, model: &Self::Model, state: &Self::State) -> Vec<(crate::ids::TransitionId, Self::Binding)>;
    fn fire(
        &self,
        model: &Self::Model,
        state: &Self::State,
        t: crate::ids::TransitionId,
        binding: &Self::Binding,
    ) -> Result<Execution<Self::State>, RuntimeError>;
}
