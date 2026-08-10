//! 共享分析引擎。所有算法只依赖 [`NetLike`]，可消费任意前端产出的网。

mod conflict;
mod dead_transition;
mod deadlock;
mod explore;
#[cfg(feature = "invariants")]
pub mod invariants;
#[cfg(feature = "timed")]
pub mod timed;

use crate::ids::TransitionId;
use crate::state::State;

pub use conflict::*;
pub use dead_transition::*;
pub use deadlock::*;
pub use explore::*;

/// 分析模式。
///
/// `Timed` 是预留模式（feature `timed`）：走状态类（DBM）可达分析，对接
/// PTPN 的时间/实时调度属性验证。未启用时退化为无时间语义的可达图。
#[derive(Clone, Debug)]
pub enum AnalysisMode {
    Untimed,
    #[cfg(feature = "timed")]
    Timed {
        clock_classes: Vec<crate::timed::ClockClass>,
        /// 是否启用固定优先级抢占语义。
        priorities: bool,
    },
}

/// 探索配置。
#[derive(Clone, Debug)]
pub struct AnalysisConfig {
    pub mode: AnalysisMode,
    pub strategy: SearchStrategy,
    pub max_states: usize,
    /// 偏序归约（sleep-set）。
    pub por: bool,
    /// 建图前先做网归约（loop/sequence/intermediate，占位）。
    pub reduce: bool,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            mode: AnalysisMode::Untimed,
            strategy: SearchStrategy::Bfs,
            max_states: 100_000,
            por: false,
            reduce: false,
        }
    }
}

/// 探索策略。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchStrategy {
    /// 广度优先（最短反例）。
    #[default]
    Bfs,
    /// 深度优先（省内存）。
    Dfs,
}

/// 单步触发记录。
#[derive(Clone, Debug)]
pub struct FiringStep {
    pub transition: TransitionId,
    pub anchors: Vec<String>,
}

/// 违规类型。
#[derive(Clone, Debug)]
pub enum PropertyViolation {
    Deadlock,
    DeadTransition { transition: TransitionId, anchors: Vec<String> },
    GoalUnmet { goal: String },
}

/// 反例：触发序列 + 终态 + 违规类型。
#[derive(Clone, Debug)]
pub struct Counterexample {
    pub kind: PropertyViolation,
    pub trace: Vec<FiringStep>,
    pub final_state: State,
}
