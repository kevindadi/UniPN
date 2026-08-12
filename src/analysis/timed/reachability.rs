//! State-class reachability graph construction (port of PTPN's
//! `src/analysis/ptpn_analysis.cpp`).
//!
//! Builds the state-class reachability graph of a P-TPN: time elapse on a joint
//! DBM, then a branch for every priority-enabled transition that can fire.

use std::collections::HashMap;

use crate::ids::{PlaceId, TransitionId};
use crate::net::Marking;
use crate::timed::{INF, TimedNet};

use super::canonicalization::{CanonicalizationMode, can_merge_into, check_equality};
use super::dbm::{DBM, INF_TIME};
use super::scheduling::Scheduling;
use super::state_class::{
    ClockKind, ClockVar, FiringEdge, StateClass, TransitionSet, contains, hash_state_class,
};

/// Build statistics.
#[derive(Debug, Clone, Default)]
pub struct Statistics {
    pub total_states: usize,
    pub total_transitions: usize,
    pub dedup_hits: usize,
    pub truncated: bool,
}

/// The state-class reachability graph (Vec-based, no petgraph dependency).
#[derive(Debug, Clone, Default)]
pub struct StateClassGraph {
    pub states: Vec<StateClass>,
    pub edges: Vec<(usize, usize, FiringEdge)>,
    pub initial: usize,
    pub stats: Statistics,
}

/// Configuration for state-class exploration.
#[derive(Debug, Clone)]
pub struct TimedReachabilityConfig {
    pub canonicalization: CanonicalizationMode,
    pub extrapolation: bool,
    /// Per-core parallelism bound (0 = unbounded).
    pub core_parallelism: HashMap<i32, i32>,
}

impl Default for TimedReachabilityConfig {
    fn default() -> Self {
        Self {
            canonicalization: CanonicalizationMode::Equality,
            extrapolation: false,
            core_parallelism: HashMap::new(),
        }
    }
}

/// Builds the state-class reachability graph.
pub struct StateClassReachabilityGraph<'a> {
    net: &'a TimedNet,
    initial_marking: Marking,
    config: TimedReachabilityConfig,
    graph: StateClassGraph,
    extrapolation_k: i32,
    next_id: usize,
    vertices_by_marking: HashMap<Marking, Vec<usize>>,
    vertices_by_hash: HashMap<u64, Vec<usize>>,
}

impl<'a> StateClassReachabilityGraph<'a> {
    pub fn new(net: &'a TimedNet, initial_marking: Marking) -> Self {
        Self::with_config(net, initial_marking, TimedReachabilityConfig::default())
    }

    pub fn with_config(
        net: &'a TimedNet,
        initial_marking: Marking,
        config: TimedReachabilityConfig,
    ) -> Self {
        let extrapolation_k = extrapolation_bound(net, config.extrapolation);
        StateClassReachabilityGraph {
            net,
            initial_marking,
            config,
            graph: StateClassGraph::default(),
            extrapolation_k,
            next_id: 0,
            vertices_by_marking: HashMap::new(),
            vertices_by_hash: HashMap::new(),
        }
    }

    pub fn get_graph(&self) -> &StateClassGraph {
        &self.graph
    }

    fn effective_earliest(&self, transition: usize) -> i32 {
        self.net
            .transition(TransitionId(transition))
            .map_or(0, |t| t.kind.interval.effective_earliest())
    }

    fn effective_latest(&self, transition: usize) -> i32 {
        self.net
            .transition(TransitionId(transition))
            .map_or(INF, |t| t.kind.interval.effective_latest())
    }

    fn recompute_sets(&self, state: &mut StateClass) {
        let (struct_enabled, priority_enabled, suspended) =
            Scheduling::compute_sets(self.net, &state.marking, &self.config.core_parallelism);
        state.struct_enabled = struct_enabled;
        state.priority_enabled = priority_enabled;
        state.suspended = suspended;
    }

    fn build_layout(&self, state: &mut StateClass) {
        let num_transitions = self.net.num_transitions();

        state.clock_vars.clear();
        state.clock_vars.push(ClockVar {
            kind: ClockKind::Zero,
            transition: 0,
        });
        state.exec_clock_of_transition = vec![-1; num_transitions];
        state.susp_clock_of_transition = vec![-1; num_transitions];

        for &t in &state.struct_enabled {
            let exec_idx = state.clock_vars.len();
            state.clock_vars.push(ClockVar {
                kind: ClockKind::Execution,
                transition: t,
            });
            state.exec_clock_of_transition[t] = exec_idx as i32;

            if contains(&state.suspended, t) {
                let susp_idx = state.clock_vars.len();
                state.clock_vars.push(ClockVar {
                    kind: ClockKind::Suspension,
                    transition: t,
                });
                state.susp_clock_of_transition[t] = susp_idx as i32;
            }
        }
    }

    pub fn compute_initial_class(&self) -> StateClass {
        let mut state = StateClass::default();
        state.marking = self.initial_marking.clone();
        state.elapsed_time = 0.0;

        self.recompute_sets(&mut state);
        self.build_layout(&mut state);

        // Every clock starts at zero, pinned to x0.
        let n = state.clock_vars.len();
        let mut zone = DBM::new(n);
        for i in 1..n {
            zone.set_constraint(0, i, 0);
            zone.set_constraint(i, 0, 0);
        }
        zone.minimize();
        state.zone = zone;
        state
    }

    /// TimeElapse operator: running clocks advance, frozen clocks stay put.
    pub fn time_elapse(&self, state: &StateClass) -> StateClass {
        let mut out = state.clone();
        let n = out.zone.size();
        if n == 0 {
            return out;
        }

        // V_run = { h_t : t in E_pri } U { w_t : t in suspended }.
        let mut running = vec![false; n];
        for i in 1..n.min(out.clock_vars.len()) {
            let var = out.clock_vars[i];
            match var.kind {
                ClockKind::Suspension => running[i] = true,
                ClockKind::Execution => {
                    if contains(&out.priority_enabled, var.transition) {
                        running[i] = true;
                    }
                }
                ClockKind::Zero => {}
            }
        }

        // Release each running clock's upper bound relative to stationary vars.
        for i in 1..n {
            if !running[i] {
                continue;
            }
            for j in 0..n {
                if j != i && !running[j] {
                    out.zone.set_constraint(i, j, INF_TIME);
                }
            }
        }

        // Strong time semantics: cap each active execution clock at its deadline.
        for &t in &out.priority_enabled {
            if !out.has_exec_clock(t) {
                continue;
            }
            let upper = self.effective_latest(t);
            if upper == INF_TIME {
                continue;
            }
            let idx = out.exec_index(t) as usize;
            let current = out.zone.get_constraint(idx, 0);
            if current == INF_TIME || upper < current {
                out.zone.set_constraint(idx, 0, upper);
            }
        }

        out.zone.minimize();
        out
    }

    pub fn is_firable(&self, elapsed: &StateClass, t: usize) -> bool {
        if !contains(&elapsed.priority_enabled, t) || !elapsed.has_exec_clock(t) {
            return false;
        }
        let idx = elapsed.exec_index(t) as usize;
        let max_h = elapsed.zone.get_constraint(idx, 0); // largest feasible h_t
        if max_h == INF_TIME {
            return true;
        }
        max_h >= self.effective_earliest(t)
    }

    fn build_successor_zone(
        &self,
        successor: &mut StateClass,
        fired: &DBM,
        source: &StateClass,
        fired_transition: usize,
    ) {
        let n = successor.clock_vars.len();
        let mut zone = DBM::new(n);

        let mut source_index = vec![-1i32; n];
        source_index[0] = 0; // x0 maps to x0
        for i in 1..n {
            let var = successor.clock_vars[i];
            let t = var.transition;
            match var.kind {
                ClockKind::Execution => {
                    if t != fired_transition && source.has_exec_clock(t) {
                        source_index[i] = source.exec_index(t);
                    }
                }
                ClockKind::Suspension => {
                    if t != fired_transition && source.has_susp_clock(t) {
                        source_index[i] = source.susp_index(t);
                    }
                }
                ClockKind::Zero => {}
            }
        }

        // Carry over joint constraints between all surviving clocks.
        for i in 0..n {
            if source_index[i] < 0 {
                continue;
            }
            for j in 0..n {
                if source_index[j] < 0 {
                    continue;
                }
                zone.set_constraint(
                    i,
                    j,
                    fired.get_constraint(source_index[i] as usize, source_index[j] as usize),
                );
            }
        }

        // Pin every freshly created clock to zero (equal to x0).
        for i in 1..n {
            if source_index[i] < 0 {
                zone.set_constraint(0, i, 0);
                zone.set_constraint(i, 0, 0);
            }
        }

        zone.minimize();
        successor.zone = zone;
    }

    pub fn fire(&self, elapsed: &StateClass, t: usize, successor: &mut StateClass) -> bool {
        if !self.is_firable(elapsed, t) {
            return false;
        }

        // Step 1: intersect the firing-domain constraint h_t >= downSI(t).
        let mut fired = elapsed.zone.clone();
        let idx = elapsed.exec_index(t) as usize;
        let lower = self.effective_earliest(t);
        if !fired.tighten(0, idx, -lower) {
            return false;
        }

        // Step 2: discrete token shuffle.
        *successor = StateClass::default();
        successor.marking = self.net.fire(&elapsed.marking, TransitionId(t));

        // Steps 3 & 4: recompute sets + layout, rebuild zone.
        self.recompute_sets(successor);
        self.build_layout(successor);
        self.build_successor_zone(successor, &fired, elapsed, t);

        let h_lower = -elapsed.zone.get_constraint(0, idx);
        let firing_instant = lower.max(h_lower);
        successor.elapsed_time = elapsed.elapsed_time + 0.max(firing_instant) as f64;

        true
    }

    fn find_match(&self, state: &StateClass) -> Option<usize> {
        if self.config.canonicalization == CanonicalizationMode::Equality {
            let hash = hash_state_class(state);
            if let Some(candidates) = self.vertices_by_hash.get(&hash) {
                for &candidate in candidates {
                    let existing = &self.graph.states[candidate];
                    if check_equality(state, existing) {
                        return Some(candidate);
                    }
                }
            }
            return None;
        }

        if let Some(candidates) = self.vertices_by_marking.get(&state.marking) {
            for &candidate in candidates {
                let existing = &self.graph.states[candidate];
                if can_merge_into(state, existing, self.config.canonicalization) {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn add_state(&mut self, mut state: StateClass) -> usize {
        state.id = self.next_id;
        self.next_id += 1;

        let idx = self.graph.states.len();
        self.graph.states.push(state);

        if self.config.canonicalization == CanonicalizationMode::Equality {
            let hash = hash_state_class(&self.graph.states[idx]);
            self.vertices_by_hash.entry(hash).or_default().push(idx);
        } else {
            let marking = self.graph.states[idx].marking.clone();
            self.vertices_by_marking
                .entry(marking)
                .or_default()
                .push(idx);
        }
        idx
    }

    /// Explores the reachability graph, stopping once `max_states` classes exist.
    pub fn build(&mut self, max_states: usize) -> usize {
        self.graph = StateClassGraph::default();
        self.vertices_by_marking.clear();
        self.vertices_by_hash.clear();
        self.next_id = 0;

        let mut initial = self.compute_initial_class();
        if self.config.extrapolation {
            initial.zone.extrapolate(self.extrapolation_k);
        }
        self.graph.initial = self.add_state(initial);
        self.graph.stats.total_states = 1;

        let mut frontier: Vec<usize> = vec![self.graph.initial];

        while !frontier.is_empty() {
            let mut next_frontier: Vec<usize> = Vec::new();

            for &u in &frontier {
                if self.graph.stats.total_states >= max_states {
                    self.graph.stats.truncated = true;
                    break;
                }

                // Extract everything from the source state up front (borrow rules).
                struct EntryBounds {
                    has_clock: bool,
                    low: i32,
                    high: i32,
                }
                let current = self.graph.states[u].clone();
                let elapsed = self.time_elapse(&current);
                let enabled: TransitionSet = current.priority_enabled.clone();
                let mut entry_bounds: Vec<EntryBounds> = Vec::with_capacity(enabled.len());
                for &t in &enabled {
                    let hcidx = current.exec_index(t);
                    if hcidx > 0 {
                        let cidx = hcidx as usize;
                        entry_bounds.push(EntryBounds {
                            has_clock: true,
                            low: -current.zone.get_constraint(0, cidx),
                            high: current.zone.get_constraint(cidx, 0),
                        });
                    } else {
                        entry_bounds.push(EntryBounds {
                            has_clock: false,
                            low: 0,
                            high: 0,
                        });
                    }
                }

                for (k, &t) in enabled.iter().enumerate() {
                    if !self.is_firable(&elapsed, t) {
                        continue;
                    }

                    let mut successor = StateClass::default();
                    if !self.fire(&elapsed, t, &mut successor) {
                        continue;
                    }
                    if self.config.extrapolation {
                        successor.zone.extrapolate(self.extrapolation_k);
                    }

                    // The real firing window of h_t.
                    let hidx = elapsed.exec_index(t) as usize;
                    let h_low = -elapsed.zone.get_constraint(0, hidx);
                    let h_high = elapsed.zone.get_constraint(hidx, 0);
                    let up = self.effective_latest(t);
                    let fire_min = 0.max(self.effective_earliest(t)).max(h_low);
                    let mut fire_max = h_high;
                    if up != INF_TIME && (fire_max == INF_TIME || up < fire_max) {
                        fire_max = up;
                    }

                    let mut dwell_min = fire_min;
                    let mut dwell_max = fire_max;
                    if entry_bounds[k].has_clock {
                        let entry_low = entry_bounds[k].low;
                        let entry_high = entry_bounds[k].high;
                        dwell_min = 0.max(fire_min - entry_high);
                        dwell_max = if fire_max == INF_TIME || entry_low == INF_TIME {
                            INF_TIME
                        } else {
                            0.max(fire_max - entry_low)
                        };
                    }
                    let mut edge = FiringEdge::new(t, fire_min, fire_max);
                    edge.dwell_min = dwell_min;
                    edge.dwell_max = dwell_max;

                    if let Some(v) = self.find_match(&successor) {
                        self.graph.stats.dedup_hits += 1;
                        self.graph.edges.push((u, v, edge));
                        self.graph.stats.total_transitions += 1;
                        continue;
                    }

                    if self.graph.stats.total_states >= max_states {
                        self.graph.stats.truncated = true;
                        continue;
                    }

                    let v = self.add_state(successor);
                    self.graph.stats.total_states += 1;
                    self.graph.edges.push((u, v, edge));
                    self.graph.stats.total_transitions += 1;
                    next_frontier.push(v);
                }

                if self.graph.stats.truncated {
                    break;
                }
            }

            if self.graph.stats.truncated {
                break;
            }
            frontier = next_frontier;
        }

        self.graph.stats.total_states
    }
}

fn extrapolation_bound(net: &TimedNet, enabled: bool) -> i32 {
    if !enabled {
        return -1;
    }
    let mut k: i32 = 0;
    for t in 0..net.num_transitions() {
        let interval = net
            .transition(TransitionId(t))
            .map(|tr| tr.kind.interval)
            .unwrap_or_else(|| crate::timed::TimeInterval::closed(0, 0));
        k = k.max(interval.effective_earliest());
        let latest = interval.effective_latest();
        if latest != INF_TIME {
            k = k.max(latest);
        }
    }
    k
}

// ---------------------------------------------------------------------------
// Formatting helpers.
// ---------------------------------------------------------------------------

pub fn escape_dot(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn format_marking(net: &TimedNet, marking: &Marking) -> String {
    let mut out = String::from("[");
    let mut first = true;
    for (i, &tokens) in marking.0.iter().enumerate() {
        if tokens == 0 {
            continue;
        }
        if !first {
            out.push_str(", ");
        }
        first = false;
        let name = net
            .place(PlaceId(i))
            .map_or_else(|| format!("P{}", i), |p| p.name.clone());
        out.push_str(&name);
        if tokens != 1 {
            out.push_str(&format!("({})", tokens));
        }
    }
    if first {
        out.push_str("empty");
    }
    out.push(']');
    out
}

/// Format a marking given the net, for external reporting.
pub fn format_marking_of(net: &TimedNet, marking: &Marking) -> String {
    format_marking(net, marking)
}

/// Collects out-edge transition ids for a vertex (test helper).
pub fn out_edge_transitions(graph: &StateClassGraph, vertex: usize) -> Vec<usize> {
    let mut result: Vec<usize> = graph
        .edges
        .iter()
        .filter(|(src, _, _)| *src == vertex)
        .map(|(_, _, e)| e.transition_id)
        .collect();
    result.sort_unstable();
    result
}

/// Collects the set of distinct markings stored in the reachability graph.
pub fn reachable_markings(graph: &StateClassGraph) -> std::collections::BTreeSet<Marking> {
    graph
        .states
        .iter()
        .map(|state| state.marking.clone())
        .collect()
}
