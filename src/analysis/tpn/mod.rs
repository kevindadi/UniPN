//! Dense-time (Merlin–Farber) time Petri net analysis: state-class reachability.
//!
//! The state-class graph (SCG) of Berthomieu–Menasche is built over a
//! difference bound matrix that keeps open/closed endpoints distinct (see
//! [`dbm`]). The analysis is compiled only with the `timed` feature, alongside
//! the PTPN state-class analysis in [`analysis::timed`](crate::analysis::timed);
//! the two are independent and do not share a DBM.

pub mod dbm;
pub mod reachability;
pub mod state_class;

pub use dbm::{Bound, Dbm, Overflow};
pub use reachability::{NewlyEnabled, Servers, StateClassSpace, TpnSemantics, state_class_graph};
pub use state_class::{InstanceKey, StateClass};
