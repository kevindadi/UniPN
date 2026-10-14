//! The [`Tag`] kind wrapper plus the [`LabeledNet`] alias.

use serde::{Deserialize, Serialize};

use crate::net::{Labeled, Net};

/// A node kind that is nothing but a label: an enum, a `String`, a set of
/// tags, `()` for "unlabeled" — whatever the caller classifies nodes by.
///
/// The wrapper is there for the firing semantics, not for the label. A blanket
/// `Semantics` impl for every `Net<PL, TL, ()>` would overlap `PtNet` and
/// `TimedNet`, which are also `Net<_, _, ()>`; `Net<Tag<PL>, Tag<TL>, ()>` is a
/// type neither of them can be, so it gets its own plain-P/T impl without
/// coherence trouble.
///
/// Serializes as the bare label (`#[serde(transparent)]`), so a net labeled
/// with an enum writes the variant and one labeled with strings writes the
/// string.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Tag<L>(pub L);

impl<L> Labeled for Tag<L> {
    type Label = L;

    fn label(&self) -> &L {
        &self.0
    }
}

/// A plain P/T net whose places are labeled with `PL` and transitions with
/// `TL`, analyzable without writing a frontend. Its state is a bare
/// [`Marking`](crate::net::Marking).
pub type LabeledNet<PL, TL> = Net<Tag<PL>, Tag<TL>, ()>;
