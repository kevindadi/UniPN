use std::collections::HashSet;

use crate::net::{PlaceId, TransitionId};
use crate::pt::PlaceType;

use super::ReductionStep;
use super::graph::ReductionGraph;

impl ReductionGraph {
    /// # Reduction rule: simple loop removal
    ///
    /// - On the reduction graph G = (P, T, F), pick a start place p_0 such that:
    ///   - p_0 is not removed, is not of type Resources, and |•p_0| = |p_0•| = 1.
    /// - Follow the unique output transition t_i and successor place p_{i+1}, building sets
    ///   C_P = {p_0, …, p_{k-1}} and C_T = {t_0, …, t_{k-1}},
    ///   where each p_i is not Resources, |•p_i| = |p_i•| = 1, each t_i satisfies
    ///   |•t_i| = |t_i•| = 1 and outputs(t_i) = {p_{(i+1) mod k}}.
    /// - Require all tokens on p_i to be zero and no element already removed, yielding a simple circuit.
    /// - When the circuit satisfies the conditions, remove all of C_P and C_T, record identities, and clean adjacency.
    pub(crate) fn remove_simple_loops(&mut self) -> Vec<ReductionStep> {
        let mut steps = Vec::new();
        let mut visited = HashSet::new();

        for start_idx in 0..self.places.len() {
            if visited.contains(&start_idx) {
                continue;
            }
            if self.places[start_idx].removed {
                continue;
            }
            if self.places[start_idx].kind.place_type == PlaceType::Resources {
                continue;
            }
            if self.places[start_idx].outgoing.len() != 1 {
                continue;
            }
            if self.places[start_idx].incoming.len() != 1 {
                continue;
            }

            let mut cycle_places = Vec::new();
            let mut cycle_transitions = Vec::new();

            let mut current_place = start_idx;
            let mut local_visited = HashSet::new();
            let mut valid_cycle = true;

            loop {
                if !local_visited.insert(current_place) {
                    valid_cycle = false;
                    break;
                }
                let transition_idx = match self.places[current_place].outgoing.first() {
                    Some(idx) => *idx,
                    None => {
                        valid_cycle = false;
                        break;
                    }
                };
                if self.transitions[transition_idx].removed {
                    valid_cycle = false;
                    break;
                }
                if self.transitions[transition_idx].inputs.len() != 1
                    || self.transitions[transition_idx].outputs.len() != 1
                {
                    valid_cycle = false;
                    break;
                }

                let next_place = self.transitions[transition_idx].outputs[0].0;

                if self.places[next_place].removed {
                    valid_cycle = false;
                    break;
                }
                if self.places[next_place].kind.place_type == PlaceType::Resources {
                    valid_cycle = false;
                    break;
                }
                if self.places[next_place].incoming.len() != 1
                    || self.places[next_place].outgoing.len() != 1
                {
                    valid_cycle = false;
                    break;
                }
                cycle_places.push(current_place);
                cycle_transitions.push(transition_idx);

                if next_place == start_idx {
                    break;
                }
                current_place = next_place;
            }

            if !valid_cycle {
                continue;
            }

            let all_tokens_zero = cycle_places.iter().all(|idx| self.places[*idx].tokens == 0);
            if !all_tokens_zero {
                continue;
            }

            for place_idx in &cycle_places {
                visited.insert(*place_idx);
            }

            let removed_places: Vec<PlaceId> = cycle_places
                .iter()
                .flat_map(|idx| self.places[*idx].originals.clone())
                .collect();
            let removed_transitions: Vec<TransitionId> = cycle_transitions
                .iter()
                .flat_map(|idx| self.transitions[*idx].originals.clone())
                .collect();

            for transition_idx in &cycle_transitions {
                self.remove_transition(*transition_idx);
            }
            for place_idx in &cycle_places {
                self.remove_place(*place_idx);
            }
            self.clean_adjacency();

            if !removed_places.is_empty() || !removed_transitions.is_empty() {
                steps.push(ReductionStep::LoopRemoved {
                    removed_places,
                    removed_transitions,
                });
            }
        }

        steps
    }
}
