//! Label-level analyses: the checks that read a net through its labels and
//! nothing else.
//!
//! Every function here takes a *labeling function* — a closure from a node to
//! its labels, returning any `IntoIterator` (`Some(l)` for one label, `None`
//! for an unlabeled node, a `Vec` or set for several). Pass
//! [`canonical`](crate::net::canonical) to use the label the kind itself
//! carries. See [`net::labels`](crate::net::labels) for why this is a closure
//! and not a fixed trait.
//!
//! Labels need only `Clone + Eq + Hash` — not `Ord`, because the classification
//! enums frontends already have (`TransitionKind`, `ResourceType`) and
//! `std::mem::discriminant` are not ordered. Results are still deterministic:
//! groups come back in order of first appearance, scanning nodes by id, and the
//! members of a group are in id order.
//!
//! A node with several labels belongs to each of their groups, and its tokens
//! count towards each of their totals.

use std::collections::HashMap;
use std::hash::Hash;

use crate::analysis::ReachabilityGraph;
use crate::net::{Marking, Net, Place, PlaceId, Transition, TransitionId};

/// Group items by label, in order of first appearance.
fn group<L, Id, I>(items: impl Iterator<Item = (Id, I)>) -> Vec<(L, Vec<Id>)>
where
    L: Clone + Eq + Hash,
    Id: Copy + PartialEq,
    I: IntoIterator<Item = L>,
{
    let mut groups: Vec<(L, Vec<Id>)> = Vec::new();
    let mut position: HashMap<L, usize> = HashMap::new();
    for (id, labels) in items {
        for label in labels {
            let slot = *position.entry(label.clone()).or_insert_with(|| {
                groups.push((label, Vec::new()));
                groups.len() - 1
            });
            let members = &mut groups[slot].1;
            // A labeling that repeats a label for one node must not list the
            // node twice; items arrive one node at a time, so a repeat is
            // always the last member.
            if members.last() != Some(&id) {
                members.push(id);
            }
        }
    }
    groups
}

/// The places carrying each label.
pub fn partition_places<PK, TK, AK, L, I>(
    net: &Net<PK, TK, AK>,
    labels: impl Fn(&Place<PK>) -> I,
) -> Vec<(L, Vec<PlaceId>)>
where
    L: Clone + Eq + Hash,
    I: IntoIterator<Item = L>,
{
    group(net.places.iter().map(|p| (p.id, labels(p))))
}

/// The transitions carrying each label.
pub fn partition_transitions<PK, TK, AK, L, I>(
    net: &Net<PK, TK, AK>,
    labels: impl Fn(&Transition<TK>) -> I,
) -> Vec<(L, Vec<TransitionId>)>
where
    L: Clone + Eq + Hash,
    I: IntoIterator<Item = L>,
{
    group(net.transitions.iter().map(|t| (t.id, labels(t))))
}

/// The labels none of whose transitions fired on any edge of `graph`, each
/// with its members.
///
/// The label-level counterpart of
/// [`unfired_transitions`](crate::analysis::unfired_transitions): with the
/// canonical labeling it answers "which *kinds* of operation never happen",
/// and with a grouping labeling it answers "which groups are entirely dead" —
/// which is exactly the CVN's disjunctive-family rule (one dead branch of an
/// OR is fine, a dead OR is not), and is how
/// [`cvn::find_dead_transitions`](crate::analysis::cvn::find_dead_transitions)
/// is written.
///
/// On a truncated graph a label can look dead only because the exploration
/// stopped before reaching it; check [`ReachabilityGraph::truncated`].
pub fn unfired_labels<PK, TK, AK, S, L, I>(
    net: &Net<PK, TK, AK>,
    graph: &ReachabilityGraph<S>,
    labels: impl Fn(&Transition<TK>) -> I,
) -> Vec<(L, Vec<TransitionId>)>
where
    L: Clone + Eq + Hash,
    I: IntoIterator<Item = L>,
{
    let fired = graph.fired_transitions();
    partition_transitions(net, labels)
        .into_iter()
        .filter(|(_, members)| !members.iter().any(|t| fired.contains(t)))
        .collect()
}

/// A place labeling computed once, for counting tokens per label in many
/// markings.
///
/// The labeling function runs once per place at construction; after that,
/// [`count`](PlaceLabeling::count) is one pass over the marking.
#[derive(Clone, Debug)]
pub struct PlaceLabeling<L> {
    labels: Vec<L>,
    position: HashMap<L, usize>,
    of_place: Vec<Vec<usize>>,
}

impl<L: Clone + Eq + Hash> PlaceLabeling<L> {
    pub fn new<PK, TK, AK, I>(net: &Net<PK, TK, AK>, labels: impl Fn(&Place<PK>) -> I) -> Self
    where
        I: IntoIterator<Item = L>,
    {
        let mut this = Self {
            labels: Vec::new(),
            position: HashMap::new(),
            of_place: Vec::with_capacity(net.places.len()),
        };
        for place in &net.places {
            let mut slots: Vec<usize> = Vec::new();
            for label in labels(place) {
                let slot = match this.position.get(&label) {
                    Some(&slot) => slot,
                    None => {
                        this.labels.push(label.clone());
                        this.position.insert(label, this.labels.len() - 1);
                        this.labels.len() - 1
                    }
                };
                if !slots.contains(&slot) {
                    slots.push(slot);
                }
            }
            this.of_place.push(slots);
        }
        this
    }

    /// Every label some place carries, in order of first appearance.
    pub fn labels(&self) -> &[L] {
        &self.labels
    }

    /// Token totals per label in `marking`. Tokens on a place the labeling
    /// does not know (an id past the net's places) are not counted.
    pub fn count(&self, marking: &Marking) -> LabelCounts<'_, L> {
        let mut counts = vec![0usize; self.labels.len()];
        for (place, tokens) in marking.iter_nonzero() {
            if let Some(slots) = self.of_place.get(place.index()) {
                for &slot in slots {
                    counts[slot] = counts[slot].saturating_add(tokens);
                }
            }
        }
        LabelCounts {
            labeling: self,
            counts,
        }
    }
}

/// Token totals per label in one marking (saturating at `usize::MAX`).
#[derive(Clone, Debug)]
pub struct LabelCounts<'a, L> {
    labeling: &'a PlaceLabeling<L>,
    counts: Vec<usize>,
}

impl<L: Eq + Hash> LabelCounts<'_, L> {
    /// Tokens on places carrying `label`; 0 for a label no place carries, so a
    /// predicate can ask about any label without checking first.
    pub fn get(&self, label: &L) -> usize {
        self.labeling
            .position
            .get(label)
            .map_or(0, |&slot| self.counts[slot])
    }

    /// `(label, tokens)` for every label, in the labeling's order.
    pub fn iter(&self) -> impl Iterator<Item = (&L, usize)> {
        self.labeling.labels.iter().zip(self.counts.iter().copied())
    }
}

/// The states of `graph` whose per-label token totals satisfy `predicate`, in
/// state order.
///
/// A reachability query phrased in labels rather than place ids: "two tokens
/// in `Critical`", "a `Busy` worker while the `Resource` is free". Each result
/// is a state index; [`ReachabilityGraph::trace_to`] turns it into a witness
/// firing sequence. Works for any state that exposes a marking, so for P/T,
/// labeled, and CVN graphs alike. Over a truncated graph an empty answer
/// proves nothing.
pub fn states_where<PK, TK, AK, S, L, I>(
    net: &Net<PK, TK, AK>,
    graph: &ReachabilityGraph<S>,
    labels: impl Fn(&Place<PK>) -> I,
    predicate: impl Fn(&LabelCounts<'_, L>) -> bool,
) -> Vec<usize>
where
    S: AsRef<Marking>,
    L: Clone + Eq + Hash,
    I: IntoIterator<Item = L>,
{
    let labeling = PlaceLabeling::new(net, labels);
    graph
        .states
        .iter()
        .enumerate()
        .filter(|(_, state)| predicate(&labeling.count(state.as_ref())))
        .map(|(i, _)| i)
        .collect()
}
