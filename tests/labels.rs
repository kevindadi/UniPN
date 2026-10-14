//! The label layer: labeled nets built from caller-chosen enum and string
//! labels, and the label-level analyses run over them and over the frontends'
//! own nets with their canonical labels.

use serde::{Deserialize, Serialize};

use unipn::analysis::labels::{
    PlaceLabeling, partition_places, partition_transitions, states_where, unfired_labels,
};
use unipn::analysis::{AnalysisConfig, explore, find_deadlocks};
use unipn::cvn::{CvnBuilder, CvnTransition, PlaceKind, ResourceType, TransitionKind};
use unipn::net::{ArcDir, ControlSub, Labeled, Place, PlaceId, TransitionId, canonical};
use unipn::pt::{PtNet, PtPlaceKind, PtTransitionKind, TransitionType};
use unipn::{BoolExpr, LabeledBuilder, LabeledNet, Marking, Tag};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Role {
    Idle,
    Critical,
    Lock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Op {
    Enter,
    Leave,
    Unreachable,
}

/// Two workers sharing one lock, plus a transition nothing can enable.
///
/// Places, in id order: idle0, idle1, crit0, crit1, lock, orphan.
/// Transitions: enter0, leave0, enter1, leave1, ghost.
fn mutex() -> (LabeledNet<Role, Op>, Marking) {
    let mut b = LabeledBuilder::new();
    let idle = [
        b.add_marked_place("idle0", Tag(Role::Idle), 1),
        b.add_marked_place("idle1", Tag(Role::Idle), 1),
    ];
    let crit = [
        b.add_place("crit0", Tag(Role::Critical)),
        b.add_place("crit1", Tag(Role::Critical)),
    ];
    let lock = b.add_marked_place("lock", Tag(Role::Lock), 1);
    let orphan = b.add_place("orphan", Tag(Role::Idle));
    for i in 0..2 {
        let enter = b.add_transition(format!("enter{i}"), Tag(Op::Enter));
        b.add_input_arc(idle[i], enter, 1);
        b.add_input_arc(lock, enter, 1);
        b.add_output_arc(crit[i], enter, 1);
        let leave = b.add_transition(format!("leave{i}"), Tag(Op::Leave));
        b.add_input_arc(crit[i], leave, 1);
        b.add_output_arc(idle[i], leave, 1);
        b.add_output_arc(lock, leave, 1);
    }
    let ghost = b.add_transition("ghost", Tag(Op::Unreachable));
    b.add_input_arc(orphan, ghost, 1);
    b.add_output_arc(crit[0], ghost, 1);
    b.build()
}

fn p(i: usize) -> PlaceId {
    PlaceId(i)
}

fn t(i: usize) -> TransitionId {
    TransitionId(i)
}

#[test]
fn an_enum_labeled_net_partitions_by_its_labels_in_first_appearance_order() {
    let (net, _) = mutex();
    assert_eq!(
        partition_places(&net, canonical),
        vec![
            (Role::Idle, vec![p(0), p(1), p(5)]),
            (Role::Critical, vec![p(2), p(3)]),
            (Role::Lock, vec![p(4)]),
        ]
    );
    assert_eq!(
        partition_transitions(&net, canonical),
        vec![
            (Op::Enter, vec![t(0), t(2)]),
            (Op::Leave, vec![t(1), t(3)]),
            (Op::Unreachable, vec![t(4)]),
        ]
    );
}

#[test]
fn label_queries_over_an_enum_labeled_net() {
    let (net, initial) = mutex();
    let graph = explore(&net, initial, &AnalysisConfig::default());
    assert_eq!(graph.state_count(), 3);

    // Mutual exclusion, phrased in labels.
    let both_critical = states_where(&net, &graph, canonical, |c| c.get(&Role::Critical) >= 2);
    assert!(both_critical.is_empty());
    let conserved = states_where(&net, &graph, canonical, |c| {
        c.get(&Role::Critical) + c.get(&Role::Lock) == 1
    });
    assert_eq!(conserved.len(), graph.state_count());

    // A witness for "someone is critical": one `Enter` away.
    let someone_critical = states_where(&net, &graph, canonical, |c| c.get(&Role::Critical) == 1);
    assert_eq!(someone_critical.len(), 2);
    for state in someone_critical {
        let trace = graph.trace_to(state);
        assert_eq!(trace.len(), 1);
        assert_eq!(net.transition(trace[0]).unwrap().label(), &Op::Enter);
    }

    assert_eq!(
        unfired_labels(&net, &graph, canonical),
        vec![(Op::Unreachable, vec![t(4)])]
    );
}

#[test]
fn place_labeling_counts_tokens_per_label_and_answers_zero_for_unknown_labels() {
    let (net, initial) = mutex();
    let labeling = PlaceLabeling::new(&net, canonical);
    assert_eq!(labeling.labels(), [Role::Idle, Role::Critical, Role::Lock]);
    let counts = labeling.count(&initial);
    assert_eq!(
        counts.iter().map(|(l, n)| (*l, n)).collect::<Vec<_>>(),
        [(Role::Idle, 2), (Role::Critical, 0), (Role::Lock, 1)]
    );

    // A labeling that leaves the lock unlabeled: its label is simply absent.
    let only_workers = PlaceLabeling::new(&net, |place: &Place<Tag<Role>>| {
        (place.kind.0 != Role::Lock).then_some(place.kind.0)
    });
    assert_eq!(only_workers.count(&initial).get(&Role::Lock), 0);
}

/// The same mutex labeled with strings, where a place's string carries two
/// tags: which worker it belongs to and what state it is. A labeling function
/// that splits it gives every place two labels.
fn string_mutex() -> (LabeledNet<String, String>, Marking) {
    let mut b = LabeledBuilder::<String, String>::new();
    let lock = b.add_marked_place("lock", Tag("lock".to_owned()), 1);
    for i in 0..2 {
        let idle = b.add_marked_place(format!("idle{i}"), Tag(format!("worker{i}/idle")), 1);
        let crit = b.add_place(format!("crit{i}"), Tag(format!("worker{i}/critical")));
        let enter = b.add_transition(format!("enter{i}"), Tag("enter".to_owned()));
        b.add_input_arc(idle, enter, 1);
        b.add_input_arc(lock, enter, 1);
        b.add_output_arc(crit, enter, 1);
        let leave = b.add_transition(format!("leave{i}"), Tag("leave".to_owned()));
        b.add_input_arc(crit, leave, 1);
        b.add_output_arc(idle, leave, 1);
        b.add_output_arc(lock, leave, 1);
    }
    b.build()
}

fn tags(place: &Place<Tag<String>>) -> Vec<String> {
    place.kind.0.split('/').map(str::to_owned).collect()
}

#[test]
fn a_string_labeled_net_with_several_labels_per_place() {
    let (net, initial) = string_mutex();
    let groups = partition_places(&net, tags);
    let names: Vec<&str> = groups.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(names, ["lock", "worker0", "idle", "critical", "worker1"]);
    let idle = &groups.iter().find(|(l, _)| l == "idle").unwrap().1;
    assert_eq!(idle, &vec![p(1), p(3)]);

    let graph = explore(&net, initial, &AnalysisConfig::default());
    assert_eq!(graph.state_count(), 3);
    // Every worker is always in exactly one of its two places.
    for worker in ["worker0", "worker1"] {
        let worker = worker.to_owned();
        assert!(states_where(&net, &graph, tags, |c| c.get(&worker) != 1).is_empty());
    }
    assert!(unfired_labels(&net, &graph, canonical).is_empty());
}

#[test]
fn labeled_nets_serialize_their_labels_bare_and_round_trip() {
    let (net, _) = mutex();
    let json = serde_json::to_value(&net).unwrap();
    assert_eq!(json["places"][2]["kind"], "Critical");
    assert_eq!(json["transitions"][4]["kind"], "Unreachable");
    let back: LabeledNet<Role, Op> = serde_json::from_value(json).unwrap();
    assert_eq!(back, net);

    let (net, initial) = string_mutex();
    let json = serde_json::to_value(&net).unwrap();
    assert_eq!(json["places"][1]["kind"], "worker0/idle");
    let back: LabeledNet<String, String> = serde_json::from_value(json).unwrap();
    assert_eq!(back, net);
    let graph = explore(&back, initial, &AnalysisConfig::default());
    assert_eq!(graph.state_count(), 3);
}

/// Two threads taking two locks in opposite orders.
fn lock_order() -> PtNet {
    let mut net = PtNet::new();
    let x = net.add_place("x", PtPlaceKind::resource());
    let y = net.add_place("y", PtPlaceKind::resource());
    for (name, first, second, first_id, second_id) in [("a", x, y, 0, 1), ("b", y, x, 1, 0)] {
        let start = net.add_place(
            format!("{name}0"),
            PtPlaceKind::control(ControlSub::FunctionStart),
        );
        let holding = net.add_place(
            format!("{name}1"),
            PtPlaceKind::control(ControlSub::BasicBlock),
        );
        let both = net.add_place(
            format!("{name}2"),
            PtPlaceKind::control(ControlSub::BasicBlock),
        );
        let end = net.add_place(
            format!("{name}3"),
            PtPlaceKind::control(ControlSub::FunctionEnd),
        );
        let steps = [
            (start, holding, Some(first), TransitionType::Lock(first_id)),
            (holding, both, Some(second), TransitionType::Lock(second_id)),
            (both, end, None, TransitionType::Unlock(first_id)),
        ];
        for (from, to, takes, ty) in steps {
            let t = net.add_transition(format!("{name}:{from:?}"), PtTransitionKind::new(ty));
            net.add_arc(from, t, ArcDir::Input, 1, ());
            net.add_arc(to, t, ArcDir::Output, 1, ());
            match takes {
                Some(lock) => net.add_arc(lock, t, ArcDir::Input, 1, ()),
                None => {
                    net.add_arc(first, t, ArcDir::Output, 1, ());
                    net.add_arc(second, t, ArcDir::Output, 1, ());
                }
            }
        }
    }
    net
}

#[test]
fn a_label_query_finds_the_same_deadlocks_as_place_role() {
    let net = lock_order();
    // x, y, a0..a3, b0..b3
    let initial = Marking::new(vec![1, 1, 1, 0, 0, 0, 1, 0, 0, 0]);
    let graph = explore(&net, initial, &AnalysisConfig::default());

    let by_role = find_deadlocks(&graph, |m| net.is_deadlock(m));
    assert_eq!(by_role.len(), 1);

    // "A thread is stuck" in labels: a token on a control place other than a
    // function's end. The canonical P/T label is the `PlaceClass`; the closure
    // coarsens it to that one yes/no question.
    let stuck_capable = |place: &Place<PtPlaceKind>| {
        Some(matches!(
            place.label().control(),
            Some(ControlSub::BasicBlock | ControlSub::FunctionStart)
        ))
    };
    let by_labels: Vec<usize> = states_where(&net, &graph, stuck_capable, |c| c.get(&true) > 0)
        .into_iter()
        .filter(|s| graph.blocked.contains(s))
        .collect();
    assert_eq!(by_labels, by_role);
}

#[test]
fn transition_labels_coarsen_through_a_closure() {
    let net = lock_order();
    // The canonical label keeps the alias, so each lock is its own label…
    assert_eq!(partition_transitions(&net, canonical).len(), 4);
    // …and dropping the payload is one closure away.
    let by_variant = partition_transitions(&net, |t| {
        Some(std::mem::discriminant(&t.kind.transition_type))
    });
    let sizes: Vec<usize> = by_variant.iter().map(|(_, m)| m.len()).collect();
    assert_eq!(sizes, [4, 2]);
}

#[test]
fn label_queries_read_the_marking_out_of_a_cvn_state() {
    let mut b = CvnBuilder::new();
    let start = b.add_marked_place("start", PlaceKind::Control(ControlSub::FunctionStart), 1);
    let lock = b.add_marked_place("m", PlaceKind::Resource(ResourceType::Mutex), 1);
    let held = b.add_place("held", PlaceKind::Control(ControlSub::BasicBlock));
    let t = b.add_transition("lock", CvnTransition::new(TransitionKind::Lock));
    b.add_input_arc(start, t, 1, BoolExpr::True);
    b.add_input_arc(lock, t, 1, BoolExpr::True);
    b.add_output_arc(t, held, 1, None);
    let (net, initial) = b.build();

    let graph = explore(&net, initial, &AnalysisConfig::default());
    let mutex_taken = states_where(&net, &graph, canonical, |c| {
        c.get(&PlaceKind::Resource(ResourceType::Mutex)) == 0
    });
    assert_eq!(mutex_taken.len(), 1);
    assert_eq!(graph.trace_to(mutex_taken[0]), vec![t]);
    assert_eq!(
        partition_transitions(&net, canonical),
        vec![(TransitionKind::Lock, vec![t])]
    );
}
