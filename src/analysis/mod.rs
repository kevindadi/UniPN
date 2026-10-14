//! Net-level analysis: reachability exploration and deadlock classification.
//!
//! The algorithms are domain-neutral and depend only on the [`NetLike`]
//! firing contract. The explorer reports which states are *blocked* (no
//! enabled transitions); whether a blocked state is a *deadlock* is decided by
//! the caller via [`find_deadlocks`], and a caller whose place kind implements
//! [`PlaceRole`](crate::net::PlaceRole) already has that predicate in
//! [`Net::is_deadlock`](crate::net::Net::is_deadlock).
//!
//! [`conflict_sets`] and [`unfired_transitions`] are here rather than under a
//! frontend because their answers do not depend on any kind: sharing an input
//! place and never appearing on an edge are properties of the structure and of
//! the explored graph.
//!
//! [`labels`] holds the checks that read a net only through its labels —
//! partitions, per-label token totals, label-level dead code, and reachability
//! queries phrased over labels — for any net and any labeling.
//!
//! [`invariants`] computes the minimal-support P- and T-semiflows of any net's
//! incidence matrix (Farkas), and says which P-semiflows are invariants of a
//! net's behavior given its reset arcs and capacities.
//!
//! P/T analysis lives in [`pt`] and CVN analysis in [`cvn`]. Timed
//! (DBM/state-class) analysis is `analysis::timed` when the `timed` feature is
//! enabled.

pub mod cvn;
mod hash;
pub mod invariants;
pub mod labels;
pub mod pt;
#[cfg(feature = "timed")]
pub mod timed;
#[cfg(feature = "timed")]
pub mod tpn;

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use hash::FxHashMap;
use std::hash::Hash;

use crate::net::{ArcDir, Net, PlaceId, TransitionId};

/// The firing contract a net must satisfy to be analyzed.
///
/// Each net (P/T, timed, CVN) supplies its own `enabled`/`fire` and its own
/// `State` (marking + per-net extra data). The model itself is not abstracted
/// here — only this minimal execution surface.
///
/// Frontends do not implement this directly: implement [`Semantics`] and the
/// impl below derives `NetLike` for any [`Net`].
pub trait NetLike {
    type State: Clone + Eq + Hash;

    fn num_places(&self) -> usize;
    fn num_transitions(&self) -> usize;

    /// Transitions enabled under `state`.
    fn enabled(&self, state: &Self::State) -> Vec<TransitionId>;

    /// Fire `transition` from `state`; `None` if it is not enabled.
    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State>;

    /// Fire a `transition` that [`enabled`](NetLike::enabled) just returned for
    /// this same `state`, without deciding enabling a second time.
    ///
    /// This is what an explorer calls in its inner loop: it already has the
    /// enabled set, and [`fire`](NetLike::fire) would re-run the whole enabling
    /// check (guards included, for the CVN) for every successor. `None` still
    /// means the firing was rejected — the CVN refuses one that would overfill
    /// a resource place. The result for a transition that is *not* enabled is
    /// unspecified; debug builds of the derived impl assert against it.
    ///
    /// The default is [`fire`](NetLike::fire), so a hand-written `NetLike`
    /// stays correct without implementing this.
    fn successor(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        self.fire(state, transition)
    }
}

/// A net's firing semantics: the two decisions a frontend makes for itself.
///
/// Everything else in [`NetLike`] — the place and transition counts, and the
/// scan that turns `can_fire` into an enabled set — is identical for all three
/// frontends and is supplied by the blanket impl below. The structural half of
/// enabling is shared too, via
/// [`Net::structurally_enabled`](crate::net::Net::structurally_enabled) and the
/// other primitives in [`net::firing`](crate::net::firing); an implementation
/// only adds what is specific to it (CVN guards and variable domains) and picks
/// how a capacity violation behaves.
pub trait Semantics {
    type State: Clone + Eq + Hash;

    /// Whether `transition` may fire from `state`.
    fn can_fire(&self, state: &Self::State, transition: TransitionId) -> bool;

    /// Fire a transition that [`Semantics::can_fire`] has already accepted.
    ///
    /// Returns `None` when a firing is rejected for a reason enabling cannot
    /// see — the CVN refuses to push a resource place past its capacity.
    fn fire_enabled(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State>;
}

impl<PK, TK, AK> NetLike for Net<PK, TK, AK>
where
    Self: Semantics,
{
    type State = <Self as Semantics>::State;

    fn num_places(&self) -> usize {
        self.places.len()
    }

    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    fn enabled(&self, state: &Self::State) -> Vec<TransitionId> {
        self.transitions
            .iter()
            .filter(|t| self.can_fire(state, t.id))
            .map(|t| t.id)
            .collect()
    }

    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        if !self.can_fire(state, transition) {
            return None;
        }
        self.fire_enabled(state, transition)
    }

    fn successor(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        debug_assert!(
            self.can_fire(state, transition),
            "successor() called with a transition that is not enabled"
        );
        self.fire_enabled(state, transition)
    }
}

/// Search strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchStrategy {
    #[default]
    Bfs,
    Dfs,
}

/// Exploration configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalysisConfig {
    pub strategy: SearchStrategy,
    pub max_states: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            strategy: SearchStrategy::Bfs,
            max_states: 100_000,
        }
    }
}

/// A reachability graph over a net's states.
#[derive(Clone, Debug)]
pub struct ReachabilityGraph<S> {
    pub states: Vec<S>,
    pub edges: Vec<(usize, usize, TransitionId)>,
    pub initial: usize,
    /// Indices of states with no enabled transitions (candidate deadlocks).
    pub blocked: Vec<usize>,
    pub truncated: bool,
    /// `preds[i]` is the edge that discovered state `i`; `None` for the initial
    /// state, which is what makes [`ReachabilityGraph::trace_to`] stop.
    preds: Vec<Option<(usize, TransitionId)>>,
}

impl<S> ReachabilityGraph<S> {
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// The set of transitions that fired on any edge of the graph (used for
    /// dead-transition detection).
    pub fn fired_transitions(&self) -> HashSet<TransitionId> {
        self.edges.iter().map(|(_, _, t)| *t).collect()
    }

    /// Reconstruct the firing sequence that reaches `target` from the initial
    /// state.
    pub fn trace_to(&self, target: usize) -> Vec<TransitionId> {
        let mut path = Vec::new();
        let mut current = target;
        while let Some(&Some((parent, t))) = self.preds.get(current) {
            path.push(t);
            current = parent;
        }
        path.reverse();
        path
    }
}

/// Explore the whole reachable state space from `initial`.
pub fn explore<N: NetLike>(
    net: &N,
    initial: N::State,
    config: &AnalysisConfig,
) -> ReachabilityGraph<N::State> {
    let mut explorer: Explorer<N> = Explorer::new(config.max_states);
    let (init_idx, _) = explorer.insert(initial).unwrap();
    let mut pending = VecDeque::from([init_idx]);

    while let Some(idx) = match config.strategy {
        SearchStrategy::Bfs => pending.pop_front(),
        SearchStrategy::Dfs => pending.pop_back(),
    } {
        let state = explorer.states[idx].clone();
        let enabled = net.enabled(&state);
        if enabled.is_empty() {
            explorer.blocked.push(idx);
            continue;
        }
        for transition in enabled {
            let Some(next) = net.successor(&state, transition) else {
                continue;
            };
            let Some((target, is_new)) = explorer.insert(next) else {
                continue;
            };
            explorer.record_edge(idx, target, transition, is_new);
            if is_new {
                pending.push_back(target);
            }
        }
    }

    explorer.finish()
}

/// Classify the blocked states of a graph into deadlock state indices using the
/// caller-supplied predicate.
///
/// For a net whose place kind implements [`PlaceRole`](crate::net::PlaceRole)
/// the predicate is already written:
/// [`Net::is_deadlock`](crate::net::Net::is_deadlock).
pub fn find_deadlocks<S>(
    graph: &ReachabilityGraph<S>,
    is_deadlock: impl Fn(&S) -> bool,
) -> Vec<usize> {
    graph
        .blocked
        .iter()
        .copied()
        .filter(|&i| is_deadlock(&graph.states[i]))
        .collect()
}

/// Transition pairs that share an input place, and so compete for it.
///
/// Purely structural — no marking, no kinds — which is why it serves both the
/// CVN's conflict reporting and ConcBugDect's race candidates.
pub fn conflict_sets<PK, TK, AK>(net: &Net<PK, TK, AK>) -> Vec<(TransitionId, TransitionId)> {
    let mut by_place: HashMap<PlaceId, Vec<TransitionId>> = HashMap::new();
    for t in net.transition_ids() {
        for arc in net.arcs_of(t, ArcDir::Input) {
            by_place.entry(arc.place).or_default().push(t);
        }
    }

    let mut pairs: BTreeSet<(TransitionId, TransitionId)> = BTreeSet::new();
    for consumers in by_place.values() {
        for i in 0..consumers.len() {
            for j in (i + 1)..consumers.len() {
                pairs.insert((
                    consumers[i].min(consumers[j]),
                    consumers[i].max(consumers[j]),
                ));
            }
        }
    }
    pairs.into_iter().collect()
}

/// The transitions that never fired on any edge of `graph`, in id order.
///
/// Behavioral dead code: for the CVN a plan step that can never run, for P/T an
/// unreachable branch. A frontend that groups transitions (the CVN's
/// disjunctive families) folds the result afterwards — see
/// [`cvn::find_dead_transitions`].
pub fn unfired_transitions<PK, TK, AK, S>(
    net: &Net<PK, TK, AK>,
    graph: &ReachabilityGraph<S>,
) -> Vec<TransitionId> {
    let fired = graph.fired_transitions();
    let mut dead: Vec<TransitionId> = net
        .transition_ids()
        .filter(|t| !fired.contains(t))
        .collect();
    dead.sort_by_key(|t| t.index());
    dead
}

/// A single firing step in a counterexample trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiringStep {
    pub transition: TransitionId,
    /// Anchoring back to the source (ConcIR sid / source line).
    pub anchors: Vec<String>,
}

/// The kind of property violation a counterexample demonstrates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertyViolation {
    Deadlock,
    DeadTransition {
        transition: TransitionId,
        anchors: Vec<String>,
    },
    GoalUnmet {
        goal: String,
    },
}

/// A counterexample: a firing sequence + final state + violation kind.
#[derive(Clone, Debug)]
pub struct Counterexample<S> {
    pub kind: PropertyViolation,
    pub trace: Vec<FiringStep>,
    pub final_state: S,
}

struct Explorer<N: NetLike> {
    max_states: usize,
    states: Vec<N::State>,
    seen: FxHashMap<N::State, usize>,
    edges: Vec<(usize, usize, TransitionId)>,
    preds: Vec<Option<(usize, TransitionId)>>,
    blocked: Vec<usize>,
    truncated: bool,
}

impl<N: NetLike> Explorer<N> {
    fn new(max_states: usize) -> Self {
        Self {
            max_states,
            states: Vec::new(),
            seen: FxHashMap::default(),
            edges: Vec::new(),
            preds: Vec::new(),
            blocked: Vec::new(),
            truncated: false,
        }
    }

    /// Look `state` up, adding it if it is new. Most successors are states
    /// already seen, so the lookup comes first and only a new state is cloned
    /// into the table.
    fn insert(&mut self, state: N::State) -> Option<(usize, bool)> {
        if let Some(&idx) = self.seen.get(&state) {
            return Some((idx, false));
        }
        if self.states.len() >= self.max_states {
            self.truncated = true;
            return None;
        }
        let idx = self.states.len();
        self.seen.insert(state.clone(), idx);
        self.states.push(state);
        self.preds.push(None);
        Some((idx, true))
    }

    /// Record an edge, and make it `dst`'s predecessor if `dst` was just
    /// discovered through it. Only a newly discovered state gets one: an edge
    /// back into the initial state must not give the root a parent, or
    /// `trace_to` would walk that cycle forever.
    fn record_edge(&mut self, src: usize, dst: usize, t: TransitionId, is_new: bool) {
        self.edges.push((src, dst, t));
        if is_new {
            self.preds[dst] = Some((src, t));
        }
    }

    fn finish(self) -> ReachabilityGraph<N::State> {
        ReachabilityGraph {
            states: self.states,
            edges: self.edges,
            initial: 0,
            blocked: self.blocked,
            truncated: self.truncated,
            preds: self.preds,
        }
    }
}
