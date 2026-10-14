//! Labels: the classification a node's kind carries, as a question any analysis
//! can ask.
//!
//! In this crate a *labeled net* is a net whose places and transitions are
//! classified — P/T's places are `Control(..)` or `Resource`, its transitions a
//! `TransitionType`; the CVN's carry a `ResourceType` and a `TransitionKind`.
//! That classification has always lived in the kind payload, so the model never
//! lacked labels. What it lacked was a way to *ask* for them without either
//! knowing the frontend's types or going through [`PlaceRole`] /
//! [`TransitionRole`], which are a fixed, concurrency-specific reading of those
//! labels ("is this a resource", "is this an acquire").
//!
//! The answer has two halves.
//!
//! - **Labeling functions are the mechanism.** The label-level analyses in
//!   [`analysis::labels`](crate::analysis::labels) take a closure from a node to
//!   its labels — any `IntoIterator`, so one label is `Some(l)`, none is
//!   `None`, and several are a `Vec` or a set. The caller decides the
//!   granularity: the whole kind, just its variant, a field, a tag parsed out of
//!   a name. Labels are typically an enum or a `String`, but nothing here
//!   assumes either.
//! - **[`Labeled`] is the default.** A kind that has an obvious classification
//!   says what it is, and [`canonical`] turns that into a labeling function, so
//!   the common call needs no closure at all.
//!
//! `Labeled` returns a *reference* into the kind: the label is the
//! classification the kind already stores, not something computed from it.
//! Anything computed — a coarser grouping, a set of tags — is a labeling
//! function. That keeps the trait from growing a parallel "tag" enum per
//! frontend, which would be the same classification written twice.
//!
//! [`PlaceRole`]: crate::net::PlaceRole
//! [`TransitionRole`]: crate::net::TransitionRole

use crate::net::{Place, PlaceClass, Transition};

/// A kind (or node) whose classification is the label.
///
/// One trait for places and transitions alike: a type is used as one or the
/// other, and `String` is a perfectly good label for both.
pub trait Labeled {
    type Label;

    /// The label this kind already carries.
    fn label(&self) -> &Self::Label;
}

/// A string label is its own label — the common case for nets read from a file.
impl Labeled for String {
    type Label = String;

    fn label(&self) -> &String {
        self
    }
}

/// The shared place classification is itself the label, payload included; a
/// caller who wants only control-versus-resource coarsens it in a closure.
impl<R> Labeled for PlaceClass<R> {
    type Label = Self;

    fn label(&self) -> &Self {
        self
    }
}

/// A node is labeled by its kind, so analyses that are handed whole nodes can
/// use the kind's canonical label without unwrapping.
impl<K: Labeled> Labeled for Place<K> {
    type Label = K::Label;

    fn label(&self) -> &K::Label {
        self.kind.label()
    }
}

impl<K: Labeled> Labeled for Transition<K> {
    type Label = K::Label;

    fn label(&self) -> &K::Label {
        self.kind.label()
    }
}

/// The canonical labeling as a labeling function: exactly one label per node,
/// the one its kind carries.
///
/// Pass it where an analysis wants a labeling function:
/// `partition_places(&net, canonical)`.
pub fn canonical<N: Labeled>(node: &N) -> Option<N::Label>
where
    N::Label: Clone,
{
    Some(node.label().clone())
}
