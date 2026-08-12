use thiserror::Error;

use crate::core::expr::{GuardExpr, Pattern, Term};
use crate::core::value::Value;
use crate::ids::Symbol;

pub type BindingEnv = indexmap::IndexMap<Symbol, Value>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TruthValue {
    False,
    Unknown,
    True,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("unknown variable {0}")]
    UnknownVariable(Symbol),
    #[error("function {0} is not defined in this domain")]
    UnknownFunction(usize),
    #[error("pattern binding conflict for variable {0}")]
    BindingConflict(Symbol),
}

pub trait Domain {
    type Value: Clone + Eq + std::hash::Hash;
    type Env: Clone;

    fn eval_term(&self, env: &Self::Env, term: &Term) -> Result<Self::Value, DomainError>;
    fn eval_guard(&self, env: &Self::Env, guard: &GuardExpr) -> Result<TruthValue, DomainError>;
    fn match_pattern(
        &self,
        env: &mut Self::Env,
        pattern: &Pattern,
        value: &Self::Value,
    ) -> Result<bool, DomainError>;
}
