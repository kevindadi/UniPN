//! Dense-time (Merlin–Farber) time Petri net analysis: state-class reachability.
//!
//! The state-class graph (SCG) of Berthomieu–Menasche is built over a
//! difference bound matrix that keeps open/closed endpoints distinct (see
//! [`dbm`]). The analysis is compiled only with the `timed` feature, alongside
//! the PTPN state-class analysis in [`analysis::timed`](crate::analysis::timed);
//! the two are independent and do not share a DBM.
//!
//! # What the SCG preserves
//!
//! No extrapolation is performed: unbounded nets are explored until
//! `max_states` is reached and the graph is then flagged `truncated`. The SCG
//! preserves the set of *reachable markings* and the set of *untimed firing
//! sequences* (every firing sequence of the timed net is a firing sequence of
//! its untimed skeleton, and the SCG keeps exactly those that are feasible in
//! time). What it does **not** preserve is CTL and state-level timed questions
//! (whether a *specific* timed state `(M, ν)` is reachable) — those need the
//! strong state class graph (SSCG), which is out of scope here.
//!
//! # Discretization (Popova-Zeugmann)
//!
//! For nets whose intervals are closed with integer endpoints, the reachable
//! markings under dense time equal those under integer time. This is the
//! discretization of L. Popova-Zeugmann (see "Time Petri nets state space
//! reduction using dynamic programming", Control and Cybernetics 35(3), 2006,
//! and the earlier "essential states" papers), stated there for the classical
//! TPN semantics — the intermediate memory policy. `tests/tpn.rs` asserts the
//! integer-time oracle against the SCG for the Intermediate rule and compares
//! the Atomic rule empirically; the theorem is not stated for Atomic, so that
//! configuration is a check, not a citation-backed assertion.

pub mod dbm;
pub mod reachability;
pub mod state_class;

pub use dbm::{Bound, Dbm, Overflow};
pub use reachability::{
    DisablingPolicy, FiringPolicy, MultiServerConfig, NewlyEnabled, Servers, StateClassSpace,
    TpnSemantics, state_class_graph,
};
pub use state_class::{InstanceKey, StateClass};
