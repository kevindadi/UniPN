//! Extensible Petri-net model library.
//!
//! The crate separates declarative structure (`core`), value domains (`domain`),
//! runtime containers (`runtime`), and execution semantics (`semantics`).
//! Analysis engines and frontend lowerings can be built on these boundaries
//! without changing the model representation.

pub mod analysis;
pub mod bug;
pub mod core;
pub mod domain;
pub mod ids;
pub mod pt;
pub mod runtime;
pub mod semantics;

pub use core::expr::{ActionExpr, GuardExpr, Pattern, Term};
pub use core::model::{
    ArcDecl, InhibitorArc, InputArc, ModelError, Multiplicity, NetModel, OutputArc, PlaceDecl,
    ReadArc, ResetArc, RoleTag, TimingSpec, TransitionDecl,
};
pub use core::sort::Sort;
pub use core::value::{Bool3, Token, Value};
pub use core::{expr, model, sort, value};
pub use domain::{BindingEnv, Domain, DomainError, TruthValue};
pub use ids::{FuncId, PlaceId, SortId, Symbol, TransitionId};
pub use pt::{
    CapacityMode, PlaceRole, PtArc, PtArcKind, PtExecutionError, PtModelError, PtNet, PtPlace,
    PtTransition, Weight,
};
pub use runtime::{
    ColoredMarking, ColoredState, Multiset, PtMarking, PtState, RuntimeError, RuntimeState,
};
pub use semantics::{Execution, PtSemantics, Semantics};
