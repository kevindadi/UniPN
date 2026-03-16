# CVN — Concurrency Verification Net

A Rust library implementing **weighted P/T Petri nets with global variable guards**, designed for formal verification of concurrent programs.

CVN (Concurrency Verification Net) is domain-specialized for concurrency bug detection: it models mutexes, RwLocks, semaphores, channels, and condition variables as resource places, and uses three-valued expression evaluation with Unknown over-approximation to soundly explore all possible interleavings.

## How it works

```
CIR (Concurrency IR) ──translate──▶ CVN ──analyze──▶ Counterexample ──map──▶ CIR sid
```

This library handles the **CVN layer only** — CIR parsing and CIR→CVN translation are out of scope.

## Quick start

```rust
use cvn::builder::CvnNetBuilder;
use cvn::model::*;
use cvn::analysis::{AnalysisConfig, explore};

let net = CvnNetBuilder::new()
    .add_control_place("p0", "main", "s0")
    .add_control_place("p1", "main", "s1")
    .set_return("p1")
    .add_transition("t0", TransitionKind::Sequential)
    .add_input_arc("p0", "t0", 1, BoolExpr::True)
    .add_output_arc("t0", "p1", 1, None)
    .set_initial_tokens("p0", 1)
    .build()
    .expect("valid net");

let result = explore(&net, &AnalysisConfig::default()).unwrap();
assert!(result.deadlocks.is_empty());
```

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `cir-anchor` | off | Transitions carry CIR statement ID anchors for mapping counterexamples back to source locations |

When `cir-anchor` is enabled, use `add_transition_with_anchor()` to attach SIDs and
`build_with_anchor_check()` to enforce W7 (every transition must have at least one anchor).

## Module structure

```
cvn
├── model/            Core data types
│   ├── place         PlaceId, Place, PlaceKind (Control / Resource / Wait)
│   ├── transition    TransitionId, Transition, TransitionKind
│   ├── arc           InputArcData, OutputArcData, VarUpdate
│   ├── val           Val (Concrete / Unknown), ConcreteVal, ResourceType
│   ├── expr          Expr, BoolExpr, GuardResult, eval_expr, eval_guard, DSL helpers
│   └── state         Marking (sparse FxHashMap), VarStore (IndexMap), State
│
├── net               CvnNet — petgraph DiGraph wrapper with enabled/fire semantics
├── builder           CvnNetBuilder — chain-style construction with build-time validation
├── validate          Well-formedness checks (W2–W9)
├── error             CvnError with error codes V0xx–V4xx
│
├── analysis/         State space exploration
│   ├── search        BFS/DFS engine, reachability graph, AnalysisConfig
│   ├── deadlock      is_terminal, is_deadlock, blocked_places
│   └── counterexample Counterexample, FiringStep, PropertyViolation
│
└── export            DOT format output for Graphviz visualization
```

## Formal definition

```
CVN = ( P, T, A_in, A_out, V, I_m, I_v, μ )
```

| Component | Description |
|-----------|-------------|
| P = P_c ⊎ P_r ⊎ P_w | Places (control / resource / wait) |
| T | Transitions |
| A_in ⊆ P × T | Input arcs with weight and guard |
| A_out ⊆ T × P | Output arcs with weight and optional var update |
| V | Global variable store |
| I_m, I_v | Initial marking and variable values |
| μ: T → 𝒫(SID) | Anchor mapping to CIR statement IDs (requires `cir-anchor` feature) |

## Key features

- **Sparse marking**: only stores places with tokens > 0 (via `FxHashMap`)
- **Three-valued evaluation**: `Unknown` absorbs through expressions; guards returning `Unknown` are treated as satisfied (sound over-approximation)
- **Builder pattern**: `CvnNetBuilder` with comprehensive well-formedness validation at build time
- **petgraph integration**: the net is a `DiGraph<NetNode, NetEdge>` bipartite graph, accessible via `net.petgraph()` for custom algorithms
- **State space search**: BFS (shortest counterexample) and DFS (lower memory), configurable state limit
- **Deadlock detection**: automatic detection with counterexample traces (anchored to CIR SIDs when `cir-anchor` is enabled)
- **DOT export**: Graphviz visualization with styled nodes by place type

## Error codes

| Range | Category |
|-------|----------|
| V0xx | Structural errors (duplicate IDs, missing references, zero weights) |
| V1xx | Well-formedness violations (W2–W7) |
| V2xx | Branch completeness (W8–W9) |
| V3xx | Analysis-phase errors (token underflow, state explosion) |
| V4xx | Resource semantics (initial token mismatches) |

## License

MIT
