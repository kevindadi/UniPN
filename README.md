# UniPN

UniPN is a Rust library for representing a **single generic Petri-net model** shared by three frontends. It is library-only: no CLI, no execution language, no project-specific verification algorithms.

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

- `PlaceRole` — is this place a shared resource, and may a thread legitimately end on it? This is what makes the deadlock definition shared: `Net::is_deadlock` says a blocked marking is only a deadlock if some token sits on a control place that is not a thread end, so a run where every thread finished and returned every lock is a normal termination rather than a false positive.
- `TransitionRole` — `is_acquire`, `is_release`, `is_thread_spawn`, `is_thread_join`, `is_atomic`, `is_unsafe_access`. A shared *vocabulary*, deliberately not a shared enum: P/T's transition variants carry what its pointer analysis inferred (`Lock(alias)`, `AtomicLoad(alias, ordering, …)`) while the CVN's are bare tags, since in ConcIR the resource identity already is the place identity.

The timed net implements neither — PTPN's places carry no control/resource split and it classifies schedulability, not deadlocks. A bound is only paid where it is used.

## Analysis

Analysis is not part of the model. The [`analysis`](src/analysis/mod.rs) module provides the minimal firing contract [`NetLike`] plus the checks that depend on no kind at all: `explore` (BFS/DFS reachability), `find_deadlocks`, `conflict_sets` (transitions competing for an input place), and `unfired_transitions` (behavioral dead code). The explorer only reports *blocked* states; a deadlock verdict comes from `PlaceRole` or from the caller.

Each frontend then has its own analysis submodule: `analysis::pt` (reachability graph, boundness, reduction), `analysis::cvn` (the generic checks wrapped in ConcIR anchors and disjunctive families), and `analysis::timed` (state classes).

Timed (DBM / state-class) analysis is optional:

```toml
unipn = { version = "0.2", features = ["timed"] }          # default
unipn = { version = "0.2", default-features = false }      # model + discrete NetLike only
```

`TimedNet`'s `NetLike` state is `TimedState` (`State<TimedExtra>`): marking only. Clock zones stay in `analysis::timed::StateClass` and are not switched in via the feature — a zone is a set of clock valuations, not a field that discrete `fire` can update.

## Development

```bash
cargo fmt --all
cargo check --all-targets
cargo test
cargo test --no-default-features
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps
```

## License

MIT
