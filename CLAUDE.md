# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working in this repository.

## Project

UniPN is a Rust edition 2024 library providing a **single generic Petri-net model** shared by three frontends. It is library-only: no CLI, no execution language, no project-specific verification algorithms.

The one model is [`net::Net`](src/net.rs), a container of places/transitions/arcs parameterized by their *kind* payloads. The three frontends instantiate it via type aliases:

- [`pt::PtNet`](src/pt.rs) — ordinary P/T net (ConcBugDect's MIR→PN lowering);
- [`timed::TimedNet`](src/timed.rs) — priority timed net (PTPN);
- [`cvn::CvnNet`](src/cvn.rs) — colored verification net with guards/updates (ConcPlanVerify).

Each net differs only in its place/transition/arc *kind* and its own firing semantics; the structure, ids, weights, and marking are shared.

## Common Commands

Run from the repository root:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo test --test net_model
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
```

No feature flags. Integration tests live in `tests/`; use `--test <file-stem>` to select a test binary and append a test-name filter for one test.

## Architecture

- `net`: the single generic model — `Place<K>`, `Transition<K>`, `Arc<K>` (with `ArcDir` and `usize` weight), `Net<PK, TK, AK>`, `Marking` (`Vec<usize>`, index = place id), and `State<E>` (marking + per-net `extra`). Pure structure; no firing semantics and no marking live inside `Net`.
- `pt`: ConcBugDect's `PtPlaceKind`/`PtTransitionKind` (place/transition metadata) + `PtNet` alias and its P/T firing (`NetLike` impl with read/inhibitor/reset arcs and capacity modes).
- `timed`: PTPN's `TimeInterval`/`TimedPlaceKind`/`TimedTransitionKind` + `TimedNet` alias. Timed (DBM/state-class) analysis is reserved and lives in PTPN itself.
- `cvn`: ConcPlanVerify's `CvnNet` + `CvnBuilder` + guard/update firing; `model` (place/transition kinds) and `expr` (values/expressions/guards) are its data model.
- `analysis`: the [`NetLike`](src/analysis/mod.rs) firing contract plus `explore` (BFS/DFS reachability) and `find_deadlocks` (caller-supplied deadlock predicate). The explorer reports *blocked* states; it never decides what a deadlock is.
- `ids`: `PlaceId`/`TransitionId` (contiguous `usize`).

Every count — weights, token counts, ids — is `usize`.

## Scope Boundaries

Keep the crate library-only. Do not add a placeholder semantics implementation that returns fake successful results; unsupported execution behavior should remain an explicit boundary until a concrete semantics layer is designed.

Intentionally out of scope for the shared model: user-defined function execution, MIR lowering, timed DBM/state-class analysis, net reduction (loop/sequence/intermediate), and test-case generation. These belong to the individual frontends (ConcBugDect, PTPN, ConcPlanVerify), which consume `Net`/`NetLike` rather than extending it.
