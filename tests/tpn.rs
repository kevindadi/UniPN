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
