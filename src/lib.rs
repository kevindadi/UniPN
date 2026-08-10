//! # UniPN — Unified Petri Net
//!
//! A fast, extensible Petri net core shared by several frontends and analysis
//! consumers:
//!
//! ```text
//! Frontends (net building)          Core (matrix storage + trait)   Consumers (analysis)
//!   ConcIR   ─┐                    Net (CSC incidence matrix)      deadlock / dead-transition / conflict
//!   Rust MIR ┼─▶ NetLike ───────▶  explore (BFS/DFS/POR)           invariants
//!   test intent┘   (object-safe)   deadlock / dead_transition       test-case generation (testgen)
//!   time (PTPN) ─▶ Timed reserve   conflict / invariants / dot      timed / real-time scheduling
//! ```
//!
//! ## Design principles
//!
//! 1. **Trait-first**: [`netlike::NetLike`] is the single contract (object-safe).
//!    Any net (CVN, ConcBugDect MIR→PN, future test/timed nets) only needs to
//!    implement it to be consumed by the shared algorithms. A pure P/T net can
//!    rely on the trait's default implementations (it only fills the structural
//!    predicates).
//! 2. **Matrix-backed**: the core [`net::Net`] stores the `Pre/Post` incidence
//!    as CSC sparse columns, so the enabled/fire hot path is O(|arcs|) instead
//!    of O(|P|·|T|); the dense `C = Post − Pre` matrix is only materialized when
//!    linear algebra is needed.
//! 3. **Semantics externalized**: `kind` is only an annotation; semantics such as
//!    "thread terminal / wait point / resource" are exposed through frontend
//!    predicates, not hardcoded in the common layer.
//! 4. **Extensible**: `timed` / `invariants` are feature-gated extension slots.
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
pub use expr::{BoolExpr, CmpOp, ConcreteVal, Expr, GuardResult, Op, Val, VarUpdate, eval_expr, eval_guard};
pub use ids::{PlaceId, TransitionId, Weight};
pub use model::{ControlSub, Place, PlaceKind, ResourceType, Transition, TransitionKind};
pub use net::Net;
pub use netlike::{FireError, NetLike};
pub use state::{Marking, State, VarStore};
