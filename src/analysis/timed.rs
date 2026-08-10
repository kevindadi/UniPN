//! 时间分析预留模块（feature `timed`）。
//!
//! 状态类（state-class）可达分析：每个状态类 = `(marking, DBM)`，
//! DBM 约束时钟差与绝对值。 canonicalization 用 equality / max-lower /
//! intersection 三法合一（对齐 PTPN 的 `canonicalization` 模块）。
//!
//! TODO: 移植 PTPN 的 state-class 探索（DBM Floyd-Warshall + 差分约束），
//! 以及对 `Transition.timing/.priority` 的使能/触发语义扩展。

use crate::netlike::NetLike;
use crate::state::State;

/// 一个状态类：标记 + 时钟差分约束矩阵（DBM）。
#[derive(Clone, Debug)]
pub struct StateClass {
    pub state: State,
    pub dbm: Vec<Vec<i64>>,
}

/// 时间分析配置。
#[derive(Clone, Debug)]
pub struct TimedConfig {
    /// 各时钟的界限（时钟 id → 上界）。
    pub clock_bounds: Vec<i64>,
    /// 是否启用优先级抢占。
    pub priorities: bool,
}

/// 探索状态类可达图（预留）。
pub fn explore_timed(
    _net: &dyn NetLike,
    _config: &TimedConfig,
) -> Result<Vec<StateClass>, String> {
    Err("timed state-class analysis not implemented yet — see PTPN bridge".into())
}
