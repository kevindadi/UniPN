//! Petri net boundness analysis via the coverability tree.
//!
//! Constructs the Karp–Miller coverability tree over any [`NetLike`] net; a
//! ω-marking (accelerated place) witnesses unboundedness.

use std::collections::VecDeque;
use std::fmt;

use crate::ids::{PlaceId, TransitionId};
use crate::netlike::NetLike;
use crate::state::State;

/// Result of a boundness check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundnessResult {
    /// The net is bounded.
    Bounded,
    /// The net is unbounded; carries witness places / firing sequence when known.
    Unbounded {
        /// Places that became ω (unbounded).
        unbounded_places: Vec<PlaceId>,
        /// Witness firing sequence if reconstructed.
        witness_sequence: Option<Vec<TransitionId>>,
    },
    /// Boundness could not be determined (e.g. state explosion).
    Unknown { reason: String },
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
                    "Petri net is unbounded; unbounded places: {:?}",
                    unbounded_places
                )?;
                if let Some(seq) = witness_sequence {
                    write!(f, "; witness sequence: {:?}", seq)?;
                }
                Ok(())
            }
            BoundnessResult::Unknown { reason } => {
                write!(f, "Could not determine boundness: {}", reason)
            }
        }
    }
}

/// A node in the coverability tree: a (possibly accelerated) marking plus its
/// parent link.
#[derive(Debug, Clone)]
struct Node {
    /// `None` = ω (unbounded).
    marking: Vec<Option<u32>>,
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
    pub fn check(&self, net: &dyn NetLike) -> BoundnessResult {
        let initial = net.initial_state();
        let root_marking: Vec<Option<u32>> = initial
            .marking
            .iter()
            .map(|(_, tokens)| Some(tokens))
            .collect();

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
            if let Some(unbounded) = node
                .marking
                .iter()
                .enumerate()
                .find_map(|(i, t)| t.is_none().then_some(PlaceId(i)))
            {
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
                let _ = unbounded;
                return BoundnessResult::Unbounded {
                    unbounded_places,
                    witness_sequence: Some(witness),
                };
            }

            // Reconstruct a concrete marking (ω nodes return above).
            let concrete: Vec<u32> = node.marking.iter().map(|t| t.unwrap_or(0)).collect();
            let state = State::new(crate::state::Marking::new(concrete), None);

            let enabled = net.enabled_transitions(&state);
            for transition in enabled {
                let Ok(next_state) = net.fire(transition, &state) else {
                    continue;
                };
                let mut next: Vec<Option<u32>> = next_state
                    .marking
                    .iter()
                    .map(|(_, tokens)| Some(tokens))
                    .collect();

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
fn is_le(a: &[Option<u32>], b: &[Option<u32>]) -> bool {
    a.iter().zip(b.iter()).all(|(x, y)| match (x, y) {
        (Some(x), Some(y)) => x <= y,
        (None, Some(_)) => false,
        (None, None) => true,
        (Some(_), None) => false,
    })
}

/// Convenience: check whether `net` is bounded.
pub fn check_boundness(net: &dyn NetLike) -> BoundnessResult {
    BoundnessAnalyzer::new().check(net)
}

/// Check boundness information restricted to a single place.
pub fn check_place_boundness(net: &dyn NetLike, place: PlaceId) -> BoundnessResult {
    match BoundnessAnalyzer::new().check(net) {
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
