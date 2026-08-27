use crate::pt::{PlaceType, TransitionType};

use super::ReductionStep;
use super::graph::{GraphTransition, ReductionGraph};

impl ReductionGraph {
    /// # Reduction rule: intermediate place elimination
    ///
    /// Formal sketch:
    /// - Pick place p in reduction graph G = (P, T, F) such that:
    ///   - p is not removed, not Resources, and tokens(p) = 0.
    ///   - There is a unique input transition t_in and unique output transition t_out, both not removed.
    ///   - |t_in•| = |•t_out| = 1 with matching weights: w(t_in, p) = w(p, t_out).
    /// - Build new transition t_new with preset •t_in and postset t_out•,
    ///   union of origin annotations; inherit transition type when t_in and t_out agree, else Normal.
    /// - Mark t_in, t_out, and p removed; insert t_new and refresh incident adjacency.
    pub(crate) fn eliminate_intermediate_places(&mut self) -> Vec<ReductionStep> {
        let mut steps = Vec::new();
        let mut changed = true;

        while changed {
            changed = false;
            for place_idx in 0..self.places.len() {
                if self.places[place_idx].removed {
                    continue;
                }
                if self.places[place_idx].kind.place_type == PlaceType::Resources {
                    continue;
                }
                if self.places[place_idx].tokens != 0 {
                    continue;
                }

                if self.places[place_idx].incoming.len() != 1
                    || self.places[place_idx].outgoing.len() != 1
                {
                    continue;
                }

                let in_transition_idx = self.places[place_idx].incoming[0];
                let out_transition_idx = self.places[place_idx].outgoing[0];

                if self.transitions[in_transition_idx].removed
                    || self.transitions[out_transition_idx].removed
                {
                    continue;
                }

                // Do not merge a detector-visible transition (e.g. the merged
                // `UnsafeAccess`) into a neutral one.
                if super::preserves_transition_type(
                    &self.transitions[in_transition_idx].kind.transition_type,
                ) || super::preserves_transition_type(
                    &self.transitions[out_transition_idx].kind.transition_type,
                ) {
                    continue;
                }

                if self.transitions[in_transition_idx].outputs.len() != 1 {
                    continue;
                }
                if self.transitions[out_transition_idx].inputs.len() != 1 {
                    continue;
                }

                let weight_in = self.transitions[in_transition_idx].outputs[0].1;
                let weight_out = self.transitions[out_transition_idx].inputs[0].1;
                if weight_in != weight_out {
                    continue;
                }

                let inputs = self.transitions[in_transition_idx].inputs.clone();
                let outputs = self.transitions[out_transition_idx].outputs.clone();
                let place_originals = self.places[place_idx].originals.clone();
                let combined_originals = {
                    let mut data = self.transitions[in_transition_idx].originals.clone();
                    data.extend(self.transitions[out_transition_idx].originals.clone());
                    data
                };

                let new_transition_type =
                    if self.transitions[in_transition_idx].kind.transition_type
                        == self.transitions[out_transition_idx].kind.transition_type
                    {
                        self.transitions[in_transition_idx]
                            .kind
                            .transition_type
                            .clone()
                    } else {
                        TransitionType::Normal
                    };

                let new_transition_idx = self.add_transition(GraphTransition::new_with_type(
                    format!("inter_merge#{}", self.merge_counter),
                    new_transition_type,
                    combined_originals.clone(),
                    inputs.clone(),
                    outputs.clone(),
                ));
                self.merge_counter += 1;

                for (p_idx, _) in &inputs {
                    self.places[*p_idx]
                        .outgoing
                        .retain(|idx| *idx != in_transition_idx);
                    self.places[*p_idx].outgoing.push(new_transition_idx);
                }
                for (p_idx, _) in &outputs {
                    self.places[*p_idx]
                        .incoming
                        .retain(|idx| *idx != out_transition_idx);
                    self.places[*p_idx].incoming.push(new_transition_idx);
                }

                self.remove_transition(in_transition_idx);
                self.remove_transition(out_transition_idx);
                self.remove_place(place_idx);
                self.clean_adjacency();

                steps.push(ReductionStep::IntermediatePlaceEliminated {
                    places: place_originals,
                    merged_transitions: combined_originals,
                });

                changed = true;
                break;
            }
        }

        steps
    }
}
