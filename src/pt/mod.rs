//! The ordinary P/T net frontend (ConcBugDect's MIR→PN lowering target).
//!
//! [`PtNet`] is [`Net`](crate::net::Net) instantiated with ConcBugDect's
//! place/transition metadata kinds and no arc kind. The module is split into
//!
//! - [`kinds`] — the `PK`/`TK` payloads plus the [`PtNet`] alias;
//! - [`semantics`] — P/T firing over a [`Marking`](crate::net::Marking)
//!   ([`NetLike`](crate::analysis::NetLike)), with read/inhibitor/reset arcs and
//!   capacity clamping;
//! - [`builder`] — chain-style construction mirroring ConcBugDect's API;
//! - [`dot`] — Graphviz export and connectivity diagnostics.
//!
//! Reachability, boundness, and net reduction live in
//! [`analysis::pt`](crate::analysis::pt).

pub mod builder;
pub mod dot;
pub mod kinds;
pub mod semantics;

pub use builder::{PtBuilder, marking};
pub use dot::DiagnosticReport;
pub use kinds::{
    AliasId, AtomicOrdering, PlaceType, PtNet, PtPlaceKind, PtTransitionKind, TransitionType,
    UnsafeOp,
};
