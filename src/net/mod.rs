//! The single generic Petri-net model.
//!
//! Every net — P/T (ConcBugDect), priority-timed (PTPN), colored/CVN
//! (ConcPlanVerify) — is an instantiation of [`Net`] over different *kind*
//! payloads. The common structure is fixed:
//!
//! - a place always has an `id` (`PlaceId`) and a `name`;
//! - a transition always has an `id` (`TransitionId`) and a `name`;
//! - an arc always has a place, a transition, a `direction`, and a `weight`.
//!
//! The domain-specific part is carried by the generic kind parameters `PK`,
//! `TK`, `AK` (place/transition/arc kind), and the marking is a dense
//! `Vec<usize>` (index = place id, value = token count) kept **separate** from
//! the net. Anything a net needs beyond the token counts (variable stores,
//! clock zones, …) lives in its own [`State`] `extra` payload.
//!
//! This module is the crate's *generic core*: [`ids`] holds the index-based
//! identifiers and [`incidence`] the derived adjacency/incidence views. The
//! frontend-specific kinds live in [`pt`](crate::pt), [`timed`](crate::timed),
//! and [`cvn`](crate::cvn).

pub mod ids;
pub mod incidence;

use serde::{Deserialize, Serialize};

pub use ids::{PlaceId, TransitionId};
pub use incidence::{Incidence, IncidenceMatrix};

/// A place node: fixed `id` + `name`, plus a domain-specific `kind`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place<K = ()> {
    pub id: PlaceId,
    pub name: String,
    pub kind: K,
}

impl<K> Place<K> {
    pub fn new(id: PlaceId, name: impl Into<String>, kind: K) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
        }
    }
}

/// A transition node: fixed `id` + `name`, plus a domain-specific `kind`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transition<K = ()> {
    pub id: TransitionId,
    pub name: String,
    pub kind: K,
}

impl<K> Transition<K> {
    pub fn new(id: TransitionId, name: impl Into<String>, kind: K) -> Self {
        Self {
            id,
            name: name.into(),
            kind,
        }
    }
}

/// Arc direction. `Read` does not consume, `Inhibitor` blocks when the place
/// holds enough tokens, `Reset` empties the place on firing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArcDir {
    Input,
    Output,
    Read,
    Inhibitor,
    Reset,
}

/// An arc: fixed endpoints + direction + weight, plus a domain-specific `kind`
/// (empty `()` for pure P/T and timed nets; guards/updates for CVN).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Arc<K = ()> {
    pub place: PlaceId,
    pub transition: TransitionId,
    pub direction: ArcDir,
    pub weight: usize,
    pub kind: K,
}

impl<K> Arc<K> {
    pub fn new(
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
        kind: K,
    ) -> Self {
        Self {
            place,
            transition,
            direction,
            weight,
            kind,
        }
    }
}

/// The generic net: places, transitions, and arcs, parameterized by their kind
/// payloads. Pure structure; no marking and no firing semantics live here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Net<PK = (), TK = (), AK = ()> {
    pub places: Vec<Place<PK>>,
    pub transitions: Vec<Transition<TK>>,
    pub arcs: Vec<Arc<AK>>,
}

impl<PK, TK, AK> Default for Net<PK, TK, AK> {
    fn default() -> Self {
        Self::new()
    }
}

impl<PK, TK, AK> Net<PK, TK, AK> {
    pub fn new() -> Self {
        Self {
            places: Vec::new(),
            transitions: Vec::new(),
            arcs: Vec::new(),
        }
    }

    pub fn num_places(&self) -> usize {
        self.places.len()
    }

    pub fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    pub fn place_ids(&self) -> impl Iterator<Item = PlaceId> + '_ {
        self.places.iter().map(|p| p.id)
    }

    pub fn transition_ids(&self) -> impl Iterator<Item = TransitionId> + '_ {
        self.transitions.iter().map(|t| t.id)
    }

    pub fn places_enumerated(&self) -> impl Iterator<Item = (PlaceId, &Place<PK>)> {
        self.places.iter().enumerate().map(|(i, p)| (PlaceId(i), p))
    }

    pub fn transitions_enumerated(&self) -> impl Iterator<Item = (TransitionId, &Transition<TK>)> {
        self.transitions
            .iter()
            .enumerate()
            .map(|(i, t)| (TransitionId(i), t))
    }

    // ── Nodes ──

    pub fn place(&self, id: PlaceId) -> Option<&Place<PK>> {
        self.places.get(id.index())
    }

    pub fn transition(&self, id: TransitionId) -> Option<&Transition<TK>> {
        self.transitions.get(id.index())
    }

    pub fn add_place(&mut self, name: impl Into<String>, kind: PK) -> PlaceId {
        let id = PlaceId(self.places.len());
        self.places.push(Place::new(id, name, kind));
        id
    }

    pub fn add_transition(&mut self, name: impl Into<String>, kind: TK) -> TransitionId {
        let id = TransitionId(self.transitions.len());
        self.transitions.push(Transition::new(id, name, kind));
        id
    }

    // ── Arcs ──

    pub fn add_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
        kind: AK,
    ) {
        self.arcs
            .push(Arc::new(place, transition, direction, weight, kind));
    }

    pub fn add_input_arc(&mut self, place: PlaceId, transition: TransitionId, kind: AK)
    where
        AK: Default,
    {
        self.add_arc(place, transition, ArcDir::Input, 1, kind);
    }

    pub fn add_output_arc(&mut self, transition: TransitionId, place: PlaceId, kind: AK)
    where
        AK: Default,
    {
        self.add_arc(place, transition, ArcDir::Output, 1, kind);
    }

    /// All arcs incident to `transition`.
    pub fn arcs_for(&self, transition: TransitionId) -> impl Iterator<Item = &Arc<AK>> {
        self.arcs.iter().filter(move |a| a.transition == transition)
    }

    /// Arcs of a given direction incident to `transition`.
    pub fn arcs_of(
        &self,
        transition: TransitionId,
        direction: ArcDir,
    ) -> impl Iterator<Item = &Arc<AK>> {
        self.arcs
            .iter()
            .filter(move |a| a.transition == transition && a.direction == direction)
    }

    /// The preset of `transition` (Input + Read + Inhibitor arcs).
    pub fn pre_arcs(&self, transition: TransitionId) -> Vec<&Arc<AK>> {
        self.arcs
            .iter()
            .filter(|a| {
                a.transition == transition
                    && matches!(
                        a.direction,
                        ArcDir::Input | ArcDir::Read | ArcDir::Inhibitor
                    )
            })
            .collect()
    }

    /// The postset of `transition` (Output arcs).
    pub fn post_arcs(&self, transition: TransitionId) -> Vec<&Arc<AK>> {
        self.arcs
            .iter()
            .filter(|a| a.transition == transition && a.direction == ArcDir::Output)
            .collect()
    }

    /// Aggregated adjacency snapshot (preset/postset plus read/inhibitor/reset).
    ///
    /// Rebuild after mutating `arcs`. See [`Incidence`] for what is and is not
    /// visible to the ordinary incidence matrix (CVN guards, colors, …).
    pub fn incidence(&self) -> Incidence {
        Incidence::of(self)
    }

    /// Ordinary token-flow matrix `C[p, t] = w_post − w_pre`.
    pub fn incidence_matrix(&self) -> IncidenceMatrix {
        self.incidence().matrix()
    }
}

/// A dense marking: index = place id, value = token count (`usize`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
pub struct Marking(pub Vec<usize>);

impl Marking {
    pub fn new(counts: Vec<usize>) -> Self {
        Self(counts)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn tokens(&self, place: PlaceId) -> usize {
        self.0.get(place.index()).copied().unwrap_or(0)
    }

    pub fn set(&mut self, place: PlaceId, count: usize) -> bool {
        let Some(slot) = self.0.get_mut(place.index()) else {
            return false;
        };
        *slot = count;
        true
    }

    pub fn iter(&self) -> impl Iterator<Item = (PlaceId, usize)> + '_ {
        self.0.iter().enumerate().map(|(i, &c)| (PlaceId(i), c))
    }

    pub fn iter_nonzero(&self) -> impl Iterator<Item = (PlaceId, usize)> + '_ {
        self.iter().filter(|(_, c)| *c > 0)
    }
}

/// A runtime state: the common marking plus a per-net `extra` payload (variable
/// store for CVN, clock zone for timed nets, `()` for plain P/T).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct State<E = ()> {
    pub marking: Marking,
    pub extra: E,
}

impl<E> State<E> {
    pub fn new(marking: Marking, extra: E) -> Self {
        Self { marking, extra }
    }
}

impl std::ops::Index<usize> for Marking {
    type Output = usize;

    fn index(&self, index: usize) -> &usize {
        &self.0[index]
    }
}

impl std::ops::Index<PlaceId> for Marking {
    type Output = usize;

    fn index(&self, place: PlaceId) -> &usize {
        &self.0[place.index()]
    }
}
