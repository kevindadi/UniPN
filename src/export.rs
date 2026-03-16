//! DOT format export for CVN networks.
//!
//! Produces Graphviz DOT output for visualization of the Petri net structure.

use crate::model::*;
use crate::net::{CvnNet, NetEdge, NetNode};
use petgraph::visit::EdgeRef;
use std::fmt::Write;

/// Export a CVN network to Graphviz DOT format.
///
/// # Place styling
/// - Control places: ellipse, lightblue fill
/// - Resource places: double circle, orange fill
/// - Wait places: ellipse, salmon fill
///
/// # Transition styling
/// - Filled box/rectangle, gray fill
///
/// # Arc labels
/// - Weight shown if > 1
/// - Guard shown if not `True`
pub fn to_dot(net: &CvnNet) -> String {
    let mut out = String::new();
    writeln!(out, "digraph CVN {{").unwrap();
    writeln!(out, "  rankdir=TB;").unwrap();
    writeln!(out, "  node [fontname=\"Helvetica\"];").unwrap();
    writeln!(out, "  edge [fontname=\"Helvetica\"];").unwrap();
    writeln!(out).unwrap();

    let graph = net.petgraph();

    // Emit place nodes
    for idx in graph.node_indices() {
        match &graph[idx] {
            NetNode::Place(place) => {
                let (shape, fillcolor, label) = match &place.kind {
                    PlaceKind::Control { fn_name, sid } => {
                        let label = if place.is_return {
                            format!("{fn_name}::{sid}\\n(ret)")
                        } else {
                            format!("{fn_name}::{sid}")
                        };
                        ("ellipse", "lightblue", label)
                    }
                    PlaceKind::Resource {
                        res_name,
                        resource_type,
                    } => {
                        let type_str = match resource_type {
                            ResourceType::Mutex => "Mutex".to_string(),
                            ResourceType::RwLock { max_readers } => {
                                format!("RwLock(N={max_readers})")
                            }
                            ResourceType::Semaphore { count } => format!("Sem({count})"),
                            ResourceType::Channel => "Chan".to_string(),
                            ResourceType::Condvar => "CV".to_string(),
                        };
                        let label = format!("{res_name}\\n[{type_str}]");
                        ("doublecircle", "orange", label)
                    }
                    PlaceKind::Wait {
                        cv_name,
                        fn_name,
                        sid,
                    } => {
                        let label = format!("wait({cv_name})\\n{fn_name}::{sid}");
                        ("ellipse", "salmon", label)
                    }
                };

                let tokens = net.initial_marking().get(&place.id).copied().unwrap_or(0);
                let token_str = if tokens > 0 {
                    format!("\\n[{tokens}]")
                } else {
                    String::new()
                };

                writeln!(
                    out,
                    "  \"{}\" [label=\"{}{}\", shape={}, style=filled, fillcolor={}];",
                    place.id, label, token_str, shape, fillcolor
                )
                .unwrap();
            }
            NetNode::Transition(t) => {
                let kind_str = format!("{:?}", t.kind);
                writeln!(
                    out,
                    "  \"{}\" [label=\"{}\\n({})\", shape=box, style=filled, fillcolor=gray90];",
                    t.id, t.id, kind_str
                )
                .unwrap();
            }
        }
    }

    writeln!(out).unwrap();

    // Emit edges
    for edge in graph.edge_references() {
        let source = &graph[edge.source()];
        let target = &graph[edge.target()];
        let (source_id, target_id) = match (source, target) {
            (NetNode::Place(p), NetNode::Transition(t)) => (&p.id.0, &t.id.0),
            (NetNode::Transition(t), NetNode::Place(p)) => (&t.id.0, &p.id.0),
            _ => continue,
        };

        let label = match edge.weight() {
            NetEdge::Input(arc) => {
                let mut parts = Vec::new();
                if arc.weight > 1 {
                    parts.push(format!("w={}", arc.weight));
                }
                if arc.guard != BoolExpr::True {
                    parts.push(format!("[{}]", arc.guard));
                }
                parts.join(" ")
            }
            NetEdge::Output(arc) => {
                let mut parts = Vec::new();
                if arc.weight > 1 {
                    parts.push(format!("w={}", arc.weight));
                }
                if let Some(update) = &arc.update {
                    for (var, expr) in update {
                        parts.push(format!("{var}:={expr}"));
                    }
                }
                parts.join(" ")
            }
        };

        if label.is_empty() {
            writeln!(out, "  \"{}\" -> \"{}\";", source_id, target_id).unwrap();
        } else {
            writeln!(
                out,
                "  \"{}\" -> \"{}\" [label=\"{}\"];",
                source_id, target_id, label
            )
            .unwrap();
        }
    }

    writeln!(out, "}}").unwrap();
    out
}
