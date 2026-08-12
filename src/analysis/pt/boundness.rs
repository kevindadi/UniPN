//! P/T boundness analysis via the Karp–Miller coverability tree (port of
//! ConcBugDect's `analysis/boundness.rs`).

use std::collections::VecDeque;
use std::fmt;

use crate::analysis::NetLike;
use crate::ids::{PlaceId, TransitionId};
use crate::net::Marking;
use crate::pt::PtNet;

/// Result of a boundness check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundnessResult {
    Bounded,
    Unbounded {
        unbounded_places: Vec<PlaceId>,
        witness_sequence: Option<Vec<TransitionId>>,
    },
    Unknown {
        reason: String,
    },
}

impl fmt::Display for BoundnessResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoundnessResult::Bounded => write!(f, "Petri net is bounded"),
            BoundnessResult::Unbounded {
                unbounded_places,
                witness_sequence,
            } => {
                write!(
                    f,
                    "Petri net is unbounded; unbounded places: {unbounded_places:?}"
                )?;
                if let Some(seq) = witness_sequence {
                    write!(f, "; witness sequence: {seq:?}")?;
                }
                Ok(())
            }
            BoundnessResult::Unknown { reason } => {
                write!(f, "Could not determine boundness: {reason}")
            }
        }
    }
}

/// A coverability-tree node: a (possibly ω-accelerated) marking.
#[derive(Debug, Clone)]
struct Node {
    /// `None` = ω (unbounded).
    marking: Vec<Option<usize>>,
    parent: Option<usize>,
    transition_from_parent: Option<TransitionId>,
}

/// Boundness analyzer with an optional state limit.
#[derive(Clone, Debug)]
pub struct BoundnessAnalyzer {
    state_limit: Option<usize>,
}

impl Default for BoundnessAnalyzer {
    fn default() -> Self {
        Self {
            state_limit: Some(10_000),
        }
    }
}

impl BoundnessAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_state_limit(mut self, limit: Option<usize>) -> Self {
        self.state_limit = limit;
        self
    }

    /// Boundness via the coverability tree.
    pub fn check(&self, net: &PtNet, initial: &Marking) -> BoundnessResult {
        let root_marking: Vec<Option<usize>> = initial.0.iter().map(|&t| Some(t)).collect();

        let mut nodes = vec![Node {
            marking: root_marking,
            parent: None,
            transition_from_parent: None,
        }];
        let mut queue = VecDeque::from([0usize]);
        let mut visited = 0usize;

        while let Some(node_index) = queue.pop_front() {
            visited += 1;
            if let Some(limit) = self.state_limit
                && visited > limit
            {
                return BoundnessResult::Unknown {
                    reason: format!("Exceeded state limit {limit}"),
                };
            }

            let node = nodes[node_index].clone();
            if node.marking.iter().any(|t| t.is_none()) {
                let unbounded_places = node
                    .marking
                    .iter()
                    .enumerate()
                    .filter_map(|(i, t)| t.is_none().then_some(PlaceId(i)))
                    .collect();
                let mut witness = Vec::new();
                let mut current = node_index;
                while let Some(parent) = nodes[current].parent {
                    if let Some(trans) = nodes[current].transition_from_parent {
                        witness.push(trans);
                    }
                    current = parent;
                }
                witness.reverse();
                return BoundnessResult::Unbounded {
                    unbounded_places,
                    witness_sequence: Some(witness),
                };
            }

            // Reconstruct a concrete marking (ω nodes return above).
            let concrete: Vec<usize> = node.marking.iter().map(|t| t.unwrap_or(0)).collect();
            let state = Marking::new(concrete);

            for transition in net.enabled(&state) {
                let Some(next_state) = net.fire(&state, transition) else {
                    continue;
                };
                let mut next: Vec<Option<usize>> = next_state.0.iter().map(|&t| Some(t)).collect();

                // ω-acceleration: if an ancestor on the path root → current is
                // componentwise ≤ next, replace the strictly-increasing places
                // with ω.
                let mut current = Some(node_index);
                while let Some(ancestor) = current {
                    if is_le(&nodes[ancestor].marking, &next) {
                        let omega_slots: Vec<usize> = nodes[ancestor]
                            .marking
                            .iter()
                            .zip(&next)
                            .enumerate()
                            .filter_map(|(slot, (anc, nxt))| match (anc, nxt) {
                                (Some(a), Some(b)) if a < b => Some(slot),
                                _ => None,
                            })
                            .collect();
                        for slot in omega_slots {
                            next[slot] = None;
                        }
                    }
                    current = nodes[ancestor].parent;
                }

                if nodes.iter().any(|node| node.marking == next) {
                    continue;
                }
                let child = nodes.len();
                nodes.push(Node {
                    marking: next,
                    parent: Some(node_index),
                    transition_from_parent: Some(transition),
                });
                queue.push_back(child);
            }
        }

        BoundnessResult::Bounded
    }
}

/// Componentwise `≤` with ω treated as +∞ (an ω component is never ≤ a finite
/// one, so it does not trigger acceleration against a finite successor).
fn is_le(a: &[Option<usize>], b: &[Option<usize>]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
        (Some(x), Some(y)) => x <= y,
        (None, Some(_)) => false,
        (None, None) => true,
        (Some(_), None) => false,
    })
}

/// Convenience: check whether `net` is bounded.
pub fn check_boundness(net: &PtNet, initial: &Marking) -> BoundnessResult {
    BoundnessAnalyzer::new().check(net, initial)
}

/// Check boundness information restricted to a single place.
pub fn check_place_boundness(net: &PtNet, initial: &Marking, place: PlaceId) -> BoundnessResult {
    match BoundnessAnalyzer::new().check(net, initial) {
        BoundnessResult::Bounded => BoundnessResult::Bounded,
        BoundnessResult::Unbounded {
            unbounded_places,
            witness_sequence,
        } => {
            if unbounded_places.contains(&place) {
                BoundnessResult::Unbounded {
                    unbounded_places: vec![place],
                    witness_sequence,
                }
            } else {
                BoundnessResult::Bounded
            }
        }
        BoundnessResult::Unknown { reason } => BoundnessResult::Unknown { reason },
    }
}
