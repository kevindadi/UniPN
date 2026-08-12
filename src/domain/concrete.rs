use crate::core::expr::{GuardExpr, Pattern, Term};
use crate::core::value::Value;
use crate::domain::mod_traits::{BindingEnv, Domain, DomainError, TruthValue};
use crate::ids::Symbol;

#[derive(Clone, Copy, Debug, Default)]
pub struct ConcreteDomain;

impl ConcreteDomain {
    fn eval_record(
        &self,
        env: &BindingEnv,
        fields: &[(Symbol, Term)],
    ) -> Result<Value, DomainError> {
        fields
            .iter()
            .map(|(name, term)| Ok((name.clone(), self.eval_term(env, term)?)))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Record)
    }
}

impl Domain for ConcreteDomain {
    type Value = Value;
    type Env = BindingEnv;

    fn eval_term(&self, env: &Self::Env, term: &Term) -> Result<Self::Value, DomainError> {
        match term {
            Term::Const(value) => Ok(value.clone()),
            Term::Var(name) => env
                .get(name)
                .cloned()
                .ok_or_else(|| DomainError::UnknownVariable(name.clone())),
            Term::Tuple(items) => items
                .iter()
                .map(|item| self.eval_term(env, item))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Tuple),
            Term::Record(fields) => self.eval_record(env, fields),
            Term::Call(function, _) => Err(DomainError::UnknownFunction(*function)),
        }
    }

    fn eval_guard(&self, env: &Self::Env, guard: &GuardExpr) -> Result<TruthValue, DomainError> {
        match guard {
            GuardExpr::True => Ok(TruthValue::True),
            GuardExpr::Eq(lhs, rhs) => {
                Ok((self.eval_term(env, lhs)? == self.eval_term(env, rhs)?).into())
            }
            GuardExpr::Pred(function, _) => Err(DomainError::UnknownFunction(*function)),
            GuardExpr::And(lhs, rhs) => {
                match (self.eval_guard(env, lhs)?, self.eval_guard(env, rhs)?) {
                    (TruthValue::False, _) | (_, TruthValue::False) => Ok(TruthValue::False),
                    (TruthValue::True, TruthValue::True) => Ok(TruthValue::True),
                    _ => Ok(TruthValue::Unknown),
                }
            }
            GuardExpr::Or(lhs, rhs) => {
                match (self.eval_guard(env, lhs)?, self.eval_guard(env, rhs)?) {
                    (TruthValue::True, _) | (_, TruthValue::True) => Ok(TruthValue::True),
                    (TruthValue::False, TruthValue::False) => Ok(TruthValue::False),
                    _ => Ok(TruthValue::Unknown),
                }
            }
            GuardExpr::Not(inner) => Ok(match self.eval_guard(env, inner)? {
                TruthValue::True => TruthValue::False,
                TruthValue::False => TruthValue::True,
                TruthValue::Unknown => TruthValue::Unknown,
            }),
        }
    }

    fn match_pattern(
        &self,
        env: &mut Self::Env,
        pattern: &Pattern,
        value: &Self::Value,
    ) -> Result<bool, DomainError> {
        match (pattern, value) {
            (Pattern::Wildcard, _) => Ok(true),
            (Pattern::Var(name), candidate) => match env.get(name) {
                Some(existing) if existing != candidate => {
                    Err(DomainError::BindingConflict(name.clone()))
                }
                Some(_) => Ok(true),
                None => {
                    env.insert(name.clone(), candidate.clone());
                    Ok(true)
                }
            },
            (Pattern::Const(expected), candidate) => Ok(expected == candidate),
            (Pattern::Tuple(patterns), Value::Tuple(values)) => {
                if patterns.len() != values.len() {
                    return Ok(false);
                }
                for (pattern, value) in patterns.iter().zip(values) {
                    if !self.match_pattern(env, pattern, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            (Pattern::Record(patterns), Value::Record(values)) => {
                for (name, pattern) in patterns {
                    let Some((_, value)) = values.iter().find(|(field, _)| field == name) else {
                        return Ok(false);
                    };
                    if !self.match_pattern(env, pattern, value)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

impl From<bool> for TruthValue {
    fn from(value: bool) -> Self {
        if value {
            TruthValue::True
        } else {
            TruthValue::False
        }
    }
}
