# UniPN

UniPN is a Rust library for representing extensible Petri-net models. It is intended to become the shared model layer for:

- ordinary P/T nets from MIR-based concurrency analysis;
- colored nets with typed tokens, arc patterns, guards, and actions;
- timed and priority nets;
- abstract verification nets with three-valued logic.

The crate is library-only. It defines model data structures and extension interfaces; it does not contain a command-line tool, an execution language, or a complete reachability analyzer yet.

## Architecture

```text
frontend lowering
      |
      v
core::NetModel  -----> runtime::State / Marking containers
      |                         |
      +--> core expressions     +--> semantics capabilities
      |          |              |
      v          v              v
    sorts     domains       timed / priority / POR extensions
```

- `core`: declarative net structure, sorts, values, tokens, patterns, terms, guards, actions, and model validation.
- `domain`: value interpretation and pattern matching. `ConcreteDomain` is the baseline domain; other domains can implement `Domain` for abstract values such as three-valued logic or clock constraints.
- `runtime`: generic state, P/T marking, colored marking, and runtime errors. Time is a type parameter rather than a hard-coded field type.
- `semantics`: the `Semantics` contract plus capability traits for timed, priority, and partial-order semantics.
- `ids`: stable index-based place, transition, sort, and function identifiers.

`RoleTag` is metadata for frontends and analyses. It does not define firing semantics. A control-flow location, mutex, condition variable, semaphore, or async task state can be represented by the model's sorts, token values, arcs, and annotations without adding a new core net type.

## Colored-net concepts

A colored transition can be represented with:

- an input arc `Pattern` that consumes and binds token values;
- a transition `GuardExpr` that accepts or rejects a binding;
- an `ActionExpr` for explicit environment updates;
- output arc `Term` expressions that construct new token values.

Read, inhibitor, and reset arcs are part of the declarative model so later semantics can choose the appropriate behavior without changing the IR.

## Current scope

The current release is the foundation for replacing the former P/T-specific implementation. It provides serializable model types, model-reference validation, concrete expression evaluation, typed token containers, and extension traits. CPN firing, MIR lowering, timed DBM analysis, and verification algorithms are intentionally left for subsequent layers.

## Development

```bash
cargo fmt --all
cargo check --all-targets --all-features
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo doc --no-deps
```

## License

MIT
