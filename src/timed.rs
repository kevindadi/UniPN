//! 时间扩展预留位（feature `timed`）。
//!
//! 目标：对接 [PTPN](https://github.com/kevindadi/PTPN) 的优先级时间 Petri 网，
//! 用状态类（state-class）DBM 可达分析验证时间相关属性（WCET、可调度性、
//! deadline、实时互斥）。
//!
//! 集成路径：与 PTPN 自身导出 Romeo `.cts` / PToPNer `.ppn` 一致 —— 统一网
//! 通过导出桥变为 PTPN 的 `.ptpn` / TDG JSON，由 PTPN 做状态类分析后回传
//! （DBM 区域、调度状态）。IR 层面只加可选标注，不动核心 firing 语义。

use serde::{Deserialize, Serialize};

/// 静态时间区间 `[dmin, dmax]`（T-timed：变迁使能后须等待区间内某一时刻）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StaticInterval {
    pub dmin: u64,
    pub dmax: u64,
}

/// 固定优先级（值越大优先级越高；抢占/调度用）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Priority(pub u32);

/// 时钟类：把若干库位/变迁归入同一时钟（状态类分析中的时钟变量）。
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClockClass {
    pub name: String,
    /// 归属该时钟的库位/变迁 id 前缀。
    pub members: Vec<String>,
}
