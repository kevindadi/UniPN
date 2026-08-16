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

pub mod analysis;
pub mod cvn;
pub mod expr;
pub mod ids;
pub mod incidence;
pub mod model;
pub mod net;
pub mod pt;
pub mod timed;

pub use analysis::{
    AnalysisConfig, Counterexample, FiringStep, NetLike, PropertyViolation, ReachabilityGraph,
    SearchStrategy, explore, find_deadlocks,
};
pub use cvn::{CvnArcKind, CvnBuilder, CvnNet, CvnState, CvnTransition, VarStore};
pub use expr::{BoolExpr, CmpOp, ConcreteVal, Expr, Op, Val, VarUpdate};
pub use ids::{PlaceId, TransitionId};
pub use model::{ControlSub, PlaceKind, ResourceType, TransitionKind};
pub use net::{Arc, ArcDir, Incidence, IncidenceMatrix, Marking, Net, Place, State, Transition};
pub use pt::{
    AliasId, AtomicOrdering, PlaceType, PtBuilder, PtNet, PtPlace, PtPlaceKind, PtTransition,
    PtTransitionKind, TransitionType, UnsafeOp, marking,
};
pub use timed::{
    CONTROL_TRANSITION_CORE, INF, TimeInterval, TimedExtra, TimedNet, TimedPlaceKind, TimedState,
    TimedTransitionKind, overflowed_places, reset_overflow_recording,
};
