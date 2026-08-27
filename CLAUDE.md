# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working in this repository.

## Project

UniPN is a Rust edition 2024 **workspace** of two crates:

- `unipn` (repository root) — a **single generic Petri-net model** shared by three frontends, plus the analyses. Library-only: no CLI, no execution language, no project-specific verification algorithms. Depends on nothing but `serde`.
- `unipn-transfer` (`transfer/`) — converters from an external format into one of those nets. Today: ConcIR JSON → CVN. It is a separate crate so `unipn` never carries `serde_json` or somebody else's schema, and it is in the same workspace so a kind change and the converter that depends on it compile together.

`unipn` stays the *root* package (`[package]` plus `[workspace]` in the same `Cargo.toml`) so a downstream `path = "../UniPN"` keeps resolving. `third_party/ConcIR` is a git submodule: the schema the converter reads, and the example corpus its tests run against.

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
cargo check --workspace --all-targets
cargo test --workspace
cargo test --no-default-features
cargo test --test net_model
cargo test -p unipn-transfer
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --no-deps
git submodule update --init
```

`--workspace` matters: a bare `cargo test` from the root only builds `unipn`, because the root package is also a workspace member.

Default feature: `timed` (DBM / state-class reachability in `analysis::timed`). The timed *model* (`TimedNet`, discrete `NetLike` over `TimedState`) is always compiled. Integration tests live in `tests/`; `timed_analysis` requires the feature. Use `--test <file-stem>` to select a test binary and append a test-name filter for one test. `unipn-transfer`'s tests read fixtures out of the `third_party/ConcIR` submodule and fail with a message telling you to initialize it.

## Architecture

`unipn`'s tree has three layers: the generic core (`net/`), one directory per frontend (`pt/`, `timed/`, `cvn/`), and the analyses (`analysis/`). `unipn-transfer` sits outside all three and depends on the public API only. Inside a frontend directory the split is always the same — `kinds.rs` for the `PK`/`TK`/`AK` payloads and the net alias, `semantics.rs` for that net's firing, `roles.rs` for its answers to the shared `PlaceRole`/`TransitionRole` questions, and then whatever else that frontend needs (`builder.rs`, `expr.rs`, `interval.rs`, `dot.rs`). Every directory's `mod.rs` holds only the module docs, the submodule declarations, and a flat re-export, so paths like `crate::pt::PtNet` stay stable when files move.

- `net`: the single generic model — `Place<K>`, `Transition<K>`, `Arc<K>` (with `ArcDir` and `usize` weight), `Net<PK, TK, AK>`, `Marking` (`Vec<usize>`, index = place id), and `State<E>` (marking + per-net `extra`). No marking lives inside `Net`. Its submodules are `net::ids` (`PlaceId`/`TransitionId`, contiguous `usize`), `net::incidence` (the `Incidence` adjacency snapshot and the ordinary `IncidenceMatrix`), `net::firing` (the structural firing primitives, below), `net::roles` (`PlaceRole`/`TransitionRole`, the questions an analysis asks a kind, below), `net::places` (`PlaceClass<R>`/`ControlSub`, the place classification P/T and the CVN both instantiate, below), and `net::builder` (`NetBuilder`, below); all are re-exported from `net`, so `use crate::net::{Marking, PlaceId}` works.
- `pt`: ConcBugDect's `PtPlaceKind`/`PtTransitionKind` (place/transition metadata; `PtPlaceKind::place_type` is `PlaceClass<()>`) + `PtNet` alias and its P/T firing (structural enabling, outputs clamped to capacity, reset arcs), plus `PtBuilder`, its `roles.rs` answers, and DOT/connectivity diagnostics.
- `timed`: PTPN's `TimeInterval`/`TimedPlaceKind`/`TimedTransitionKind` + `TimedNet` alias and discrete firing over `TimedState` (`State<TimedExtra>`, marking only). `TimedExtra` is empty *on purpose* — it holds the `State<E>` shape open for a future timed-state field; a clock zone is not that field, since a zone is a set of valuations. Overflow is a **return value**: `fire_reporting_overflow` reports the non-saturating places it had to clamp, and `StateClassReachabilityGraph::build` accumulates them into `stats.overflowed_places`. Do not reintroduce a global (this used to be a `thread_local!` that callers had to reset by hand). The timed **analysis** (DBM/state-class reachability) lives in `analysis::timed` behind the `timed` feature — clock zones stay on `StateClass`, not on `NetLike::State`.
- `cvn`: ConcPlanVerify's `CvnNet` + `CvnBuilder` + guard/update firing. `cvn::kinds` carries the transition classification (`TransitionKind`), the resource types (`ResourceType`), its `PlaceKind = PlaceClass<ResourceType>` alias, and the CVN-specific `CvnArcKind`/`CvnTransition`/`CvnExtra`; `cvn::expr` is its value/expression/guard language. Both were previously the top-level `model` and `expr` modules. Tokens stay plain `usize` — the CVN's data lives in the state's variable store, not in colored tokens, which is what keeps its state space tractable. The store is flat (`BTreeMap<String, Val>`): variable *scoping* is the frontend's business, expressed in the names it generates. What UniPN does provide is `CvnArcKind::DropVars`, because a local left in the store after its scope ends keeps splitting states that are otherwise equal — that is a state-space concern, not a naming one.
- `transfer`: `unipn_transfer::concir` — `ast.rs` (the ConcIR wire format), `expr.rs` (its expression strings), `lower.rs` (the walk). See "The conversion layer" below.
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

`net::roles` holds the questions an analysis asks a *kind*, in the same shape as `PlaceCapacity`: the question is shared, the answer is the frontend's. The place side went one step further — see `net::places` below, where the *kind* is shared too and the answer is given once.

- `PlaceRole` (`is_resource` / `is_terminal`) is what makes the deadlock definition shared. `Net::blocked_places` and `Net::is_deadlock` are available whenever `PK: PlaceRole`, and they encode the one rule both tools need: being blocked is *not* being deadlocked, because a run where every thread reached its end and returned every lock also has nothing left to fire. P/T used to count those runs as deadlocks — `PlaceType` had `Resources` and `FunctionEnd` all along, they were simply never asked.
- `Net::is_sink` is the structural half of the same question and needs no kind: no `Input` and no `Reset` arc leaves the place, so a token there can never move. `Net::is_terminal` accepts it in addition to the annotated answer, which covers an exit the lowering never labelled — a detached thread's last place, or a MIR block P/T did not mark `FunctionEnd`. The cost is real and deliberate: a control place that simply *forgot* its outgoing arc now reads as an ending rather than as a modeling bug. Do not lean on it as the normal path.
- `TransitionRole` (`is_acquire`, `is_release`, `is_thread_spawn`, `is_thread_join`, `is_atomic`, `is_unsafe_access`, `is_blocking_wait`) is a shared *vocabulary*, deliberately not a shared enum. `PtTransitionKind` and `CvnTransition` overlap on a dozen concepts but not on shape: P/T's variants carry what its pointer analysis inferred (`Lock(alias)`, `AtomicLoad(alias, ordering, …)`) while the CVN's are bare tags, because in ConcIR the resource identity already is the place identity. Merging them would mean a nested `Shared(..) | Frontend(..)` enum and two-level matches in code the two tools do not share.

- `Net::is_wait_point` (needs `PK: PlaceRole` *and* `TK: TransitionRole`) is what turns one blocked place into a specific diagnosis: a non-resource place every one of whose exits `is_blocking_wait` holds a token that is waiting for an event, not for a resource. A lost notification and a lock-order deadlock are both "a token that cannot move", and this is where they become distinguishable. `is_blocking_wait` is the *event* counterpart of `is_acquire`, not a superset — a semaphore permit blocks, but somebody is holding it, so `Acquire` answers `false`. That also makes it narrower than ConcIR's own `Op::is_blocking`, which only asks whether an operation can block.

`is_wait_point` is derived from the arcs rather than annotated on the place, and that was a deliberate reversal: `ControlSub` used to have `WaitPoint` and `CallWait`. Both frontends already keep *what operation this is* on the transition (`TransitionType::Wait` / `Function`, `CondvarWaitEnter` / `Call`) with source attribution in a field (`PtPlaceKind::span`, `CvnTransition::anchors`), so a place kind saying "wait point" was that fact copied onto the place, free to drift out of step with the arcs. Asking the *way out* is also what makes one definition serve both lowerings: the CVN parks a token on a `:waiting` place between `CondvarWaitEnter` and `CondvarWakeByNotify`, while P/T's single atomic `Wait` transition has the token resting in front of it. The cost is that a wait construct assembled in some unforeseen shape reads as an ordinary blocked place — the diagnosis degrades, it does not turn wrong.

`TimedPlaceKind` and `TimedTransitionKind` implement neither: PTPN's places carry no control/resource split and it classifies schedulability, not deadlocks. A bound is only paid where it is used — do not add a vacuous impl to make a signature uniform.

### P/T and CVN are two precision tiers of one model

Both lower a control-flow skeleton over shared resources; the CVN additionally keeps guards and variable updates, so `CvnArcKind::Plain` *is* a P/T arc and P/T is the CVN with the data projected away. ConcBugDect comes from Rust MIR (dropping numeric dataflow, keeping resource use) and can already emit ConcIR via `--viz-cir`; the CVN comes from LLM-generated ConcIR and is checked before code generation proceeds.

This is why they share the structural rules and the `roles` vocabulary but **not the state representation**. P/T's state is a bare `Marking`; making it a `State<CvnExtra>` would put a `BTreeMap` into every hash and comparison, and ConcBugDect explores whole crates (its state limit, POR, and net reduction all exist to contain that) where the CVN explores generated fragments. Do not collapse the two frontends into one.

What they *do* share, as of the place merge, is the place classification itself. `net::places` holds it:

```rust
pub enum ControlSub { BasicBlock, FunctionStart, FunctionEnd }
pub enum PlaceClass<R> { Control(ControlSub), Resource(R) }
```

`pt::PlaceType` is `PlaceClass<()>` and `cvn::PlaceKind` is `PlaceClass<ResourceType>`; both are type aliases, like the net aliases themselves. The two had converged on the same three control roles independently, and keeping that as two enums made the agreement a claim in this file that nothing checked — now it is the type. `PlaceRole` is implemented once, on `PlaceClass<R>`; `PtPlaceKind` only forwards past its span and capacity fields.

`R` is the one arm that genuinely differs, and it stays the frontend's: the CVN's resource *type* decides the place's capacity (`PlaceCapacity for PlaceClass<ResourceType>`) because in ConcIR the resource identity already is the place identity, while P/T keeps identity on the transition (`Lock(alias)`) and the bound in `PtPlaceKind::capacity`, so its arm is empty. Write `PtPlaceKind::control(sub)` / `PtPlaceKind::resource()` rather than spelling `Resource(())` at call sites.

Note this is a shared **enum**, where `TransitionRole` is deliberately a set of predicates. That is not inconsistent: the argument against merging transitions is that the two frontends' variants disagree on shape (`Lock(alias)` against a bare tag), and the control variants carry no payload in either frontend and mean the same thing in both, so it does not reach them. `R` keeps the part that does differ separate.

Serde compatibility runs one way: `PtPlaceKind` still *reads* the flat `"BasicBlock"` / `"Resources"` strings ConcBugDect wrote before the merge (`place_type_accepting_legacy`), and writes the nested shape. The CVN's wire format is unchanged, since it was already nested. Ordering did change — `PlaceClass<()>` still derives `Ord`, but `Resource` now sorts after every `Control`, where `Resources` used to sort first.

Each `ControlSub` variant is a distinction some analysis makes, and that is the whole list:

- No `Statement` / `BasicBlock` split. ConcIR's flat statement list gives one control point per statement, MIR one per basic block; that difference *is* the precision tier, not a difference in what the place is. `BasicBlock` covers both.
- No `ThreadEnd`. In ConcIR a thread **is** a function — `spawn`, `scope`, and `async_call` all target an ordinary `Function`, and the same function can be both called and spawned — so being a thread's terminal is a property of the call site, which a static place kind cannot express. `FunctionEnd` absorbed it and `Return` (a return *is* where control comes to rest); it is now literally the same variant P/T uses.
- No `Reacquire` / `SpawnBridge` / `TestPoint`. A lowering still creates those intermediate places; they are `BasicBlock` with a telling name. A kind variant is for a question an analysis asks, and nothing asks "is this the reacquire place".
- No `WaitPoint` / `CallWait`. Both named the *operation* a token waits on, which both frontends keep on the transition; the question they were there to answer is `Net::is_wait_point`, derived from the way out. See the roles layer above.

Adding a variant back means naming the analysis that would branch on it.

### Construction

There is one place/transition *representation*: `Place<K>` and `Transition<K>`. Do not introduce a second, construction-time struct that repeats `name` plus the kind's fields — that was what `PtPlace`/`PtTransition` used to be, and it made the generic `Place<K>` pointless. Extra attributes go in the kind `K`; initial token counts go in the `Marking`.

`NetBuilder<PK, TK, AK, E>` owns the one invariant construction has to maintain: the marking vector staying index-aligned with the places. `E` mirrors `State`'s `extra`, so a frontend accumulates whatever else its initial state needs. Both frontend builders are **type aliases**, not wrappers:

- `PtBuilder = NetBuilder<PtPlaceKind, PtTransitionKind, ()>` adds only ConcBugDect's arc handling (`add_*_arc` accumulates onto a parallel arc, `set_*_weight` overwrites) and its DOT/diagnostic forwarding;
- `CvnBuilder = NetBuilder<PlaceKind, CvnTransition, CvnArcKind, CvnExtra>` adds guard/update arcs and the variable declarations, which write straight into `extra`;
- `TimedBuilder = NetBuilder<TimedPlaceKind, TimedTransitionKind, (), TimedExtra>` adds nothing but its `build()` — timed arcs carry no payload, so the generic `add_arc` is the whole API.

Each alias defines its own `build()` over the generic `into_parts()` / `into_net_and_state()`, so `(PtNet, Marking)`, `(CvnNet, CvnState)`, and `(TimedNet, TimedState)` stay the promised shapes. A new frontend should be an alias too.

### The conversion layer (`unipn-transfer`)

`unipn_transfer::cvn_from_concir_json` lowers a ConcIR program to a `CvnNet`. Shape: one `Control(BasicBlock)` per ConcIR statement holding the token *before* it runs, a `FunctionStart` / `FunctionEnd` pair per function, one transition per statement (several when one operation is several events — a `branch` is two, a `condvar_wait` is three). `kind: "sync"` resources become resource places, `kind: "var"` resources become entries in `CvnExtra::vars` with the `{"Int": [lo, hi]}` domain when it is declared.

Three rules hold the design together, and none of them is negotiable:

1. **Over-approximate, never narrow.** An expression the parser cannot read becomes `Val::Unknown`; a condition becomes an `Unknown`-valued comparison. The CVN's three-valued guard treats `Unknown` as satisfied, so both arms of a branch stay open and the net admits at least the program's runs. A degraded guard must **not** be `BoolExpr::True` — the else-arm carries `Not(guard)`, and `Not(True)` is `False`, which would silently delete a path.
2. **Report every loss.** `LoweringReport` carries the degraded expressions, the `RwLock`s that took `LoweringConfig::default_max_readers`, and the unmodeled declarations. An invisible over-approximation is worse than none: "no deadlock" over a net whose guards all degraded proves nothing.
3. **Never skip an operation.** A recognized-but-unlowered op is `TransferError::UnsupportedOp`, naming the sid and the kind. Dropping a `join` or a `channel_recv` deletes exactly the blocking behavior the analysis exists to find. An op *kind* the AST has never seen is a serde error instead, because a new operation changes what the program means.

The ConcIR AST is re-declared in `concir/ast.rs` rather than imported. The `concir` crate is small (five modules, three deps), so size is not the reason — these three are:

1. **ConcIR marks its structs `deny_unknown_fields`; we must not.** Every struct except `Stmt` has it, including the flattened `Op`. A field added upstream — a source line on a statement, say — would turn every conversion into a hard error while our submodule pin lags behind. Being strict is right for a validator and wrong for a reader.
2. **A `path` dependency on a submodule makes this crate unpublishable** and forces the submodule on everyone who uses it. Today only the *tests* need `third_party/ConcIR`; `cargo build -p unipn-transfer` does not.
3. **`concir::ast::Program` in the public API would pin the caller's ConcIR revision to ours.** ConcPlanVerify already depends on `concir`; two revisions in one binary are two incompatible `Program` types. `cvn_from_concir_json(&str)` shares no type at all, which is the point.

The cost is drift, and `transfer/tests/schema_sync.rs` is the alarm: `concir` is a **dev-dependency**, both ASTs parse ConcIR's example corpus, and every field the lowering branches on has to agree. It already earned its keep by catching `count`/`capacity` being `i64` upstream and `usize` here. Do not promote that dev-dependency to a real one.

Mirror the wire types, not UniPN's: `count` and `capacity` are `Option<i64>` because ConcIR accepts a negative and diagnoses it in its own validator (E001). `Resource::permits` / `Resource::slots` do the conversion, reading a negative as zero — fewer permits means more waiting, so the net can only report a stall that is not there, never miss one.

Names carry the scoping, since the store is one flat map: a shared variable is `module::name`, a function slot is `module::function::name`, and parsed expressions are rewritten to match. A `return` drops that function's slots through `CvnArcKind::DropVars` — its first real user.

Known approximations, all documented in `lower.rs`: one net per function rather than per invocation (two live invocations share places); a condvar is a waiter count plus a signal count, so `notify_all` releases one waiter rather than all; a notification is only produced when the waiter count is non-zero (read arc) and lost otherwise (inhibitor arc), so both outcomes are explored.

## Scope Boundaries

Keep both crates library-only. Do not add a placeholder semantics implementation that returns fake successful results; unsupported execution behavior should remain an explicit boundary until a concrete semantics layer is designed. The same rule is why `unipn-transfer` errors on an op it cannot lower.

Intentionally out of scope for the shared model: user-defined function execution, MIR lowering, net reduction (loop/sequence/intermediate), and test-case generation. These belong to the individual frontends (ConcBugDect, PTPN, ConcPlanVerify), which consume `Net`/`NetLike` rather than extending it. The timed **model** and its DBM/state-class **analysis** live in UniPN; PTPN keeps only its TDG lowering, `.ptpn` parser, CLI, Romeo/PToPNer export, and scheduling metrics.

`unipn-transfer` reads formats and produces nets; it does not analyze them and it does not go the other way. A ConcIR **writer**, ConcIR's own validation (its E-codes), and the repair loop that acts on a counterexample all stay in ConcPlanVerify. Nothing in `transfer` may be a dependency of `unipn`.
