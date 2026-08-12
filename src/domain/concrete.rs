use crate::core::expr::{GuardExpr, Pattern, Term};
use crate::core::value::Value;
use crate::domain::mod_traits::{BindingEnv, Domain, TruthValue};

#[derive(Clone, Copy, Debug, Default)]
pub struct ConcreteDomain;

impl Domain for ConcreteDomain {
    type Value = Value;
    type Env = BindingEnv;

    fn eval_term(&self, env: &Self::Env, term: &Term) -> Self::Value {
        match term {
            Term::Const(v) => v.clone(),
            Term::Var(name) => env.get(name).cloned().unwrap_or(Value::Unit),
            Term::Tuple(items) => Value::Tuple(items.iter().map(|t| self.eval_term(env, t)).collect()),
            Term::Record(fields) => Value::Record(
                fields
                    .iter()
                    .map(|(k, v)| (k.clone(), self.eval_term(env, v)))
                    .collect(),
            ),
            Term::Call(_, _) => Value::Unit,
        }
    }

    fn eval_guard(&self, env: &Self::Env, guard: &GuardExpr) -> TruthValue {
        match guard {
            GuardExpr::True => TruthValue::True,
            GuardExpr::Eq(lhs, rhs) => {
                if self.eval_term(env, lhs) == self.eval_term(env, rhs) {
                    TruthValue::True
                } else {
                    TruthValue::False
                }
            }
            GuardExpr::Pred(_, _) => TruthValue::Unknown,
            GuardExpr::And(a, b) => match (self.eval_guard(env, a), self.eval_guard(env, b)) {
                (TruthValue::False, _) | (_, TruthValue::False) => TruthValue::False,
                (TruthValue::True, TruthValue::True) => TruthValue::True,
                _ => TruthValue::Unknown,
            },
            GuardExpr::Or(a, b) => match (self.eval_guard(env, a), self.eval_guard(env, b)) {
                (TruthValue::True, _) | (_, TruthValue::True) => TruthValue::True,
                (TruthValue::False, TruthValue::False) => TruthValue::False,
                _ => TruthValue::Unknown,
            },
            GuardExpr::Not(inner) => match self.eval_guard(env, inner) {
                TruthValue::True => TruthValue::False,
                TruthValue::False => TruthValue::True,
                TruthValue::Unknown => TruthValue::Unknown,
            },
        }
    }

    fn match_pattern(&self, env: &mut Self::Env, pattern: &Pattern, value: &Self::Value) -> bool {
        match (pattern, value) {
            (Pattern::Wildcard, _) => true,
            (Pattern::Var(name), v) => {
                env.insert(name.clone(), v.clone());
                true
            }
            (Pattern::Const(v1), v2) => v1 == v2,
            (Pattern::Tuple(patterns), Value::Tuple(values)) => {
                patterns.len() == values.len()
                    && patterns
                        .iter()
                        .zip(values)
                        .all(|(p, v)| self.match_pattern(env, p, v))
            }
            (Pattern::Record(patterns), Value::Record(values)) => patterns.iter().all(|(k, p)| {
                values
                    .iter()
                    .find_map(|(vk, v)| if vk == k { Some(v) } else { None })
                    .is_some_and(|v| self.match_pattern(env, p, v))
            }),
            _ => false,
        }
    }
}
