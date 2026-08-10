//! 状态空间探索：BFS / DFS / 偏序归约（sleep-set）。
//!
//! 使用独立存储的可达图（无 petgraph 依赖）：
//! `states: Vec<State>` + `edges: Vec<(src, dst, transition)>`。

use std::collections::VecDeque;
use std::collections::hash_map::Entry;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ids::TransitionId;
use crate::netlike::NetLike;
use crate::state::State;

use super::{AnalysisConfig, Counterexample, FiringStep, PropertyViolation, SearchStrategy};

/// 可达图。
#[derive(Clone, Debug)]
pub struct ReachabilityGraph {
    pub states: Vec<State>,
    pub edges: Vec<(usize, usize, TransitionId)>,
    pub initial: usize,
    pub deadlocks: Vec<Counterexample>,
    pub truncated: bool,
}

impl ReachabilityGraph {
    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// 可达图中出现过的变迁集合（死迁移判定用）。
    pub fn fired_transitions(&self) -> FxHashSet<TransitionId> {
        self.edges.iter().map(|(_, _, t)| *t).collect()
    }
}

/// 探索整个可达状态空间。
pub fn explore(net: &dyn NetLike, config: &AnalysisConfig) -> ReachabilityGraph {
    if config.por {
        return explore_por(net, config.max_states);
    }
    match config.strategy {
        SearchStrategy::Bfs => explore_bfs(net, config.max_states),
        SearchStrategy::Dfs => explore_dfs(net, config.max_states),
    }
}

// ── BFS ──

fn explore_bfs(net: &dyn NetLike, max_states: usize) -> ReachabilityGraph {
    let mut e = Explorer::new(net, max_states);
    let initial = net.initial_state();
    let (init_idx, _) = e.insert_state(initial).unwrap();
    let mut queue = VecDeque::new();
    queue.push_back(init_idx);

    while let Some(idx) = queue.pop_front() {
        let state = e.states[idx].clone();
        let enabled = net.enabled_transitions(&state);
        if enabled.is_empty() {
            if super::deadlock::is_deadlock(net, &state) {
                e.push_deadlock(idx, &state);
            }
            continue;
        }
        for t in enabled {
            if let Ok(next) = net.fire(t, &state)
                && let Some((target, _)) = e.insert_state(next)
            {
                e.record_edge(idx, target, t);
                queue.push_back(target);
            }
        }
    }
    e.finish()
}

// ── DFS ──

fn explore_dfs(net: &dyn NetLike, max_states: usize) -> ReachabilityGraph {
    let mut e = Explorer::new(net, max_states);
    let initial = net.initial_state();
    let (init_idx, _) = e.insert_state(initial).unwrap();
    let mut stack = vec![init_idx];

    while let Some(idx) = stack.pop() {
        let state = e.states[idx].clone();
        let enabled = net.enabled_transitions(&state);
        if enabled.is_empty() {
            if super::deadlock::is_deadlock(net, &state) {
                e.push_deadlock(idx, &state);
            }
            continue;
        }
        for t in enabled {
            if let Ok(next) = net.fire(t, &state)
                && let Some((target, _)) = e.insert_state(next)
            {
                e.record_edge(idx, target, t);
                stack.push(target);
            }
        }
    }
    e.finish()
}

// ── POR（sleep-set）──

fn explore_por(net: &dyn NetLike, max_states: usize) -> ReachabilityGraph {
    let mut e = Explorer::new(net, max_states);
    let initial = net.initial_state();
    let (init_idx, _) = e.insert_state(initial).unwrap();

    let mut queue: VecDeque<(usize, FxHashSet<TransitionId>)> = VecDeque::new();
    let mut sleep_sets: FxHashMap<usize, FxHashSet<TransitionId>> = FxHashMap::default();
    queue.push_back((init_idx, FxHashSet::default()));

    while let Some((idx, sleep)) = queue.pop_front() {
        if e.states.len() > max_states {
            break;
        }
        let state = e.states[idx].clone();
        let enabled: FxHashSet<TransitionId> =
            net.enabled_transitions(&state).into_iter().collect();

        if enabled.is_empty() {
            if super::deadlock::is_deadlock(net, &state) {
                e.push_deadlock(idx, &state);
            }
            continue;
        }

        let to_fire: Vec<TransitionId> = enabled.difference(&sleep).copied().collect();
        for t in to_fire {
            let Ok(next) = net.fire(t, &state) else {
                continue;
            };
            let enabled_next: FxHashSet<TransitionId> =
                net.enabled_transitions(&next).into_iter().collect();

            // 与 t 独立的使能变迁加入 sleep：其交错可交换，只展开一个代表序。
            let mut new_sleep = sleep.clone();
            for &tt in &enabled {
                if tt != t && transitions_are_independent(net, t, tt) {
                    new_sleep.insert(tt);
                }
            }
            new_sleep = new_sleep.intersection(&enabled_next).copied().collect();

            let Some((target, is_new)) = e.insert_state(next) else {
                continue;
            };
            e.record_edge(idx, target, t);

            if is_new {
                // 新状态：必须以本次 sleep 入队展开。
                sleep_sets.insert(target, new_sleep.clone());
                queue.push_back((target, new_sleep));
            } else {
                // 同一标记以不同 sleep 到达时按交集合并，保证 deadlock 不丢。
                let old_sleep = sleep_sets.get(&target).cloned().unwrap_or_default();
                let merged: FxHashSet<TransitionId> =
                    old_sleep.intersection(&new_sleep).copied().collect();
                if merged != old_sleep {
                    sleep_sets.insert(target, merged.clone());
                    queue.push_back((target, merged));
                }
            }
        }
    }
    e.finish()
}

/// 两变迁是否独立：不共享任何库位（pre/post 均不共享）。独立变迁交错可交换。
fn transitions_are_independent(net: &dyn NetLike, t1: TransitionId, t2: TransitionId) -> bool {
    if t1 == t2 {
        return false;
    }
    let mut places: FxHashSet<usize> = FxHashSet::default();
    for (p, _) in net.pre_arcs(t1).into_iter().chain(net.post_arcs(t1)) {
        places.insert(p.index());
    }
    for (p, _) in net.pre_arcs(t2).into_iter().chain(net.post_arcs(t2)) {
        if places.contains(&p.index()) {
            return false;
        }
    }
    true
}

// ── 共享探索器 ──

struct Explorer<'a> {
    net: &'a dyn NetLike,
    max_states: usize,
    states: Vec<State>,
    seen: FxHashMap<u64, usize>,
    edges: Vec<(usize, usize, TransitionId)>,
    preds: FxHashMap<usize, (usize, TransitionId)>,
    deadlocks: Vec<Counterexample>,
    truncated: bool,
}

impl<'a> Explorer<'a> {
    fn new(net: &'a dyn NetLike, max_states: usize) -> Self {
        Self {
            net,
            max_states,
            states: Vec::new(),
            seen: FxHashMap::default(),
            edges: Vec::new(),
            preds: FxHashMap::default(),
            deadlocks: Vec::new(),
            truncated: false,
        }
    }

    fn insert_state(&mut self, state: State) -> Option<(usize, bool)> {
        let hash = hash_state(&state);
        match self.seen.entry(hash) {
            Entry::Occupied(e) => Some((*e.get(), false)),
            Entry::Vacant(v) => {
                if self.states.len() >= self.max_states {
                    self.truncated = true;
                    return None;
                }
                let idx = self.states.len();
                self.states.push(state);
                v.insert(idx);
                Some((idx, true))
            }
        }
    }

    fn record_edge(&mut self, src: usize, dst: usize, t: TransitionId) {
        self.edges.push((src, dst, t));
        self.preds.entry(dst).or_insert((src, t));
    }

    fn push_deadlock(&mut self, target: usize, state: &State) {
        let trace = self.reconstruct_trace(target);
        self.deadlocks.push(Counterexample {
            kind: PropertyViolation::Deadlock,
            trace,
            final_state: state.clone(),
        });
    }

    fn reconstruct_trace(&self, target: usize) -> Vec<FiringStep> {
        let mut path = Vec::new();
        let mut current = target;
        while let Some(&(parent, t)) = self.preds.get(&current) {
            path.push(FiringStep {
                transition: t,
                anchors: self.net.transition_anchors(t),
            });
            current = parent;
        }
        path.reverse();
        path
    }

    fn finish(self) -> ReachabilityGraph {
        ReachabilityGraph {
            states: self.states,
            edges: self.edges,
            initial: 0,
            deadlocks: self.deadlocks,
            truncated: self.truncated,
        }
    }
}

fn hash_state(state: &State) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = rustc_hash::FxHasher::default();
    state.hash(&mut hasher);
    hasher.finish()
}
