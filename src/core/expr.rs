use serde::{Deserialize, Serialize};

use crate::ids::{FuncId, Symbol};

use super::value::Value;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pattern {
    Wildcard,
    Var(Symbol),
    Const(Value),
    Tuple(Vec<Pattern>),
    Record(Vec<(Symbol, Pattern)>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Term {
    Const(Value),
    Var(Symbol),
    Tuple(Vec<Term>),
    Record(Vec<(Symbol, Term)>),
    Call(FuncId, Vec<Term>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardExpr {
    True,
    Eq(Term, Term),
    Pred(FuncId, Vec<Term>),
    And(Box<GuardExpr>, Box<GuardExpr>),
    Or(Box<GuardExpr>, Box<GuardExpr>),
    Not(Box<GuardExpr>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionExpr {
    Noop,
    Let(Symbol, Term),
    AssignGlobal(Symbol, Term),
    Seq(Vec<ActionExpr>),
}
