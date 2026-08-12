//! P/T reachability-graph construction (port of ConcBugDect's
//! `analysis/reachability.rs`): snapshots, BFS/DFS exploration, partial-order
//! reduction, and deadlock (blocked-state) reporting.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::Path;

use crate::analysis::NetLike;
use crate::ids::{PlaceId, TransitionId};
use crate::net::{ArcDir, Marking};
use crate::pt::{PlaceType, PtNet, TransitionType};

/// A per-state view of a single place.
#[derive(Debug, Clone)]
pub struct StatePlaceSnapshot {
    pub place: PlaceId,
    pub name: String,
    pub place_type: PlaceType,
    pub span: String,
    pub tokens: usize,
    pub capacity: usize,
}

impl StatePlaceSnapshot {
    fn new(
        place_id: PlaceId,
        name: String,
        place_type: &PlaceType,
        span: &str,
        tokens: usize,
        capacity: usize,
    ) -> Self {
        Self {
            place: place_id,
            name,
            place_type: place_type.clone(),
            span: span.to_string(),
            tokens,
            capacity,
        }
    }
}

/// A token-count change on one place.
#[derive(Debug, Clone)]
pub struct TokenChange {
    pub place: PlaceId,
    pub name: String,
    pub before: usize,
    pub after: usize,
    pub delta: i64,
}

impl TokenChange {
    fn new(place_id: PlaceId, name: String, before: usize, after: usize) -> Option<Self> {
        if before == after {
            return None;
        }
        Some(Self {
            place: place_id,
            name,
            before,
            after,
            delta: after as i64 - before as i64,
        })
    }
}

/// An arc attached to a fired transition, as a visualization slice.
#[derive(Debug, Clone)]
pub struct ArcSnapshot {
    pub place: PlaceId,
    pub name: String,
    pub kind: ArcDir,
    pub weight: usize,
}

/// A transition label on a state node.
#[derive(Debug, Clone)]
pub struct TransitionSummary {
    pub id: TransitionId,
    pub name: String,
    pub transition_type: TransitionType,
}

/// Full marking stored on the node; `places` is a visualization slice only.
#[derive(Debug, Clone)]
pub struct StateNode {
    pub index: usize,
    pub marking: Marking,
    pub places: Vec<StatePlaceSnapshot>,
    pub enabled: Vec<TransitionSummary>,
}

impl StateNode {
    fn new(index: usize, marking: Marking, net: &PtNet, include_zero_tokens: bool) -> Self {
        let mut places = Vec::new();
        for (i, place) in net.places.iter().enumerate() {
            let place_id = PlaceId(i);
            let tokens = marking.tokens(place_id);
            if tokens > 0 || include_zero_tokens {
                places.push(StatePlaceSnapshot::new(
                    place_id,
                    place.name.clone(),
                    &place.kind.place_type,
                    &place.kind.span,
                    tokens,
                    place.kind.capacity.unwrap_or(usize::MAX),
                ));
            }
        }
        Self {
            index,
            marking,
            places,
            enabled: Vec::new(),
        }
    }

    fn update_enabled(&mut self, net: &PtNet, transitions: &[TransitionId]) {
        self.enabled = transitions
            .iter()
            .map(|&id| {
                let transition = net.transition(id).unwrap();
                TransitionSummary {
                    id,
                    name: transition.name.clone(),
                    transition_type: transition.kind.transition_type.clone(),
                }
            })
            .collect();
    }
}

/// An edge of the state graph.
#[derive(Debug, Clone)]
pub struct StateEdge {
    pub transition: TransitionSummary,
    pub changes: Vec<TokenChange>,
    pub arcs: Vec<ArcSnapshot>,
}

impl StateEdge {
    fn new(net: &PtNet, transition_id: TransitionId, before: &Marking, after: &Marking) -> Self {
        let transition = net.transition(transition_id).unwrap();
        let transition = TransitionSummary {
            id: transition_id,
            name: transition.name.clone(),
            transition_type: transition.kind.transition_type.clone(),
        };

        let mut changes = Vec::new();
        for (i, place) in net.places.iter().enumerate() {
            let place_id = PlaceId(i);
            let before_tokens = before.tokens(place_id);
            let after_tokens = after.tokens(place_id);
            if let Some(change) =
                TokenChange::new(place_id, place.name.clone(), before_tokens, after_tokens)
            {
                changes.push(change);
            }
        }

        let mut arcs = Vec::new();
        for arc in net.arcs_for(transition_id) {
            match arc.direction {
                ArcDir::Input | ArcDir::Output | ArcDir::Inhibitor | ArcDir::Reset => {
                    arcs.push(ArcSnapshot {
                        place: arc.place,
                        name: net
                            .place(arc.place)
                            .map(|p| p.name.clone())
                            .unwrap_or_default(),
                        kind: arc.direction,
                        weight: arc.weight,
                    });
                }
                ArcDir::Read => {}
            }
        }

        Self {
            transition,
            changes,
            arcs,
        }
    }
}

/// Failure recorded while expanding the reachability graph.
#[derive(Debug, Clone)]
pub struct TransitionFailure {
    pub source: usize,
    pub transition: TransitionId,
    pub transition_name: String,
    pub reason: String,
}

/// Summary statistics of a state graph.
#[derive(Debug, Clone)]
pub struct StateGraphStats {
    pub state_count: usize,
    pub edge_count: usize,
    pub deadlock_count: usize,
    pub truncated: bool,
}

/// Exploration configuration.
#[derive(Debug, Clone)]
pub struct StateGraphConfig {
    /// Maximum number of states to explore (`None` = unbounded).
    pub state_limit: Option<usize>,
    /// Include zero-token places in per-state snapshots.
    pub include_zero_tokens: bool,
    /// Enable partial-order reduction (POR) to skip redundant interleavings.
    pub use_por: bool,
}

impl Default for StateGraphConfig {
    fn default() -> Self {
        Self {
            state_limit: Some(50_000),
            include_zero_tokens: false,
            use_por: false,
        }
    }
}

/// Return true if two transitions are independent (share no places).
fn transitions_are_independent(net: &PtNet, t1: TransitionId, t2: TransitionId) -> bool {
    if t1 == t2 {
        return false;
    }
    let mut places: HashSet<PlaceId> = HashSet::new();
    for arc in net.arcs_for(t1) {
        places.insert(arc.place);
    }
    for arc in net.arcs_for(t2) {
        if places.contains(&arc.place) {
            return false;
        }
    }
    true
}

/// The reachability graph of a P/T net.
#[derive(Debug)]
pub struct StateGraph {
    pub states: Vec<StateNode>,
    pub edges: Vec<(usize, usize, StateEdge)>,
    pub initial: usize,
    pub deadlocks: HashSet<usize>,
    pub truncated: bool,
    pub failures: Vec<TransitionFailure>,
    pub markings: HashMap<Marking, usize>,
    /// The underlying net (for arc inspection).
    pub net: Option<PtNet>,
}

impl StateGraph {
    pub fn dot(&self) -> String {
        let mut out = String::from("digraph StateGraph {\n  rankdir=LR;\n");
        for node in &self.states {
            let marking_lines: Vec<String> = node
                .places
                .iter()
                .map(|place| format!("{}:{}", place.name, place.tokens))
                .collect();
            let enabled: Vec<String> = node.enabled.iter().map(|t| t.name.clone()).collect();
            let mut label = format!("s{}\\nmarking: {}", node.index, marking_lines.join(", "));
            if !enabled.is_empty() {
                label.push_str(&format!("\\nenabled: {}", enabled.join(", ")));
            }
            out.push_str(&format!(
                "  s{} [label=\"{}\"];\n",
                node.index,
                label.replace('\\', "\\\\").replace('"', "\\\"")
            ));
        }
        for (src, tgt, edge) in &self.edges {
            out.push_str(&format!(
                "  s{} -> s{} [label=\"{}\"];\n",
                src,
                tgt,
                edge.transition.name.replace('"', "\\\"")
            ));
        }
        out.push_str("}\n");
        out
    }

    pub fn write_dot<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let dot = self.dot();
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, dot)
    }

    pub fn from_net(net: &PtNet, initial_marking: Marking) -> Self {
        Self::with_config(net, initial_marking, StateGraphConfig::default())
    }

    pub fn with_config(net: &PtNet, initial_marking: Marking, config: StateGraphConfig) -> Self {
        if config.use_por {
            Self::with_config_por(net, initial_marking, config)
        } else {
            Self::with_config_standard(net, initial_marking, config)
        }
    }

    fn with_config_standard(
        net: &PtNet,
        initial_marking: Marking,
        config: StateGraphConfig,
    ) -> Self {
        let mut states = Vec::new();
        let mut edges = Vec::new();
        let mut markings: HashMap<Marking, usize> = HashMap::new();
        let mut queue: VecDeque<usize> = VecDeque::new();
        let mut deadlocks = HashSet::new();
        let mut failures = Vec::new();
        let mut truncated = false;

        states.push(StateNode::new(
            0,
            initial_marking.clone(),
            net,
            config.include_zero_tokens,
        ));
        markings.insert(initial_marking, 0);
        queue.push_back(0);

        while let Some(state_index) = queue.pop_front() {
            let current_marking = states[state_index].marking.clone();
            let enabled = net.enabled(&current_marking);
            states[state_index].update_enabled(net, &enabled);

            if enabled.is_empty() {
                deadlocks.insert(state_index);
                continue;
            }

            for transition_id in enabled {
                match net.fire(&current_marking, transition_id) {
                    Some(next_marking) => {
                        let target_index = match markings.entry(next_marking.clone()) {
                            std::collections::hash_map::Entry::Occupied(entry) => *entry.get(),
                            std::collections::hash_map::Entry::Vacant(entry) => {
                                if let Some(limit) = config.state_limit
                                    && states.len() >= limit
                                {
                                    truncated = true;
                                    continue;
                                }
                                let index = states.len();
                                states.push(StateNode::new(
                                    index,
                                    next_marking.clone(),
                                    net,
                                    config.include_zero_tokens,
                                ));
                                entry.insert(index);
                                queue.push_back(index);
                                index
                            }
                        };

                        let edge = StateEdge::new(
                            net,
                            transition_id,
                            &current_marking,
                            &states[target_index].marking,
                        );
                        edges.push((state_index, target_index, edge));
                    }
                    None => {
                        failures.push(TransitionFailure {
                            source: state_index,
                            transition: transition_id,
                            transition_name: net
                                .transition(transition_id)
                                .map(|t| t.name.clone())
                                .unwrap_or_default(),
                            reason: "not enabled".to_string(),
                        });
                    }
                }
            }
        }

        Self {
            states,
            edges,
            initial: 0,
            deadlocks,
            truncated,
            failures,
            markings,
            net: Some(net.clone()),
        }
    }

    /// Build the state graph using sleep-set partial-order reduction.
    fn with_config_por(net: &PtNet, initial_marking: Marking, config: StateGraphConfig) -> Self {
        let mut states = Vec::new();
        let mut edges = Vec::new();
        let mut markings: HashMap<Marking, usize> = HashMap::new();
        let mut sleep_sets: HashMap<usize, HashSet<TransitionId>> = HashMap::new();
        let mut queue: VecDeque<(usize, HashSet<TransitionId>)> = VecDeque::new();
        let mut deadlocks = HashSet::new();
        let mut failures = Vec::new();
        let mut truncated = false;

        states.push(StateNode::new(
            0,
            initial_marking.clone(),
            net,
            config.include_zero_tokens,
        ));
        markings.insert(initial_marking, 0);
        queue.push_back((0, HashSet::new()));

        while let Some((state_index, sleep)) = queue.pop_front() {
            let current_marking = states[state_index].marking.clone();
            let enabled: HashSet<TransitionId> =
                net.enabled(&current_marking).into_iter().collect();
            states[state_index].update_enabled(net, &enabled.iter().copied().collect::<Vec<_>>());

            if enabled.is_empty() {
                deadlocks.insert(state_index);
                continue;
            }

            let to_fire: Vec<TransitionId> = enabled.difference(&sleep).copied().collect();

            for transition_id in to_fire {
                match net.fire(&current_marking, transition_id) {
                    Some(next_marking) => {
                        let enabled_next: HashSet<TransitionId> =
                            net.enabled(&next_marking).into_iter().collect();
                        let mut new_sleep = sleep.clone();
                        for &t in &enabled {
                            if t != transition_id
                                && transitions_are_independent(net, transition_id, t)
                            {
                                new_sleep.insert(t);
                            }
                        }
                        new_sleep = new_sleep.intersection(&enabled_next).copied().collect();

                        let target_index = match markings.entry(next_marking.clone()) {
                            std::collections::hash_map::Entry::Occupied(entry) => {
                                let old_ni = *entry.get();
                                let old_sleep =
                                    sleep_sets.get(&old_ni).cloned().unwrap_or_default();
                                let merged_sleep: HashSet<TransitionId> =
                                    old_sleep.intersection(&new_sleep).copied().collect();
                                if merged_sleep != old_sleep {
                                    sleep_sets.insert(old_ni, merged_sleep.clone());
                                    queue.push_back((old_ni, merged_sleep));
                                }
                                old_ni
                            }
                            std::collections::hash_map::Entry::Vacant(entry) => {
                                if let Some(limit) = config.state_limit
                                    && states.len() >= limit
                                {
                                    truncated = true;
                                    continue;
                                }
                                let index = states.len();
                                states.push(StateNode::new(
                                    index,
                                    next_marking.clone(),
                                    net,
                                    config.include_zero_tokens,
                                ));
                                entry.insert(index);
                                sleep_sets.insert(index, new_sleep.clone());
                                queue.push_back((index, new_sleep));
                                index
                            }
                        };

                        let edge = StateEdge::new(
                            net,
                            transition_id,
                            &current_marking,
                            &states[target_index].marking,
                        );
                        edges.push((state_index, target_index, edge));
                    }
                    None => {
                        failures.push(TransitionFailure {
                            source: state_index,
                            transition: transition_id,
                            transition_name: net
                                .transition(transition_id)
                                .map(|t| t.name.clone())
                                .unwrap_or_default(),
                            reason: "not enabled".to_string(),
                        });
                    }
                }
            }
        }

        Self {
            states,
            edges,
            initial: 0,
            deadlocks,
            truncated,
            failures,
            markings,
            net: Some(net.clone()),
        }
    }

    pub fn stats(&self) -> StateGraphStats {
        StateGraphStats {
            state_count: self.states.len(),
            edge_count: self.edges.len(),
            deadlock_count: self.deadlocks.len(),
            truncated: self.truncated,
        }
    }

    pub fn node(&self, index: usize) -> &StateNode {
        &self.states[index]
    }

    pub fn contains_marking(&self, marking: &Marking) -> bool {
        self.markings.contains_key(marking)
    }

    pub fn node_indices(&self) -> std::ops::Range<usize> {
        0..self.states.len()
    }

    pub fn node_count(&self) -> usize {
        self.states.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Outgoing edges of a node, as petgraph-style references.
    pub fn edges(&self, node: usize) -> impl Iterator<Item = EdgeRef<'_>> {
        self.edges.iter().enumerate().filter_map(move |(id, (src, tgt, e))| {
            (*src == node).then_some(EdgeRef {
                id,
                target: *tgt,
                weight: e,
            })
        })
    }

    pub fn edge_weights(&self) -> impl Iterator<Item = &StateEdge> {
        self.edges.iter().map(|(_, _, e)| e)
    }

    pub fn edge_weight(&self, id: usize) -> Option<&StateEdge> {
        self.edges.get(id).map(|(_, _, e)| e)
    }

    /// Get the input resources (places + required tokens) of a transition.
    pub fn get_transition_resources(&self, transition_id: TransitionId) -> Vec<(PlaceId, usize)> {
        match &self.net {
            Some(net) => net
                .arcs_of(transition_id, ArcDir::Input)
                .map(|arc| (arc.place, arc.weight))
                .collect(),
            None => Vec::new(),
        }
    }
}

/// A petgraph-style edge reference over the Vec-based state graph.
pub struct EdgeRef<'a> {
    pub id: usize,
    pub target: usize,
    pub weight: &'a StateEdge,
}

impl EdgeRef<'_> {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn target(&self) -> usize {
        self.target
    }

    pub fn weight(&self) -> &StateEdge {
        self.weight
    }
}
