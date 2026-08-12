use crate::core::expr::{GuardExpr, Pattern, Term};
use crate::core::sort::Symbol;
use crate::core::value::Value;

pub type BindingEnv = indexmap::IndexMap<Symbol, Value>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TruthValue {
    False,
    Unknown,
    True,
}

pub trait Domain {
    type Value: Clone + Eq + std::hash::Hash;
    type Env: Clone;

    fn eval_term(&self, env: &Self::Env, term: &Term) -> Self::Value;
    fn eval_guard(&self, env: &Self::Env, guard: &GuardExpr) -> TruthValue;
    fn match_pattern(&self, env: &mut Self::Env, pattern: &Pattern, value: &Self::Value) -> bool;
}
