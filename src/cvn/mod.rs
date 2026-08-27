//! The CVN (Concurrency Verification Net) frontend — ConcPlanVerify's lowering
//! target.
//!
//! Guards live on input arcs, variable updates on output arcs, and the variable
//! store is the net's `State` extra payload. The module is split into
//!
//! - [`kinds`] — the `PK`/`TK`/`AK` payloads plus the [`CvnNet`]/[`CvnState`]
//!   aliases;
//! - [`expr`] — values, expressions and three-valued guards;
//! - [`semantics`] — guard-gated enabling and firing ([`NetLike`](crate::analysis::NetLike));
//! - [`builder`] — chain-style construction;
//! - [`dot`] — Graphviz export.
//!
//! The CVN's deadlock / dead-transition / conflict analysis lives in
//! [`analysis::cvn`](crate::analysis::cvn), next to the P/T and timed analyses.

pub mod builder;
pub mod dot;
pub mod expr;
pub mod kinds;
pub mod semantics;

pub use builder::CvnBuilder;
pub use dot::to_dot;
pub use kinds::{
    ControlSub, CvnArcKind, CvnExtra, CvnNet, CvnState, CvnTransition, PlaceKind, ResourceType,
    TransitionKind, VarStore,
};
