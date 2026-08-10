//! DOT-export smoke test.

mod common;

use unipn::export::to_dot;

#[test]
fn dot_export_produces_graph() {
    let net = common::mutex_deadlock();
    let dot = to_dot(&net);
    assert!(dot.starts_with("digraph PetriNet {"));
    assert!(dot.contains("->"));
    assert!(dot.contains("lock"));
}
