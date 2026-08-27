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
//!   (identifiers) and [`net::incidence`] (adjacency / incidence matrix);
//! - [`pt`], [`timed`], [`cvn`] — one directory per frontend, each holding its
//!   `kinds` (the payloads and net alias) and `semantics` (its firing), plus
//!   whatever else that frontend needs (`builder`, `expr`, `interval`, `dot`);
//! - [`analysis`] — the [`NetLike`] firing contract, the generic explorer, and
//!   one analysis module per frontend: [`analysis::pt`], [`analysis::cvn`], and
//!   `analysis::timed` (behind the `timed` feature).

pub mod analysis;
pub mod cvn;
pub mod net;
pub mod pt;
pub mod timed;

pub use analysis::{
    AnalysisConfig, Counterexample, FiringStep, NetLike, PropertyViolation, ReachabilityGraph,
    SearchStrategy, explore, find_deadlocks,
};
pub use cvn::expr::{BoolExpr, CmpOp, ConcreteVal, Expr, Op, Val, VarUpdate};
pub use cvn::{
    ControlSub, CvnArcKind, CvnBuilder, CvnExtra, CvnNet, CvnState, CvnTransition, PlaceKind,
    ResourceType, TransitionKind, VarStore,
};
pub use net::{
    Arc, ArcDir, Incidence, IncidenceMatrix, Marking, Net, Place, PlaceId, State, Transition,
    TransitionId,
};
pub use pt::{
    AliasId, AtomicOrdering, PlaceType, PtBuilder, PtNet, PtPlace, PtPlaceKind, PtTransition,
    PtTransitionKind, TransitionType, UnsafeOp, marking,
};
pub use timed::{
    CONTROL_TRANSITION_CORE, INF, TimeInterval, TimedExtra, TimedNet, TimedPlaceKind, TimedState,
    TimedTransitionKind, overflowed_places, reset_overflow_recording,
};
