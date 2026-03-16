//! Expressions and boolean guards for the CVN.
//!
//! Provides recursive enum types for value expressions ([`Expr`]) and boolean
//! guard expressions ([`BoolExpr`]), along with three-valued evaluation functions
//! and a DSL for convenient construction.

use crate::model::{ConcreteVal, Val};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Arithmetic operators
// ---------------------------------------------------------------------------

/// Binary arithmetic operator.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Op {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Modulo.
    Mod,
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Comparison operators
// ---------------------------------------------------------------------------

/// Comparison operator used in boolean guard expressions.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CmpOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Eq => "==",
            Self::Ne => "!=",
            Self::Lt => "<",
            Self::Le => "<=",
            Self::Gt => ">",
            Self::Ge => ">=",
        };
        write!(f, "{s}")
    }
}

// ---------------------------------------------------------------------------
// Value expressions
// ---------------------------------------------------------------------------

/// A value expression that evaluates to a [`Val`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Literal value.
    Lit(Val),
    /// Variable reference (resolved via the variable store).
    Ref(String),
    /// Binary arithmetic operation.
    BinOp {
        /// The operator.
        op: Op,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lit(v) => write!(f, "{v}"),
            Self::Ref(name) => write!(f, "{name}"),
            Self::BinOp { op, lhs, rhs } => write!(f, "({lhs} {op} {rhs})"),
        }
    }
}

// ---------------------------------------------------------------------------
// Boolean guard expressions
// ---------------------------------------------------------------------------

/// A boolean guard expression used on input arcs.
///
/// Evaluates to a three-valued [`GuardResult`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BoolExpr {
    /// Always true (default guard).
    True,
    /// Comparison of two value expressions.
    Cmp {
        /// The comparison operator.
        op: CmpOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
    /// Logical AND of two boolean expressions.
    And(Box<BoolExpr>, Box<BoolExpr>),
    /// Logical OR of two boolean expressions.
    Or(Box<BoolExpr>, Box<BoolExpr>),
    /// Logical NOT of a boolean expression.
    Not(Box<BoolExpr>),
}

impl fmt::Display for BoolExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::True => write!(f, "true"),
            Self::Cmp { op, lhs, rhs } => write!(f, "({lhs} {op} {rhs})"),
            Self::And(a, b) => write!(f, "({a} && {b})"),
            Self::Or(a, b) => write!(f, "({a} || {b})"),
            Self::Not(inner) => write!(f, "!{inner}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Three-valued guard result
// ---------------------------------------------------------------------------

/// Result of evaluating a [`BoolExpr`] under three-valued logic.
///
/// When `Unknown`, the transition is still considered enabled (over-approximation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuardResult {
    /// Definitely true.
    True,
    /// Definitely false.
    False,
    /// Cannot be determined (over-approximation: treated as enabled).
    Unknown,
}

impl GuardResult {
    /// Returns `true` if the guard does not definitely evaluate to `false`.
    ///
    /// Used for enabling check: a transition fires unless its guard is `False`.
    pub fn is_not_false(self) -> bool {
        self != Self::False
    }
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Evaluate a value expression against a variable store.
pub fn eval_expr(expr: &Expr, vars: &IndexMap<String, Val>) -> Val {
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

/// Evaluate a boolean guard expression against a variable store.
///
/// Returns a three-valued [`GuardResult`]. When `Unknown`, the guard is treated
/// as satisfied (over-approximation for soundness).
pub fn eval_guard(guard: &BoolExpr, vars: &IndexMap<String, Val>) -> GuardResult {
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
            let result = eval_concrete_cmp(op, l, r);
            if result {
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

// ---------------------------------------------------------------------------
// DSL helper functions
// ---------------------------------------------------------------------------

/// Create a literal expression from a [`Val`].
pub fn lit(v: Val) -> Expr {
    Expr::Lit(v)
}

/// Create a literal integer expression.
pub fn lit_int(i: i64) -> Expr {
    Expr::Lit(Val::int(i))
}

/// Create a literal boolean expression.
pub fn lit_bool(b: bool) -> Expr {
    Expr::Lit(Val::bool(b))
}

/// Create a variable reference expression.
pub fn var(name: impl Into<String>) -> Expr {
    Expr::Ref(name.into())
}

/// Create an addition expression.
pub fn add(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp {
        op: Op::Add,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a subtraction expression.
pub fn sub(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp {
        op: Op::Sub,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a multiplication expression.
pub fn mul(lhs: Expr, rhs: Expr) -> Expr {
    Expr::BinOp {
        op: Op::Mul,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a comparison guard: equal.
pub fn eq(lhs: Expr, rhs: Expr) -> BoolExpr {
    BoolExpr::Cmp {
        op: CmpOp::Eq,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a comparison guard: not equal.
pub fn ne(lhs: Expr, rhs: Expr) -> BoolExpr {
    BoolExpr::Cmp {
        op: CmpOp::Ne,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a comparison guard: greater than.
pub fn gt(lhs: Expr, rhs: Expr) -> BoolExpr {
    BoolExpr::Cmp {
        op: CmpOp::Gt,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a comparison guard: greater than or equal.
pub fn ge(lhs: Expr, rhs: Expr) -> BoolExpr {
    BoolExpr::Cmp {
        op: CmpOp::Ge,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a comparison guard: less than.
pub fn lt(lhs: Expr, rhs: Expr) -> BoolExpr {
    BoolExpr::Cmp {
        op: CmpOp::Lt,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a comparison guard: less than or equal.
pub fn le(lhs: Expr, rhs: Expr) -> BoolExpr {
    BoolExpr::Cmp {
        op: CmpOp::Le,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}

/// Create a logical AND guard.
pub fn and(a: BoolExpr, b: BoolExpr) -> BoolExpr {
    BoolExpr::And(Box::new(a), Box::new(b))
}

/// Create a logical OR guard.
pub fn or(a: BoolExpr, b: BoolExpr) -> BoolExpr {
    BoolExpr::Or(Box::new(a), Box::new(b))
}

/// Create a logical NOT guard.
pub fn not(inner: BoolExpr) -> BoolExpr {
    BoolExpr::Not(Box::new(inner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_int_arithmetic() {
        let vars = IndexMap::new();
        let expr = add(lit_int(3), lit_int(4));
        assert_eq!(eval_expr(&expr, &vars), Val::int(7));
    }

    #[test]
    fn eval_unknown_absorbs() {
        let vars = IndexMap::new();
        let expr = add(lit(Val::Unknown), lit_int(4));
        assert_eq!(eval_expr(&expr, &vars), Val::Unknown);
    }

    #[test]
    fn eval_guard_true() {
        let vars = IndexMap::new();
        assert_eq!(eval_guard(&BoolExpr::True, &vars), GuardResult::True);
    }

    #[test]
    fn eval_guard_cmp_concrete() {
        let vars = IndexMap::new();
        let g = gt(lit_int(5), lit_int(3));
        assert_eq!(eval_guard(&g, &vars), GuardResult::True);

        let g2 = gt(lit_int(1), lit_int(3));
        assert_eq!(eval_guard(&g2, &vars), GuardResult::False);
    }

    #[test]
    fn eval_guard_unknown_var() {
        let mut vars = IndexMap::new();
        vars.insert("x".to_string(), Val::Unknown);
        let g = gt(var("x"), lit_int(0));
        assert_eq!(eval_guard(&g, &vars), GuardResult::Unknown);
    }

    #[test]
    fn eval_guard_and_short_circuit() {
        let vars = IndexMap::new();
        let g = and(
            gt(lit_int(1), lit_int(5)),
            gt(lit(Val::Unknown), lit_int(0)),
        );
        assert_eq!(eval_guard(&g, &vars), GuardResult::False);
    }

    #[test]
    fn eval_guard_or_short_circuit() {
        let vars = IndexMap::new();
        let g = or(
            gt(lit_int(5), lit_int(1)),
            gt(lit(Val::Unknown), lit_int(0)),
        );
        assert_eq!(eval_guard(&g, &vars), GuardResult::True);
    }
}
