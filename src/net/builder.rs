//! Generic construction: a net plus the initial state being accumulated.
//!
//! [`Net`] deliberately holds no marking, so everything that builds a net has
//! to carry the initial token counts alongside it and keep the two index spaces
//! aligned. That bookkeeping is identical for every frontend and lives here.
//!
//! A frontend does not wrap this type — it *is* this type, instantiated:
//! [`PtBuilder`](crate::pt::PtBuilder) and [`CvnBuilder`](crate::cvn::CvnBuilder)
//! are aliases that add their own methods (P/T weight accumulation, CVN
//! variable declarations) in their own `impl` block. The `E` parameter mirrors
//! [`State`]'s `extra`: whatever a frontend accumulates besides the marking.

use crate::net::{ArcDir, Marking, Net, PlaceId, State, TransitionId};

/// A net and its initial marking under construction, plus a frontend-specific
/// `extra` payload (`()` when the marking is the whole initial state).
#[derive(Clone, Debug, PartialEq)]
pub struct NetBuilder<PK = (), TK = (), AK = (), E = ()> {
    net: Net<PK, TK, AK>,
    marking: Vec<usize>,
    extra: E,
}

impl<PK, TK, AK, E: Default> Default for NetBuilder<PK, TK, AK, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<PK, TK, AK, E: Default> NetBuilder<PK, TK, AK, E> {
    pub fn new() -> Self {
        Self {
            net: Net::new(),
            marking: Vec::new(),
            extra: E::default(),
        }
    }
}

impl<PK, TK, AK, E> NetBuilder<PK, TK, AK, E> {
    /// Add an empty place. Keeps the marking aligned with the place ids, which
    /// is the one invariant this type exists to maintain.
    pub fn add_place(&mut self, name: impl Into<String>, kind: PK) -> PlaceId {
        let id = self.net.add_place(name, kind);
        self.marking.push(0);
        id
    }

    /// Add a place already holding `tokens`.
    pub fn add_marked_place(
        &mut self,
        name: impl Into<String>,
        kind: PK,
        tokens: usize,
    ) -> PlaceId {
        let id = self.add_place(name, kind);
        self.marking[id.index()] = tokens;
        id
    }

    pub fn add_transition(&mut self, name: impl Into<String>, kind: TK) -> TransitionId {
        self.net.add_transition(name, kind)
    }

    pub fn add_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
        kind: AK,
    ) {
        self.net.add_arc(place, transition, direction, weight, kind);
    }

    pub fn set_initial_tokens(&mut self, place: PlaceId, tokens: usize) {
        if let Some(slot) = self.marking.get_mut(place.index()) {
            *slot = tokens;
        }
    }

    pub fn initial_marking(&self) -> Marking {
        Marking::new(self.marking.clone())
    }

    pub fn num_places(&self) -> usize {
        self.net.num_places()
    }

    pub fn num_transitions(&self) -> usize {
        self.net.num_transitions()
    }

    /// The net being built (read-only view).
    pub fn net(&self) -> &Net<PK, TK, AK> {
        &self.net
    }

    /// The net being built, mutably — for the arc-rewriting a frontend builder
    /// needs (P/T weight accumulation).
    pub fn net_mut(&mut self) -> &mut Net<PK, TK, AK> {
        &mut self.net
    }

    pub fn extra(&self) -> &E {
        &self.extra
    }

    pub fn extra_mut(&mut self) -> &mut E {
        &mut self.extra
    }

    /// Mutable access to a place's kind (post-creation mutation).
    pub fn place_kind_mut(&mut self, place: PlaceId) -> Option<&mut PK> {
        self.net.places.get_mut(place.index()).map(|p| &mut p.kind)
    }

    /// Mutable access to a transition's kind (post-creation mutation).
    pub fn transition_kind_mut(&mut self, transition: TransitionId) -> Option<&mut TK> {
        self.net
            .transitions
            .get_mut(transition.index())
            .map(|t| &mut t.kind)
    }

    /// The finished net, its initial marking, and the accumulated extra.
    ///
    /// Each frontend's own `build` wraps this into the shape it promises —
    /// `(PtNet, Marking)` or `(CvnNet, CvnState)`.
    pub fn into_parts(self) -> (Net<PK, TK, AK>, Marking, E) {
        (self.net, Marking::new(self.marking), self.extra)
    }

    /// The finished net and its initial [`State`].
    pub fn into_net_and_state(self) -> (Net<PK, TK, AK>, State<E>) {
        let (net, marking, extra) = self.into_parts();
        (net, State::new(marking, extra))
    }
}
