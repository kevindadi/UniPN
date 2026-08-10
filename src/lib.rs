//! # UniPN — Unified Petri Net
//!
//! A fast, extensible Petri net core shared by several frontends and analysis
//! consumers:
//!
//! ```text
//! Frontends（建网）               Core（矩阵存储 + trait）          Consumers（分析）
//!   ConcIR  ─┐                   Net（CSC incidence 矩阵）          死锁/死迁移/冲突集
//!   Rust MIR┼─▶ NetLike ──────▶  explore (BFS/DFS/POR)              不变量 (invariants)
//!   测试意图┘   (object-safe)    deadlock / dead_transition          测试用例生成 (testgen)
//!   时间(PTPN) ─▶ Timed 预留     conflict / invariants / dot         时间/实时调度属性
//! ```
//!
//! ## 设计原则
//!
//! 1. **Trait-first**：[`netlike::NetLike`] 是唯一契约（object-safe），任何网
//!    （CVN、ConcBugDect MIR→PN、未来的测试/时间网）只需实现它即可被共享算法
//!    消费。纯 P/T 网可直接用 trait 默认实现（只填结构谓词）。
//! 2. **矩阵底层**：核心 [`net::Net`] 用 CSC 稀疏列存储 `Pre/Post` incidence，
//!    enabled/fire 热路径是 O(弧数) 而非 O(|P|·|T|)；需要线性代数时才物化
//!    稠密 `C = Post − Pre`。
//! 3. **语义外置**：`kind` 只是标注；"线程终点/等待点/资源"等语义由前端谓词
//!    暴露，common 层不做硬编码。
//! 4. **可扩展**：`timed` / `invariants` 是 feature 门控的扩展位。
#![allow(clippy::collapsible_if)]

pub mod analysis;
pub mod builder;
pub mod export;
pub mod expr;
pub mod ids;
pub mod model;
pub mod net;
pub mod netlike;
pub mod state;
pub mod storage;
pub mod testgen;
#[cfg(feature = "timed")]
pub mod timed;

pub use builder::NetBuilder;
pub use expr::{BoolExpr, CmpOp, ConcreteVal, Expr, GuardResult, Op, Val, eval_expr, eval_guard};
pub use ids::{PlaceId, TransitionId, Weight};
pub use net::Net;
pub use netlike::{FireError, NetLike};
pub use state::{Marking, State, VarStore};
