//! Graphviz DOT export and connectivity diagnostics for P/T nets.

use crate::net::{ArcDir, PlaceId, TransitionId};

use super::kinds::PtNet;

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
