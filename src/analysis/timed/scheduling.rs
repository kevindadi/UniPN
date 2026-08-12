//! Structural and priority enabling (port of PTPN's `src/analysis/scheduling.cpp`).

use std::collections::HashMap;

use crate::ids::TransitionId;
use crate::net::Marking;
use crate::timed::TimedNet;

use super::state_class::{TransitionSet, contains};

pub struct Scheduling;

impl Scheduling {
    /// E_struct(M): transitions whose input places hold enough tokens. Sorted.
    pub fn structural_enabled(net: &TimedNet, marking: &Marking) -> TransitionSet {
        let mut enabled = Vec::new();
        for t in 0..net.num_transitions() {
            if net.is_enabled(marking, TransitionId(t)) {
                enabled.push(t);
            }
        }
        enabled
    }

    /// E_pri(M): within every core group keep the highest-priority structurally
    /// enabled transitions. Bounded cores keep at most `capacity` transitions
    /// (highest priority first, ties by transition index); unbounded groups keep
    /// every transition sharing the maximal priority.
    pub fn filter_priority_per_core(
        struct_enabled: &TransitionSet,
        net: &TimedNet,
        core_parallelism: &HashMap<i32, i32>,
    ) -> TransitionSet {
        let mut per_core: HashMap<i32, Vec<usize>> = HashMap::new();
        for &t in struct_enabled {
            let core = net.transition(TransitionId(t)).map_or(0, |tr| tr.kind.core);
            per_core.entry(core).or_default().push(t);
        }

        let mut active: TransitionSet = Vec::new();

        for (core, mut group) in per_core {
            let capacity = parallelism_of_core(core_parallelism, core);

            if capacity <= 0 {
                // Unbounded group: keep every transition sharing the highest priority.
                let max_priority = group
                    .iter()
                    .map(|&t| {
                        net.transition(TransitionId(t))
                            .map_or(0, |tr| tr.kind.priority)
                    })
                    .max()
                    .unwrap_or(0);
                for &t in &group {
                    let priority = net
                        .transition(TransitionId(t))
                        .map_or(0, |tr| tr.kind.priority);
                    if priority == max_priority {
                        active.push(t);
                    }
                }
                continue;
            }

            // Bounded resource: keep the highest-priority ones, ties by index.
            group.sort_by(|&a, &b| {
                let pa = net
                    .transition(TransitionId(a))
                    .map_or(0, |tr| tr.kind.priority);
                let pb = net
                    .transition(TransitionId(b))
                    .map_or(0, |tr| tr.kind.priority);
                pb.cmp(&pa).then(a.cmp(&b))
            });
            let keep = (capacity as usize).min(group.len());
            for &t in &group[..keep] {
                active.push(t);
            }
        }

        active.sort();
        active
    }

    /// Computes E_struct / E_pri / suspended for a marking.
    pub fn compute_sets(
        net: &TimedNet,
        marking: &Marking,
        core_parallelism: &HashMap<i32, i32>,
    ) -> (TransitionSet, TransitionSet, TransitionSet) {
        let struct_enabled = Self::structural_enabled(net, marking);
        let priority_enabled =
            Self::filter_priority_per_core(&struct_enabled, net, core_parallelism);

        let mut suspended = Vec::new();
        for &t in &struct_enabled {
            if contains(&priority_enabled, t) {
                continue;
            }
            if net
                .transition(TransitionId(t))
                .is_some_and(|tr| tr.kind.suspendable)
            {
                suspended.push(t);
            }
        }
        (struct_enabled, priority_enabled, suspended)
    }
}

/// A core's parallelism bound: 0 means "no bound" (control core / legacy).
fn parallelism_of_core(core_parallelism: &HashMap<i32, i32>, core_id: i32) -> i32 {
    if core_id < 0 {
        return 0;
    }
    core_parallelism.get(&core_id).copied().unwrap_or(0)
}
