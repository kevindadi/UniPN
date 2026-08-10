//! Extensibility: a "foreign net" (independent data format) implementing
//! `NetLike` is immediately consumed by the shared algorithms.

use unipn::analysis::{AnalysisConfig, PropertyViolation, explore};
use unipn::model::{ControlSub, PlaceKind, TransitionKind};
use unipn::netlike::{FireError, NetLike};
use unipn::{PlaceId, State, TransitionId, Weight};

/// A minimal, self-contained net: a completely different internal
/// representation (Vec of (name, kind)).
struct ForeignNet {
    places: Vec<(String, PlaceKind)>,
    transitions: Vec<(String, TransitionKind)>,
    pre: Vec<Vec<(usize, Weight)>>,
    post: Vec<Vec<(usize, Weight)>>,
    initial: Vec<u32>,
}

impl NetLike for ForeignNet {
    fn num_places(&self) -> usize {
        self.places.len()
    }
    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }
    fn place_label(&self, p: PlaceId) -> String {
        self.places[p.index()].0.clone()
    }
    fn place_kind(&self, p: PlaceId) -> Option<PlaceKind> {
        Some(self.places[p.index()].1.clone())
    }
    fn transition_label(&self, t: TransitionId) -> String {
        self.transitions[t.index()].0.clone()
    }
    fn transition_kind(&self, t: TransitionId) -> Option<TransitionKind> {
        Some(self.transitions[t.index()].1.clone())
    }
    fn pre_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)> {
        self.pre[t.index()]
            .iter()
            .map(|&(p, w)| (PlaceId(p), w))
            .collect()
    }
    fn post_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)> {
        self.post[t.index()]
            .iter()
            .map(|&(p, w)| (PlaceId(p), w))
            .collect()
    }
    fn initial_state(&self) -> State {
        State::new(unipn::Marking(self.initial.clone()), None)
    }
    // enabled/fire use the trait default implementations (pure P/T).
}

/// Resource-blocked deadlock: the thread at p0 waits for the mutex token,
/// but the lock is empty initially.
fn resource_blocked_deadlock() -> ForeignNet {
    ForeignNet {
        places: vec![
            ("t1_s1".into(), PlaceKind::Control(ControlSub::Statement)),
            ("t1_done".into(), PlaceKind::Control(ControlSub::ThreadEnd)),
            ("mtx".into(), PlaceKind::Resource(unipn::model::ResourceType::Mutex)),
        ],
        transitions: vec![("t1_lock".into(), TransitionKind::Lock)],
        pre: vec![vec![(0, 1), (2, 1)]], // p0 + mtx
        post: vec![vec![(1, 1)]],        // → p1
        initial: vec![1, 0, 0],          // p0=1, mtx=0 (lock held elsewhere)
    }
}

#[test]
fn foreign_net_is_consumed_by_shared_explore() {
    let net = resource_blocked_deadlock();
    // The shared algorithm works directly — no need to convert ForeignNet
    // into any unified structure.
    let rg = explore(&net, &AnalysisConfig::default());

    assert!(!rg.deadlocks.is_empty(), "expect deadlock");
    assert!(matches!(rg.deadlocks[0].kind, PropertyViolation::Deadlock));
    assert!(rg.deadlocks[0].trace.is_empty(), "no steps possible");

    // Semantic predicates are frontend-supplied: a resource place is not a
    // thread terminal.
    assert!(net.is_resource(PlaceId(2)));
    assert!(!net.is_thread_terminal(PlaceId(0)));
    assert!(net.is_thread_terminal(PlaceId(1)));

    // The default fire semantics works.
    let e = net.enabled_transitions(&net.initial_state());
    assert!(e.is_empty());
    assert_eq!(
        net.fire(TransitionId(0), &net.initial_state()),
        Err(FireError::NotEnabled(TransitionId(0)))
    );
}
