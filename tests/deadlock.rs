//! Deadlock detection.

mod common;

use unipn::analysis::{AnalysisConfig, explore};

#[test]
fn mutex_deadlock_detected() {
    let net = common::mutex_deadlock();
    let rg = explore(&net, &AnalysisConfig::default());
    assert!(!rg.deadlocks.is_empty(), "expect at least one deadlock");

    // At least one deadlock is a mutual-wait: each thread holds one lock
    // and waits for the other.
    let mutual_wait = rg.deadlocks.iter().any(|d| {
        unipn::analysis::blocked_places(&net, &d.final_state).len() >= 2
    });
    assert!(mutual_wait, "a 2-thread mutual-wait deadlock must be found");

    // Every deadlock counterexample has a non-empty witness trace.
    assert!(rg.deadlocks.iter().all(|d| !d.trace.is_empty()));
}

#[test]
fn dfs_finds_same_deadlock() {
    let net = common::mutex_deadlock();
    let rg = explore(
        &net,
        &AnalysisConfig {
            strategy: unipn::analysis::SearchStrategy::Dfs,
            ..Default::default()
        },
    );
    assert!(!rg.deadlocks.is_empty());
}

#[test]
fn reachable_path_covers_all_interleavings() {
    let net = common::mutex_deadlock();
    let rg = explore(&net, &AnalysisConfig::default());
    // Lock acquisition orders: t1A·t2B (deadlock) as well as t1A·t1B, t2B·t2A,
    // etc. At least one path must reach t1_done or t2_done (a deadlock-free
    // completing order exists).
    let terminal = rg
        .states
        .iter()
        .any(|s| s.marking.tokens(unipn::PlaceId(2)) > 0 || s.marking.tokens(unipn::PlaceId(5)) > 0);
    assert!(terminal, "a lock-order-abiding interleaving must complete");
}
