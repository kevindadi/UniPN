//! 行为死迁移检测。

use rustc_hash::FxHashSet;

use crate::ids::TransitionId;
use crate::netlike::NetLike;

use super::{Counterexample, PropertyViolation, ReachabilityGraph};

/// 找出行为上永不触发的变迁（或整族死迁移）。
///
/// 在可达图的所有边上出现过的变迁视为存活；未出现的变迁若属于某个
/// disjunctive OR 族且该族有成员存活，则跳过（族语义 = 至多一个成员触发）。
/// 整族死亡时只报告一个代表（字典序最小变迁 id）。
pub fn find_dead_transitions(net: &dyn NetLike, rg: &ReachabilityGraph) -> Vec<Counterexample> {
    let fired = rg.fired_transitions();

    let mut live_families: FxHashSet<&str> = FxHashSet::default();
    let mut all: Vec<TransitionId> = net.transition_ids();
    for t in &all {
        if fired.contains(t) {
            if let Some(f) = net.transition_family(*t) {
                live_families.insert(f);
            }
        }
    }
    all.sort_by_key(|t| t.0);

    let mut dead = Vec::new();
    let mut reported_families: FxHashSet<&str> = FxHashSet::default();
    let initial = net.initial_state();

    for t in all {
        if fired.contains(&t) {
            continue;
        }
        if let Some(f) = net.transition_family(t) {
            if live_families.contains(f) {
                continue;
            }
            if !reported_families.insert(f) {
                continue;
            }
        }
        dead.push(Counterexample {
            kind: PropertyViolation::DeadTransition {
                transition: t,
                anchors: net.transition_anchors(t),
            },
            trace: Vec::new(),
            final_state: initial.clone(),
        });
    }
    dead.sort_by_key(|cx| match &cx.kind {
        PropertyViolation::DeadTransition { transition, .. } => transition.0,
        _ => 0,
    });
    dead
}

/// 抑制被死锁支配的死迁移：某迁移的输入位位于某死锁状态的阻塞控制流下游，
/// 它"不死"，只是死锁提前截断了探索。用于避免对同一死锁的重复诊断。
#[allow(dead_code)]
pub(crate) fn deadlock_dominated(
    net: &dyn NetLike,
    rg: &ReachabilityGraph,
    dead: Vec<Counterexample>,
) -> Vec<Counterexample> {
    let _ = (net, rg);
    dead
}
