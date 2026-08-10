//! 冲突集：共享输入库位的变迁对。测试生成用它挑选要加压的竞争点。

use rustc_hash::FxHashMap;
use std::collections::BTreeSet;

use crate::ids::{PlaceId, TransitionId};
use crate::netlike::NetLike;

/// 共享同一输入库位的变迁对（潜在竞争/冲突）。
///
/// 返回升序 `(t1, t2), t1 < t2`，且两变迁在某个库位上都消耗 token。
pub fn conflict_sets(net: &dyn NetLike) -> Vec<(TransitionId, TransitionId)> {
    let mut by_place: FxHashMap<PlaceId, Vec<TransitionId>> = FxHashMap::default();
    for t in net.transition_ids() {
        for (p, _) in net.pre_arcs(t) {
            by_place.entry(p).or_default().push(t);
        }
    }

    let mut pairs: BTreeSet<(TransitionId, TransitionId)> = BTreeSet::new();
    for consumers in by_place.values() {
        if consumers.len() < 2 {
            continue;
        }
        let mut sorted = consumers.clone();
        sorted.sort_by_key(|t| t.0);
        for i in 0..sorted.len() {
            for j in (i + 1)..sorted.len() {
                pairs.insert((sorted[i], sorted[j]));
            }
        }
    }
    pairs.into_iter().collect()
}
