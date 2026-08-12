# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working in this repository.

## Project

UniPN is a Rust edition 2024 library for representing extensible Petri-net models. It is library-only: it provides a declarative IR, typed values and expressions, runtime containers, domain interfaces, and semantic capability traits. It does not provide a CLI, execution language, reachability analyzer, or MIR lowering implementation yet.

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

The crate currently has no feature flags. Integration tests live in `tests/`; use `--test <file-stem>` to select a test binary and append a test-name filter for one test.

## Architecture

The public boundary is `src/lib.rs`, which exposes five layers:

- `core`: declarative `NetModel`, place/transition declarations, input/output/read/inhibitor/reset arcs, sorts, typed tokens, values, patterns, terms, guards, actions, and model-reference validation.
- `domain`: `Domain` defines how a value domain evaluates terms, evaluates guards, and matches patterns. `ConcreteDomain` is the baseline implementation; abstract domains can provide alternate value and three-valued interpretations.
- `runtime`: generic `RuntimeState<M, G, T>`, P/T markings, colored typed-token multisets, and runtime errors. Time remains a state type parameter rather than a fixed implementation choice.
- `semantics`: the generic `Semantics` contract, the concrete `PtSemantics` weighted P/T adapter, and separate timed, priority, and partial-order capability traits. Concrete firing engines can implement these interfaces without changing the IR.
- `ids`: stable place, transition, sort, and function identifiers shared by the other layers.

`RoleTag` is analysis/frontend metadata only. It does not define firing behavior; control-flow locations and shared resources are represented through places, sorts, token values, arcs, expressions, and future annotations.

## Scope Boundaries

Keep the crate library-only. Do not restore the removed legacy `NetLike`, `NetBuilder`, matrix-storage, analysis, export, test-generation, or timed-reservation APIs. Do not add a placeholder semantics implementation that returns fake successful results; unsupported execution behavior should remain an explicit boundary until a concrete semantics layer is designed.

The current core intentionally stops short of complete colored-net firing, user-defined function execution, MIR lowering, timed DBM analysis, reachability, deadlock detection, and verification algorithms. Changes should preserve the separation between declarative model data, value domains, runtime containers, and semantic capabilities.
