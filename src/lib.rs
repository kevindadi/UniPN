//! # CVN — Concurrency Verification Net
//!
//! A weighted P/T Petri net library with global variable guards, designed for
//! concurrent program verification.
//!
//! CVN works with an upstream CIR (Concurrency Intermediate Representation):
//! CIR is translated into a CVN, then state space search discovers concurrency
//! bugs (deadlocks, livelocks, signal loss, etc.), and counterexamples are mapped
//! back to CIR statement IDs.
//!
//! **This library handles the CVN layer only** — CIR parsing and CIR→CVN
//! translation are out of scope.
//!
//! ## Quick start
//!
//! ```rust
//! use cvn::builder::CvnNetBuilder;
//! use cvn::model::*;
//! use cvn::analysis::{AnalysisConfig, explore};
//!
//! let net = CvnNetBuilder::new()
//!     .add_control_place("p0", "main", "s0")
//!     .add_control_place("p1", "main", "s1")
//!     .set_return("p1")
//!     .add_transition("t0", TransitionKind::Sequential, &["s0"])
//!     .add_input_arc("p0", "t0", 1, BoolExpr::True)
//!     .add_output_arc("t0", "p1", 1, None)
//!     .set_initial_tokens("p0", 1)
//!     .build()
//!     .expect("valid net");
//!
//! let result = explore(&net, &AnalysisConfig::default()).unwrap();
//! assert!(result.deadlocks.is_empty());
//! ```

pub mod analysis;
pub mod builder;
pub mod error;
pub mod export;
pub mod model;
pub mod net;
mod validate;
