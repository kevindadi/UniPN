//! 值、表达式与守卫（可选数据模型）。
//!
//! 纯 P/T 网不建模数据（`State::vars = None`）；带数据的前端（ConcIR→CVN）用它
//! 表达输入弧 guard 与输出弧 update。

use indexmap::IndexMap;
use std::fmt;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// 一个完全已知的具体值。
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

/// 变量库中的值。`Unknown`(⊤) 是吸收元。
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

/// 二元算术运算符。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

/// 比较运算符。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// 值表达式。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Lit(Val),
    Ref(String),
    BinOp {
        op: Op,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

/// 布尔守卫（三值求值）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

/// 变量更新表。
pub type VarUpdate = IndexMap<String, Expr>;

/// 三值守卫结果。`Unknown` 按满足处理（over-approximation）。
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

/// 对变量库求值值表达式。
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

/// 对变量库求值布尔守卫。
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

// 兼容：使 val/expr 可用于 JSON 序列化场景。
pub(crate) fn _assert_serde<T: Serialize + DeserializeOwned>() {}
