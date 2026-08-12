//! The ordinary P/T net backend (ConcBugDect's MIR→PN lowering target).
//!
//! `PtNet` is [`Net`] instantiated with ConcBugDect's place/transition metadata
//! kinds and no arc kind (the `ArcDir` already distinguishes input/output/read/
//! inhibitor/reset). The kinds mirror ConcBugDect's `net/structure.rs`, with
//! the rustc-private `AliasId` decoupled to plain integers.

use serde::{Deserialize, Serialize};

use crate::analysis::NetLike;
use crate::ids::{PlaceId, TransitionId};
use crate::net::{ArcDir, Marking, Net};

/// ConcBugDect place classification.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PlaceType {
    Resources,
    FunctionStart,
    FunctionEnd,
    BasicBlock,
}

/// Atomic memory ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AtomicOrdering {
    Relaxed,
    Release,
    Acquire,
    AcqRel,
    SeqCst,
}

/// A decoupled pointer-analysis alias identifier (ConcBugDect's rustc-private
/// `AliasId`, reduced to plain integers).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AliasId {
    pub instance_id: usize,
    pub local: usize,
    pub array_index: Option<u64>,
    pub field: Option<u32>,
}

/// An unsafe memory access.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UnsafeOp {
    /// Unsafe alias-group id.
    pub alias: usize,
    pub is_write: bool,
    pub span: String,
    pub basic_block: usize,
    pub ty: String,
}

/// ConcBugDect transition classification.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionType {
    Start(usize),
    Goto,
    Switch,
    Return(usize),
    Unlock(usize),
    DropRead(usize),
    DropWrite(usize),
    Drop,
    Assert,

    UnsafeRead(usize, String, usize, String),
    UnsafeWrite(usize, String, usize, String),
    /// One merged transition per basic block summarizing every unsafe access.
    UnsafeAccess(Vec<UnsafeOp>),

    Lock(usize),
    RwLockRead(usize),
    RwLockWrite(usize),
    Notify(usize),
    Wait,

    AtomicLoad(AliasId, AtomicOrdering, String, usize),
    AtomicStore(AliasId, AtomicOrdering, String, usize),
    AtomicCmpXchg(AliasId, AtomicOrdering, AtomicOrdering, String, usize),
    Spawn(String),
    Join(String),

    Function,
    Normal,
    Inhibitor,
    Reset,
}

/// ConcBugDect place attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtPlaceKind {
    pub place_type: PlaceType,
    pub span: String,
    /// `None` = unbounded.
    pub capacity: Option<usize>,
}

impl PtPlaceKind {
    pub fn new(place_type: PlaceType) -> Self {
        Self {
            place_type,
            span: String::new(),
            capacity: None,
        }
    }
}

/// ConcBugDect transition attributes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtTransitionKind {
    pub transition_type: TransitionType,
}

impl PtTransitionKind {
    pub fn new(transition_type: TransitionType) -> Self {
        Self { transition_type }
    }
}

/// The ordinary P/T net (no arc payload).
pub type PtNet = Net<PtPlaceKind, PtTransitionKind, ()>;

/// A place under construction (mirrors ConcBugDect's `Place`); `tokens` is the
/// initial marking and `usize::MAX` capacity means unbounded.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtPlace {
    pub name: String,
    pub tokens: usize,
    pub capacity: usize,
    pub place_type: PlaceType,
    pub span: String,
}

impl PtPlace {
    pub fn new(
        name: impl Into<String>,
        tokens: usize,
        capacity: usize,
        place_type: PlaceType,
        span: String,
    ) -> Self {
        Self {
            name: name.into(),
            tokens,
            capacity,
            place_type,
            span,
        }
    }
}

/// A transition under construction (mirrors ConcBugDect's `Transition`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PtTransition {
    pub name: String,
    pub transition_type: TransitionType,
}

impl PtTransition {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            transition_type: TransitionType::Normal,
        }
    }

    pub fn new_with_transition_type(
        name: impl Into<String>,
        transition_type: TransitionType,
    ) -> Self {
        Self {
            name: name.into(),
            transition_type,
        }
    }
}

/// Chain-style P/T builder (mirrors ConcBugDect's `Net` construction API):
/// accumulates the net plus its initial marking.
#[derive(Clone, Debug, Default)]
pub struct PtBuilder {
    net: PtNet,
    marking: Vec<usize>,
}

impl PtBuilder {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn add_place(&mut self, place: PtPlace) -> PlaceId {
        let id = self.net.add_place(
            place.name,
            PtPlaceKind {
                place_type: place.place_type,
                span: place.span,
                capacity: (place.capacity != usize::MAX).then_some(place.capacity),
            },
        );
        self.marking.push(place.tokens);
        id
    }

    pub fn add_transition(&mut self, transition: PtTransition) -> TransitionId {
        self.net.add_transition(
            transition.name,
            PtTransitionKind {
                transition_type: transition.transition_type,
            },
        )
    }

    pub fn add_input_arc(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.add_weighted_arc(place, transition, ArcDir::Input, weight);
    }

    pub fn add_output_arc(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.add_weighted_arc(place, transition, ArcDir::Output, weight);
    }

    pub fn set_input_weight(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.set_weighted_arc(place, transition, ArcDir::Input, weight);
    }

    pub fn set_output_weight(&mut self, place: PlaceId, transition: TransitionId, weight: usize) {
        self.set_weighted_arc(place, transition, ArcDir::Output, weight);
    }

    fn add_weighted_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
    ) {
        if weight == 0 {
            return;
        }
        if let Some(arc) =
            self.net.arcs.iter_mut().find(|a| {
                a.place == place && a.transition == transition && a.direction == direction
            })
        {
            arc.weight = arc.weight.saturating_add(weight);
        } else {
            self.net.add_arc(place, transition, direction, weight, ());
        }
    }

    fn set_weighted_arc(
        &mut self,
        place: PlaceId,
        transition: TransitionId,
        direction: ArcDir,
        weight: usize,
    ) {
        if let Some(arc) =
            self.net.arcs.iter_mut().find(|a| {
                a.place == place && a.transition == transition && a.direction == direction
            })
        {
            arc.weight = weight;
        } else {
            self.net.add_arc(place, transition, direction, weight, ());
        }
    }

    pub fn places_len(&self) -> usize {
        self.net.num_places()
    }

    pub fn transitions_len(&self) -> usize {
        self.net.num_transitions()
    }

    pub fn initial_marking(&self) -> Marking {
        Marking::new(self.marking.clone())
    }

    /// Mutable access to a transition's kind (for post-creation mutation).
    pub fn transition_mut(&mut self, transition: TransitionId) -> Option<&mut PtTransitionKind> {
        self.net
            .transitions
            .get_mut(transition.index())
            .map(|t| &mut t.kind)
    }

    /// Mutable access to a place's kind (for post-creation capacity mutation).
    pub fn place_mut(&mut self, place: PlaceId) -> Option<&mut PtPlaceKind> {
        self.net.places.get_mut(place.index()).map(|p| &mut p.kind)
    }

    pub fn set_place_tokens(&mut self, place: PlaceId, tokens: usize) {
        if let Some(slot) = self.marking.get_mut(place.index()) {
            *slot = tokens;
        }
    }

    /// The underlying net (read-only view while building).
    pub fn net(&self) -> &PtNet {
        &self.net
    }

    /// A cloned snapshot of the built net + initial marking (without consuming
    /// the builder, so construction can continue).
    pub fn snapshot(&self) -> (PtNet, Marking) {
        (self.net.clone(), self.initial_marking())
    }

    pub fn to_dot(&self) -> String {
        self.net.to_dot()
    }

    pub fn write_dot<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        self.net.write_dot(path)
    }

    pub fn diagnose_connectivity(&self) -> DiagnosticReport {
        self.net.diagnose_connectivity()
    }

    pub fn log_diagnostics(&self) {
        self.net.log_diagnostics()
    }

    pub fn build(self) -> (PtNet, Marking) {
        (self.net, Marking::new(self.marking))
    }
}

impl NetLike for PtNet {
    type State = Marking;

    fn num_places(&self) -> usize {
        self.places.len()
    }

    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    fn enabled(&self, state: &Self::State) -> Vec<TransitionId> {
        self.transitions
            .iter()
            .filter(|t| self.is_enabled(state, t.id))
            .map(|t| t.id)
            .collect()
    }

    fn fire(&self, state: &Self::State, transition: TransitionId) -> Option<Self::State> {
        if !self.is_enabled(state, transition) {
            return None;
        }
        let mut next = state.clone();

        for arc in self.arcs_of(transition, ArcDir::Input) {
            let current = next.tokens(arc.place);
            next.set(arc.place, current.checked_sub(arc.weight)?);
        }

        for arc in self.arcs_of(transition, ArcDir::Output) {
            let current = next.tokens(arc.place);
            let produced = current.checked_add(arc.weight)?;
            // Saturating capacity clamp (ConcBugDect's firing semantics).
            let clamped = self
                .place(arc.place)
                .and_then(|p| p.kind.capacity)
                .map_or(produced, |cap| produced.min(cap));
            next.set(arc.place, clamped);
        }

        for arc in self.arcs_of(transition, ArcDir::Reset) {
            next.set(arc.place, 0);
        }

        Some(next)
    }
}

impl PtNet {
    /// The aggregate input weight on `place → transition` (0 if no arc).
    pub fn input_weight(&self, place: PlaceId, transition: TransitionId) -> usize {
        self.arcs
            .iter()
            .filter(|a| a.place == place && a.transition == transition && a.direction == ArcDir::Input)
            .map(|a| a.weight)
            .sum()
    }

    /// The aggregate output weight on `transition → place` (0 if no arc).
    pub fn output_weight(&self, place: PlaceId, transition: TransitionId) -> usize {
        self.arcs
            .iter()
            .filter(|a| {
                a.place == place && a.transition == transition && a.direction == ArcDir::Output
            })
            .map(|a| a.weight)
            .sum()
    }

    /// `Result`-shaped firing (mirrors ConcBugDect's `fire_transition`).
    pub fn fire_transition(
        &self,
        marking: &Marking,
        transition: TransitionId,
    ) -> Result<Marking, ()> {
        NetLike::fire(self, marking, transition).ok_or(())
    }

    /// The enabled transitions under a marking (mirrors ConcBugDect's API).
    pub fn enabled_transitions(&self, marking: &Marking) -> Vec<TransitionId> {
        NetLike::enabled(self, marking)
    }

    fn is_enabled(&self, state: &Marking, transition: TransitionId) -> bool {
        // Aggregate input-arc weights per place.
        let mut required: Vec<(PlaceId, usize)> = Vec::new();
        for arc in self.arcs_for(transition) {
            match arc.direction {
                ArcDir::Input => {
                    if let Some((_, total)) = required.iter_mut().find(|(p, _)| *p == arc.place) {
                        *total = total.checked_add(arc.weight).unwrap_or(usize::MAX);
                    } else {
                        required.push((arc.place, arc.weight));
                    }
                }
                ArcDir::Read => {
                    if state.tokens(arc.place) < arc.weight {
                        return false;
                    }
                }
                ArcDir::Inhibitor => {
                    if state.tokens(arc.place) >= arc.weight {
                        return false;
                    }
                }
                ArcDir::Output | ArcDir::Reset => {}
            }
        }
        required
            .into_iter()
            .all(|(place, count)| state.tokens(place) >= count)
    }
}

/// Convenience: build a marking directly from a slice of counts.
pub fn marking(counts: impl IntoIterator<Item = usize>) -> Marking {
    Marking::new(counts.into_iter().collect())
}

/// Petri net connectivity diagnostic report.
#[derive(Clone, Debug, Default)]
pub struct DiagnosticReport {
    pub isolated_places: Vec<(PlaceId, String)>,
    pub isolated_transitions: Vec<(TransitionId, String)>,
    pub warnings: Vec<String>,
    pub total_places: usize,
    pub total_transitions: usize,
}

impl DiagnosticReport {
    pub fn has_issues(&self) -> bool {
        !self.isolated_places.is_empty()
            || !self.isolated_transitions.is_empty()
            || !self.warnings.is_empty()
    }

    pub fn save_to_file(&self, path: &str) -> std::io::Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(path)?;
        writeln!(file, "=== Petri net connectivity diagnostics ===")?;
        writeln!(
            file,
            "Totals: {} places, {} transitions",
            self.total_places, self.total_transitions
        )?;
        for (id, name) in &self.isolated_places {
            writeln!(file, "  [{}] {}", id.index(), name)?;
        }
        for (id, name) in &self.isolated_transitions {
            writeln!(file, "  [{}] {}", id.index(), name)?;
        }
        for warning in &self.warnings {
            writeln!(file, "  - {warning}")?;
        }
        Ok(())
    }
}

impl PtNet {
    pub fn to_dot(&self) -> String {
        let mut out = String::from("digraph PetriNet {\n  rankdir=LR;\n");
        for (i, place) in self.places.iter().enumerate() {
            let cap = place
                .kind
                .capacity
                .map_or("inf".to_string(), |c| c.to_string());
            out.push_str(&format!(
                "  p{i} [label=\"{}\\n{:?}\\n{}\", shape=circle];\n",
                place.name, place.kind.place_type, cap
            ));
        }
        for (i, transition) in self.transitions.iter().enumerate() {
            out.push_str(&format!(
                "  t{i} [label=\"{}\\n{:?}\", shape=box];\n",
                transition.name, transition.kind.transition_type
            ));
        }
        for arc in &self.arcs {
            match arc.direction {
                ArcDir::Input => out.push_str(&format!(
                    "  p{} -> t{};\n",
                    arc.place.index(),
                    arc.transition.index()
                )),
                ArcDir::Output => out.push_str(&format!(
                    "  t{} -> p{};\n",
                    arc.transition.index(),
                    arc.place.index()
                )),
                ArcDir::Inhibitor => out.push_str(&format!(
                    "  p{} -> t{} [style=dotted];\n",
                    arc.place.index(),
                    arc.transition.index()
                )),
                _ => {}
            }
        }
        out.push_str("}\n");
        out
    }

    pub fn write_dot<P: AsRef<std::path::Path>>(&self, path: P) -> std::io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.to_dot())
    }

    pub fn diagnose_connectivity(&self) -> DiagnosticReport {
        let mut report = DiagnosticReport {
            total_places: self.num_places(),
            total_transitions: self.num_transitions(),
            ..DiagnosticReport::default()
        };

        for (i, place) in self.places.iter().enumerate() {
            let pid = PlaceId(i);
            let has_incoming = self
                .arcs
                .iter()
                .any(|a| a.place == pid && a.direction == ArcDir::Output);
            let has_outgoing = self
                .arcs
                .iter()
                .any(|a| a.place == pid && a.direction == ArcDir::Input);
            if !has_incoming && !has_outgoing {
                report.isolated_places.push((pid, place.name.clone()));
            }
        }

        for (i, transition) in self.transitions.iter().enumerate() {
            let tid = TransitionId(i);
            let has_preset = self
                .arcs
                .iter()
                .any(|a| a.transition == tid && a.direction == ArcDir::Input);
            let has_postset = self
                .arcs
                .iter()
                .any(|a| a.transition == tid && a.direction == ArcDir::Output);
            if !has_preset && !has_postset {
                report
                    .isolated_transitions
                    .push((tid, transition.name.clone()));
            }
        }

        report
    }

    pub fn log_diagnostics(&self) {
        let report = self.diagnose_connectivity();
        if report.has_issues() {
            for (id, name) in &report.isolated_places {
                eprintln!("isolated place [{}] {name}", id.index());
            }
            for (id, name) in &report.isolated_transitions {
                eprintln!("isolated transition [{}] {name}", id.index());
            }
        }
    }
}
