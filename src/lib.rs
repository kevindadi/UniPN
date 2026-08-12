//! Extensible Petri-net model library.
//!
//! The crate separates declarative structure (`core`), value domains (`domain`),
//! runtime containers (`runtime`), and execution semantics (`semantics`).
//! Analysis engines and frontend lowerings can be built on these boundaries
//! without changing the model representation.

pub mod analysis;
pub mod bug;
pub mod builder;
pub mod core;
pub mod domain;
pub mod export;
pub mod expr;
pub mod ids;
pub mod model;
pub mod net;
pub mod netlike;
pub mod pt;
pub mod runtime;
pub mod semantics;
pub mod state;
pub mod storage;
#[cfg(feature = "timed")]
pub mod timed;

pub use builder::NetBuilder;
pub use core::expr::{ActionExpr, GuardExpr, Pattern, Term};
pub use core::model::{
    ArcDecl, InhibitorArc, InputArc, ModelError, Multiplicity, NetModel, OutputArc, PlaceDecl,
    ReadArc, ResetArc, RoleTag, TimingSpec, TransitionDecl,
};
pub use core::sort::Sort;
pub use core::value::{Bool3, Token, Value};
pub use domain::{BindingEnv, Domain, DomainError, TruthValue};
pub use expr::{BoolExpr, CmpOp, ConcreteVal, Expr, Op, Val, VarUpdate};
pub use ids::{FuncId, PlaceId, SortId, Symbol, TransitionId, Weight};
pub use model::{ControlSub, Place, PlaceKind, ResourceType, Transition, TransitionKind};
pub use net::Net;
pub use netlike::{FireError, NetLike};
pub use pt::{
    CapacityMode, PlaceRole, PtArc, PtArcKind, PtExecutionError, PtModelError, PtNet, PtPlace,
    PtTransition,
};
pub use runtime::{
    ColoredMarking, ColoredState, Multiset, PtMarking, PtState, RuntimeError, RuntimeState,
};
pub use semantics::{ColoredSemantics, Execution, PtSemantics, Semantics};
pub use state::{Marking, State, VarStore};
