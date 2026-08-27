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

## Source layout

Three layers: the generic core, one directory per frontend, and the analyses.

```text
src/
  net/      mod.rs (the model) + ids.rs + incidence.rs
  pt/       kinds.rs + semantics.rs + builder.rs + dot.rs
  timed/    kinds.rs + semantics.rs + interval.rs
  cvn/      kinds.rs + semantics.rs + builder.rs + expr.rs + dot.rs
  analysis/ mod.rs (NetLike + explore) + pt/ + cvn/ + timed/
```

Within a frontend directory `kinds.rs` holds the payloads and the net alias, `semantics.rs` holds that net's firing, and each `mod.rs` re-exports flatly — so `unipn::pt::PtNet` and the crate-root re-exports (`unipn::PtNet`, `unipn::PlaceId`, …) do not depend on which file a symbol lives in.

## Analysis

Analysis is not part of the model. The [`analysis`](src/analysis/mod.rs) module provides the minimal firing contract [`NetLike`] plus `explore` (BFS/DFS reachability) and `find_deadlocks`. The explorer only reports *blocked* states; the caller decides what counts as a deadlock.

Each frontend then has its own analysis submodule: `analysis::pt` (reachability graph, boundness, reduction), `analysis::cvn` (deadlock, dead transitions, conflict sets), and `analysis::timed` (state classes).

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
