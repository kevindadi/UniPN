# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working in this repository.

## Project

UniPN is a Rust library implementing a shared, extensible Petri-net core for multiple frontends and analysis consumers. The crate uses Rust edition 2024 and exposes a public API from `src/lib.rs`.

## Common Commands

Run from the repository root:

```bash
cargo check
cargo test
cargo test --features timed
cargo test --no-default-features
cargo test --test basic
cargo test --test deadlock mutex_deadlock_detected
cargo fmt --all
cargo fmt --all -- --check
```

`invariants` is enabled by default. The `timed` feature is opt-in; `--no-default-features` excludes invariant code and verifies the feature-gated build. The test suite is organized as integration-test binaries under `tests/`, so use `--test <file-stem>` to run one file or append a test-name filter to run one test.

At the time this file was created, all three documented `cargo test` variants passed. `cargo fmt --all -- --check` reported pre-existing formatting differences in source and test files; use `cargo fmt --all` when formatting changes are intended.

## Architecture

The core boundary is the object-safe `NetLike` trait in `src/netlike.rs`. Shared algorithms accept `&dyn NetLike`, so a custom frontend can provide places, transitions, weighted pre/post arcs, and an initial state while inheriting pure P/T implementations of transition enabling and firing. Frontends with guards, variable updates, capacities, or other semantics override those runtime methods and can supply semantic predicates such as `is_thread_terminal`, `is_wait_point`, and `is_resource`.

`NetBuilder` in `src/builder.rs` constructs the default `Net` implementation in `src/net.rs`. `Net` stores transition incidence in `storage::Incidence`, a sparse column-oriented representation of the pre/post matrices. Its firing path operates on sparse arcs and optionally evaluates input guards, output variable updates, capacities, and bounded integer domains. `State` combines a dense token marking with an optional ordered variable store; state equality and hashing determine reachability-state identity.

Model kinds in `src/model.rs` are annotations used by default predicates, diagnostics, and DOT styling. They do not themselves change firing semantics; arc structure and any `NetLike` overrides do. `src/expr.rs` contains the optional data model for values, expressions, three-valued guards, and variable updates.

`src/analysis/` contains consumers of the `NetLike` contract:

- `explore.rs` builds a standalone reachability graph using BFS, DFS, or sleep-set partial-order reduction, with a configurable state limit and deadlock counterexamples.
- `deadlock.rs`, `dead_transition.rs`, and `conflict.rs` derive behavioral diagnostics from a net or reachability graph.
- `invariants.rs` is compiled only with the default `invariants` feature and computes exact place/transition invariants from the dense effect matrix.
- `timed.rs` and the root `timed.rs` are feature-gated reservation types and APIs for future timed/state-class analysis; the timed explorer is not implemented yet.

`src/export.rs` exports any `NetLike` implementation as Graphviz DOT. `src/testgen.rs` consumes reachability graphs to extract schedules; its broader criteria-based test generation API is currently reserved and returns no generated cases. Integration tests in `tests/` exercise firing, exploration, deadlocks, dead transitions, conflicts, POR, DOT export, invariants, and custom `NetLike` implementations. `tests/common/mod.rs` contains reusable fixture nets.

## Feature Flags

- `invariants` (default): enables exact invariant computation and its optional numeric dependencies.
- `timed` (off by default): enables timing/priority model fields and timed analysis APIs.

When changing feature-gated code, run the default, `--features timed`, and `--no-default-features` test commands.
