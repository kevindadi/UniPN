//! 死锁检测。

mod common;

use unipn::analysis::{AnalysisConfig, explore};

#[test]
fn mutex_deadlock_detected() {
    let net = common::mutex_deadlock();
    let rg = explore(&net, &AnalysisConfig::default());
    assert!(!rg.deadlocks.is_empty(), "expect at least one deadlock");

    // 至少一个死锁是"互等"死锁：两个线程各持一锁等另一锁。
    let mutual_wait = rg.deadlocks.iter().any(|d| {
        unipn::analysis::blocked_places(&net, &d.final_state).len() >= 2
    });
    assert!(mutual_wait, "a 2-thread mutual-wait deadlock must be found");

    // 每个死锁反例都有非空 witness trace。
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
    // 两个锁的获取顺序：t1A·t2B（死锁）以及 t1A·t1B、t2B·t2A 等。
    // 至少一条路径走到 t1_done 或 t2_done（无死锁完成序存在）。
    let terminal = rg
        .states
        .iter()
        .any(|s| s.marking.tokens(unipn::PlaceId(2)) > 0 || s.marking.tokens(unipn::PlaceId(5)) > 0);
    assert!(terminal, "a lock-order-abiding interleaving must complete");
}
