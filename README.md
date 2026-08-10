# UniPN — Unified Petri Net

A fast, extensible Petri net core shared by several frontends and analysis consumers:

```text
Frontends（建网）               Core（矩阵存储 + trait）          Consumers（分析）
  ConcIR  ─┐                   Net（CSC incidence 矩阵）          死锁/死迁移/冲突集
  Rust MIR┼─▶ NetLike ──────▶  explore (BFS/DFS/POR)              不变量 (invariants)
  测试意图┘   (object-safe)    deadlock / dead_transition          测试用例生成 (testgen)
  时间(PTPN) ─▶ Timed 预留     conflict / invariants / dot         时间/实时调度属性
```

## 设计原则

1. **Trait-first**：`NetLike` 是唯一契约（object-safe）。任何网（CVN、
   ConcBugDect MIR→PN、未来的测试/时间网）只需实现它即可被共享算法消费。
   **纯 P/T 网**只需填结构谓词（`pre_arcs` / `post_arcs` / `initial_state`），
   `enabled_transitions` 与 `fire` 直接用 trait 默认实现。
2. **矩阵底层**：核心 `Net` 用 **CSC 稀疏列**存储 `Pre/Post` incidence，
   enabled/fire 热路径是 O(弧数) 而非 O(|P|·|T|)；需要线性代数时
   （不变量等）才物化稠密 `C = Post − Pre`。
3. **语义外置**：`PlaceKind` / `TransitionKind` 只是标注；"线程终点 / 等待点 /
   资源"等语义由前端谓词（`is_thread_terminal` / `is_wait_point` /
   `is_resource`）暴露，common 层不做硬编码。`return` 是函数返回而非线程结束；
   spawn/join/branch 都是弧结构模式。
4. **可扩展**：`timed` / `invariants` 是 feature 门控的扩展位。

## 快速开始

```rust
use unipn::analysis::{AnalysisConfig, explore};
use unipn::expr::BoolExpr;
use unipn::model::{ControlSub, PlaceKind, TransitionKind};
use unipn::{NetBuilder, NetLike};

let mut b = NetBuilder::new();
let p0 = b.add_place("p0", PlaceKind::Control(ControlSub::Statement));
let p1 = b.add_place("p1", PlaceKind::Control(ControlSub::ThreadEnd));
let t0 = b.add_transition("t0", TransitionKind::Sequential);
b.add_input_arc(p0, t0, 1, BoolExpr::True);
b.add_output_arc(t0, p1, 1, None);
b.set_initial_tokens(p0, 1);
let net = b.build();

let rg = explore(&net, &AnalysisConfig::default());
assert!(rg.deadlocks.is_empty());
```

### 扩展一个自定义网

```rust
impl NetLike for MyNet {
    fn num_places(&self) -> usize { /* ... */ }
    fn num_transitions(&self) -> usize { /* ... */ }
    fn pre_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)> { /* ... */ }
    fn post_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)> { /* ... */ }
    fn initial_state(&self) -> State { /* ... */ }
    // enabled_transitions / fire 用默认纯 P/T 实现；带 guard 的前端自行覆盖。
}
// 之后共享算法直接可用：
let rg = explore(&my_net, &AnalysisConfig::default());
```

## 模块

```
src/
├── ids.rs        PlaceId/TransitionId（索引制）、Weight
├── model.rs      PlaceKind / TransitionKind / Place / Transition（标注）
├── expr.rs       Val / Expr / BoolExpr（可选数据模型，guard/update 用）
├── state.rs      Marking（稠密向量）/ VarStore / State
├── storage.rs    CSC 稀疏列 Incidence + 稠密效果矩阵 C = Post − Pre
├── netlike.rs    NetLike trait（object-safe）+ 纯 P/T 默认实现
├── net.rs        Net：矩阵存储网（可选 guard/update/容量/变量域）
├── builder.rs    NetBuilder
├── analysis/
│   ├── explore.rs      BFS / DFS / POR(sleep-set) → ReachabilityGraph
│   ├── deadlock.rs     死锁判定 + 阻塞库位
│   ├── dead_transition.rs  行为死迁移（含 OR 族处理）
│   ├── conflict.rs     共享输入库位的变迁对（测试生成选竞争点）
│   ├── invariants.rs   库位/变迁不变量（feature `invariants`）
│   └── timed.rs        状态类 DBM 时间分析预留（feature `timed`）
├── export.rs      Graphviz DOT
├── testgen.rs     可达图路径 → 测试用例 schedule（纯 consumer）
└── timed.rs       时间扩展类型：StaticInterval / Priority / ClockClass
```

## Feature flags

| Feature      | Default | 说明 |
| ------------ | ------- | ---- |
| `invariants` | on      | 库位/变迁不变量（Gaussian nullspace，BigInt 精确） |
| `timed`      | off     | 时间/优先级扩展（`Transition.timing/.priority`、`AnalysisMode::Timed`、PTPN 状态类 DBM 桥） |

## 时间分析（PTPN）预留

`timed` 特征引入静态时间区间 `[dmin, dmax]`、固定优先级与时钟类。目标是对接
[PTPN](https://github.com/kevindadi/PTPN)：统一网经导出桥变为 PTPN 的
`.ptpn` / TDG JSON，由 PTPN 做状态类（DBM）可达分析后回传结果。IR 层面只加
可选标注，不动核心 firing 语义。

## 测试

```bash
cargo test
cargo test --features timed
cargo test --no-default-features
```

## License

MIT
