use crate::core::model::NetModel;
use crate::domain::concrete::ConcreteDomain;
use crate::domain::mod_traits::BindingEnv;
use crate::runtime::error::RuntimeError;
use crate::runtime::state::ColoredState;
use crate::semantics::mod_traits::{Execution, Semantics};

#[derive(Clone, Debug, Default)]
pub struct InterpEngine<S> {
    pub semantics: S,
}

impl<S> InterpEngine<S> {
    pub fn new(semantics: S) -> Self {
        Self { semantics }
    }
}

impl<S> InterpEngine<S>
where
    S: Semantics<Model = NetModel, State = ColoredState, Binding = BindingEnv, Domain = ConcreteDomain>,
{
    pub fn step(
        &self,
        model: &NetModel,
        state: &ColoredState,
        t: crate::ids::TransitionId,
        binding: &BindingEnv,
    ) -> Result<Execution<ColoredState>, RuntimeError> {
        self.semantics.fire(model, state, t, binding)
    }
}
