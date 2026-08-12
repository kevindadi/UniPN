use crate::ids::{PlaceId, TransitionId};
use crate::net::{ArcDir, Marking};
use crate::pt::{PtBuilder, PtNet, PtPlace, PtTransition, TransitionType};

use super::ReductionTrace;

pub(crate) struct ReductionGraph {
    pub(crate) places: Vec<GraphPlace>,
    pub(crate) transitions: Vec<GraphTransition>,
    pub(crate) merge_counter: usize,
}

pub(crate) struct GraphPlace {
    pub(crate) place: PtPlace,
    pub(crate) originals: Vec<PlaceId>,
    pub(crate) incoming: Vec<usize>,
    pub(crate) outgoing: Vec<usize>,
    pub(crate) removed: bool,
}

pub(crate) struct GraphTransition {
    pub(crate) transition: PtTransition,
    pub(crate) originals: Vec<TransitionId>,
    pub(crate) inputs: Vec<(usize, usize)>,
    pub(crate) outputs: Vec<(usize, usize)>,
    pub(crate) removed: bool,
}

pub(crate) struct MaterializedNet {
    pub(crate) net: PtNet,
    pub(crate) marking: Marking,
    pub(crate) trace: ReductionTrace,
}

impl ReductionGraph {
    pub(crate) fn from_net(net: &PtNet, marking: &Marking) -> Self {
        let mut places: Vec<GraphPlace> = net
            .places
            .iter()
            .enumerate()
            .map(|(i, place)| GraphPlace {
                place: PtPlace {
                    name: place.name.clone(),
                    tokens: marking.tokens(PlaceId(i)),
                    capacity: place.kind.capacity.unwrap_or(usize::MAX),
                    place_type: place.kind.place_type.clone(),
                    span: place.kind.span.clone(),
                },
                originals: vec![PlaceId(i)],
                incoming: Vec::new(),
                outgoing: Vec::new(),
                removed: false,
            })
            .collect();

        let mut transitions: Vec<GraphTransition> = net
            .transitions
            .iter()
            .enumerate()
            .map(|(i, transition)| GraphTransition {
                transition: PtTransition {
                    name: transition.name.clone(),
                    transition_type: transition.kind.transition_type.clone(),
                },
                originals: vec![TransitionId(i)],
                inputs: Vec::new(),
                outputs: Vec::new(),
                removed: false,
            })
            .collect();

        for arc in &net.arcs {
            let p = arc.place.index();
            let t = arc.transition.index();
            match arc.direction {
                ArcDir::Input => {
                    places[p].outgoing.push(t);
                    transitions[t].inputs.push((p, arc.weight));
                }
                ArcDir::Output => {
                    places[p].incoming.push(t);
                    transitions[t].outputs.push((p, arc.weight));
                }
                _ => {}
            }
        }

        Self {
            places,
            transitions,
            merge_counter: 0,
        }
    }

    pub(crate) fn materialize(&self) -> MaterializedNet {
        let mut place_mapping: Vec<Option<PlaceId>> = vec![None; self.places.len()];
        let mut transition_mapping: Vec<Option<TransitionId>> = vec![None; self.transitions.len()];

        let mut builder = PtBuilder::empty();
        for (idx, place) in self.places.iter().enumerate() {
            if place.removed {
                continue;
            }
            let new_id = builder.add_place(place.place.clone());
            place_mapping[idx] = Some(new_id);
        }
        for (idx, transition) in self.transitions.iter().enumerate() {
            if transition.removed {
                continue;
            }
            let new_id = builder.add_transition(transition.transition.clone());
            transition_mapping[idx] = Some(new_id);
        }

        for (t_idx, transition) in self.transitions.iter().enumerate() {
            let Some(new_t) = transition_mapping[t_idx] else {
                continue;
            };
            for (place_idx, weight) in &transition.inputs {
                if let Some(new_p) = place_mapping[*place_idx] {
                    builder.set_input_weight(new_p, new_t, *weight);
                }
            }
            for (place_idx, weight) in &transition.outputs {
                if let Some(new_p) = place_mapping[*place_idx] {
                    builder.set_output_weight(new_p, new_t, *weight);
                }
            }
        }

        let (net, marking) = builder.build();

        let place_trace: Vec<Vec<PlaceId>> = self
            .places
            .iter()
            .enumerate()
            .filter_map(|(idx, place)| place_mapping[idx].map(|_| place.originals.clone()))
            .collect();

        let transition_trace: Vec<Vec<TransitionId>> = self
            .transitions
            .iter()
            .enumerate()
            .filter_map(|(idx, transition)| {
                transition_mapping[idx].map(|_| transition.originals.clone())
            })
            .collect();

        MaterializedNet {
            net,
            marking,
            trace: ReductionTrace {
                place_mapping: place_trace,
                transition_mapping: transition_trace,
            },
        }
    }

    pub(crate) fn add_transition(&mut self, transition: GraphTransition) -> usize {
        self.transitions.push(transition);
        self.transitions.len() - 1
    }

    pub(crate) fn remove_transition(&mut self, idx: usize) {
        if idx >= self.transitions.len() || self.transitions[idx].removed {
            return;
        }
        let inputs = self.transitions[idx].inputs.clone();
        let outputs = self.transitions[idx].outputs.clone();
        for (place_idx, _) in inputs {
            if let Some(place) = self.places.get_mut(place_idx) {
                place.outgoing.retain(|t| *t != idx);
            }
        }
        for (place_idx, _) in outputs {
            if let Some(place) = self.places.get_mut(place_idx) {
                place.incoming.retain(|t| *t != idx);
            }
        }
        self.transitions[idx].inputs.clear();
        self.transitions[idx].outputs.clear();
        self.transitions[idx].removed = true;
    }

    pub(crate) fn remove_place(&mut self, idx: usize) {
        if idx >= self.places.len() || self.places[idx].removed {
            return;
        }
        let incoming = self.places[idx].incoming.clone();
        let outgoing = self.places[idx].outgoing.clone();
        for transition_idx in incoming {
            if let Some(transition) = self.transitions.get_mut(transition_idx) {
                transition
                    .outputs
                    .retain(|(place_idx, _)| *place_idx != idx);
            }
        }
        for transition_idx in outgoing {
            if let Some(transition) = self.transitions.get_mut(transition_idx) {
                transition.inputs.retain(|(place_idx, _)| *place_idx != idx);
            }
        }
        self.places[idx].incoming.clear();
        self.places[idx].outgoing.clear();
        self.places[idx].removed = true;
    }

    pub(crate) fn clean_adjacency(&mut self) {
        for place in &mut self.places {
            place
                .incoming
                .retain(|idx| *idx < self.transitions.len() && !self.transitions[*idx].removed);
            place
                .outgoing
                .retain(|idx| *idx < self.transitions.len() && !self.transitions[*idx].removed);
        }
        for transition in &mut self.transitions {
            if transition.removed {
                continue;
            }
            transition
                .inputs
                .retain(|(idx, _)| *idx < self.places.len() && !self.places[*idx].removed);
            transition
                .outputs
                .retain(|(idx, _)| *idx < self.places.len() && !self.places[*idx].removed);
        }
    }
}

impl GraphTransition {
    pub(crate) fn new_with_type(
        name: String,
        transition_type: TransitionType,
        originals: Vec<TransitionId>,
        inputs: Vec<(usize, usize)>,
        outputs: Vec<(usize, usize)>,
    ) -> Self {
        Self {
            transition: PtTransition::new_with_transition_type(name, transition_type),
            originals,
            inputs,
            outputs,
            removed: false,
        }
    }
}
