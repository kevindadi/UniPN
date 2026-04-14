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
                let (kind_label, color, style) = transition_style(&t.kind);
                writeln!(
                    out,
                    "  \"{}\" [label=\"{}\\n({})\", shape=box, style=\"{}\", fillcolor=gray90, color={}];",
                    t.id, t.id, kind_label, style, color
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

/// Returns `(label, color, style)` for a transition node based on its kind.
fn transition_style(kind: &TransitionKind) -> (String, &'static str, &'static str) {
    match kind {
        TransitionKind::Sequential => ("sequential".into(), "black", "filled"),
        TransitionKind::Lock => ("lock".into(), "red", "filled"),
        TransitionKind::Unlock => ("unlock".into(), "green", "filled"),
        TransitionKind::ReadLock => ("read_lock".into(), "blue", "filled"),
        TransitionKind::ReadUnlock => ("read_unlock".into(), "teal", "filled"),
        TransitionKind::Acquire => ("acquire".into(), "red", "filled,dashed"),
        TransitionKind::Release => ("release".into(), "green", "filled,dashed"),
        TransitionKind::Send => ("send".into(), "cyan4", "filled"),
        TransitionKind::Recv => ("recv".into(), "cyan4", "filled,bold"),
        TransitionKind::VarRead => ("var_read".into(), "black", "filled"),
        TransitionKind::VarWrite => ("var_write".into(), "orange", "filled"),
        TransitionKind::AtomicLoad => ("atomic_load".into(), "black", "filled"),
        TransitionKind::AtomicStore => ("atomic_store".into(), "orange", "filled,bold"),
        TransitionKind::BranchTrue => ("branch_T".into(), "green", "filled"),
        TransitionKind::BranchFalse => ("branch_F".into(), "red", "filled"),
        TransitionKind::Switch { label } => (format!("switch({label})"), "orange", "filled"),
        TransitionKind::CasSuccess => ("cas_succ".into(), "green", "filled"),
        TransitionKind::CasFailure => ("cas_fail".into(), "red", "filled"),
        TransitionKind::Spawn => ("spawn".into(), "blue", "filled"),
        TransitionKind::Join => ("join".into(), "blue", "filled,dashed"),
        TransitionKind::Call => ("call".into(), "black", "filled,rounded"),
        TransitionKind::CondvarWaitEnter => ("cv_wait_enter".into(), "purple", "filled"),
        TransitionKind::CondvarWakeByNotify => ("cv_wake1".into(), "purple", "filled"),
        TransitionKind::CondvarWakeByNotifyAll => ("cv_wakeA".into(), "purple", "filled"),
        TransitionKind::CondvarReacquire => ("cv_reacquire".into(), "purple", "filled,dotted"),
        TransitionKind::CondvarNotify => ("cv_notify".into(), "purple", "filled,dashed"),
        TransitionKind::CondvarNotifyLost => ("cv_notify_lost".into(), "purple", "filled,dashed"),
        TransitionKind::CondvarNotifyAll => ("cv_notify_all".into(), "purple", "filled,dashed"),
        TransitionKind::CondvarNotifyAllLost => {
            ("cv_notify_all_lost".into(), "purple", "filled,dashed")
        }
        TransitionKind::Return => ("return".into(), "black", "filled"),
    }
}
