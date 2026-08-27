//! UniPN — a generic Petri-net model shared by three frontends.
//!
//! The crate provides a **single** generic model ([`net::Net`]) instantiated by
//! three type aliases:
//!
//! - [`pt::PtNet`] — the ordinary P/T net (ConcBugDect's MIR→PN lowering);
//! - [`timed::TimedNet`] — the priority timed net (PTPN);
//! - [`cvn::CvnNet`] — the colored verification net with guards/updates
//!   (ConcPlanVerify).
//!
//! Each net differs only in its place/transition/arc *kind* payloads and its
//! own firing semantics; the structure, ids, weights, and marking are shared.
//!
//! # Layout
//!
//! The source tree has three layers:
//!
//! - [`net`] — the generic core: the model itself plus [`net::ids`]
//!   (identifiers), [`net::incidence`] (adjacency / incidence matrix),
//!   [`net::firing`] (the structural firing primitives all three nets share),
//!   [`net::roles`] ([`PlaceRole`] / [`TransitionRole`], the questions an
//!   analysis asks a kind), and [`net::builder`] ([`NetBuilder`], which every
//!   frontend builder aliases);
//! - [`pt`], [`timed`], [`cvn`] — one directory per frontend, each holding its
//!   `kinds` (the payloads and net alias) and `semantics` (its firing), plus
//!   whatever else that frontend needs (`builder`, `expr`, `interval`, `dot`);
//! - [`analysis`] — the [`NetLike`] firing contract, the generic explorer, and
//!   one analysis module per frontend: [`analysis::pt`], [`analysis::cvn`], and
//!   `analysis::timed` (behind the `timed` feature).
//!
//! A frontend implements [`Semantics`] (`can_fire` + `fire_enabled`) and gets
//! [`NetLike`] for free; the structural half of firing comes from
//! [`net::firing`] and the capacity of a place from [`PlaceCapacity`].
//!
//! P/T and CVN model the same thing at two precisions — a control-flow skeleton
//! over shared resources, with the CVN additionally keeping guards and variable
//! updates — so they answer the same [`PlaceRole`] / [`TransitionRole`]
//! questions and share the deadlock, conflict, and dead-transition definitions
//! that follow from them. What they do *not* share is the state
//! representation; see `CLAUDE.md` for why.

pub mod analysis;
pub mod cvn;
pub mod net;
pub mod pt;
pub mod timed;

pub use analysis::{
    AnalysisConfig, Counterexample, FiringStep, NetLike, PropertyViolation, ReachabilityGraph,
    SearchStrategy, Semantics, conflict_sets, explore, find_deadlocks, unfired_transitions,
};
pub use cvn::expr::{BoolExpr, CmpOp, ConcreteVal, Expr, Op, Val, VarUpdate};
pub use cvn::{
    ControlSub, CvnArcKind, CvnBuilder, CvnExtra, CvnNet, CvnState, CvnTransition, PlaceKind,
    ResourceType, TransitionKind, VarStore,
};
pub use net::{
    Arc, ArcDir, Incidence, IncidenceMatrix, Marking, Net, NetBuilder, Place, PlaceCapacity,
    PlaceId, PlaceRole, State, Transition, TransitionId, TransitionRole,
};
pub use pt::{
    AliasId, AtomicOrdering, PlaceType, PtBuilder, PtNet, PtPlaceKind, PtTransitionKind,
    TransitionType, UnsafeOp, marking,
};
pub use timed::{
    CONTROL_TRANSITION_CORE, INF, TimeInterval, TimedBuilder, TimedExtra, TimedNet, TimedPlaceKind,
    TimedState, TimedTransitionKind,
};
