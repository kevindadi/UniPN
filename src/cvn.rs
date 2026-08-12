//! The CVN (Concurrency Verification Net) backend — ConcPlanVerify's lowering
//! target. Guards live on input arcs, variable updates on output arcs, and the
//! variable store is the net's `State` extra payload.
//!
//! The CVN's core analysis (deadlock / dead-transition / conflict detection and
//! DOT export) also lives here; ConcPlanVerify keeps only its translator,
//! repair, and goal checking (which depend on ConcIR).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::analysis::{Counterexample, FiringStep, NetLike, PropertyViolation, ReachabilityGraph};
use crate::expr::{BoolExpr, ConcreteVal, Val, VarUpdate, eval_expr, eval_guard};
use crate::ids::{PlaceId, TransitionId};
use crate::model::{ControlSub, PlaceKind, ResourceType, TransitionKind};
use crate::net::{ArcDir, Marking, Net, State};

/// The per-arc payload: guards (input arcs) and updates (output arcs).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CvnArcKind {
    Plain,
    Guard(BoolExpr),
    Update(VarUpdate),
}

/// A CVN transition: the kind annotation plus source-attribution metadata used
/// by the repair layer (scope = source function, anchors = ConcIR sids, family
/// = disjunctive OR group).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CvnTransition {
    pub kind: TransitionKind,
    pub scope: Option<String>,
    pub anchors: Vec<String>,
    pub family: Option<String>,
}

/// Ordered variable store.
pub type VarStore = BTreeMap<String, Val>;

/// The CVN state extra: the variable store plus bounded-Int domains (an update
/// leaving a domain disables the transition, keeping the state space finite).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CvnExtra {
    pub vars: VarStore,
    pub domains: BTreeMap<String, (i64, i64)>,
}

/// The CVN net.
pub type CvnNet = Net<PlaceKind, CvnTransition, CvnArcKind>;

/// The CVN state: marking + variable store + bounded-Int domains.
pub type CvnState = State<CvnExtra>;

impl CvnNet {
    /// Resource capacity derived from a place kind (Mutex=1, RwLock=max_readers,
    /// Semaphore=count; control places and channels are unbounded).
    pub fn capacity_of(&self, place: PlaceId) -> Option<usize> {
        match &self.place(place)?.kind {
            PlaceKind::Resource(ResourceType::Mutex) => Some(1),
            PlaceKind::Resource(ResourceType::RwLock { max_readers }) => {
                Some(*max_readers as usize)
            }
            PlaceKind::Resource(ResourceType::Semaphore { count }) => Some(*count as usize),
            _ => None,
        }
    }

    /// Whether `place` is a control-flow place.
    pub fn is_control_flow(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Control(_))
        )
    }

    /// Whether `place` is a resource place.
    pub fn is_resource(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Resource(_))
        )
    }

    /// Whether `place` is a thread terminal (used by deadlock classification).
    pub fn is_thread_terminal(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Control(
                ControlSub::ThreadEnd | ControlSub::FunctionEnd
            ))
        )
    }

    /// Whether `place` is a condvar wait point (signal-loss classification).
    pub fn is_wait_point(&self, place: PlaceId) -> bool {
        matches!(
            self.place(place).map(|p| &p.kind),
            Some(PlaceKind::Control(ControlSub::WaitPoint))
        )
    }

    pub fn place_label(&self, place: PlaceId) -> String {
        self.place(place)
            .map_or_else(|| format!("p{}", place.index()), |p| p.name.clone())
    }

    pub fn transition_label(&self, transition: TransitionId) -> String {
        self.transition(transition)
            .map_or_else(|| format!("t{}", transition.index()), |t| t.name.clone())
    }

    fn is_enabled(&self, state: &CvnState, transition: TransitionId) -> bool {
        let mut required: Vec<(PlaceId, usize)> = Vec::new();
        for arc in self.arcs_for(transition) {
            match arc.direction {
                ArcDir::Input => {
                    if let Some((_, total)) = required.iter_mut().find(|(p, _)| *p == arc.place) {
                        *total = total.checked_add(arc.weight).unwrap_or(usize::MAX);
                    } else {
                        required.push((arc.place, arc.weight));
                    }
                    if let CvnArcKind::Guard(guard) = &arc.kind
                        && !eval_guard(guard, &state.extra.vars).is_not_false()
                    {
                        return false;
                    }
                }
                ArcDir::Read => {
                    if state.marking.tokens(arc.place) < arc.weight {
                        return false;
                    }
                }
                ArcDir::Inhibitor => {
                    if state.marking.tokens(arc.place) >= arc.weight {
                        return false;
                    }
                }
                ArcDir::Output | ArcDir::Reset => {}
            }
        }
        if required
            .iter()
            .any(|(place, count)| state.marking.tokens(*place) < *count)
        {
            return false;
        }

        // Bounded Int domains: an update leaving the domain disables the
        // transition (decidability).
        for arc in self.arcs_of(transition, ArcDir::Output) {
            if let CvnArcKind::Update(update) = &arc.kind {
                for (var, expr) in update {
                    if let Some((lo, hi)) = state.extra.domains.get(var)
                        && let Val::Concrete(ConcreteVal::Int(v)) = eval_expr(expr, &state.extra.vars)
                        && (v < *lo || v > *hi)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

impl NetLike for CvnNet {
    type State = CvnState;

    fn num_places(&self) -> usize {
        self.places.len()
    }

    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    fn enabled(&self, state: &Self::State) -> Vec<TransitionId> {
        self.transitions
            .iter()
            .filter(|t| self.is_enabled(state, t.id))
            .map(|t| t.id)
            .collect()
    }

    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        if !self.is_enabled(state, transition) {
            return None;
        }

        let mut next = state.clone();

        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = next.marking.tokens(arc.place);
            next.marking
                .set(arc.place, current.checked_sub(arc.weight)?);
        }

        for arc in self.arcs_of(transition, ArcDir::Output) {
            let current = next.marking.tokens(arc.place);
            let produced = current.checked_add(arc.weight)?;
            if let Some(cap) = self.capacity_of(arc.place)
                && produced > cap
            {
                return None;
            }
            next.marking.set(arc.place, produced);

            if let CvnArcKind::Update(update) = &arc.kind {
                for (var, expr) in update {
                    next.extra
                        .vars
                        .insert(var.clone(), eval_expr(expr, &state.extra.vars));
                }
            }
        }

        for arc in self.arcs_of(transition, ArcDir::Reset) {
            next.marking.set(arc.place, 0);
        }

        Some(next)
    }
}

// ── CVN core analysis ─────────────────────────────────────────────────────

/// Decide whether a state with no enabled transitions is a deadlock: at least
/// one control-flow token sits on a non-terminal, non-resource place.
pub fn is_deadlock(net: &CvnNet, state: &CvnState) -> bool {
    state
        .marking
        .iter_nonzero()
        .any(|(p, _)| !net.is_resource(p) && !net.is_thread_terminal(p))
}

/// The blocked (control-flow, non-terminal) places in a deadlock state.
pub fn blocked_places(net: &CvnNet, state: &CvnState) -> Vec<PlaceId> {
    state
        .marking
        .iter_nonzero()
        .filter(|(p, _)| !net.is_resource(*p) && !net.is_thread_terminal(*p))
        .map(|(p, _)| p)
        .collect()
}

fn trace_of(net: &CvnNet, graph: &ReachabilityGraph<CvnState>, target: usize) -> Vec<FiringStep> {
    graph
        .trace_to(target)
        .into_iter()
        .map(|t| FiringStep {
            transition: t,
            anchors: net
                .transition(t)
                .map_or_else(Vec::new, |tr| tr.kind.anchors.clone()),
        })
        .collect()
}

/// Classify the graph's blocked states into deadlock counterexamples.
pub fn find_deadlocks(
    net: &CvnNet,
    graph: &ReachabilityGraph<CvnState>,
) -> Vec<Counterexample<CvnState>> {
    graph
        .blocked
        .iter()
        .filter(|&&i| is_deadlock(net, &graph.states[i]))
        .map(|&i| Counterexample {
            kind: PropertyViolation::Deadlock,
            trace: trace_of(net, graph, i),
            final_state: graph.states[i].clone(),
        })
        .collect()
}

/// Find transitions that never fire behaviorally (or whole disjunctive families
/// that are dead).
pub fn find_dead_transitions(
    net: &CvnNet,
    graph: &ReachabilityGraph<CvnState>,
) -> Vec<Counterexample<CvnState>> {
    let fired = graph.fired_transitions();

    let mut live_families: HashSet<&str> = HashSet::new();
    let mut all: Vec<TransitionId> = net.transition_ids().collect();
    for t in &all {
        if fired.contains(t)
            && let Some(f) = net.transition(*t).and_then(|tr| tr.kind.family.as_deref())
        {
            live_families.insert(f);
        }
    }
    all.sort_by_key(|t| t.index());

    let initial = graph.states[graph.initial].clone();
    let mut reported_families: HashSet<&str> = HashSet::new();
    let mut dead = Vec::new();

    for t in all {
        if fired.contains(&t) {
            continue;
        }
        if let Some(f) = net.transition(t).and_then(|tr| tr.kind.family.as_deref()) {
            if live_families.contains(f) {
                continue;
            }
            if !reported_families.insert(f) {
                continue;
            }
        }
        dead.push(Counterexample {
            kind: PropertyViolation::DeadTransition {
                transition: t,
                anchors: net
                    .transition(t)
                    .map_or_else(Vec::new, |tr| tr.kind.anchors.clone()),
            },
            trace: Vec::new(),
            final_state: initial.clone(),
        });
    }

    dead.sort_by_key(|cx| match &cx.kind {
        PropertyViolation::DeadTransition { transition, .. } => transition.index(),
        _ => 0,
    });
    dead
}

/// Transition pairs sharing an input place (potential races/conflicts).
pub fn conflict_sets(net: &CvnNet) -> Vec<(TransitionId, TransitionId)> {
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
                let a = consumers[i].min(consumers[j]);
                let b = consumers[i].max(consumers[j]);
                pairs.insert((a, b));
            }
        }
    }
    pairs.into_iter().collect()
}

/// Export the net as Graphviz DOT.
pub fn to_dot(net: &CvnNet) -> String {
    let mut out = String::from("digraph PetriNet {\n  rankdir=LR;\n");

    for p in net.place_ids() {
        let place = net.place(p).unwrap();
        let tag = match &place.kind {
            PlaceKind::Resource(ResourceType::Mutex) => "mutex",
            PlaceKind::Resource(ResourceType::RwLock { .. }) => "rwlock",
            PlaceKind::Resource(ResourceType::Semaphore { .. }) => "sem",
            PlaceKind::Resource(ResourceType::Channel) => "ch",
            PlaceKind::Resource(ResourceType::Condvar) => "cv",
            PlaceKind::Control(ControlSub::ThreadEnd) => "end",
            PlaceKind::Control(ControlSub::WaitPoint) => "wait",
            _ => "",
        };
        out.push_str(&format!(
            "  p{} [label=\"{}\\n{}\"];\n",
            p.index(),
            escape_dot(&place.name),
            escape_dot(tag)
        ));
    }

    for t in net.transition_ids() {
        let tr = net.transition(t).unwrap();
        out.push_str(&format!(
            "  t{} [label=\"{}\\n{:?}\", shape=box];\n",
            t.index(),
            escape_dot(&tr.name),
            tr.kind.kind
        ));
    }

    for arc in &net.arcs {
        match arc.direction {
            ArcDir::Input => out.push_str(&format!(
                "  p{} -> t{};\n",
                arc.place.index(),
                arc.transition.index()
            )),
            ArcDir::Output => out.push_str(&format!(
                "  t{} -> p{};\n",
                arc.transition.index(),
                arc.place.index()
            )),
            ArcDir::Inhibitor => out.push_str(&format!(
                "  p{} -> t{} [style=dotted];\n",
                arc.place.index(),
                arc.transition.index()
            )),
            _ => {}
        }
    }

    out.push_str("}\n");
    out
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

// ── Builder ───────────────────────────────────────────────────────────────

/// Chain-style CVN builder: produces the net plus its initial state.
#[derive(Default)]
pub struct CvnBuilder {
    net: CvnNet,
    marking: Vec<usize>,
    vars: VarStore,
    domains: BTreeMap<String, (i64, i64)>,
}

impl CvnBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_place(&mut self, name: impl Into<String>, kind: PlaceKind) -> PlaceId {
        let id = self.net.add_place(name, kind);
        self.marking.push(0);
        id
    }

    pub fn add_transition(
        &mut self,
        name: impl Into<String>,
        kind: TransitionKind,
    ) -> TransitionId {
        self.net.add_transition(
            name,
            CvnTransition {
                kind,
                scope: None,
                anchors: Vec::new(),
                family: None,
            },
        )
    }

    pub fn set_anchor(&mut self, transition: TransitionId, anchor: impl Into<String>) -> &mut Self {
        if let Some(t) = self.net.transitions.get_mut(transition.index()) {
            t.kind.anchors.push(anchor.into());
        }
        self
    }

    pub fn set_scope(&mut self, transition: TransitionId, scope: impl Into<String>) -> &mut Self {
        if let Some(t) = self.net.transitions.get_mut(transition.index()) {
            t.kind.scope = Some(scope.into());
        }
        self
    }

    pub fn set_family(&mut self, transition: TransitionId, family: impl Into<String>) -> &mut Self {
        if let Some(t) = self.net.transitions.get_mut(transition.index()) {
            t.kind.family = Some(family.into());
        }
        self
    }

    pub fn add_input_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        weight: usize,
        guard: BoolExpr,
    ) -> &mut Self {
        let kind = if guard == BoolExpr::True {
            CvnArcKind::Plain
        } else {
            CvnArcKind::Guard(guard)
        };
        self.net
            .add_arc(place, transition, ArcDir::Input, weight, kind);
        self
    }

    pub fn add_output_arc(
        &mut self,
        transition: TransitionId,
        place: PlaceId,
        weight: usize,
        update: Option<VarUpdate>,
    ) -> &mut Self {
        let kind = update.map_or(CvnArcKind::Plain, CvnArcKind::Update);
        self.net
            .add_arc(place, transition, ArcDir::Output, weight, kind);
        self
    }

    pub fn set_initial_tokens(&mut self, place: PlaceId, count: usize) -> &mut Self {
        if let Some(slot) = self.marking.get_mut(place.index()) {
            *slot = count;
        }
        self
    }

    pub fn add_variable(&mut self, name: impl Into<String>, initial: Val) -> &mut Self {
        self.vars.insert(name.into(), initial);
        self
    }

    /// Declare a bounded Int domain (an update leaving the domain disables the
    /// transition, keeping the state space finite).
    pub fn set_variable_domain(&mut self, name: impl Into<String>, lo: i64, hi: i64) -> &mut Self {
        self.domains.insert(name.into(), (lo, hi));
        self
    }

    pub fn build(self) -> (CvnNet, CvnState) {
        (
            self.net,
            State::new(
                Marking::new(self.marking),
                CvnExtra {
                    vars: self.vars,
                    domains: self.domains,
                },
            ),
        )
    }
}
