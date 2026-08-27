//! The priority timed Petri net frontend (PTPN's lowering target).
//!
//! [`TimedNet`] is [`Net`](crate::net::Net) instantiated with PTPN's
//! place/transition kind payloads and no arc kind. The module is split into
//!
//! - [`interval`] — [`TimeInterval`] and the [`INF`] sentinel;
//! - [`kinds`] — the `PK`/`TK` payloads plus the [`TimedNet`]/[`TimedState`]
//!   aliases;
//! - [`semantics`] — the discrete (untimed) firing exposed through
//!   [`NetLike`](crate::analysis::NetLike), plus overflow recording.
//!
//! Time is an *annotation* here. Clock zones are **not** part of
//! [`TimedState`]: the state-class (DBM) reachability lives in
//! `analysis::timed` and is compiled only with the `timed` feature.

pub mod interval;
pub mod kinds;
pub mod semantics;

pub use interval::{INF, TimeInterval};
pub use kinds::{
    CONTROL_TRANSITION_CORE, TimedExtra, TimedNet, TimedPlaceKind, TimedState, TimedTransitionKind,
};
pub use semantics::{overflowed_places, reset_overflow_recording};
