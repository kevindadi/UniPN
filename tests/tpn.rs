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
    use unipn::Tag;
    use unipn::analysis::AnalysisConfig;
    use unipn::analysis::tpn::{NewlyEnabled, Servers, TpnSemantics, state_class_graph};
    use unipn::tpn::{Interval, TpnBuilder, TpnTransition};

    fn reachable_markings<PL: Clone + Eq + std::hash::Hash, TL: Clone + Eq + std::hash::Hash>(
        net: &unipn::tpn::TpnNet<PL, TL>,
        initial: unipn::Marking,
        semantics: TpnSemantics,
    ) -> std::collections::BTreeSet<unipn::Marking> {
        let graph = state_class_graph(net, initial, semantics, &AnalysisConfig::default()).unwrap();
        graph.states.iter().map(|c| c.marking.clone()).collect()
    }

    #[test]
    fn background_example_intermediate_vs_atomic() {
        // p has 1 token; `loop: p→p [1,1]`, `t: p→q [2,2]`.
        let mut b = TpnBuilder::new();
        let p = b.add_marked_place("p", Tag(()), 1);
        let q = b.add_place("q", Tag(()));
        let lp = b.add_transition("loop", TpnTransition::new((), Interval::closed(1, 1)));
        let t = b.add_transition("t", TpnTransition::new((), Interval::closed(2, 2)));
        b.add_input_arc(p, lp, 1);
        b.add_output_arc(p, lp, 1);
        b.add_input_arc(p, t, 1);
        b.add_output_arc(q, t, 1);
        let (net, initial) = b.build();

        let intermediate = TpnSemantics {
            newly_enabled: NewlyEnabled::Intermediate,
            servers: Servers::Single,
        };
        let atomic = TpnSemantics {
            newly_enabled: NewlyEnabled::Atomic,
            servers: Servers::Single,
        };

        let m_inter = reachable_markings(&net, initial.clone(), intermediate);
        let m_atomic = reachable_markings(&net, initial.clone(), atomic);

        // Intermediate: t resets after every loop firing, so q is unreachable.
        assert!(!m_inter.iter().any(|m| m.tokens(q) > 0));
        // Atomic: t keeps its clock across loop firings, so q is reachable.
        assert!(m_atomic.iter().any(|m| m.tokens(q) > 0));
    }
}
