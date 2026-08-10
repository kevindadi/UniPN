//! 导出：Graphviz DOT。

use crate::model::{ControlSub, PlaceKind, ResourceType, TransitionKind};
use crate::netlike::NetLike;

/// 将任意 `NetLike` 网导出为 DOT。
pub fn to_dot(net: &dyn NetLike) -> String {
    let mut out = String::new();
    out.push_str("digraph PetriNet {\n");
    out.push_str("    rankdir=LR;\n");

    for p in net.place_ids() {
        let (shape, fill) = place_style(net.place_kind(p));
        out.push_str(&format!(
            "    p{} [label=\"{}\\n{}\", shape={shape}, style=filled, fillcolor=\"{fill}\"];\n",
            p.index(),
            escape(&net.place_label(p)),
            kind_tag(net.place_kind(p)),
        ));
    }

    for t in net.transition_ids() {
        let (label, fill) = transition_style(net.transition_kind(t));
        out.push_str(&format!(
            "    t{} [label=\"{}\\n{}\", shape=box, style=filled, fillcolor=\"{fill}\"];\n",
            t.index(),
            escape(&net.transition_label(t)),
            escape(&label),
        ));
    }

    for t in net.transition_ids() {
        for (p, w) in net.pre_arcs(t) {
            let l = if w == 1 {
                String::new()
            } else {
                format!(" [label=\"{w}\"]")
            };
            out.push_str(&format!("    p{} -> t{}{l};\n", p.index(), t.index()));
        }
        for (p, w) in net.post_arcs(t) {
            let l = if w == 1 {
                String::new()
            } else {
                format!(" [label=\"{w}\"]")
            };
            out.push_str(&format!("    t{} -> p{}{l};\n", t.index(), p.index()));
        }
    }

    out.push_str("}\n");
    out
}

fn place_style(kind: Option<PlaceKind>) -> (&'static str, &'static str) {
    match kind {
        Some(PlaceKind::Resource(_)) => ("box", "#c8e6c9"),
        Some(PlaceKind::Control(ControlSub::WaitPoint)) => ("diamond", "#ffe0b2"),
        _ => ("circle", "#e3f2fd"),
    }
}

fn transition_style(kind: Option<TransitionKind>) -> (String, &'static str) {
    let fill: &'static str = match kind {
        Some(TransitionKind::Lock)
        | Some(TransitionKind::ReadLock)
        | Some(TransitionKind::Acquire) => "#ffcdd2",
        Some(TransitionKind::Unlock)
        | Some(TransitionKind::ReadUnlock)
        | Some(TransitionKind::Release) => "#c8e6c9",
        Some(TransitionKind::Spawn) => "#fff9c4",
        Some(TransitionKind::Join) => "#b3e5fc",
        Some(TransitionKind::Send) | Some(TransitionKind::Recv) => "#d1c4e9",
        Some(TransitionKind::CondvarWaitEnter)
        | Some(TransitionKind::CondvarNotify)
        | Some(TransitionKind::CondvarNotifyAll) => "#ffe0b2",
        Some(TransitionKind::Return) | Some(TransitionKind::FunctionExit) => "#e0e0e0",
        Some(TransitionKind::BranchTrue) | Some(TransitionKind::BranchFalse) => "#f0f0f0",
        _ => "#ffe0b2",
    };
    let label = match kind {
        Some(TransitionKind::Lock)
        | Some(TransitionKind::ReadLock)
        | Some(TransitionKind::Acquire) => "lock".to_string(),
        Some(TransitionKind::Unlock)
        | Some(TransitionKind::ReadUnlock)
        | Some(TransitionKind::Release) => "unlock".to_string(),
        Some(TransitionKind::Spawn) => "spawn".to_string(),
        Some(TransitionKind::Join) => "join".to_string(),
        Some(TransitionKind::Send) => "send".to_string(),
        Some(TransitionKind::Recv) => "recv".to_string(),
        Some(TransitionKind::CondvarWaitEnter) => "cv_wait".to_string(),
        Some(TransitionKind::CondvarNotify) | Some(TransitionKind::CondvarNotifyAll) => {
            "cv_notify".to_string()
        }
        Some(TransitionKind::Return) | Some(TransitionKind::FunctionExit) => "return".to_string(),
        Some(TransitionKind::BranchTrue) | Some(TransitionKind::BranchFalse) => {
            "branch".to_string()
        }
        Some(TransitionKind::Switch { label }) => label,
        _ => String::new(),
    };
    (label, fill)
}

fn kind_tag(kind: Option<PlaceKind>) -> String {
    match kind {
        Some(PlaceKind::Resource(ResourceType::Mutex)) => "mutex".into(),
        Some(PlaceKind::Resource(ResourceType::RwLock { .. })) => "rwlock".into(),
        Some(PlaceKind::Resource(ResourceType::Semaphore { .. })) => "sem".into(),
        Some(PlaceKind::Resource(ResourceType::Channel)) => "ch".into(),
        Some(PlaceKind::Resource(ResourceType::Condvar)) => "cv".into(),
        Some(PlaceKind::Control(ControlSub::ThreadEnd)) => "end".into(),
        Some(PlaceKind::Control(ControlSub::WaitPoint)) => "wait".into(),
        _ => "".into(),
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
