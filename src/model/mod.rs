//! Core data model for the CVN (Concurrency Verification Net).
//!
//! This module defines the fundamental types used throughout the library:
//! places, transitions, arcs, values, expressions, and runtime state.

mod arc;
mod expr;
mod place;
mod state;
mod transition;
mod val;

pub use arc::*;
pub use expr::*;
pub use place::*;
pub use state::*;
pub use transition::*;
pub use val::*;
