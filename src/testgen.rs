//! 用 Petri 网指导并发测试用例生成。
//!
//! 核心洞察：**可达图的每条路径就是一个线程交错 schedule**，测试用例 =
//! 路径（schedule）+ 各步的数据约束（变量库）。本模块是可达图之上的纯
//! consumer，只读 [`crate::netlike::NetLike`]。

use crate::analysis::{FiringStep, ReachabilityGraph};
use crate::state::{State, VarStore};

/// 覆盖准则。
#[derive(Clone, Debug, Default)]
pub enum CoverageCriteria {
    /// 路径覆盖：每条可执行路径一个用例（贪心去重）。
    #[default]
    Path,
    /// 冲突对覆盖：为每个共享输入库位的变迁对生成一个"交错对"用例。
    ConflictPair,
    /// 边界状态覆盖：覆盖所有 guard 临界状态（`x==k` / `x>=k`）。
    BoundaryState,
    /// 死锁回归：直接取死锁反例的 trace。
    DeadlockRegression,
}

/// 一个生成的测试用例。
#[derive(Clone, Debug)]
pub struct TestCase {
    /// schedule：按序执行的 (变迁, 回映锚点)。
    pub schedule: Vec<FiringStep>,
    /// 每步前的状态（用于断言/回放）。
    pub states: Vec<State>,
    /// 初始变量绑定（数据约束）。
    pub input_bindings: VarStore,
    /// 期望断言（目标状态/不变量）。
    pub expectations: Vec<String>,
}

/// 从可达图提取"最长终止路径"作为基础用例集。
pub fn extract_schedules(rg: &ReachabilityGraph) -> Vec<Vec<FiringStep>> {
    // 简单启发：从初始状态出发做 DFS 到 terminal（无可使能或死锁），
    // 每次取一条深度优先路径。
    let mut out = Vec::new();
    let mut visited = vec![false; rg.states.len()];
    dfs_paths(rg, rg.initial, &mut visited, &mut Vec::new(), &mut out);
    out
}

fn dfs_paths(
    rg: &ReachabilityGraph,
    idx: usize,
    visited: &mut Vec<bool>,
    path: &mut Vec<FiringStep>,
    out: &mut Vec<Vec<FiringStep>>,
) {
    visited[idx] = true;
    let outgoing: Vec<(usize, usize, crate::ids::TransitionId)> = rg
        .edges
        .iter()
        .filter(|(s, _, _)| *s == idx)
        .copied()
        .collect();

    if outgoing.is_empty() {
        if !path.is_empty() {
            out.push(path.clone());
        }
        visited[idx] = false;
        return;
    }

    for (_, dst, t) in outgoing {
        let anchors = Vec::new();
        path.push(FiringStep {
            transition: t,
            anchors,
        });
        if !visited[dst] {
            dfs_paths(rg, dst, visited, path, out);
        }
        path.pop();
    }
    visited[idx] = false;
}

/// 生成测试用例（预留）。
pub fn generate_tests(
    _net: &dyn crate::netlike::NetLike,
    _rg: &ReachabilityGraph,
    _criteria: CoverageCriteria,
) -> Vec<TestCase> {
    // TODO: 按准则从 extract_schedules 派生用例 + 变量绑定 + 断言。
    Vec::new()
}
