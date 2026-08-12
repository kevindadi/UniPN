# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working in this repository.

## Project

UniPN is a Rust edition 2024 library for representing extensible Petri-net models. It is library-only: it provides a declarative IR, typed values and expressions, runtime containers, domain interfaces, semantic capability traits, and domain-neutral state-space exploration. It does not provide a CLI, execution language, or project-specific verification algorithms.

## Common Commands

Run from the repository root:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo test --all-features
cargo test --all-features --test core
cargo test --all-features --test core concrete_domain_matches_and_evaluates
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps
```

Feature flags: `invariants` (default; `num-*` exact nullspace invariants) and `timed` (reserved PTPN bridge). Integration tests live in `tests/`; use `--test <file-stem>` to select a test binary and append a test-name filter for one test.

## Architecture

The public boundary is `src/lib.rs`, which exposes these layers:

- `core`: declarative `NetModel`, place/transition declarations, input/output/read/inhibitor/reset arcs, sorts, typed tokens, values, patterns, terms, guards, actions, and model-reference validation.
- `domain`: `Domain` defines how a value domain evaluates terms, evaluates guards, and matches patterns. `ConcreteDomain` is the baseline implementation; abstract domains can provide alternate value and three-valued interpretations.
- `runtime`: generic `RuntimeState<M, G, T>`, P/T markings, colored typed-token multisets, and runtime errors. Time remains a state type parameter rather than a fixed implementation choice.
- `semantics`: the generic `Semantics` contract, the concrete `PtSemantics` weighted P/T adapter, `ColoredSemantics` (pattern binding + guards + actions over `NetModel`), and separate timed, priority, and partial-order capability traits.
- `analysis`: two families. The module root is the **NetLike-based** engines (BFS/DFS/POR reachability, `find_deadlocks`/`blocked_places`, dead-transition detection, conflict sets, coverability-tree boundness, and feature-gated `invariants`/`timed`). `analysis::generic` is the `Semantics`-trait-based explorer with caller-supplied deadlock filtering.
- `model`/`expr`/`state`/`storage`/`net`/`netlike`/`builder`/`export`: the **CVN net subsystem** (matrix-backed `Net` with guards/updates/capacities, `NetLike` contract, `NetBuilder`, DOT export). This is the shared backend the ConcPlanVerify/ConcBugDect frontends lower into.
- `ids`: stable place, transition, sort, and function identifiers (plus the `u32` `Weight`) shared by the other layers.
- `pt`: the concrete P/T net backend (`PtNet`, weighted arcs, capacity modes) and `bug` (ConcBugDect metadata tags).

`RoleTag` and the `model::PlaceKind`/`TransitionKind` annotations are analysis/frontend metadata only. They do not define firing behavior. Domain predicates (`is_resource`, `is_thread_terminal`, `is_wait_point`) default to `false` on `NetLike`; `Net` overrides them from annotations. The explorers never decide what a deadlock is — they report blocked states and the caller classifies them.

## Scope Boundaries

Keep the crate library-only. Do not add a placeholder semantics implementation that returns fake successful results; unsupported execution behavior should remain an explicit boundary until a concrete semantics layer is designed.

The current core intentionally stops short of: user-defined function execution, MIR lowering, timed DBM/state-class analysis (reserved via the `timed` feature and PTPN bridge), net reduction (loop/sequence/intermediate), and test-case generation. Changes should preserve the separation between declarative model data, value domains, runtime containers, and semantic capabilities.
