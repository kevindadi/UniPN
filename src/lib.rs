//! # CVN — Concurrency Verification Net
//!
//! A weighted P/T Petri net library with global variable guards, designed for
//! concurrent program verification.
//!
//! ## Feature flags
//!
//! | Feature | Default | Description |
//! |---------|---------|-------------|
//! | `cir-anchor` | off | Transitions carry CIR statement ID anchors for mapping counterexamples back to source locations |
//!
//! When `cir-anchor` is enabled, transitions can carry CIR statement ID anchors
//! via [`Transition::with_anchor()`](model::Transition::with_anchor) and
//! [`CvnNetBuilder::add_transition_with_anchor()`](builder::CvnNetBuilder::add_transition_with_anchor).
//! Use [`CvnNetBuilder::build_with_anchor_check()`](builder::CvnNetBuilder::build_with_anchor_check)
//! to additionally validate that every transition has at least one anchor (W7 / V105).
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
//!     .add_transition("t0", TransitionKind::Sequential)
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
pub(crate) mod validate;
