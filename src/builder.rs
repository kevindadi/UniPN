//! Builder for constructing a [`CvnNet`] with validation.
//!
//! Use [`CvnNetBuilder`] to incrementally add places, transitions, and arcs,
//! then call [`build()`](CvnNetBuilder::build) to perform well-formedness
//! validation and produce a [`CvnNet`].

use crate::error::CvnError;
use crate::model::*;
use crate::net::{CvnNet, NetEdge, NetNode};
use crate::validate;
use indexmap::IndexMap;
use petgraph::graph::DiGraph;
use rustc_hash::FxHashMap;

/// Builder for constructing a CVN network.
///
/// # Example
///
/// ```
/// use cvn::builder::CvnNetBuilder;
/// use cvn::model::*;
///
/// let net = CvnNetBuilder::new()
///     .add_control_place("p0", "main", "s0")
///     .add_control_place("p1", "main", "s1")
///     .set_return("p1")
///     .add_transition("t0", TransitionKind::Sequential)
///     .add_input_arc("p0", "t0", 1, BoolExpr::True)
///     .add_output_arc("t0", "p1", 1, None)
///     .set_initial_tokens("p0", 1)
///     .build();
/// assert!(net.is_ok());
/// ```
pub struct CvnNetBuilder {
    places: IndexMap<String, Place>,
    transitions: IndexMap<String, Transition>,
    input_arcs: Vec<InputArcData>,
    output_arcs: Vec<OutputArcData>,
    initial_tokens: FxHashMap<String, u32>,
    initial_vars: IndexMap<String, Val>,
    return_places: Vec<String>,
}

impl Default for CvnNetBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl CvnNetBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            places: IndexMap::new(),
            transitions: IndexMap::new(),
            input_arcs: Vec::new(),
            output_arcs: Vec::new(),
            initial_tokens: FxHashMap::default(),
            initial_vars: IndexMap::new(),
            return_places: Vec::new(),
        }
    }

    /// Add a control place (thread at a specific statement).
    pub fn add_control_place(
        mut self,
        id: impl Into<String>,
        fn_name: impl Into<String>,
        sid: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let place = Place::new(
            PlaceId::new(id.clone()),
            PlaceKind::Control {
                fn_name: fn_name.into(),
                sid: sid.into(),
            },
        );
        self.places.insert(id, place);
        self
    }

    /// Add a resource place.
    pub fn add_resource_place(
        mut self,
        id: impl Into<String>,
        res_name: impl Into<String>,
        resource_type: ResourceType,
    ) -> Self {
        let id = id.into();
        let place = Place::new(
            PlaceId::new(id.clone()),
            PlaceKind::Resource {
                res_name: res_name.into(),
                resource_type,
            },
        );
        self.places.insert(id, place);
        self
    }

    /// Add a wait place (condvar wait point).
    pub fn add_wait_place(
        mut self,
        id: impl Into<String>,
        cv_name: impl Into<String>,
        fn_name: impl Into<String>,
        sid: impl Into<String>,
    ) -> Self {
        let id = id.into();
        let place = Place::new(
            PlaceId::new(id.clone()),
            PlaceKind::Wait {
                cv_name: cv_name.into(),
                fn_name: fn_name.into(),
                sid: sid.into(),
            },
        );
        self.places.insert(id, place);
        self
    }

    /// Mark a place as a return/terminal place.
    pub fn set_return(mut self, place_id: impl Into<String>) -> Self {
        self.return_places.push(place_id.into());
        self
    }

    /// Add a transition.
    pub fn add_transition(
        mut self,
        id: impl Into<String>,
        kind: TransitionKind,
    ) -> Self {
        let id = id.into();
        let t = Transition::new(TransitionId::new(id.clone()), kind);
        self.transitions.insert(id, t);
        self
    }

    /// Add a transition with CIR statement ID anchors.
    #[cfg(feature = "cir-anchor")]
    pub fn add_transition_with_anchor(
        mut self,
        id: impl Into<String>,
        kind: TransitionKind,
        sids: &[impl AsRef<str>],
    ) -> Self {
        let id = id.into();
        let t = Transition::with_anchor(
            TransitionId::new(id.clone()),
            kind,
            sids.iter().map(|s| s.as_ref().to_string()),
        );
        self.transitions.insert(id, t);
        self
    }

    /// Add an input arc (Place → Transition).
    pub fn add_input_arc(
        mut self,
        place_id: impl Into<String>,
        transition_id: impl Into<String>,
        weight: u32,
        guard: BoolExpr,
    ) -> Self {
        self.input_arcs.push(InputArcData {
            place: PlaceId::new(place_id),
            transition: TransitionId::new(transition_id),
            weight,
            guard,
        });
        self
    }

    /// Add an output arc (Transition → Place).
    pub fn add_output_arc(
        mut self,
        transition_id: impl Into<String>,
        place_id: impl Into<String>,
        weight: u32,
        update: Option<VarUpdate>,
    ) -> Self {
        self.output_arcs.push(OutputArcData {
            transition: TransitionId::new(transition_id),
            place: PlaceId::new(place_id),
            weight,
            update,
        });
        self
    }

    /// Set initial token count for a place.
    pub fn set_initial_tokens(mut self, place_id: impl Into<String>, count: u32) -> Self {
        self.initial_tokens.insert(place_id.into(), count);
        self
    }

    /// Add a variable with its initial value.
    pub fn add_variable(mut self, name: impl Into<String>, initial_value: Val) -> Self {
        self.initial_vars.insert(name.into(), initial_value);
        self
    }

    /// Construct the graph and apply return flags; shared by both build methods.
    fn build_net(mut self) -> (CvnNet, Vec<InputArcData>, Vec<OutputArcData>) {
        for pid in &self.return_places {
            if let Some(place) = self.places.get_mut(pid) {
                place.is_return = true;
            }
        }

        let mut graph = DiGraph::<NetNode, NetEdge>::new();
        let mut place_index = FxHashMap::default();
        let mut transition_index = FxHashMap::default();

        for (_, place) in &self.places {
            let idx = graph.add_node(NetNode::Place(place.clone()));
            place_index.insert(place.id.clone(), idx);
        }

        for (_, transition) in &self.transitions {
            let idx = graph.add_node(NetNode::Transition(transition.clone()));
            transition_index.insert(transition.id.clone(), idx);
        }

        for arc in &self.input_arcs {
            if let (Some(&p_idx), Some(&t_idx)) =
                (place_index.get(&arc.place), transition_index.get(&arc.transition))
            {
                graph.add_edge(p_idx, t_idx, NetEdge::Input(arc.clone()));
            }
        }

        for arc in &self.output_arcs {
            if let (Some(&t_idx), Some(&p_idx)) =
                (transition_index.get(&arc.transition), place_index.get(&arc.place))
            {
                graph.add_edge(t_idx, p_idx, NetEdge::Output(arc.clone()));
            }
        }

        let mut initial_marking = Marking::default();
        for (pid, count) in &self.initial_tokens {
            if *count > 0 {
                initial_marking.insert(PlaceId::new(pid.clone()), *count);
            }
        }

        let net = CvnNet::from_parts(
            graph,
            place_index,
            transition_index,
            initial_marking,
            self.initial_vars,
        );

        (net, self.input_arcs, self.output_arcs)
    }

    /// Build the CVN network, performing well-formedness validation.
    ///
    /// Returns the constructed [`CvnNet`] on success, or a list of validation
    /// errors on failure.
    pub fn build(self) -> Result<CvnNet, Vec<CvnError>> {
        let (net, input_arcs, output_arcs) = self.build_net();
        let errors = validate::validate(&net, &input_arcs, &output_arcs);
        if errors.is_empty() {
            Ok(net)
        } else {
            Err(errors)
        }
    }

    /// Build the CVN network with additional anchor completeness validation (W7).
    ///
    /// Like [`build()`](Self::build), but additionally checks that every transition
    /// has at least one CIR statement ID anchor (V105).
    #[cfg(feature = "cir-anchor")]
    pub fn build_with_anchor_check(self) -> Result<CvnNet, Vec<CvnError>> {
        let (net, input_arcs, output_arcs) = self.build_net();
        let mut errors = validate::validate(&net, &input_arcs, &output_arcs);
        errors.extend(validate::check_anchor_sids(&net));
        if errors.is_empty() {
            Ok(net)
        } else {
            Err(errors)
        }
    }
}
