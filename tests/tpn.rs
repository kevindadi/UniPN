//! Dense-time time Petri net (TPN) frontend and state-class analysis.
//!
//! The frontend tests (serde round-trips) run without the `timed` feature; the
//! state-class analysis tests are gated on it, since `analysis::tpn` is
//! compiled only with `timed`.

use unipn::tpn::{Interval, TpnBuilder, TpnNet, TpnTransition, UpperBound};
use unipn::{Marking, Tag};

fn roundtrip<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(
    value: &T,
) {
    let json = serde_json::to_string(value).unwrap();
    let back: T = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, value, "round-trip mismatch for {json}");
}

#[test]
fn interval_serde_roundtrip() {
    roundtrip(&Interval::closed(0, 5));
    roundtrip(&Interval::open(0, 1));
    roundtrip(&Interval::left_open(1, 2));
    roundtrip(&Interval::right_open(0, 1));
    roundtrip(&Interval::unbounded(3));
    roundtrip(&Interval {
        lower: 2,
        lower_open: true,
        upper: UpperBound::Infinity,
    });
}

#[test]
fn interval_legality_is_dense_time() {
    assert!(
        Interval::new(
            0,
            true,
            UpperBound::Finite {
                value: 1,
                open: true
            }
        )
        .is_ok()
    ); // (0,1)
    assert!(
        Interval::new(
            1,
            false,
            UpperBound::Finite {
                value: 1,
                open: true
            }
        )
        .is_err()
    ); // [1,1)
    assert!(
        Interval::new(
            1,
            false,
            UpperBound::Finite {
                value: 1,
                open: false
            }
        )
        .is_ok()
    ); // [1,1]
    assert!(
        Interval::new(
            1,
            true,
            UpperBound::Finite {
                value: 1,
                open: true
            }
        )
        .is_err()
    ); // (1,1)
    assert!(Interval::new(0, false, UpperBound::Infinity).is_ok()); // [0,∞)
}

#[test]
fn tpn_net_serde_roundtrip() {
    let mut b = TpnBuilder::new();
    let p = b.add_marked_place("p", Tag("in".to_string()), 1);
    let q = b.add_place("q", Tag("out".to_string()));
    let t = b.add_transition(
        "t",
        TpnTransition::new("fire".to_string(), Interval::closed(1, 2)),
    );
    b.add_input_arc(p, t, 1);
    b.add_output_arc(q, t, 1);
    let (net, marking): (TpnNet<String, String>, Marking) = b.build();

    roundtrip(&net);
    roundtrip(&marking);
    assert_eq!(net.num_places(), 2);
    assert_eq!(net.num_transitions(), 1);
}

#[cfg(feature = "timed")]
mod analysis {
    use std::collections::{BTreeSet, HashSet, VecDeque};

    use unipn::Tag;
    use unipn::analysis::tpn::{NewlyEnabled, Servers, TpnSemantics, state_class_graph};
    use unipn::analysis::{AnalysisConfig, NetLike, explore};
    use unipn::net::{Marking, TransitionId};
    use unipn::tpn::{Interval, TpnBuilder, TpnNet, TpnTransition, UpperBound};

    // ── Shared helpers ────────────────────────────────────────────────────

    fn scg_markings(
        net: &TpnNet<(), ()>,
        initial: &Marking,
        semantics: TpnSemantics,
    ) -> BTreeSet<Marking> {
        let graph =
            state_class_graph(net, initial.clone(), semantics, &AnalysisConfig::default()).unwrap();
        graph.states.iter().map(|c| c.marking.clone()).collect()
    }

    /// SCG markings, or `None` if the state limit is hit.
    pub(crate) fn scg_markings_limited(
        net: &TpnNet<(), ()>,
        initial: &Marking,
        semantics: TpnSemantics,
        max_states: usize,
    ) -> Option<BTreeSet<Marking>> {
        let graph = state_class_graph(
            net,
            initial.clone(),
            semantics,
            &AnalysisConfig {
                max_states,
                ..AnalysisConfig::default()
            },
        )
        .unwrap();
        if graph.truncated {
            return None;
        }
        Some(graph.states.iter().map(|c| c.marking.clone()).collect())
    }

    /// Untimed markings, or `None` if the state limit is hit.
    pub(crate) fn untimed_markings_limited(
        net: &TpnNet<(), ()>,
        initial: &Marking,
        max_states: usize,
    ) -> Option<BTreeSet<Marking>> {
        let graph = explore(
            net,
            initial.clone(),
            &AnalysisConfig {
                max_states,
                ..AnalysisConfig::default()
            },
        );
        if graph.truncated {
            return None;
        }
        Some(graph.states.into_iter().collect())
    }

    // ── 1. The background example, and agreement with PTPN ───────────────

    /// p has 1 token; `loop: p→p [1,1]`, `t: p→q [2,2]`.
    fn background_net() -> (TpnNet<(), ()>, Marking) {
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 1);
        let q = b.add_place("q", Tag(()));
        let lp = b.add_transition("loop", TpnTransition::new((), Interval::closed(1, 1)));
        let t = b.add_transition("t", TpnTransition::new((), Interval::closed(2, 2)));
        b.add_input_arc(p, lp, 1);
        b.add_output_arc(p, lp, 1);
        b.add_input_arc(p, t, 1);
        b.add_output_arc(q, t, 1);
        b.build()
    }

    #[test]
    fn background_example_intermediate_vs_atomic() {
        let (net, initial) = background_net();
        let intermediate = TpnSemantics {
            newly_enabled: NewlyEnabled::Intermediate,
            servers: Servers::Single,
        };
        let atomic = TpnSemantics {
            newly_enabled: NewlyEnabled::Atomic,
            servers: Servers::Single,
        };

        let m_inter = scg_markings(&net, &initial, intermediate);
        let m_atomic = scg_markings(&net, &initial, atomic);
        let q = net.places.iter().find(|p| p.name == "q").unwrap().id;

        // Intermediate: t resets after every loop firing, so q is unreachable.
        assert!(!m_inter.iter().any(|m| m.tokens(q) > 0));
        // Atomic: t keeps its clock across loop firings, so q is reachable.
        assert!(m_atomic.iter().any(|m| m.tokens(q) > 0));
    }

    #[test]
    fn background_example_matches_ptpn_under_atomic() {
        // PTPN is single-instance + atomic, so its reachable markings must
        // equal the TPN SCG under { Atomic, Single }.
        let (net, initial) = background_net();
        let atomic = TpnSemantics {
            newly_enabled: NewlyEnabled::Atomic,
            servers: Servers::Single,
        };
        let m_atomic = scg_markings(&net, &initial, atomic);

        let (ptpn, ptpn_initial) = to_ptpn(&net, &initial);
        let ptpn_markings = ptpn_reachable_markings(&ptpn, &ptpn_initial);

        assert_eq!(m_atomic, ptpn_markings);
    }

    // ── 2. Open intervals change the answer ───────────────────────────────

    /// Two transitions `a` and `b` compete for one token on `p`; `a` moves it
    /// to `qa`, `b` to `qb`.
    fn conflict_net(ia: Interval, ib: Interval) -> (TpnNet<(), ()>, Marking) {
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 1);
        let qa = b.add_place("qa", Tag(()));
        let qb = b.add_place("qb", Tag(()));
        let a = b.add_transition("a", TpnTransition::new((), ia));
        let bb = b.add_transition("b", TpnTransition::new((), ib));
        b.add_input_arc(p, a, 1);
        b.add_output_arc(qa, a, 1);
        b.add_input_arc(p, bb, 1);
        b.add_output_arc(qb, bb, 1);
        b.build()
    }

    #[test]
    fn open_interval_fires_where_closed_does_not() {
        // `a` is [1,1], `b` is (0,1). Both are enabled at time 0. `b` can fire
        // at any instant in (0,1), strictly before `a`'s only instant 1, so `b`
        // is firable and `qb` is reachable. `a` can only fire at 1, by which
        // time `b`'s deadline (1, open) has passed, so `a` is not firable.
        let (net, initial) = conflict_net(Interval::closed(1, 1), Interval::open(0, 1));
        let m = scg_markings(
            &net,
            &initial,
            TpnSemantics {
                newly_enabled: NewlyEnabled::Atomic,
                servers: Servers::Single,
            },
        );
        let qb = net.places.iter().find(|p| p.name == "qb").unwrap().id;
        let qa = net.places.iter().find(|p| p.name == "qa").unwrap().id;
        assert!(m.iter().any(|m| m.tokens(qb) > 0), "b (0,1) must fire");
        assert!(!m.iter().any(|m| m.tokens(qa) > 0), "a [1,1] must not fire");
    }

    #[test]
    fn open_interval_beats_closed_at_the_same_upper_bound() {
        // `a` is (1,2), `b` is [2,2]. `a` can fire at any instant in (1,2),
        // strictly before `b`'s only instant 2, so `a` fires and `qa` is
        // reachable; `b` never gets to fire.
        let (net, initial) = conflict_net(Interval::open(1, 2), Interval::closed(2, 2));
        let m = scg_markings(
            &net,
            &initial,
            TpnSemantics {
                newly_enabled: NewlyEnabled::Atomic,
                servers: Servers::Single,
            },
        );
        let qa = net.places.iter().find(|p| p.name == "qa").unwrap().id;
        let qb = net.places.iter().find(|p| p.name == "qb").unwrap().id;
        assert!(m.iter().any(|m| m.tokens(qa) > 0), "a (1,2) must fire");
        assert!(!m.iter().any(|m| m.tokens(qb) > 0), "b [2,2] must not fire");
    }

    #[test]
    fn half_open_beats_the_closed_successor_where_closed_ties() {
        // `a` is [0,1), `b` is [1,1]. Hand derivation: the firing domain is
        // θ_a ∈ [0,1), θ_b = 1. `b` is firable iff D ∧ {θ_b ≤ θ_a} is non-empty,
        // i.e. 1 ≤ θ_a < 1 — empty, so `b` cannot fire; `a` fires at some
        // instant < 1 first. Contrast with the closed pair below, where
        // θ_a = θ_b = 1 is feasible and both can fire: the open endpoint is what
        // changes the answer.
        let (net, initial) = conflict_net(Interval::right_open(0, 1), Interval::closed(1, 1));
        let m = scg_markings(
            &net,
            &initial,
            TpnSemantics {
                newly_enabled: NewlyEnabled::Atomic,
                servers: Servers::Single,
            },
        );
        let qa = net.places.iter().find(|p| p.name == "qa").unwrap().id;
        let qb = net.places.iter().find(|p| p.name == "qb").unwrap().id;
        assert!(m.iter().any(|m| m.tokens(qa) > 0), "a [0,1) must fire");
        assert!(
            !m.iter().any(|m| m.tokens(qb) > 0),
            "b [1,1] cannot fire: it needs θ_a ≥ 1 but θ_a < 1"
        );

        // With both intervals closed, θ_a = θ_b = 1 is feasible, so both fire.
        let (net2, initial2) = conflict_net(Interval::closed(0, 1), Interval::closed(1, 1));
        let m2 = scg_markings(
            &net2,
            &initial2,
            TpnSemantics {
                newly_enabled: NewlyEnabled::Atomic,
                servers: Servers::Single,
            },
        );
        let qa2 = net2.places.iter().find(|p| p.name == "qa").unwrap().id;
        let qb2 = net2.places.iter().find(|p| p.name == "qb").unwrap().id;
        assert!(m2.iter().any(|m| m.tokens(qa2) > 0));
        assert!(m2.iter().any(|m| m.tokens(qb2) > 0));
    }

    #[test]
    fn closed_reachable_open_unreachable() {
        // A single transition `t: p→q` with interval [1,1] fires (q reachable);
        // with (1,1) it has an empty dense-time domain and is rejected at
        // construction, so the net cannot even be built — the closed interval
        // is the one that is reachable. We assert the closed case reaches q and
        // that the open case is rejected.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 1);
        let q = b.add_place("q", Tag(()));
        let t = b.add_transition("t", TpnTransition::new((), Interval::closed(1, 1)));
        b.add_input_arc(p, t, 1);
        b.add_output_arc(q, t, 1);
        let (net, initial) = b.build();
        let m = scg_markings(
            &net,
            &initial,
            TpnSemantics {
                newly_enabled: NewlyEnabled::Atomic,
                servers: Servers::Single,
            },
        );
        assert!(m.iter().any(|m| m.tokens(q) > 0));

        assert!(
            Interval::new(
                1,
                true,
                UpperBound::Finite {
                    value: 1,
                    open: true
                }
            )
            .is_err()
        );
    }

    // ── 3. Oracle A: integer-time discretization ──────────────────────────

    /// The maximum finite constant in the net (0 if every interval is [0,∞)).
    pub(crate) fn max_constant(net: &TpnNet<(), ()>) -> i64 {
        net.transitions
            .iter()
            .map(|t| {
                let lo = t.kind.interval.lower as i64;
                let hi = match t.kind.interval.upper {
                    UpperBound::Finite { value, .. } => value as i64,
                    UpperBound::Infinity => 0,
                };
                lo.max(hi)
            })
            .max()
            .unwrap_or(0)
    }

    /// An integer-time state: marking + one integer clock per transition
    /// (`-1` = disabled).
    #[derive(Clone, PartialEq, Eq, Hash)]
    struct IntState {
        marking: Marking,
        clocks: Vec<i64>,
    }

    impl IntState {
        fn initial(net: &TpnNet<(), ()>, marking: &Marking) -> Self {
            let clocks = net
                .transition_ids()
                .map(|t| {
                    if net.structurally_enabled(marking, t) {
                        0
                    } else {
                        -1
                    }
                })
                .collect();
            IntState {
                marking: marking.clone(),
                clocks,
            }
        }
    }

    fn newly_enabled_pred(
        net: &TpnNet<(), ()>,
        u: TransitionId,
        marking: &Marking,
        t_f: TransitionId,
        marking_prime: &Marking,
        rule: NewlyEnabled,
    ) -> bool {
        if !net.structurally_enabled(marking_prime, u) {
            return false;
        }
        if u == t_f {
            return true;
        }
        let memory = match rule {
            NewlyEnabled::Intermediate => {
                let mut m = marking.clone();
                net.consume_inputs(&mut m, t_f);
                m
            }
            NewlyEnabled::Atomic => marking.clone(),
        };
        !net.structurally_enabled(&memory, u)
    }

    /// The independent integer-time explicit-state explorer (oracle A).
    ///
    /// Time elapses by integer amounts; a transition fires when its clock is in
    /// its closed integer interval and no other enabled transition's deadline is
    /// violated (strong time). Clocks with an ∞ upper bound are capped at
    /// `max_constant + 1`. Returns `None` if the state limit is hit.
    fn integer_reachable_markings(
        net: &TpnNet<(), ()>,
        initial: &Marking,
        rule: NewlyEnabled,
        max_states: usize,
    ) -> Option<BTreeSet<Marking>> {
        let cap = max_constant(net) + 1;
        let mut seen: HashSet<IntState> = HashSet::new();
        let mut queue: VecDeque<IntState> = VecDeque::new();
        let init = IntState::initial(net, initial);
        seen.insert(init.clone());
        queue.push_back(init);
        let mut markings = BTreeSet::new();

        while let Some(state) = queue.pop_front() {
            markings.insert(state.marking.clone());
            for t in net.transition_ids() {
                if !net.structurally_enabled(&state.marking, t) {
                    continue;
                }
                let interval = net.transition(t).unwrap().kind.interval;
                let eft = interval.lower as i64;
                let t_clock = state.clocks[t.index()];
                let delta_min = (eft - t_clock).max(0);
                // Strong time: no enabled transition may pass its deadline.
                let mut delta_max: Option<i64> = None;
                for u in net.transition_ids() {
                    if !net.structurally_enabled(&state.marking, u) {
                        continue;
                    }
                    if let UpperBound::Finite { value, .. } =
                        net.transition(u).unwrap().kind.interval.upper
                    {
                        let b = value as i64 - state.clocks[u.index()];
                        delta_max = Some(delta_max.map_or(b, |x| x.min(b)));
                    }
                }
                let delta_max = delta_max.unwrap_or(cap - t_clock);
                for delta in delta_min..=delta_max {
                    let next = state.fire(net, t, delta, rule, cap);
                    if seen.insert(next.clone()) {
                        if seen.len() >= max_states {
                            return None;
                        }
                        queue.push_back(next);
                    }
                }
            }
        }
        Some(markings)
    }

    impl IntState {
        fn fire(
            &self,
            net: &TpnNet<(), ()>,
            t: TransitionId,
            delta: i64,
            rule: NewlyEnabled,
            cap: i64,
        ) -> IntState {
            let marking2 = NetLike::fire(net, &self.marking, t).expect("enabled transition fires");
            let mut clocks2 = vec![-1; net.num_transitions()];
            for u in net.transition_ids() {
                if !net.structurally_enabled(&marking2, u) {
                    continue;
                }
                let persistent =
                    u != t && !newly_enabled_pred(net, u, &self.marking, t, &marking2, rule);
                clocks2[u.index()] = if persistent {
                    (self.clocks[u.index()] + delta).min(cap)
                } else {
                    0
                };
            }
            IntState {
                marking: marking2,
                clocks: clocks2,
            }
        }
    }

    /// A deterministic LCG (no `rand` dependency).
    pub(crate) struct Lcg(u64);

    impl Lcg {
        pub(crate) fn new(seed: u64) -> Self {
            Lcg(seed)
        }
        pub(crate) fn next(&mut self, bound: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) % bound
        }
    }

    /// A small random TPN with closed integer intervals (constants 0..=2) and,
    /// when `allow_infinity`, occasional `[a, ∞)` upper bounds.
    pub(crate) fn random_tpn(seed: u64, allow_infinity: bool) -> (TpnNet<(), ()>, Marking) {
        let mut rng = Lcg::new(seed);
        let places = 1 + rng.next(4) as usize;
        let transitions = 1 + rng.next(4) as usize;
        let mut b = TpnBuilder::new();
        let ps: Vec<_> = (0..places)
            .map(|i| b.add_marked_place(format!("p{i}"), Tag(()), rng.next(3) as usize))
            .collect();
        for j in 0..transitions {
            let a = rng.next(3);
            let interval = if allow_infinity && rng.next(4) == 0 {
                Interval::unbounded(a)
            } else {
                Interval::closed(a, a + rng.next(3))
            };
            let t = b.add_transition(format!("t{j}"), TpnTransition::new((), interval));
            for &p in &ps {
                let weight = |w| [0, 0, 0, 1, 1, 2][w as usize];
                let pre = weight(rng.next(6));
                let post = weight(rng.next(6));
                if pre > 0 {
                    b.add_input_arc(p, t, pre);
                }
                if post > 0 {
                    b.add_output_arc(p, t, post);
                }
            }
        }
        b.build()
    }

    #[test]
    fn oracle_a_discretization_matches_scg_on_closed_integer_intervals() {
        // Popova-Zeugmann's discretization: for a TPN with closed integer
        // intervals, the reachable markings under dense time equal those under
        // integer time. The published result is for the classical (intermediate)
        // semantics, so that configuration is asserted strictly; the atomic
        // configuration is compared empirically (the theorem is not stated for
        // it, so a mismatch would be investigated, not papered over).
        let mut compared = 0usize;
        let mut scg_strictly_smaller = 0usize;
        let mut rules_differ = 0usize;

        for seed in 0..400u64 {
            let (net, initial) = random_tpn(seed, true);
            let Some(untimed) = untimed_markings_limited(&net, &initial, 10_000) else {
                continue;
            };

            let intermediate = TpnSemantics {
                newly_enabled: NewlyEnabled::Intermediate,
                servers: Servers::Single,
            };
            let atomic = TpnSemantics {
                newly_enabled: NewlyEnabled::Atomic,
                servers: Servers::Single,
            };

            let Some(scg_inter) = scg_markings_limited(&net, &initial, intermediate, 10_000) else {
                continue;
            };
            let Some(scg_atomic) = scg_markings_limited(&net, &initial, atomic, 10_000) else {
                continue;
            };

            let Some(int_inter) =
                integer_reachable_markings(&net, &initial, NewlyEnabled::Intermediate, 10_000)
            else {
                continue;
            };
            let Some(int_atomic) =
                integer_reachable_markings(&net, &initial, NewlyEnabled::Atomic, 10_000)
            else {
                continue;
            };

            // Theorem-covered: intermediate semantics, strict assertion.
            assert_eq!(
                scg_inter, int_inter,
                "seed {seed}: SCG (intermediate) disagrees with integer-time oracle"
            );
            // Not theorem-covered: atomic semantics, empirical comparison.
            assert_eq!(
                scg_atomic, int_atomic,
                "seed {seed}: SCG (atomic) disagrees with integer-time oracle"
            );

            compared += 1;
            if scg_atomic.len() < untimed.len() {
                scg_strictly_smaller += 1;
            }
            if scg_inter != scg_atomic {
                rules_differ += 1;
            }
        }

        assert!(
            compared >= 200,
            "only {compared} nets compared (state limits hit too often)"
        );
        assert!(
            scg_strictly_smaller > 0,
            "corpus is trivial: no net has SCG markings strictly smaller than untimed"
        );
        assert!(
            rules_differ > 0,
            "corpus is trivial: no net distinguishes intermediate from atomic"
        );
    }

    // ── 4. Oracle B: agreement with PTPN ──────────────────────────────────

    fn to_ptpn(net: &TpnNet<(), ()>, initial: &Marking) -> (unipn::TimedNet, Marking) {
        use unipn::TimeInterval;
        use unipn::timed::{TimedPlaceKind, TimedTransitionKind};
        let mut out = unipn::TimedNet::new();
        for p in &net.places {
            out.add_place(
                p.name.clone(),
                TimedPlaceKind {
                    capacity: None,
                    saturate: false,
                },
            );
        }
        for t in &net.transitions {
            let (lo, hi) = match t.kind.interval.upper {
                UpperBound::Finite { value, .. } => (t.kind.interval.lower, value),
                UpperBound::Infinity => (t.kind.interval.lower, unipn::INF as u64),
            };
            out.add_transition(
                t.name.clone(),
                TimedTransitionKind {
                    interval: TimeInterval::closed(lo as i32, hi as i32),
                    priority: 0,
                    core: unipn::CONTROL_TRANSITION_CORE,
                    suspendable: false,
                },
            );
        }
        for arc in net.arcs() {
            out.add_arc(arc.place, arc.transition, arc.direction, arc.weight, ());
        }
        (out, initial.clone())
    }

    fn ptpn_reachable_markings(net: &unipn::TimedNet, initial: &Marking) -> BTreeSet<Marking> {
        use unipn::analysis::timed::{StateClassReachabilityGraph, reachable_markings};
        let mut graph = StateClassReachabilityGraph::new(net, initial.clone());
        graph.build(50_000);
        reachable_markings(graph.get_graph())
    }

    #[test]
    fn oracle_b_atomic_single_matches_ptpn() {
        // On nets where PTPN's extra features are inert (equal priorities,
        // core -1, non-suspendable, closed integer intervals, finite upper
        // bounds), { Atomic, Single } reachable markings must equal PTPN's.
        let mut compared = 0usize;
        for seed in 0..400u64 {
            let (net, initial) = random_tpn(seed, false);
            let atomic = TpnSemantics {
                newly_enabled: NewlyEnabled::Atomic,
                servers: Servers::Single,
            };
            let Some(scg) = scg_markings_limited(&net, &initial, atomic, 10_000) else {
                continue;
            };

            let (ptpn, ptpn_initial) = to_ptpn(&net, &initial);
            let mut graph =
                unipn::analysis::timed::StateClassReachabilityGraph::new(&ptpn, ptpn_initial);
            graph.build(10_000);
            if graph.get_graph().stats.truncated {
                continue; // PTPN hit its state limit: skip
            }
            let ptpn_markings = unipn::analysis::timed::reachable_markings(graph.get_graph());

            assert_eq!(
                scg, ptpn_markings,
                "seed {seed}: TPN (atomic, single) disagrees with PTPN"
            );
            compared += 1;
        }
        assert!(
            compared >= 200,
            "only {compared} nets compared against PTPN"
        );
    }

    // ── 5. SCG ⊆ untimed, and [0,∞) equality ──────────────────────────────

    #[test]
    fn scg_markings_are_a_subset_of_untimed() {
        for seed in 0..100u64 {
            let (net, initial) = random_tpn(seed, true);
            let Some(untimed) = untimed_markings_limited(&net, &initial, 10_000) else {
                continue;
            };
            for rule in [NewlyEnabled::Intermediate, NewlyEnabled::Atomic] {
                let Some(scg) = scg_markings_limited(
                    &net,
                    &initial,
                    TpnSemantics {
                        newly_enabled: rule,
                        servers: Servers::Single,
                    },
                    10_000,
                ) else {
                    continue;
                };
                assert!(
                    scg.is_subset(&untimed),
                    "seed {seed}: SCG markings are not a subset of untimed"
                );
            }
        }
    }

    #[test]
    fn all_unbounded_intervals_equal_untimed_and_class_count_is_marking_count() {
        // With every interval [0,∞), time never constrains a firing, so the SCG
        // markings equal the untimed markings and there is exactly one class per
        // marking.
        for seed in 0..100u64 {
            let (mut net, initial) = random_tpn(seed, false);
            for t in &mut net.transitions {
                t.kind.interval = Interval::unbounded(0);
            }
            let Some(untimed) = untimed_markings_limited(&net, &initial, 10_000) else {
                continue;
            };
            let Some(scg) = scg_markings_limited(&net, &initial, TpnSemantics::default(), 10_000)
            else {
                continue;
            };
            assert_eq!(scg, untimed, "seed {seed}");
            assert_eq!(scg.len(), untimed.len(), "seed {seed}");
        }
    }

    // ── 6. Hand-derived class counts, labels, truncation ──────────────────

    #[test]
    fn single_transition_has_two_classes() {
        // t: p→q [1,2]. Class 0 = {p=1, θ_t ∈ [1,2]}; firing t yields
        // {q=1} with no enabled transition, so exactly 2 classes.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 1);
        let q = b.add_place("q", Tag(()));
        let t = b.add_transition("t", TpnTransition::new((), Interval::closed(1, 2)));
        b.add_input_arc(p, t, 1);
        b.add_output_arc(q, t, 1);
        let (net, initial) = b.build();
        let graph = state_class_graph(
            &net,
            initial,
            TpnSemantics::default(),
            &AnalysisConfig::default(),
        )
        .unwrap();
        assert_eq!(graph.states.len(), 2);
    }

    #[test]
    fn two_independent_transitions_have_three_classes() {
        // a: p→qa [1,1] and b: r→qb [2,2], independent (no shared place).
        // Class 0 = {p=1,r=1, θ_a=1, θ_b=2}. Firing a gives {qa=1,r=1, θ_b=1}
        // (b's clock shifted by 1); firing b gives {p=1,qb=1, θ_a=1}. Each
        // successor has one enabled transition, so 1 + 2 = 3 classes.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 1);
        let r = b.add_marked_place("r", Tag(()), 1);
        let qa = b.add_place("qa", Tag(()));
        let qb = b.add_place("qb", Tag(()));
        let a = b.add_transition("a", TpnTransition::new((), Interval::closed(1, 1)));
        let bb = b.add_transition("b", TpnTransition::new((), Interval::closed(2, 2)));
        b.add_input_arc(p, a, 1);
        b.add_output_arc(qa, a, 1);
        b.add_input_arc(r, bb, 1);
        b.add_output_arc(qb, bb, 1);
        let (net, initial) = b.build();
        let graph = state_class_graph(
            &net,
            initial,
            TpnSemantics::default(),
            &AnalysisConfig::default(),
        )
        .unwrap();
        assert_eq!(graph.states.len(), 3);
    }

    #[test]
    fn label_analyses_run_on_the_scg() {
        use unipn::analysis::find_deadlocks;
        use unipn::analysis::labels::{states_where, unfired_labels};
        use unipn::net::canonical;

        // Two labeled transitions: `go` (p→q) and `stop` (p→r), one token on p.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag("in"), 1);
        let q = b.add_place("q", Tag("done"));
        let r = b.add_place("r", Tag("done"));
        let go = b.add_transition("go", TpnTransition::new("go", Interval::closed(0, 1)));
        let stop = b.add_transition("stop", TpnTransition::new("stop", Interval::closed(0, 1)));
        b.add_input_arc(p, go, 1);
        b.add_output_arc(q, go, 1);
        b.add_input_arc(p, stop, 1);
        b.add_output_arc(r, stop, 1);
        let (net, initial) = b.build();

        let graph = state_class_graph(
            &net,
            initial,
            TpnSemantics::default(),
            &AnalysisConfig::default(),
        )
        .unwrap();

        // Both labels fire somewhere.
        let unfired = unfired_labels(&net, &graph, canonical);
        assert!(unfired.is_empty(), "unfired labels: {unfired:?}");

        // A state with a token on a `done` place exists.
        let done_states = states_where(&net, &graph, canonical, |c| c.get(&"done") >= 1);
        assert!(!done_states.is_empty());

        // The blocked states are exactly the terminal states (a token on a
        // `done` place); with no resource/control split there is no finer
        // deadlock verdict, so the predicate accepts every blocked state.
        let deadlocks = find_deadlocks(&graph, |_| true);
        assert_eq!(deadlocks.len(), 2);
    }

    #[test]
    fn unbounded_net_truncates_without_looping() {
        // t: p→p [0,0] plus q: p→r [0,0]? No — a self-loop that also produces a
        // token makes the marking unbounded, so the SCG must truncate.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 1);
        let r = b.add_place("r", Tag(()));
        let t = b.add_transition("t", TpnTransition::new((), Interval::closed(0, 0)));
        b.add_input_arc(p, t, 1);
        b.add_output_arc(p, t, 1);
        b.add_output_arc(r, t, 1);
        let (net, initial) = b.build();

        let graph = state_class_graph(
            &net,
            initial,
            TpnSemantics::default(),
            &AnalysisConfig {
                max_states: 100,
                ..AnalysisConfig::default()
            },
        )
        .unwrap();
        assert!(graph.truncated, "unbounded net must truncate");
    }
}

#[cfg(feature = "timed")]
mod multi_server {
    use super::analysis::{
        max_constant, random_tpn, scg_markings_limited, untimed_markings_limited,
    };
    use std::collections::{BTreeSet, HashSet, VecDeque};

    use unipn::Tag;
    use unipn::analysis::NetLike;
    use unipn::analysis::tpn::{
        DisablingPolicy, FiringPolicy, MultiServerConfig, NewlyEnabled, Servers, TpnSemantics,
    };
    use unipn::net::{Marking, TransitionId};
    use unipn::tpn::{Interval, TpnBuilder, TpnNet, TpnTransition, UpperBound};

    fn multiple(disabling: DisablingPolicy) -> TpnSemantics {
        TpnSemantics {
            newly_enabled: NewlyEnabled::Atomic,
            servers: Servers::Multiple(MultiServerConfig {
                firing: FiringPolicy::OldestFirst,
                disabling,
            }),
        }
    }

    fn scg_markings_multi(
        net: &TpnNet<(), ()>,
        initial: &Marking,
        semantics: TpnSemantics,
        max_states: usize,
    ) -> BTreeSet<Marking> {
        scg_markings_limited(net, initial, semantics, max_states).unwrap()
    }

    // ── Integer-time oracle, multi-server ────────────────────────────────

    #[derive(Clone, PartialEq, Eq, Hash)]
    struct MultiIntState {
        marking: Marking,
        /// One clock per instance, sorted descending (oldest first).
        clocks: Vec<Vec<i64>>,
    }

    fn degree(net: &TpnNet<(), ()>, marking: &Marking, t: TransitionId) -> usize {
        if !net.structurally_enabled(marking, t) {
            return 0;
        }
        let demand: Vec<_> = net
            .arcs_of(t, unipn::ArcDir::Input)
            .map(|a| {
                let w = net
                    .arcs_of(t, unipn::ArcDir::Input)
                    .filter(|b| b.place == a.place)
                    .map(|b| b.weight)
                    .sum::<usize>();
                (a.place, w)
            })
            .collect();
        let mut seen = HashSet::new();
        let mut deg = usize::MAX;
        for (p, w) in demand {
            if !seen.insert(p.index()) {
                continue;
            }
            deg = deg.min(marking.tokens(p) / w);
        }
        if seen.is_empty() { 1 } else { deg }
    }

    fn multi_fire(
        net: &TpnNet<(), ()>,
        state: &MultiIntState,
        t: TransitionId,
        i_f: usize,
        delta: i64,
        semantics: TpnSemantics,
        cap: i64,
    ) -> MultiIntState {
        let marking2 = NetLike::fire(net, &state.marking, t).unwrap();
        let disabling = match semantics.servers {
            Servers::Multiple(c) => c.disabling,
            Servers::Single => DisablingPolicy::OldestFirst,
        };
        let memory = match semantics.newly_enabled {
            NewlyEnabled::Intermediate => {
                let mut m = state.marking.clone();
                net.consume_inputs(&mut m, t);
                m
            }
            NewlyEnabled::Atomic => state.marking.clone(),
        };
        let mut clocks2 = vec![Vec::new(); net.num_transitions()];
        for u in net.transition_ids() {
            let old = &state.clocks[u.index()];
            let survivors: Vec<i64> = if u == t {
                // The fired instance is removed; all others survive.
                old.iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i_f)
                    .map(|(_, c)| (c + delta).min(cap))
                    .collect()
            } else {
                let count = match semantics.servers {
                    Servers::Single => {
                        if net.structurally_enabled(&marking2, u)
                            && net.structurally_enabled(&memory, u)
                        {
                            1
                        } else {
                            0
                        }
                    }
                    Servers::Multiple(_) => {
                        let deg_mem = degree(net, &memory, u);
                        let deg_new = degree(net, &marking2, u);
                        match semantics.newly_enabled {
                            NewlyEnabled::Intermediate => deg_mem.min(old.len()),
                            NewlyEnabled::Atomic => old.len().min(deg_new),
                        }
                    }
                };
                let start = match disabling {
                    DisablingPolicy::OldestFirst => old.len() - count,
                    DisablingPolicy::NewestFirst => 0,
                };
                old[start..start + count]
                    .iter()
                    .map(|c| (c + delta).min(cap))
                    .collect()
            };
            let fresh_count = degree(net, &marking2, u) - survivors.len();
            let mut list = survivors;
            list.extend(std::iter::repeat_n(0, fresh_count));
            list.sort_by(|a, b| b.cmp(a));
            clocks2[u.index()] = list;
        }
        MultiIntState {
            marking: marking2,
            clocks: clocks2,
        }
    }

    /// The integer-time explicit-state oracle for multi-server nets. Fires the
    /// oldest instance of each transition (matching the SCG), or every firable
    /// instance under `FiringPolicy::Any`.
    fn multi_integer_markings(
        net: &TpnNet<(), ()>,
        initial: &Marking,
        semantics: TpnSemantics,
        max_states: usize,
    ) -> Option<BTreeSet<Marking>> {
        let cap = max_constant(net) + 1;
        let mut clocks0 = vec![Vec::new(); net.num_transitions()];
        for t in net.transition_ids() {
            for _ in 0..degree(net, initial, t) {
                clocks0[t.index()].push(0);
            }
        }
        let init = MultiIntState {
            marking: initial.clone(),
            clocks: clocks0,
        };
        let mut seen: HashSet<MultiIntState> = HashSet::new();
        let mut queue: VecDeque<MultiIntState> = VecDeque::new();
        seen.insert(init.clone());
        queue.push_back(init);
        let mut markings = BTreeSet::new();

        while let Some(state) = queue.pop_front() {
            markings.insert(state.marking.clone());
            for t in net.transition_ids() {
                let instances = state.clocks[t.index()].len();
                let firable: Vec<usize> = match semantics.servers {
                    Servers::Multiple(c) if c.firing == FiringPolicy::Any => {
                        (0..instances).collect()
                    }
                    _ => {
                        if instances > 0 {
                            vec![0]
                        } else {
                            vec![]
                        }
                    }
                };
                for i_f in firable {
                    let interval = net.transition(t).unwrap().kind.interval;
                    let eft = interval.lower as i64;
                    let clock = state.clocks[t.index()][i_f];
                    let delta_min = (eft - clock).max(0);
                    let mut delta_max: Option<i64> = None;
                    for u in net.transition_ids() {
                        if let UpperBound::Finite { value, .. } =
                            net.transition(u).unwrap().kind.interval.upper
                        {
                            for &c in &state.clocks[u.index()] {
                                let b = value as i64 - c;
                                delta_max = Some(delta_max.map_or(b, |x| x.min(b)));
                            }
                        }
                    }
                    let delta_max = delta_max.unwrap_or(cap - clock);
                    for delta in delta_min..=delta_max {
                        let next = multi_fire(net, &state, t, i_f, delta, semantics, cap);
                        if seen.insert(next.clone()) {
                            if seen.len() >= max_states {
                                return None;
                            }
                            queue.push_back(next);
                        }
                    }
                }
            }
        }
        Some(markings)
    }

    // ── Safe nets: multi-server equals single-server ─────────────────────

    #[test]
    fn safe_nets_multi_server_equals_single_server() {
        // A safe net has at most one token per place, so every enabling degree
        // is at most 1 and the multi-server semantics collapses to
        // single-server.
        for seed in 0..200u64 {
            let (net, initial) = random_tpn(seed, false);
            // Keep only weight-1 nets with a 0/1 initial marking; verify safety
            // by exploring the untimed net.
            let Some(untimed) = untimed_markings_limited(&net, &initial, 5_000) else {
                continue;
            };
            if untimed.iter().any(|m| m.0.iter().any(|&c| c > 1)) {
                continue; // not safe
            }
            let single = TpnSemantics::default();
            let multi = multiple(DisablingPolicy::OldestFirst);
            let m_single = scg_markings_multi(&net, &initial, single, 5_000);
            let m_multi = scg_markings_multi(&net, &initial, multi, 5_000);
            assert_eq!(m_single, m_multi, "seed {seed}");
        }
    }

    // ── Hand-derived multi-instance examples ─────────────────────────────

    #[test]
    fn two_instances_fire_sequentially_or_simultaneously() {
        // p has 2 tokens; t: p→q [1,2]. Hand derivation: both instances start
        // with clock 0, so their firing times are in [1,2]. Fire one at
        // τ1 ∈ [1,2] ({p=1,q=1}); the survivor's clock is τ1, so its remaining
        // firing time is [max(0,1−τ1), 2−τ1] — [0,0] when τ1=2 (the two
        // instances fire at the same instant 2), and a positive interval
        // otherwise (they fire one after the other). Both ways, {q=2} is
        // reached.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 2);
        let q = b.add_place("q", Tag(()));
        let t = b.add_transition("t", TpnTransition::new((), Interval::closed(1, 2)));
        b.add_input_arc(p, t, 1);
        b.add_output_arc(q, t, 1);
        let (net, initial) = b.build();

        let m = scg_markings_multi(&net, &initial, multiple(DisablingPolicy::OldestFirst), 100);
        assert!(m.contains(&Marking::new(vec![2, 0])), "initial");
        assert!(m.contains(&Marking::new(vec![1, 1])), "one fired");
        assert!(m.contains(&Marking::new(vec![0, 2])), "both fired");
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn self_loop_grows_instances() {
        // p has 1 token; t: p→p+p [2,2]. Hand derivation: one instance at
        // start; firing it at time 2 leaves two fresh instances (the degree
        // grew 1 → 2); firing again at time 4 leaves 1 survivor (clock 2) and
        // two fresh ones (degree 2 → 3). Markings p=1,2,3 are reachable.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 1);
        let t = b.add_transition("t", TpnTransition::new((), Interval::closed(2, 2)));
        b.add_input_arc(p, t, 1);
        b.add_output_arc(p, t, 2);
        let (net, initial) = b.build();

        // The net is unbounded (p grows by one per firing), so the SCG
        // truncates; check the first few markings are found.
        let graph = unipn::analysis::tpn::state_class_graph(
            &net,
            initial.clone(),
            multiple(DisablingPolicy::OldestFirst),
            &unipn::analysis::AnalysisConfig {
                max_states: 20,
                ..unipn::analysis::AnalysisConfig::default()
            },
        )
        .unwrap();
        assert!(graph.truncated);
        let m: BTreeSet<Marking> = graph.states.iter().map(|c| c.marking.clone()).collect();
        assert!(m.contains(&Marking::new(vec![1])));
        assert!(m.contains(&Marking::new(vec![2])));
        assert!(m.contains(&Marking::new(vec![3])));
    }

    #[test]
    fn disabling_policy_changes_the_reachable_markings() {
        // p has 2 tokens; c, c2, c3 hold one token each and exist only to keep
        // the degree of t, v, x at 1 (they would otherwise inherit p's 2
        // tokens as two instances). u: p→q [5,10]; t: p+c→r [1,1];
        // w: r→p [2,2]; v: p+c2→s [4,4]; x: p+c3→xq [5,5].
        //
        // Hand derivation: t fires at 1 ({p=1,r=1}, one instance of u
        // disabled); w fires at 3, restoring p=2, so u has an old instance
        // (clock 3) and a fresh one (clock 0). v fires at 4 ({p=1,s=1}),
        // disabling one instance of u again:
        //  - OldestFirst (FEFD) disables the old instance; the fresh one
        //    (clock 1) fires no earlier than clock 5, i.e. time 8. x fires at
        //    time 5, taking the last token of p, so the fresh instance is
        //    disabled: q is never reached, and {s=1, xq=1} is final.
        //  - NewestFirst disables the fresh instance; the old one (clock 4)
        //    can fire from clock 5 on, i.e. at time 5 — simultaneously with x.
        //    One run fires it there, giving {q=1, s=1}; the other lets x win,
        //    giving {s=1, xq=1}. So {q=1} is reachable only under
        //    NewestFirst.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 2);
        let c = b.add_marked_place("c", Tag(()), 1);
        let c2 = b.add_marked_place("c2", Tag(()), 1);
        let c3 = b.add_marked_place("c3", Tag(()), 1);
        let q = b.add_place("q", Tag(()));
        let r = b.add_place("r", Tag(()));
        let s = b.add_place("s", Tag(()));
        let xq = b.add_place("xq", Tag(()));
        let u = b.add_transition("u", TpnTransition::new((), Interval::closed(5, 10)));
        let t = b.add_transition("t", TpnTransition::new((), Interval::closed(1, 1)));
        let w = b.add_transition("w", TpnTransition::new((), Interval::closed(2, 2)));
        let v = b.add_transition("v", TpnTransition::new((), Interval::closed(4, 4)));
        let x = b.add_transition("x", TpnTransition::new((), Interval::closed(5, 5)));
        b.add_input_arc(p, u, 1);
        b.add_output_arc(q, u, 1);
        b.add_input_arc(p, t, 1);
        b.add_input_arc(c, t, 1);
        b.add_output_arc(r, t, 1);
        b.add_input_arc(r, w, 1);
        b.add_output_arc(p, w, 1);
        b.add_input_arc(p, v, 1);
        b.add_input_arc(c2, v, 1);
        b.add_output_arc(s, v, 1);
        b.add_input_arc(p, x, 1);
        b.add_input_arc(c3, x, 1);
        b.add_output_arc(xq, x, 1);
        let (net, initial) = b.build();

        let oldest =
            scg_markings_multi(&net, &initial, multiple(DisablingPolicy::OldestFirst), 100);
        let newest =
            scg_markings_multi(&net, &initial, multiple(DisablingPolicy::NewestFirst), 100);

        let q_tokens = |m: &BTreeSet<Marking>| m.iter().any(|m| m.tokens(q) > 0);
        assert!(
            !q_tokens(&oldest),
            "FEFD: the fresh survivor is disabled by x before its clock reaches 5"
        );
        assert!(
            q_tokens(&newest),
            "newest-first: the old survivor fires at time 5, reaching q"
        );
        assert_ne!(oldest, newest, "the policies must disagree");

        // Both policies agree the oracle: compare against the integer-time
        // oracle under each policy.
        let oracle_oldest = multi_integer_markings(
            &net,
            &initial,
            multiple(DisablingPolicy::OldestFirst),
            50_000,
        )
        .unwrap();
        let oracle_newest = multi_integer_markings(
            &net,
            &initial,
            multiple(DisablingPolicy::NewestFirst),
            50_000,
        )
        .unwrap();
        assert_eq!(oldest, oracle_oldest);
        assert_eq!(newest, oracle_newest);
    }

    // ── Oracle A with multiple tokens ────────────────────────────────────

    /// A small random TPN with weight-1 arcs and 0/1 initial tokens, so the
    /// enabling degree — and with it the DBM size — stays small.
    fn random_unit_tpn(seed: u64) -> (TpnNet<(), ()>, Marking) {
        use super::analysis::Lcg;
        let mut rng = Lcg::new(seed);
        let places = 1 + rng.next(3) as usize;
        let transitions = 1 + rng.next(3) as usize;
        let mut b = TpnBuilder::new();
        let ps: Vec<_> = (0..places)
            .map(|i| b.add_marked_place(format!("p{i}"), Tag(()), rng.next(2) as usize))
            .collect();
        for j in 0..transitions {
            let a = rng.next(3);
            let interval = Interval::closed(a, a + rng.next(3));
            let t = b.add_transition(format!("t{j}"), TpnTransition::new((), interval));
            for &p in &ps {
                if rng.next(3) == 0 {
                    b.add_input_arc(p, t, 1);
                }
                if rng.next(3) == 0 {
                    b.add_output_arc(p, t, 1);
                }
            }
        }
        b.build()
    }

    #[test]
    fn oracle_a_multi_server_matches_scg_on_random_nets() {
        let mut compared = 0usize;
        for seed in 0..800u64 {
            let (net, initial) = random_unit_tpn(seed);
            // Only nets whose untimed state space is small and whose token
            // counts stay small: the multi-server SCG's DBM has one column per
            // instance, so large markings are out of reach by design.
            let Some(untimed) = untimed_markings_limited(&net, &initial, 500) else {
                continue;
            };
            if untimed.iter().any(|m| m.0.iter().sum::<usize>() > 6) {
                continue;
            }
            for disabling in [DisablingPolicy::OldestFirst, DisablingPolicy::NewestFirst] {
                let semantics = TpnSemantics {
                    newly_enabled: NewlyEnabled::Intermediate,
                    servers: Servers::Multiple(MultiServerConfig {
                        firing: FiringPolicy::OldestFirst,
                        disabling,
                    }),
                };
                let Some(scg) = scg_markings_limited(&net, &initial, semantics, 500) else {
                    continue;
                };
                let Some(oracle) = multi_integer_markings(&net, &initial, semantics, 500) else {
                    continue;
                };
                assert_eq!(
                    scg, oracle,
                    "seed {seed}, disabling {disabling:?}: SCG disagrees with the oracle"
                );
                compared += 1;
            }
        }
        assert!(compared >= 200, "only {compared} nets compared");
    }
}
