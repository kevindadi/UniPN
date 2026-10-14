//! The dense-time (Merlin–Farber) time Petri net frontend.
//!
//! [`TpnNet`] is [`Net`](crate::net::Net) instantiated with labeled places
//! ([`Tag<PL>`](crate::labeled::Tag)) and interval-annotated transitions
//! ([`TpnTransition<TL>`]), and no arc kind. The module is split into
//!
//! - [`kinds`] — the [`Interval`] / [`UpperBound`] payloads, the
//!   [`TpnTransition`] kind, and the [`TpnNet`] alias;
//! - [`semantics`] — the untimed P/T skeleton exposed through
//!   [`NetLike`](crate::analysis::NetLike);
//! - [`builder`] — [`TpnBuilder`], the net-plus-marking construction.
//!
//! Time is an *annotation* here, exactly as in [`timed`](crate::timed): the
//! dense-time state-class (DBM) reachability lives in `analysis::tpn` and is
//! compiled only with the `timed` feature. Exploring a `TpnNet` directly gives
//! the untimed over-approximation — the firing intervals are ignored by the
//! P/T skeleton.

pub mod builder;
pub mod kinds;
pub mod semantics;

pub use builder::TpnBuilder;
pub use kinds::{Interval, IntervalError, TpnNet, TpnTransition, UpperBound};
