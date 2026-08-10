//! 库位/变迁的种类标注。
//!
//! `kind` 只是标注，不参与 firing 语义 —— 引擎一律当普通 P/T 位处理；
//! "线程终点/等待点/资源"等语义通过 [`crate::netlike::NetLike`] 谓词暴露。

use crate::ids::Weight;

/// 库位种类（两类：控制流 / 资源）。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlaceKind {
    /// 控制流位：token = 一个线程实例位于某个控制点。
    Control(ControlSub),
    /// 资源位：token 数 = 可用单元（初始 marking 由前端给出）。
    Resource(ResourceType),
}

/// 控制位结构子类（仅用于可视化/回映/语义谓词默认值）。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ControlSub {
    /// ConcIR 单条语句。
    Statement,
    /// MIR 基本块。
    BasicBlock,
    FunctionStart,
    FunctionEnd,
    /// 函数返回点（普通控制转移，**不是**线程结束）。
    Return,
    /// 线程作用域终点（入口 / spawn 目标函数的返回位，前端标注）。
    ThreadEnd,
    /// 同步调用停车位。
    CallWait,
    /// condvar 等待点（控制流标注，供 signal-loss 分类）。
    WaitPoint,
    /// condvar 重新加锁位。
    Reacquire,
    /// spawn 骨架桥。
    SpawnBridge,
    /// 测试编排点。
    TestPoint,
}

/// 资源类型（决定初始 token 的语义）。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ResourceType {
    /// 互斥锁（初始 token = 1）。
    Mutex,
    /// 读写锁（初始 token = N = 并发实体数）。
    RwLock { max_readers: u32 },
    /// 计数信号量（初始 token = count）。
    Semaphore { count: u32 },
    /// 信道（初始 token = 0）。
    Channel,
    /// 条件变量（配合 WaitPoint 控制位使用）。
    Condvar,
}

/// 统一变迁分类（标注；firing 语义由弧结构决定）。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TransitionKind {
    // ── 顺序 / 控制 ──
    Sequential,
    Goto,
    FunctionEnter,
    FunctionExit,
    /// 函数返回（普通控制转移，非线程结束）。
    Return,
    Drop,
    BranchTrue,
    BranchFalse,
    Switch { label: String },
    // ── 同步资源 ──
    /// Mutex / RwLock 写锁。
    Lock,
    Unlock,
    ReadLock,
    ReadUnlock,
    /// Semaphore 获取/释放。
    Acquire,
    Release,
    /// Channel 收发。
    Send,
    Recv,
    // ── 数据 ──
    VarRead,
    VarWrite,
    AtomicLoad,
    AtomicStore,
    AtomicCmpXchg,
    CasSuccess,
    CasFailure,
    /// MIR unsafe 访问（datarace 检测用）。
    UnsafeRead,
    UnsafeWrite,
    UnsafeAccess,
    // ── 线程 ──
    Spawn,
    Join,
    Call,
    // ── condvar ──
    CondvarWaitEnter,
    CondvarWakeByNotify,
    CondvarWakeByNotifyAll,
    CondvarReacquire,
    CondvarNotify,
    CondvarNotifyLost,
    CondvarNotifyAll,
    CondvarNotifyAllLost,
    // ── 测试编排 ──
    TestBarrier,
    TestInject,
    TestPoint,
    // ── 兜底 ──
    Other(String),
}

/// 库位。`capacity` 可选（ConcBugDect 有，CVN 无）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub id: crate::ids::PlaceId,
    pub name: String,
    pub kind: PlaceKind,
    pub capacity: Option<Weight>,
}

/// 变迁。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    pub id: crate::ids::TransitionId,
    pub name: String,
    pub kind: TransitionKind,
    /// 回映锚点（ConcIR sid 或源码行号）。
    pub anchors: Vec<String>,
    /// disjunctive OR 族（互斥变体防死迁移误报）。
    pub family: Option<String>,
    /// 时间扩展（feature `timed`）：静态延迟区间。
    #[cfg(feature = "timed")]
    pub timing: Option<crate::timed::StaticInterval>,
    /// 时间扩展（feature `timed`）：固定优先级。
    #[cfg(feature = "timed")]
    pub priority: Option<crate::timed::Priority>,
}
