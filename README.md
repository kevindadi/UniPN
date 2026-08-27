# UniPN

UniPN is a Rust library for representing a **single generic Petri-net model** shared by three frontends. It is library-only: no CLI, no execution language, no project-specific verification algorithms.

The workspace has two crates:

| Crate | Path | What it is |
| --- | --- | --- |
| `unipn` | repository root | the model, the firing rules, and the analyses |
| `unipn-transfer` | [`transfer/`](transfer/) | converters from somebody else's file format into one of the nets |

`unipn` stays the root package so a downstream `path = "../UniPN"` keeps resolving, and it depends on nothing but `serde` — the schemas and the JSON parsing live in `unipn-transfer`.

## The model

One generic net, three instantiations:

```rust
// src/net/mod.rs — the generic model
pub struct Place<K = ()>     { id: PlaceId, name: String, kind: K }
pub struct Transition<K = ()> { id: TransitionId, name: String, kind: K }
pub struct Arc<K = ()>       { place, transition, direction: ArcDir, weight: usize, kind: K }
pub struct Net<PK, TK, AK>   { places: Vec<Place<PK>>, transitions: Vec<Transition<TK>>, arcs: Vec<Arc<AK>> }
pub struct Marking(pub Vec<usize>);       // index = place id, value = tokens
pub struct State<E = ()>      { marking: Marking, extra: E }
```

The common structure (id, name, direction, weight) is fixed; the domain-specific part is carried by the kind payloads `PK`/`TK`/`AK`:

| Frontend | Alias | Place kind | Transition kind | Arc kind |
| --- | --- | --- | --- | --- |
| ConcBugDect (MIR→PN) | `pt::PtNet` | `PtPlaceKind` | `PtTransitionKind` | `()` |
| PTPN (priority timed) | `timed::TimedNet` | `TimedPlaceKind` | `TimedTransitionKind` | `()` |
| ConcPlanVerify (CVN) | `cvn::CvnNet` | `cvn::PlaceKind` | `cvn::TransitionKind` | `cvn::CvnArcKind` |

The marking is kept separate from the net; anything a net needs beyond token counts (a CVN variable store, a timed clock zone, …) lives in its own `State` `extra` payload.

There is exactly one place and one transition *representation*, `Place<K>` and `Transition<K>`, for all three nets — a place's extra attributes are the kind `K`, its token count is in the `Marking`, and no frontend defines a second struct of its own. Tokens are plain `usize` everywhere, including the CVN: its data lives in the variable store rather than in colored tokens.

Construction follows the same rule. [`NetBuilder<PK, TK, AK, E>`](src/net/builder.rs) keeps the marking index-aligned with the places, and each frontend's builder is a *type alias* over it that adds only its own methods:

```rust
pub type PtBuilder = NetBuilder<PtPlaceKind, PtTransitionKind, ()>;
pub type CvnBuilder = NetBuilder<PlaceKind, CvnTransition, CvnArcKind, CvnExtra>;
pub type TimedBuilder = NetBuilder<TimedPlaceKind, TimedTransitionKind, (), TimedExtra>;
```

## Source layout

Three layers: the generic core, one directory per frontend, and the analyses.

```text
src/
  net/      mod.rs (the model) + ids.rs + incidence.rs + firing.rs + roles.rs + builder.rs
  pt/       kinds.rs + semantics.rs + roles.rs + builder.rs + dot.rs
  timed/    kinds.rs + semantics.rs + interval.rs + builder.rs
  cvn/      kinds.rs + semantics.rs + roles.rs + builder.rs + expr.rs + dot.rs
  analysis/ mod.rs (Semantics + NetLike + explore) + pt/ + cvn/ + timed/

transfer/src/
  lib.rs            top-level API + LoweringConfig / LoweringReport / TransferError
  concir/           ast.rs (the wire format) + expr.rs (the string parser) + lower.rs

third_party/ConcIR  git submodule: the schema this converter reads, and its examples
```

Within a frontend directory `kinds.rs` holds the payloads and the net alias, `semantics.rs` holds that net's firing, `roles.rs` its answers to the shared role questions, and each `mod.rs` re-exports flatly — so `unipn::pt::PtNet` and the crate-root re-exports (`unipn::PtNet`, `unipn::PlaceId`, …) do not depend on which file a symbol lives in.

## Firing

Whatever follows from the arc structure alone is shared, in [`net::firing`](src/net/firing.rs): `structurally_enabled` (parallel input weights summed per place, read arcs satisfied, inhibitor arcs clear), `consume_inputs`, `apply_resets`, and the capacity lookup behind the `PlaceCapacity` trait.

What a frontend decides for itself is its `Semantics` impl — two methods, `can_fire` and `fire_enabled`; `NetLike` is then derived for it. The capacity policy is the clearest example of a genuine difference: `PtNet` clamps an over-capacity place, `TimedNet` clamps and reports which non-saturating places it clamped (the state-class graph collects these in `stats.overflowed_places`), and `CvnNet` rejects the firing.

```rust
impl Semantics for PtNet {
    type State = Marking;

    fn can_fire(&self, state: &Marking, t: TransitionId) -> bool {
        self.structurally_enabled(state, t)
    }

    fn fire_enabled(&self, state: &Marking, t: TransitionId) -> Option<Marking> {
        let mut next = state.clone();
        self.consume_inputs(&mut next, t);
        self.produce_outputs_clamped(&mut next, t);
        self.apply_resets(&mut next, t);
        Some(next)
    }
}
```

## Roles

Some questions an analysis asks are the same for more than one frontend, while the answers are not. Those live in [`net::roles`](src/net/roles.rs), in the same shape as `PlaceCapacity`:

- `PlaceRole` — is this place a shared resource, and may a thread legitimately end on it? This is what makes the deadlock definition shared: `Net::is_deadlock` says a blocked marking is only a deadlock if some token sits on a control place that is not a thread end, so a run where every thread finished and returned every lock is a normal termination rather than a false positive. Alongside the annotated answer there is a purely structural one, `Net::is_sink` — no input or reset arc leaves the place, so a token there can never move again. It backs up an exit the lowering did not label, which P/T needs because MIR does not always give it one.
- `TransitionRole` — `is_acquire`, `is_release`, `is_thread_spawn`, `is_thread_join`, `is_atomic`, `is_unsafe_access`, `is_blocking_wait`. A shared *vocabulary*, deliberately not a shared enum: P/T's transition variants carry what its pointer analysis inferred (`Lock(alias)`, `AtomicLoad(alias, ordering, …)`) while the CVN's are bare tags, since in ConcIR the resource identity already is the place identity.
- `Net::is_wait_point` — the two roles together turn one blocked place into a diagnosis. A non-resource place whose every exit is a blocking wait holds a token waiting for an *event* somebody must send, as opposed to a resource somebody is holding; a lost notification and a lock-order deadlock are both "a token that cannot move", and this is where they part. It reads the transitions rather than a place kind, because that is where both lowerings already record what the operation is — and it is why the same definition covers the CVN, which parks the token on its own place between `CondvarWaitEnter` and the wake, and P/T, whose single atomic `Wait` leaves the token resting in front of it.

The timed net implements neither — PTPN's places carry no control/resource split and it classifies schedulability, not deadlocks. A bound is only paid where it is used.

On the place side the sharing goes further than the question: the *kind* is shared too. [`net::places`](src/net/places.rs) holds

```rust
pub enum ControlSub { BasicBlock, FunctionStart, FunctionEnd }
pub enum PlaceClass<R> { Control(ControlSub), Resource(R) }
```

and `pt::PlaceType` is `PlaceClass<()>` while `cvn::PlaceKind` is `PlaceClass<ResourceType>` — type aliases, like the net aliases themselves. P/T and the CVN had arrived at the same three control roles independently, so `PlaceRole` is implemented once and the agreement can no longer drift. `R` is the arm that genuinely differs and stays the frontend's: the CVN's resource type decides the place's capacity, because in ConcIR the resource identity already is the place identity, while P/T keeps identity on the transition and its bound in a field. Construct P/T places with `PtPlaceKind::control(sub)` and `PtPlaceKind::resource()`.

This is a shared enum where `TransitionRole` is a set of predicates, and the difference is deliberate: transitions could not merge because the two frontends' variants disagree on shape, and the control variants carry no payload in either and mean the same thing in both. `PtPlaceKind` still deserializes the flat `"BasicBlock"` / `"Resources"` strings written before the merge; it now writes the nested shape.

## Analysis

Analysis is not part of the model. The [`analysis`](src/analysis/mod.rs) module provides the minimal firing contract [`NetLike`] plus the checks that depend on no kind at all: `explore` (BFS/DFS reachability), `find_deadlocks`, `conflict_sets` (transitions competing for an input place), and `unfired_transitions` (behavioral dead code). The explorer only reports *blocked* states; a deadlock verdict comes from `PlaceRole` or from the caller.

Each frontend then has its own analysis submodule: `analysis::pt` (reachability graph, boundness, reduction), `analysis::cvn` (the generic checks wrapped in ConcIR anchors and disjunctive families), and `analysis::timed` (state classes).

Timed (DBM / state-class) analysis is optional:

```toml
unipn = { version = "0.2", features = ["timed"] }          # default
unipn = { version = "0.2", default-features = false }      # model + discrete NetLike only
```

`TimedNet`'s `NetLike` state is `TimedState` (`State<TimedExtra>`): marking only. Clock zones stay in `analysis::timed::StateClass` and are not switched in via the feature — a zone is a set of clock valuations, not a field that discrete `fire` can update.

## Conversion (`unipn-transfer`)

```rust
let lowered = unipn_transfer::cvn_from_concir_json(&json)?;
let graph = explore(&lowered.net, lowered.initial, &AnalysisConfig::default());
```

[ConcIR](https://github.com/kevindadi/ConcIR) is an LLM-generated concurrency IR that ConcPlanVerify checks before code generation continues; `unipn-transfer` lowers a ConcIR program to a `CvnNet`. The mapping is direct because the two models line up: one control place per ConcIR statement, a `FunctionStart` and a `FunctionEnd` per function, a resource place per `sync` resource, and a store entry per `var` resource.

Three properties are worth stating because they are what make the result trustworthy:

- **Precision is lost in one direction only.** A condition the expression parser cannot read becomes an `Unknown` guard, which the CVN's three-valued evaluation treats as satisfied, so *both* arms of a branch stay reachable. The net admits at least the runs the program has. This is also why a degraded guard must not be `BoolExpr::True`: the else-arm carries `Not(guard)`, and `Not(True)` would delete it.
- **Every degradation is reported.** `LoweringReport` lists each expression that became `Unknown`, each `RwLock` that took the default reader count, and each declaration left out of the store. An over-approximation nobody can see is indistinguishable from a bug — "no deadlock" over a net whose guards all degraded proves nothing.
- **An unlowered operation is an error.** `TransferError::UnsupportedOp` names the statement and the kind. Silently skipping a `join` or a `channel_recv` would delete exactly the blocking behavior the analysis exists to find.

The ConcIR schema and its examples come in as a submodule, so a change upstream shows up as a failing test rather than as drift:

```bash
git submodule update --init
```

`concir/ast.rs` re-declares ConcIR's structs instead of importing them, for three reasons that are all about coupling rather than convenience: ConcIR marks its structs `deny_unknown_fields` and a reader must not, or a field added upstream becomes a hard error; a `path` dependency on a submodule makes this crate unpublishable and forces the submodule on every consumer; and `concir::ast::Program` in the public API would pin the *caller's* ConcIR revision to ours, since two revisions in one binary are two incompatible types. `cvn_from_concir_json(&str)` shares no type at all.

The cost of a mirror is drift, so `concir` stays on as a **dev-dependency** and [`transfer/tests/schema_sync.rs`](transfer/tests/schema_sync.rs) parses ConcIR's whole example corpus with both ASTs, comparing every field the lowering branches on.

Currently lowered: `nop`, `assign_local`, `read_shared`, `write_shared`, `abstract_step`, `mutex_lock`, `mutex_unlock`, `condvar_wait`, `condvar_notify`, `condvar_notify_all`, `scope`, `branch`, `goto`, `return`. Next: `select`, `async_call` / `await`, `atomic_*`, `switch`, `semaphore_*`, `channel_*`, and cross-module `call`. PNML after that.

## Development

```bash
cargo fmt --all
cargo check --workspace --all-targets
cargo test --workspace
cargo test --no-default-features
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --no-deps
```

## License

MIT
