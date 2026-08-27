use crate::net::PlaceId;
use crate::pt::{PlaceType, TransitionType};

use super::ReductionStep;
use super::graph::{GraphTransition, ReductionGraph};

impl ReductionGraph {
    /// # Reduction rule: linear sequence merge
    ///
    /// - Given reduction graph G = (P, T, F), let p_0 be the head candidate place.
    /// - Recursively extend an alternating chain p_0, t_0, p_1, t_1, …, t_{k-1}, p_k such that:
    ///   - For all 0 ≤ i ≤ k, p_i ∈ P is not removed, not Resources, p_0 has a single outgoing arc, p_k has a single incoming arc,
    ///     and for 0 < i < k we have |•p_i| = |p_i•| = 1.
    ///   - For all 0 ≤ i < k, t_i ∈ T is not removed and |•t_i| = |t_i•| = 1.
    ///   - Arc weights satisfy w(•t_i) = w(t_i•) and are uniform along the chain.
    /// - If k ≥ 2, introduce a new transition t_new wiring p_0 to p_k directly,
    ///   inheriting the union of origin transitions and a unified transition type (falls back to Normal if inconsistent).
    pub(crate) fn merge_linear_sequences(&mut self) -> Vec<ReductionStep> {
        let mut steps = Vec::new();
        let mut changed = true;

        while changed {
            changed = false;
            for head_idx in 0..self.places.len() {
                if self.places[head_idx].removed {
                    continue;
                }
                if self.places[head_idx].place.place_type == PlaceType::Resources {
                    continue;
                }
                if self.places[head_idx].outgoing.len() != 1 {
                    continue;
                }

                let mut place_chain = vec![head_idx];
                let mut transition_chain = Vec::new();
                let mut current_place = head_idx;
                let mut weights = Vec::new();
                let mut transition_types = Vec::new();
                let mut valid_chain = true;

                loop {
                    let Some(&transition_idx) = self.places[current_place].outgoing.first() else {
                        valid_chain = false;
                        break;
                    };
                    if self.transitions[transition_idx].removed {
                        valid_chain = false;
                        break;
                    }
                    if super::preserves_transition_type(
                        &self.transitions[transition_idx].transition.transition_type,
                    ) {
                        valid_chain = false;
                        break;
                    }
                    if self.transitions[transition_idx].inputs.len() != 1
                        || self.transitions[transition_idx].outputs.len() != 1
                    {
                        valid_chain = false;
                        break;
                    }

                    let next_place = self.transitions[transition_idx].outputs[0].0;
                    if self.places[next_place].removed {
                        valid_chain = false;
                        break;
                    }
                    if self.places[next_place].place.place_type == PlaceType::Resources {
                        valid_chain = false;
                        break;
                    }
                    if self.places[next_place].incoming.len() != 1 {
                        break;
                    }

                    let weight_in = self.transitions[transition_idx].inputs[0].1;
                    let weight_out = self.transitions[transition_idx].outputs[0].1;
                    if weight_in != weight_out {
                        valid_chain = false;
                        break;
                    }
                    weights.push(weight_in);
                    transition_types.push(
                        self.transitions[transition_idx]
                            .transition
                            .transition_type
                            .clone(),
                    );

                    transition_chain.push(transition_idx);
                    current_place = next_place;
                    place_chain.push(next_place);

                    if self.places[current_place].outgoing.len() != 1 {
                        break;
                    }
                }

                if !valid_chain {
                    continue;
                }
                if transition_chain.len() < 2 {
                    continue;
                }

                let tail_idx = *place_chain.last().unwrap();
                if self.places[tail_idx].place.place_type == PlaceType::Resources {
                    continue;
                }
                let consistent_weight = weights.windows(2).all(|window| window[0] == window[1]);
                if !consistent_weight {
                    continue;
                }
                let weight = *weights.first().unwrap_or(&1);

                let mut original_transitions = Vec::new();
                for idx in &transition_chain {
                    original_transitions.extend(self.transitions[*idx].originals.clone());
                }

                let new_transition_type = if transition_types
                    .windows(2)
                    .all(|window| window[0] == window[1])
                {
                    transition_types
                        .first()
                        .cloned()
                        .unwrap_or(TransitionType::Normal)
                } else {
                    TransitionType::Normal
                };

                let head_place = head_idx;
                let tail_place = tail_idx;
                let head_originals = self.places[head_place].originals.clone();
                let tail_originals = self.places[tail_place].originals.clone();
                let removed_places: Vec<PlaceId> = place_chain[1..place_chain.len() - 1]
                    .iter()
                    .flat_map(|idx| self.places[*idx].originals.clone())
                    .collect();

                let new_transition_idx = self.add_transition(GraphTransition::new_with_type(
                    format!("seq_merge#{}", self.merge_counter),
                    new_transition_type,
                    original_transitions.clone(),
                    vec![(head_place, weight)],
                    vec![(tail_place, weight)],
                ));
                self.merge_counter += 1;

                self.places[head_place]
                    .outgoing
                    .retain(|idx| !transition_chain.contains(idx));
                self.places[head_place].outgoing.push(new_transition_idx);

                self.places[tail_place]
                    .incoming
                    .retain(|idx| !transition_chain.contains(idx));
                self.places[tail_place].incoming.push(new_transition_idx);

                for transition_idx in &transition_chain {
                    self.remove_transition(*transition_idx);
                }
                for place_idx in place_chain
                    .iter()
                    .skip(1)
                    .take(place_chain.len().saturating_sub(2))
                {
                    self.remove_place(*place_idx);
                }
                self.clean_adjacency();

                steps.push(ReductionStep::SequenceMerged {
                    head_places: head_originals,
                    tail_places: tail_originals,
                    merged_transitions: original_transitions,
                    removed_places,
                });

                changed = true;
                break;
            }
        }

        steps
    }
}
