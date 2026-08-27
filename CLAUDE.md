# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working in this repository.

## Project

UniPN is a Rust edition 2024 library providing a **single generic Petri-net model** shared by three frontends. It is library-only: no CLI, no execution language, no project-specific verification algorithms.

The one model is [`net::Net`](src/net/mod.rs), a container of places/transitions/arcs parameterized by their *kind* payloads. The three frontends instantiate it via type aliases:

- [`pt::PtNet`](src/pt/kinds.rs) — ordinary P/T net (ConcBugDect's MIR→PN lowering);
- [`timed::TimedNet`](src/timed/kinds.rs) — priority timed net (PTPN);
- [`cvn::CvnNet`](src/cvn/kinds.rs) — colored verification net with guards/updates (ConcPlanVerify).

Each net differs only in its place/transition/arc *kind* and its own firing semantics; the structure, ids, weights, marking, and the structural firing rules are shared.

## Common Commands

Run from the repository root:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo check --all-targets
cargo test
cargo test --no-default-features
cargo test --test net_model
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
```

Default feature: `timed` (DBM / state-class reachability in `analysis::timed`). The timed *model* (`TimedNet`, discrete `NetLike` over `TimedState`) is always compiled. Integration tests live in `tests/`; `timed_analysis` requires the feature. Use `--test <file-stem>` to select a test binary and append a test-name filter for one test.

## Architecture

The tree has three layers: the generic core (`net/`), one directory per frontend (`pt/`, `timed/`, `cvn/`), and the analyses (`analysis/`). Inside a frontend directory the split is always the same — `kinds.rs` for the `PK`/`TK`/`AK` payloads and the net alias, `semantics.rs` for that net's firing, `roles.rs` for its answers to the shared `PlaceRole`/`TransitionRole` questions, and then whatever else that frontend needs (`builder.rs`, `expr.rs`, `interval.rs`, `dot.rs`). Every directory's `mod.rs` holds only the module docs, the submodule declarations, and a flat re-export, so paths like `crate::pt::PtNet` stay stable when files move.

- `net`: the single generic model — `Place<K>`, `Transition<K>`, `Arc<K>` (with `ArcDir` and `usize` weight), `Net<PK, TK, AK>`, `Marking` (`Vec<usize>`, index = place id), and `State<E>` (marking + per-net `extra`). No marking lives inside `Net`. Its submodules are `net::ids` (`PlaceId`/`TransitionId`, contiguous `usize`), `net::incidence` (the `Incidence` adjacency snapshot and the ordinary `IncidenceMatrix`), `net::firing` (the structural firing primitives, below), `net::roles` (`PlaceRole`/`TransitionRole`, the questions an analysis asks a kind, below), and `net::builder` (`NetBuilder`, below); all are re-exported from `net`, so `use crate::net::{Marking, PlaceId}` works.
- `pt`: ConcBugDect's `PtPlaceKind`/`PtTransitionKind` (place/transition metadata) + `PtNet` alias and its P/T firing (structural enabling, outputs clamped to capacity, reset arcs), plus `PtBuilder`, its `roles.rs` answers, and DOT/connectivity diagnostics.
- `timed`: PTPN's `TimeInterval`/`TimedPlaceKind`/`TimedTransitionKind` + `TimedNet` alias and discrete firing over `TimedState` (`State<TimedExtra>`, marking only). `TimedExtra` is empty *on purpose* — it holds the `State<E>` shape open for a future timed-state field; a clock zone is not that field, since a zone is a set of valuations. Overflow is a **return value**: `fire_reporting_overflow` reports the non-saturating places it had to clamp, and `StateClassReachabilityGraph::build` accumulates them into `stats.overflowed_places`. Do not reintroduce a global (this used to be a `thread_local!` that callers had to reset by hand). The timed **analysis** (DBM/state-class reachability) lives in `analysis::timed` behind the `timed` feature — clock zones stay on `StateClass`, not on `NetLike::State`.
- `cvn`: ConcPlanVerify's `CvnNet` + `CvnBuilder` + guard/update firing. `cvn::kinds` carries both the place/transition classification (`PlaceKind`, `ControlSub`, `ResourceType`, `TransitionKind`) and the CVN-specific `CvnArcKind`/`CvnTransition`/`CvnExtra`; `cvn::expr` is its value/expression/guard language. Both were previously the top-level `model` and `expr` modules. Tokens stay plain `usize` — the CVN's data lives in the state's variable store, not in colored tokens, which is what keeps its state space tractable. The store is flat (`BTreeMap<String, Val>`): variable *scoping* is the frontend's business, expressed in the names it generates. What UniPN does provide is `CvnArcKind::DropVars`, because a local left in the store after its scope ends keeps splitting states that are otherwise equal — that is a state-space concern, not a naming one.
- `analysis`: the [`Semantics`](src/analysis/mod.rs) and [`NetLike`](src/analysis/mod.rs) firing contracts plus the checks that need no kind at all — `explore` (BFS/DFS reachability), `find_deadlocks` (caller-supplied deadlock predicate), `conflict_sets` (transitions sharing an input place), `unfired_transitions` (never fired on any edge) — and one submodule per frontend. `analysis::pt` is ConcBugDect's P/T reachability (`StateGraph`) and boundness (coverability tree); `analysis::cvn` wraps the generic checks in the CVN's ConcIR anchors and disjunctive families; `analysis::timed` is the PTPN state-class (DBM) reachability. The explorers report *blocked* states; a blocked state becomes a deadlock only through the `PlaceRole` predicate or a caller's own.

Every count — weights, token counts, ids — is `usize`.

### The semantics layer

A frontend implements `Semantics` (`can_fire` + `fire_enabled`), never `NetLike` — a blanket impl derives `NetLike` for every `Net<PK, TK, AK>` whose `Self: Semantics`, so the place/transition counts and the enabled-set scan exist once instead of three times.

Everything that follows from the arc structure alone lives in `net::firing` as inherent methods on `Net`, and each rule has exactly one definition:

- `structurally_enabled` — parallel input-arc weights summed per place, read arcs satisfied, inhibitor arcs clear. This is the *complete* enabling condition for `PtNet` and `TimedNet`; `CvnNet` calls it and then adds its guards and variable domains.
- `consume_inputs` / `apply_resets` — token consumption (saturating; establish enabling first) and reset-arc clearing.
- `produce_outputs_clamped` vs `produce_outputs_bounded` — the one genuine semantic fork. Choosing a method *is* the capacity policy: clamp and report which places were clamped (`PtNet` ignores the report, `TimedNet` turns it into the overflow metric), or reject the firing outright (`CvnNet` resource places).
- `PlaceCapacity` — a trait on the place *kind*, because the three frontends express capacity differently (a field, a field plus a `saturate` flag, or a value derived from `ResourceType`). `Net::capacity_of` is available whenever `PK: PlaceCapacity`.
- `accumulate` in `net/mod.rs` is the single definition of how parallel arcs of one direction combine; both `net::firing` and the `Incidence` snapshot use it.

Add a shared rule here only when it follows from the structure. A rule that encodes one tool's *choice* (guards, clock zones, priorities, overflow reporting) belongs in that frontend's `semantics.rs`; do not push it down behind a hook.

### The roles layer

`net::roles` holds the questions an analysis asks a *kind*, in the same shape as `PlaceCapacity`: the question is shared, the answer is the frontend's.

- `PlaceRole` (`is_resource` / `is_terminal`) is what makes the deadlock definition shared. `Net::blocked_places` and `Net::is_deadlock` are available whenever `PK: PlaceRole`, and they encode the one rule both tools need: being blocked is *not* being deadlocked, because a run where every thread reached its end and returned every lock also has nothing left to fire. P/T used to count those runs as deadlocks — `PlaceType` had `Resources` and `FunctionEnd` all along, they were simply never asked.
- `TransitionRole` (`is_acquire`, `is_release`, `is_thread_spawn`, `is_thread_join`, `is_atomic`, `is_unsafe_access`) is a shared *vocabulary*, deliberately not a shared enum. `PtTransitionKind` and `CvnTransition` overlap on a dozen concepts but not on shape: P/T's variants carry what its pointer analysis inferred (`Lock(alias)`, `AtomicLoad(alias, ordering, …)`) while the CVN's are bare tags, because in ConcIR the resource identity already is the place identity. Merging them would mean a nested `Shared(..) | Frontend(..)` enum and two-level matches in code the two tools do not share.

`TimedPlaceKind` and `TimedTransitionKind` implement neither: PTPN's places carry no control/resource split and it classifies schedulability, not deadlocks. A bound is only paid where it is used — do not add a vacuous impl to make a signature uniform.

### P/T and CVN are two precision tiers of one model

Both lower a control-flow skeleton over shared resources; the CVN additionally keeps guards and variable updates, so `CvnArcKind::Plain` *is* a P/T arc and P/T is the CVN with the data projected away. ConcBugDect comes from Rust MIR (dropping numeric dataflow, keeping resource use) and can already emit ConcIR via `--viz-cir`; the CVN comes from LLM-generated ConcIR and is checked before code generation proceeds.

This is why they share the structural rules and the `roles` vocabulary but **not the state representation**. P/T's state is a bare `Marking`; making it a `State<CvnExtra>` would put a `BTreeMap` into every hash and comparison, and ConcBugDect explores whole crates (its state limit, POR, and net reduction all exist to contain that) where the CVN explores generated fragments. Do not collapse the two frontends into one.

### Construction

There is one place/transition *representation*: `Place<K>` and `Transition<K>`. Do not introduce a second, construction-time struct that repeats `name` plus the kind's fields — that was what `PtPlace`/`PtTransition` used to be, and it made the generic `Place<K>` pointless. Extra attributes go in the kind `K`; initial token counts go in the `Marking`.

`NetBuilder<PK, TK, AK, E>` owns the one invariant construction has to maintain: the marking vector staying index-aligned with the places. `E` mirrors `State`'s `extra`, so a frontend accumulates whatever else its initial state needs. Both frontend builders are **type aliases**, not wrappers:

- `PtBuilder = NetBuilder<PtPlaceKind, PtTransitionKind, ()>` adds only ConcBugDect's arc handling (`add_*_arc` accumulates onto a parallel arc, `set_*_weight` overwrites) and its DOT/diagnostic forwarding;
- `CvnBuilder = NetBuilder<PlaceKind, CvnTransition, CvnArcKind, CvnExtra>` adds guard/update arcs and the variable declarations, which write straight into `extra`;
- `TimedBuilder = NetBuilder<TimedPlaceKind, TimedTransitionKind, (), TimedExtra>` adds nothing but its `build()` — timed arcs carry no payload, so the generic `add_arc` is the whole API.

Each alias defines its own `build()` over the generic `into_parts()` / `into_net_and_state()`, so `(PtNet, Marking)`, `(CvnNet, CvnState)`, and `(TimedNet, TimedState)` stay the promised shapes. A new frontend should be an alias too.

## Scope Boundaries

Keep the crate library-only. Do not add a placeholder semantics implementation that returns fake successful results; unsupported execution behavior should remain an explicit boundary until a concrete semantics layer is designed.

Intentionally out of scope for the shared model: user-defined function execution, MIR lowering, net reduction (loop/sequence/intermediate), and test-case generation. These belong to the individual frontends (ConcBugDect, PTPN, ConcPlanVerify), which consume `Net`/`NetLike` rather than extending it. The timed **model** and its DBM/state-class **analysis** live in UniPN; PTPN keeps only its TDG lowering, `.ptpn` parser, CLI, Romeo/PToPNer export, and scheduling metrics.
