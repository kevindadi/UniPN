use crate::core::model::{ArcDecl, NetModel};
use crate::core::sort::Sort;
use crate::core::value::{Token, Value};
use crate::ids::{PlaceId, TransitionId};
use crate::runtime::{PtState, RuntimeError};

use super::{Execution, Semantics};

#[derive(Clone, Copy, Debug, Default)]
pub struct PtSemantics;

impl PtSemantics {
    fn validate_model(&self, model: &NetModel) -> Result<(), RuntimeError> {
        model.validate()?;

        if model
            .places
            .iter()
            .any(|place| !matches!(model.sorts.get(place.sort), Some(Sort::Unit)))
        {
            return Err(RuntimeError::Unsupported);
        }

        if model.transitions.iter().any(|transition| {
            transition.guard.is_some()
                || transition.action.is_some()
                || transition.priority.is_some()
                || transition.timing.is_some()
        }) {
            return Err(RuntimeError::Unsupported);
        }

        if model.arcs.iter().any(|arc| {
            matches!(
                arc,
                ArcDecl::Read { .. } | ArcDecl::Inhibitor { .. } | ArcDecl::Reset { .. }
            )
        }) {
            return Err(RuntimeError::Unsupported);
        }

        Ok(())
    }

    fn validate_state(&self, model: &NetModel, state: &PtState) -> Result<(), RuntimeError> {
        let actual = state.marking.0.len();
        if actual != model.places.len() {
            return Err(RuntimeError::InvalidMarkingLength {
                expected: model.places.len(),
                actual,
            });
        }
        Ok(())
    }

    fn checked_input_totals(
        &self,
        model: &NetModel,
        transition: TransitionId,
    ) -> Result<Vec<(PlaceId, u32)>, RuntimeError> {
        let mut totals = Vec::new();
        for arc in model.input_arcs(transition) {
            let Some((_, total)) = totals.iter_mut().find(|(place, _)| *place == arc.place) else {
                totals.push((arc.place, arc.weight));
                continue;
            };
            *total = total
                .checked_add(arc.weight)
                .ok_or(RuntimeError::ArithmeticOverflow { place: arc.place })?;
        }
        Ok(totals)
    }

    fn checked_output_totals(
        &self,
        model: &NetModel,
        transition: TransitionId,
    ) -> Result<Vec<(PlaceId, u32)>, RuntimeError> {
        let mut totals = Vec::new();
        for arc in model.output_arcs(transition) {
            let Some((_, total)) = totals.iter_mut().find(|(place, _)| *place == arc.place) else {
                totals.push((arc.place, arc.weight));
                continue;
            };
            *total = total
                .checked_add(arc.weight)
                .ok_or(RuntimeError::ArithmeticOverflow { place: arc.place })?;
        }
        Ok(totals)
    }

    fn initial_token_count(
        &self,
        model: &NetModel,
        place: PlaceId,
        tokens: &[Token],
    ) -> Result<u32, RuntimeError> {
        let place_sort = model.place(place).expect("validated place").sort;
        let mut count = 0_u32;
        for token in tokens {
            if token.sort != place_sort || !matches!(token.value, Value::Unit) {
                return Err(RuntimeError::TypeMismatch);
            }
            count = count
                .checked_add(1)
                .ok_or(RuntimeError::ArithmeticOverflow { place })?;
        }
        Ok(count)
    }
}

impl Semantics for PtSemantics {
    type Model = NetModel;
    type State = PtState;
    type Binding = ();
    type Domain = ();

    fn initial_state(&self, model: &Self::Model) -> Result<Self::State, RuntimeError> {
        self.validate_model(model)?;
        let mut marking = crate::runtime::PtMarking::new(model.places.len());

        for (place, tokens) in &model.initial_marking {
            let count = self.initial_token_count(model, *place, tokens)?;
            let current = marking.tokens(*place);
            let total = current
                .checked_add(count)
                .ok_or(RuntimeError::ArithmeticOverflow { place: *place })?;
            marking.set(*place, total);
        }

        Ok(PtState::new(marking, (), ()))
    }

    fn enabled(
        &self,
        model: &Self::Model,
        state: &Self::State,
    ) -> Result<Vec<(TransitionId, Self::Binding)>, RuntimeError> {
        self.validate_model(model)?;
        self.validate_state(model, state)?;

        let mut enabled = Vec::new();
        for transition in &model.transitions {
            let inputs = self.checked_input_totals(model, transition.id)?;
            if inputs
                .iter()
                .all(|(place, required)| state.marking.tokens(*place) >= *required)
            {
                enabled.push((transition.id, ()));
            }
        }
        Ok(enabled)
    }

    fn fire(
        &self,
        model: &Self::Model,
        state: &Self::State,
        transition: TransitionId,
        _binding: &Self::Binding,
    ) -> Result<Execution<Self::State>, RuntimeError> {
        self.validate_model(model)?;
        self.validate_state(model, state)?;
        if model.transition(transition).is_none() {
            return Err(RuntimeError::UnknownTransition(transition));
        }

        let inputs = self.checked_input_totals(model, transition)?;
        if inputs
            .iter()
            .any(|(place, required)| state.marking.tokens(*place) < *required)
        {
            return Err(RuntimeError::NotEnabled);
        }
        let outputs = self.checked_output_totals(model, transition)?;

        let mut marking = state.marking.clone();
        for (place, consumed) in inputs {
            let current = marking.tokens(place);
            let next = current
                .checked_sub(consumed)
                .ok_or(RuntimeError::ArithmeticUnderflow { place })?;
            marking.set(place, next);
        }
        for (place, produced) in outputs {
            let current = marking.tokens(place);
            let next = current
                .checked_add(produced)
                .ok_or(RuntimeError::ArithmeticOverflow { place })?;
            marking.set(place, next);
        }

        Ok(Execution {
            state: PtState::new(marking, (), ()),
            fired: transition,
        })
    }
}
