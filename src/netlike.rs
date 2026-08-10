//! `NetLike`：统一网契约（object-safe）。
//!
//! 任何网（CVN、ConcBugDect MIR→PN、测试/时间网）实现该 trait 即可被
//! [`crate::analysis`] 的共享算法消费。**纯 P/T 网**只需填结构谓词
//! （`pre_arcs`/`post_arcs`/`initial_state`/`num_*`），`enabled_transitions` 与
//! `fire` 直接用默认实现；带 guard/update/容量/时间的前端自行覆盖。

use thiserror::Error;

use crate::ids::{PlaceId, TransitionId, Weight};
use crate::model::{ControlSub, PlaceKind, TransitionKind};
use crate::state::State;

/// 触发错误。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FireError {
    #[error("transition {0} is out of bounds")]
    OutOfBounds(TransitionId),
    #[error("transition {0} is not enabled under the supplied state")]
    NotEnabled(TransitionId),
    #[error("place {place} capacity exceeded: {after} > {capacity}")]
    Capacity {
        place: PlaceId,
        after: Weight,
        capacity: Weight,
    },
}

/// 统一网契约。
pub trait NetLike {
    // ── 结构 ──

    fn num_places(&self) -> usize;
    fn num_transitions(&self) -> usize;

    fn place_ids(&self) -> Vec<PlaceId> {
        (0..self.num_places()).map(PlaceId).collect()
    }

    fn transition_ids(&self) -> Vec<TransitionId> {
        (0..self.num_transitions()).map(TransitionId).collect()
    }

    fn place_label(&self, _p: PlaceId) -> String {
        String::new()
    }

    fn place_kind(&self, _p: PlaceId) -> Option<PlaceKind> {
        None
    }

    fn transition_label(&self, _t: TransitionId) -> String {
        String::new()
    }

    fn transition_kind(&self, _t: TransitionId) -> Option<TransitionKind> {
        None
    }

    /// 回映锚点（ConcIR sid / 源码行号），默认空。
    fn transition_anchors(&self, _t: TransitionId) -> Vec<String> {
        Vec::new()
    }

    /// disjunctive OR 族，默认无。
    fn transition_family(&self, _t: TransitionId) -> Option<&str> {
        None
    }

    /// 变迁 t 的 preset：`(place, weight)`。
    fn pre_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)>;

    /// 变迁 t 的 postset：`(place, weight)`。
    fn post_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)>;

    // ── 语义谓词（前端提供，common 层默认按标注推断）──

    /// 该库位是否为线程作用域终点（死锁判定用）。
    fn is_thread_terminal(&self, p: PlaceId) -> bool {
        matches!(
            self.place_kind(p),
            Some(PlaceKind::Control(ControlSub::ThreadEnd | ControlSub::FunctionEnd))
        )
    }

    /// 该库位是否为 condvar 等待点（signal-loss 分类用）。
    fn is_wait_point(&self, p: PlaceId) -> bool {
        matches!(self.place_kind(p), Some(PlaceKind::Control(ControlSub::WaitPoint)))
    }

    /// 该库位是否为资源位。
    fn is_resource(&self, p: PlaceId) -> bool {
        matches!(self.place_kind(p), Some(PlaceKind::Resource(_)))
    }

    // ── 运行时 ──

    fn initial_state(&self) -> State;

    /// 给定状态下的使能变迁集合。默认实现为纯 P/T 语义。
    fn enabled_transitions(&self, s: &State) -> Vec<TransitionId> {
        let mut out = Vec::new();
        for t in self.transition_ids() {
            let mut ok = true;
            for (p, w) in self.pre_arcs(t) {
                if s.marking.tokens(p) < w {
                    ok = false;
                    break;
                }
            }
            if ok {
                out.push(t);
            }
        }
        out
    }

    /// 触发变迁。默认实现为纯 P/T 语义（消费 preset、产出 postset）。
    fn fire(&self, t: TransitionId, s: &State) -> Result<State, FireError> {
        if t.index() >= self.num_transitions() {
            return Err(FireError::OutOfBounds(t));
        }
        let mut next = s.clone();
        for (p, w) in self.pre_arcs(t) {
            let tokens = next.marking.tokens(p);
            if tokens < w {
                return Err(FireError::NotEnabled(t));
            }
            next.marking.set(p, tokens - w);
        }
        for (p, w) in self.post_arcs(t) {
            let after = next.marking.tokens(p) + w;
            if let Some(PlaceKind::Resource(ty)) = self.place_kind(p) {
                if let Some(cap) = capacity_of(&ty) {
                    if after > cap {
                        return Err(FireError::Capacity {
                            place: p,
                            after,
                            capacity: cap,
                        });
                    }
                }
            }
            next.marking.set(p, after);
        }
        Ok(next)
    }
}

/// 资源类型的容量（Mutex=1、RwLock=max_readers、Semaphore=count；其余无界）。
fn capacity_of(ty: &crate::model::ResourceType) -> Option<u32> {
    match ty {
        crate::model::ResourceType::Mutex => Some(1),
        crate::model::ResourceType::RwLock { max_readers } => Some(*max_readers),
        crate::model::ResourceType::Semaphore { count } => Some(*count),
        _ => None,
    }
}
