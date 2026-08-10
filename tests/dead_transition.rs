//! 行为死迁移检测。

use unipn::analysis::{AnalysisConfig, PropertyViolation, explore, find_dead_transitions};
use unipn::expr::BoolExpr;
use unipn::model::{ControlSub, PlaceKind, TransitionKind};
use unipn::NetBuilder;

fn net_with_dead_transition() -> unipn::Net {
    let mut b = NetBuilder::new();
    let p0 = b.add_place("p0", PlaceKind::Control(ControlSub::Statement));
    let p1 = b.add_place("p1", PlaceKind::Control(ControlSub::ThreadEnd));
    let p_dead = b.add_place("p_dead", PlaceKind::Control(ControlSub::Statement));
    let t_live = b.add_transition("t_live", TransitionKind::Sequential);
    let t_dead = b.add_transition("t_dead", TransitionKind::Sequential);
    b.add_input_arc(p0, t_live, 1, BoolExpr::True);
    b.add_output_arc(t_live, p1, 1, None);
    b.add_input_arc(p_dead, t_dead, 1, BoolExpr::True);
    b.add_output_arc(t_dead, p1, 1, None);
    b.set_initial_tokens(p0, 1);
    b.build()
}

#[test]
fn reports_transition_never_enabled() {
    let net = net_with_dead_transition();
    let rg = explore(&net, &AnalysisConfig::default());
    let dead = find_dead_transitions(&net, &rg);
    assert_eq!(dead.len(), 1, "exactly one dead transition");
    match &dead[0].kind {
        PropertyViolation::DeadTransition { transition, .. } => {
            assert_eq!(transition.0, 1, "t_dead is the dead one");
        }
        other => panic!("expected DeadTransition, got {other:?}"),
    }
}

#[test]
fn live_transition_not_reported() {
    let net = net_with_dead_transition();
    let rg = explore(&net, &AnalysisConfig::default());
    for cx in find_dead_transitions(&net, &rg) {
        if let PropertyViolation::DeadTransition { transition, .. } = cx.kind {
            assert_ne!(transition.0, 0, "t_live fires and must not be dead");
        }
    }
}
