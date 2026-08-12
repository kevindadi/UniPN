//! Colored-net firing semantics: pattern matching, guards, actions, and typed
//! token production over the declarative [`crate::core::model::NetModel`].

use crate::core::expr::GuardExpr;
use crate::core::model::{InhibitorArc, InputArc, NetModel, ReadArc};
use crate::core::value::Token;
use crate::domain::concrete::ConcreteDomain;
use crate::domain::mod_traits::{BindingEnv, Domain, TruthValue};
use crate::ids::{PlaceId, TransitionId};
use crate::runtime::error::RuntimeError;
use crate::runtime::state::ColoredState;

use super::mod_traits::{Execution, Semantics};

#[derive(Clone, Debug, Default)]
pub struct ColoredSemantics;

impl ColoredSemantics {
    fn validate_model(&self, model: &NetModel) -> Result<(), RuntimeError> {
        model.validate()?;
        Ok(())
    }

    fn domain(&self) -> ConcreteDomain {
        ConcreteDomain
    }

    /// Enumerate every variable binding under which `transition` is enabled.
    fn bindings(
        &self,
        model: &NetModel,
        state: &ColoredState,
        transition: TransitionId,
    ) -> Result<Vec<BindingEnv>, RuntimeError> {
        let inputs = model.input_arcs(transition);
        let reads = model.read_arcs(transition);
        let inhibitors = model.inhibitor_arcs(transition);
        let guard = model.transition(transition).and_then(|t| t.guard.as_ref());

        // Working token pool per place (index = place id).
        let mut available: Vec<Vec<Token>> = (0..model.places.len())
            .map(|p| {
                state
                    .marking
                    .place(PlaceId(p))
                    .map(|ms| ms.items().to_vec())
                    .unwrap_or_default()
            })
            .collect();

        let mut results = Vec::new();
        let mut env = BindingEnv::new();
        self.match_inputs(
            &inputs,
            0,
            &mut available,
            &mut env,
            &mut results,
            &reads,
            &inhibitors,
            guard,
        );
        Ok(results)
    }

    #[allow(clippy::too_many_arguments)]
    fn match_inputs(
        &self,
        inputs: &[&InputArc],
        index: usize,
        available: &mut [Vec<Token>],
        env: &mut BindingEnv,
        results: &mut Vec<BindingEnv>,
        reads: &[&ReadArc],
        inhibitors: &[&InhibitorArc],
        guard: Option<&GuardExpr>,
    ) {
        if index == inputs.len() {
            // All input tokens are chosen; check read / inhibitor / guard.
            if self.accepts(available, env, reads, inhibitors, guard) {
                results.push(env.clone());
            }
            return;
        }

        let arc = inputs[index];
        let weight = arc.weight as usize;
        let pool = available[arc.place.index()].clone();
        if pool.len() < weight {
            return;
        }

        // Enumerate each choice of `weight` distinct tokens from this place.
        for combination in combinations(pool.len(), weight) {
            let mut next_env = env.clone();
            let mut matched = true;
            for &tok_idx in &combination {
                let token_value = pool[tok_idx].value.clone();
                match self
                    .domain()
                    .match_pattern(&mut next_env, &arc.pattern, &token_value)
                {
                    Ok(true) => {}
                    Ok(false) | Err(_) => {
                        matched = false;
                        break;
                    }
                }
            }
            if !matched {
                continue;
            }

            // Consume the chosen tokens.
            let mut remaining = pool.clone();
            for &tok_idx in combination.iter().rev() {
                remaining.remove(tok_idx);
            }
            available[arc.place.index()] = remaining;

            self.match_inputs(
                inputs,
                index + 1,
                available,
                &mut next_env,
                results,
                reads,
                inhibitors,
                guard,
            );

            // Restore the pool for the next combination.
            available[arc.place.index()] = pool.clone();
        }
    }

    fn accepts(
        &self,
        available: &[Vec<Token>],
        env: &mut BindingEnv,
        reads: &[&ReadArc],
        inhibitors: &[&InhibitorArc],
        guard: Option<&GuardExpr>,
    ) -> bool {
        // Read arcs: some remaining token must match the pattern (binding may
        // extend), but the token is not consumed.
        for read in reads {
            let pool = &available[read.place.index()];
            if !pool.iter().any(|token| {
                self.domain()
                    .match_pattern(env, &read.pattern, &token.value)
                    == Ok(true)
            }) {
                return false;
            }
        }

        // Inhibitor arcs: no remaining token may match the pattern.
        for inhibitor in inhibitors {
            let pool = &available[inhibitor.place.index()];
            if pool.iter().any(|token| {
                self.domain()
                    .match_pattern(env, &inhibitor.pattern, &token.value)
                    == Ok(true)
            }) {
                return false;
            }
        }

        if let Some(guard) = guard
            && self.domain().eval_guard(env, guard) == Ok(TruthValue::False)
        {
            return false;
        }

        true
    }
}

impl Semantics for ColoredSemantics {
    type Model = NetModel;
    type State = ColoredState;
    type Binding = BindingEnv;
    type Domain = ConcreteDomain;

    fn initial_state(&self, model: &Self::Model) -> Result<Self::State, RuntimeError> {
        self.validate_model(model)?;
        let mut marking = crate::runtime::ColoredMarking::new(model.places.len());
        for (place, tokens) in &model.initial_marking {
            for token in tokens {
                marking.insert(*place, token.clone());
            }
        }
        Ok(ColoredState::new(marking, BindingEnv::new(), ()))
    }

    fn enabled(
        &self,
        model: &Self::Model,
        state: &Self::State,
    ) -> Result<Vec<(TransitionId, Self::Binding)>, RuntimeError> {
        self.validate_model(model)?;
        let mut out = Vec::new();
        for transition in &model.transitions {
            for binding in self.bindings(model, state, transition.id)? {
                out.push((transition.id, binding));
            }
        }
        Ok(out)
    }

    fn fire(
        &self,
        model: &Self::Model,
        state: &Self::State,
        transition: TransitionId,
        binding: &Self::Binding,
    ) -> Result<Execution<Self::State>, RuntimeError> {
        self.validate_model(model)?;
        if model.transition(transition).is_none() {
            return Err(RuntimeError::UnknownTransition(transition));
        }
        if !self
            .bindings(model, state, transition)?
            .iter()
            .any(|candidate| candidate == binding)
        {
            return Err(RuntimeError::NotEnabled);
        }

        let mut marking = state.marking.clone();
        let mut globals = state.globals.clone();

        // Consume input tokens: remove one matching token per arc weight.
        for arc in model.input_arcs(transition) {
            for _ in 0..arc.weight {
                let Some(multiset) = marking.place(arc.place).cloned() else {
                    return Err(RuntimeError::NotEnabled);
                };
                let position = multiset.items().iter().position(|token| {
                    self.domain()
                        .match_pattern(&mut binding.clone(), &arc.pattern, &token.value)
                        == Ok(true)
                });
                let Some(index) = position else {
                    return Err(RuntimeError::NotEnabled);
                };
                let token = multiset.items()[index].clone();
                marking.remove_one(arc.place, &token);
            }
        }

        // Produce output tokens from the output-arc terms.
        for arc in model.output_arcs(transition) {
            let value = self
                .domain()
                .eval_term(binding, &arc.term)
                .map_err(|_| RuntimeError::TypeMismatch)?;
            let sort = model.place(arc.place).expect("validated place").sort;
            for _ in 0..arc.weight {
                marking.insert(arc.place, Token::new(sort, value.clone()));
            }
        }

        // Reset arcs: empty the place.
        for arc in model.reset_arcs(transition) {
            marking.clear(arc.place);
        }

        // Apply the transition action (global variable updates).
        if let Some(action) = model.transition(transition).and_then(|t| t.action.as_ref()) {
            self.apply_action(&mut globals, binding, action)?;
        }

        Ok(Execution {
            state: ColoredState::new(marking, globals, ()),
            fired: transition,
        })
    }
}

impl ColoredSemantics {
    fn apply_action(
        &self,
        globals: &mut BindingEnv,
        binding: &BindingEnv,
        action: &crate::core::expr::ActionExpr,
    ) -> Result<(), RuntimeError> {
        match action {
            crate::core::expr::ActionExpr::Noop => Ok(()),
            crate::core::expr::ActionExpr::Let(name, term)
            | crate::core::expr::ActionExpr::AssignGlobal(name, term) => {
                let value = self
                    .domain()
                    .eval_term(binding, term)
                    .map_err(|_| RuntimeError::TypeMismatch)?;
                globals.insert(name.clone(), value);
                Ok(())
            }
            crate::core::expr::ActionExpr::Seq(actions) => {
                for action in actions {
                    self.apply_action(globals, binding, action)?;
                }
                Ok(())
            }
        }
    }
}

/// Enumerate all `k`-combinations of `0..n`.
fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    if k == 0 {
        return vec![Vec::new()];
    }
    if n < k {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut combo: Vec<usize> = (0..k).collect();
    loop {
        out.push(combo.clone());
        let mut i = k;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if combo[i] != i + n - k {
                combo[i] += 1;
                for j in (i + 1)..k {
                    combo[j] = combo[j - 1] + 1;
                }
                break;
            }
        }
    }
}
