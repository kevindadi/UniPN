//! The generic labeled net: a plain P/T net whose nodes carry caller-chosen
//! labels.
//!
//! Every frontend here is a labeled net — P/T's places are `Control` or
//! `Resource`, the CVN's transitions are `TransitionKind`s — but each of those
//! labels comes bundled with a frontend: its own kinds, builder, and firing.
//! [`LabeledNet<PL, TL>`] is the net for a caller who has only the labels: an
//! enum of their own, `String`s read from a file, `()` for none. It fires as a
//! plain P/T net and works with every kind-agnostic analysis, including the
//! label-level ones in [`analysis::labels`](crate::analysis::labels).
//!
//! - [`kinds`] — the [`Tag`] wrapper and the [`LabeledNet`] alias;
//! - [`semantics`] — plain P/T firing over a [`Marking`](crate::net::Marking);
//! - [`builder`] — [`LabeledBuilder`], a [`NetBuilder`](crate::net::NetBuilder)
//!   alias.
//!
//! A net whose labels *do* change firing — a capacity per resource type, a
//! guard — is a frontend, not a `LabeledNet`.

pub mod builder;
pub mod kinds;
pub mod semantics;

pub use builder::LabeledBuilder;
pub use kinds::{LabeledNet, Tag};
