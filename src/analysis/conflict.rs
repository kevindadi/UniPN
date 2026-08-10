//! Conflict sets: transition pairs sharing an input place. Test-case generation
//! uses them to select contention points to stress.

use rustc_hash::FxHashMap;
use std::collections::BTreeSet;

use crate::ids::{PlaceId, TransitionId};
use crate::netlike::NetLike;

/// Transition pairs sharing the same input place (potential races/conflicts).
///
/// Returns ascending `(t1, t2), t1 < t2`, where both transitions consume tokens
/// from some place.
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
