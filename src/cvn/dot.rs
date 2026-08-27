//! Graphviz DOT export for CVN nets.

use crate::net::ArcDir;

use super::kinds::{ControlSub, CvnNet, PlaceKind, ResourceType};

/// Export the net as Graphviz DOT.
pub fn to_dot(net: &CvnNet) -> String {
    let mut out = String::from("digraph PetriNet {\n  rankdir=LR;\n");

    for p in net.place_ids() {
        let place = net.place(p).unwrap();
        let tag = match &place.kind {
            PlaceKind::Resource(ResourceType::Mutex) => "mutex",
            PlaceKind::Resource(ResourceType::RwLock { .. }) => "rwlock",
            PlaceKind::Resource(ResourceType::Semaphore { .. }) => "sem",
            PlaceKind::Resource(ResourceType::Channel { .. }) => "ch",
            PlaceKind::Resource(ResourceType::Condvar) => "cv",
            PlaceKind::Control(ControlSub::FunctionEnd) => "end",
            PlaceKind::Control(ControlSub::WaitPoint) => "wait",
            PlaceKind::Control(ControlSub::CallWait) => "call",
            _ => "",
        };
        out.push_str(&format!(
            "  p{} [label=\"{}\\n{}\"];\n",
            p.index(),
            escape_dot(&place.name),
            escape_dot(tag)
        ));
    }

    for t in net.transition_ids() {
        let tr = net.transition(t).unwrap();
        out.push_str(&format!(
            "  t{} [label=\"{}\\n{:?}\", shape=box];\n",
            t.index(),
            escape_dot(&tr.name),
            tr.kind.kind
        ));
    }

    for arc in &net.arcs {
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

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
