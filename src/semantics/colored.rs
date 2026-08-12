use crate::core::model::NetModel;
use crate::domain::mod_traits::BindingEnv;
use crate::runtime::error::RuntimeError;
use crate::runtime::state::ColoredState;

use super::mod_traits::{Execution, Semantics};

#[derive(Clone, Debug, Default)]
pub struct ColoredSemantics;

impl Semantics for ColoredSemantics {
    type Model = NetModel;
    type State = ColoredState;
    type Binding = BindingEnv;
    type Domain = crate::domain::concrete::ConcreteDomain;

    fn initial_state(&self, _model: &Self::Model) -> Self::State {
        ColoredState::default()
    }

    fn enabled(
        &self,
        _model: &Self::Model,
        _state: &Self::State,
    ) -> Vec<(crate::ids::TransitionId, Self::Binding)> {
        Vec::new()
    }

    fn fire(
        &self,
        _model: &Self::Model,
        state: &Self::State,
        _t: crate::ids::TransitionId,
        _binding: &Self::Binding,
    ) -> Result<Execution<Self::State>, RuntimeError> {
        Ok(Execution {
            state: state.clone(),
            fired: None,
        })
    }
}
