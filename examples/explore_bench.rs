//! Reachability throughput on n dining philosophers who pick up their left fork
//! first — a net whose state space grows exponentially in n while each
//! transition touches only three or four arcs, so it shows whether per-state
//! work scales with a transition's own arcs or with the whole arc list.
//!
//! ```bash
//! cargo run --release --example explore_bench            # n = 6 8 10 12
//! cargo run --release --example explore_bench -- 9 11    # chosen sizes
//! ```
//!
//! Prints the state, edge, and deadlock counts with the wall-clock time; the
//! counts are what must not change when the firing code does.

use std::time::Instant;

use unipn::Marking;
use unipn::analysis::{AnalysisConfig, explore, find_deadlocks};
use unipn::net::ControlSub;
use unipn::pt::{PtBuilder, PtNet, PtPlaceKind, PtTransitionKind, TransitionType};

fn philosophers(n: usize) -> (PtNet, Marking) {
    let mut b = PtBuilder::new();
    let think: Vec<_> = (0..n)
        .map(|i| {
            b.add_marked_place(
                format!("think{i}"),
                PtPlaceKind::control(ControlSub::BasicBlock),
                1,
            )
        })
        .collect();
    let left: Vec<_> = (0..n)
        .map(|i| {
            b.add_place(
                format!("left{i}"),
                PtPlaceKind::control(ControlSub::BasicBlock),
            )
        })
        .collect();
    let eat: Vec<_> = (0..n)
        .map(|i| {
            b.add_place(
                format!("eat{i}"),
                PtPlaceKind::control(ControlSub::BasicBlock),
            )
        })
        .collect();
    let fork: Vec<_> = (0..n)
        .map(|i| b.add_marked_place(format!("fork{i}"), PtPlaceKind::resource(), 1))
        .collect();

    for i in 0..n {
        let next = (i + 1) % n;
        let take_left = b.add_transition(
            format!("takeL{i}"),
            PtTransitionKind::new(TransitionType::Lock(i)),
        );
        b.add_input_arc(think[i], take_left, 1);
        b.add_input_arc(fork[i], take_left, 1);
        b.add_output_arc(left[i], take_left, 1);

        let take_right = b.add_transition(
            format!("takeR{i}"),
            PtTransitionKind::new(TransitionType::Lock(next)),
        );
        b.add_input_arc(left[i], take_right, 1);
        b.add_input_arc(fork[next], take_right, 1);
        b.add_output_arc(eat[i], take_right, 1);

        let release = b.add_transition(
            format!("release{i}"),
            PtTransitionKind::new(TransitionType::Unlock(i)),
        );
        b.add_input_arc(eat[i], release, 1);
        b.add_output_arc(think[i], release, 1);
        b.add_output_arc(fork[i], release, 1);
        b.add_output_arc(fork[next], release, 1);
    }
    b.build()
}

fn main() {
    let sizes: Vec<usize> = std::env::args()
        .skip(1)
        .map(|a| a.parse().expect("sizes must be integers"))
        .collect();
    let sizes = if sizes.is_empty() {
        vec![6, 8, 10, 12]
    } else {
        sizes
    };

    let config = AnalysisConfig {
        max_states: usize::MAX,
        ..AnalysisConfig::default()
    };

    println!(
        "{:>3} {:>7} {:>9} {:>10} {:>9} {:>10}",
        "n", "arcs", "states", "edges", "deadlocks", "ms"
    );
    for n in sizes {
        let (net, initial) = philosophers(n);
        let start = Instant::now();
        let graph = explore(&net, initial, &config);
        let deadlocks = find_deadlocks(&graph, |m| net.is_deadlock(m));
        let elapsed = start.elapsed();
        println!(
            "{:>3} {:>7} {:>9} {:>10} {:>9} {:>10.1}",
            n,
            net.arcs().len(),
            graph.state_count(),
            graph.edge_count(),
            deadlocks.len(),
            elapsed.as_secs_f64() * 1000.0
        );
    }
}
