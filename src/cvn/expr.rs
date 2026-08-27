//! Values, expressions and guards (the optional data model).
//!
//! A pure P/T net does not model data (`State::vars = None`); frontends that do
//! (ConcIR→CVN) use this module for input-arc guards and output-arc updates.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A fully-known concrete value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConcreteVal {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Enum(String),
}

impl Eq for ConcreteVal {}

impl std::hash::Hash for ConcreteVal {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            Self::Bool(b) => b.hash(state),
            Self::Int(i) => i.hash(state),
            Self::Float(f) => f.to_bits().hash(state),
            Self::Str(s) => s.hash(state),
            Self::Enum(s) => s.hash(state),
        }
    }
}

impl fmt::Display for ConcreteVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Str(s) => write!(f, "\"{s}\""),
            Self::Enum(s) => write!(f, "{s}"),
        }
    }
}

/// A value in the variable store. `Unknown`(⊤) is the absorbing element.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Val {
    Concrete(ConcreteVal),
    Unknown,
}

impl Val {
    pub fn bool(b: bool) -> Self {
        Self::Concrete(ConcreteVal::Bool(b))
    }
    pub fn int(i: i64) -> Self {
        Self::Concrete(ConcreteVal::Int(i))
    }
    pub fn float(f: f64) -> Self {
        Self::Concrete(ConcreteVal::Float(f))
    }
    pub fn string(s: impl Into<String>) -> Self {
        Self::Concrete(ConcreteVal::Str(s.into()))
    }
    pub fn enum_val(s: impl Into<String>) -> Self {
        Self::Concrete(ConcreteVal::Enum(s.into()))
    }
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Concrete(v) => write!(f, "{v}"),
            Self::Unknown => write!(f, "⊤"),
        }
    }
}

/// Binary arithmetic operator.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

/// Comparison operator.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Value expression.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Expr {
    Lit(Val),
    Ref(String),
    BinOp {
        op: Op,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

/// Boolean guard (three-valued evaluation).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BoolExpr {
    True,
    Cmp {
        op: CmpOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    And(Box<BoolExpr>, Box<BoolExpr>),
    Or(Box<BoolExpr>, Box<BoolExpr>),
    Not(Box<BoolExpr>),
}

/// Variable update map.
pub type VarUpdate = BTreeMap<String, Expr>;

/// Three-valued guard result. `Unknown` is treated as satisfied
/// (over-approximation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuardResult {
    True,
    False,
    Unknown,
}

impl GuardResult {
    pub fn is_not_false(self) -> bool {
        self != Self::False
    }
}

/// Evaluate a value expression against the variable store.
pub fn eval_expr(expr: &Expr, vars: &BTreeMap<String, Val>) -> Val {
    match expr {
        Expr::Lit(v) => v.clone(),
        Expr::Ref(name) => vars.get(name).cloned().unwrap_or(Val::Unknown),
        Expr::BinOp { op, lhs, rhs } => {
            let l = eval_expr(lhs, vars);
            let r = eval_expr(rhs, vars);
            eval_binop(op, &l, &r)
        }
    }
}

fn eval_binop(op: &Op, lhs: &Val, rhs: &Val) -> Val {
    match (lhs, rhs) {
        (Val::Concrete(l), Val::Concrete(r)) => eval_concrete_binop(op, l, r),
        _ => Val::Unknown,
    }
}

fn eval_concrete_binop(op: &Op, lhs: &ConcreteVal, rhs: &ConcreteVal) -> Val {
    match (lhs, rhs) {
        (ConcreteVal::Int(a), ConcreteVal::Int(b)) => {
            let result = match op {
                Op::Add => a.checked_add(*b),
                Op::Sub => a.checked_sub(*b),
                Op::Mul => a.checked_mul(*b),
                Op::Div => {
                    if *b == 0 {
                        return Val::Unknown;
                    }
                    a.checked_div(*b)
                }
                Op::Mod => {
                    if *b == 0 {
                        return Val::Unknown;
                    }
                    a.checked_rem(*b)
                }
            };
            result.map_or(Val::Unknown, Val::int)
        }
        (ConcreteVal::Float(a), ConcreteVal::Float(b)) => {
            let result = match op {
                Op::Add => a + b,
                Op::Sub => a - b,
                Op::Mul => a * b,
                Op::Div => a / b,
                Op::Mod => a % b,
            };
            Val::float(result)
        }
        _ => Val::Unknown,
    }
}

/// Evaluate a boolean guard against the variable store.
pub fn eval_guard(guard: &BoolExpr, vars: &BTreeMap<String, Val>) -> GuardResult {
    match guard {
        BoolExpr::True => GuardResult::True,
        BoolExpr::Cmp { op, lhs, rhs } => {
            let l = eval_expr(lhs, vars);
            let r = eval_expr(rhs, vars);
            eval_cmp(op, &l, &r)
        }
        BoolExpr::And(a, b) => {
            let ra = eval_guard(a, vars);
            if ra == GuardResult::False {
                return GuardResult::False;
            }
            let rb = eval_guard(b, vars);
            if rb == GuardResult::False {
                return GuardResult::False;
            }
            if ra == GuardResult::True && rb == GuardResult::True {
                GuardResult::True
            } else {
                GuardResult::Unknown
            }
        }
        BoolExpr::Or(a, b) => {
            let ra = eval_guard(a, vars);
            if ra == GuardResult::True {
                return GuardResult::True;
            }
            let rb = eval_guard(b, vars);
            if rb == GuardResult::True {
                return GuardResult::True;
            }
            if ra == GuardResult::False && rb == GuardResult::False {
                GuardResult::False
            } else {
                GuardResult::Unknown
            }
        }
        BoolExpr::Not(inner) => match eval_guard(inner, vars) {
            GuardResult::True => GuardResult::False,
            GuardResult::False => GuardResult::True,
            GuardResult::Unknown => GuardResult::Unknown,
        },
    }
}

fn eval_cmp(op: &CmpOp, lhs: &Val, rhs: &Val) -> GuardResult {
    match (lhs, rhs) {
        (Val::Concrete(l), Val::Concrete(r)) => {
            if eval_concrete_cmp(op, l, r) {
                GuardResult::True
            } else {
                GuardResult::False
            }
        }
        _ => GuardResult::Unknown,
    }
}

fn eval_concrete_cmp(op: &CmpOp, lhs: &ConcreteVal, rhs: &ConcreteVal) -> bool {
    match (lhs, rhs) {
        (ConcreteVal::Int(a), ConcreteVal::Int(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
        },
        (ConcreteVal::Float(a), ConcreteVal::Float(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Gt => a > b,
            CmpOp::Ge => a >= b,
        },
        (ConcreteVal::Bool(a), ConcreteVal::Bool(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            _ => false,
        },
        (ConcreteVal::Str(a), ConcreteVal::Str(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            _ => false,
        },
        (ConcreteVal::Enum(a), ConcreteVal::Enum(b)) => match op {
            CmpOp::Eq => a == b,
            CmpOp::Ne => a != b,
            _ => false,
        },
        _ => false,
    }
}
