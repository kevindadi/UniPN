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
#![allow(clippy::collapsible_if)]

pub mod analysis;
pub mod builder;
pub mod core;
pub mod domain;
pub mod engine;
pub mod export;
pub mod expr;
pub mod ids;
pub mod model;
pub mod net;
pub mod netlike;
pub mod runtime;
pub mod semantics;
pub mod state;
pub mod storage;
pub mod testgen;
#[cfg(feature = "timed")]
pub mod timed;

pub use builder::NetBuilder;
pub use core::{expr::*, model::*, sort::*, value::*};
pub use domain::mod_traits::{BindingEnv, Domain, TruthValue};
pub use engine::interp::InterpEngine;
pub use ids::{PlaceId, TransitionId, Weight};
pub use model::{ControlSub, Place, PlaceKind, ResourceType, Transition, TransitionKind};
pub use net::Net;
pub use netlike::{FireError, NetLike};
pub use runtime::marking::{ColoredMarking, Multiset, PtMarking};
pub use runtime::state::{ColoredState, PtState, RuntimeState};
pub use state::{Marking, State, VarStore};
