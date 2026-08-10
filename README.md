# UniPN — Unified Petri Net

A fast, extensible Petri net core shared by several frontends and analysis consumers:

```text
Frontends (net building)          Core (matrix storage + trait)   Consumers (analysis)
  ConcIR   ─┐                    Net (CSC incidence matrix)      deadlock / dead-transition / conflict
  Rust MIR ┼─▶ NetLike ───────▶  explore (BFS/DFS/POR)           invariants
  test intent┘   (object-safe)   deadlock / dead_transition       test-case generation (testgen)
  time (PTPN) ─▶ Timed reserve   conflict / invariants / dot      timed / real-time scheduling
```

## Design principles

1. **Trait-first**: `NetLike` is the single contract (object-safe). Any net
   (CVN, ConcBugDect MIR→PN, future test/timed nets) only needs to implement it
   to be consumed by the shared algorithms. A **pure P/T net** only fills the
   structural predicates (`pre_arcs` / `post_arcs` / `initial_state`);
   `enabled_transitions` and `fire` use the trait's default implementations.
2. **Matrix-backed**: the core `Net` stores the `Pre/Post` incidence as **CSC
   sparse columns**, so the enabled/fire hot path is O(|arcs|) instead of
   O(|P|·|T|); the dense `C = Post − Pre` matrix is only materialized when
   linear algebra is needed (invariants etc.).
3. **Semantics externalized**: `PlaceKind` / `TransitionKind` are only
   annotations; semantics such as "thread terminal / wait point / resource" are
   exposed through frontend predicates (`is_thread_terminal` / `is_wait_point` /
   `is_resource`), not hardcoded in the common layer. `return` is a function
   return, not thread end; spawn/join/branch are all arc-structure patterns.
4. **Extensible**: `timed` / `invariants` are feature-gated extension slots.

## Quick start

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

### Extending with a custom net

```rust
impl NetLike for MyNet {
    fn num_places(&self) -> usize { /* ... */ }
    fn num_transitions(&self) -> usize { /* ... */ }
    fn pre_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)> { /* ... */ }
    fn post_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)> { /* ... */ }
    fn initial_state(&self) -> State { /* ... */ }
    // enabled_transitions / fire use the default pure P/T implementations;
    // frontends with guards override them as needed.
}
// The shared algorithms now work directly:
let rg = explore(&my_net, &AnalysisConfig::default());
```

## Modules

```
src/
├── ids.rs        PlaceId/TransitionId (index-based), Weight
├── model.rs      PlaceKind / TransitionKind / Place / Transition (annotations)
├── expr.rs       Val / Expr / BoolExpr (optional data model, for guards/updates)
├── state.rs      Marking (dense vector) / VarStore / State
├── storage.rs    CSC sparse-column Incidence + dense effect matrix C = Post − Pre
├── netlike.rs    NetLike trait (object-safe) + pure P/T default implementations
├── net.rs        Net: the matrix-backed net (optional guards/updates/capacities/var domains)
├── builder.rs    NetBuilder
├── analysis/
│   ├── explore.rs          BFS / DFS / POR(sleep-set) → ReachabilityGraph
│   ├── deadlock.rs         deadlock detection + blocked places
│   ├── dead_transition.rs  behavioral dead transitions (OR-family aware)
│   ├── conflict.rs         transition pairs sharing an input place (contention for testgen)
│   ├── invariants.rs       place/transition invariants (feature `invariants`)
│   └── timed.rs            state-class DBM time-analysis reserve (feature `timed`)
├── export.rs       Graphviz DOT
├── testgen.rs      reachability-graph paths → test-case schedules (pure consumer)
└── timed.rs        time-extension types: StaticInterval / Priority / ClockClass
```

## Feature flags

| Feature      | Default | Description |
| ------------ | ------- | ----------- |
| `invariants` | on      | place/transition invariants (Gaussian nullspace, exact BigInt) |
| `timed`      | off     | time/priority extension (`Transition.timing/.priority`, `AnalysisMode::Timed`, PTPN state-class DBM bridge) |

## Timed analysis (PTPN) reserve

The `timed` feature introduces static time intervals `[dmin, dmax]`, fixed
priorities and clock classes. The goal is to bridge
[PTPN](https://github.com/kevindadi/PTPN): the unified net is exported (via an
export bridge) as PTPN's `.ptpn` / TDG JSON, PTPN runs the state-class (DBM)
reachability analysis, and the results come back. On the IR side only optional
annotations are added; the core firing semantics is untouched.

## Tests

```bash
cargo test
cargo test --features timed
cargo test --no-default-features
```

## License

MIT
