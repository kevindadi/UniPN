//! 死锁判定：无可使能变迁，且至少一个线程未到线程终点位。

use crate::netlike::NetLike;
use crate::state::State;

/// 判断一个"无可使能变迁"的状态是否为死锁。
///
/// 资源位 token（Mutex/RwLock/Semaphore/Channel）不属于控制流；若所有
/// 控制流 token 都位于 [`NetLike::is_thread_terminal`] 位，线程已全部完成，
/// 不算死锁。
pub fn is_deadlock(net: &dyn NetLike, state: &State) -> bool {
    state
        .marking
        .iter_nonzero()
        .any(|(p, _)| !net.is_resource(p) && !net.is_thread_terminal(p))
}

/// 死锁状态下被阻塞的库位集合（诊断用）。
pub fn blocked_places(net: &dyn NetLike, state: &State) -> Vec<crate::ids::PlaceId> {
    state
        .marking
        .iter_nonzero()
        .filter(|(p, _)| !net.is_resource(*p) && !net.is_thread_terminal(*p))
        .map(|(p, _)| p)
        .collect()
}
