# UniPN

UniPN is a Rust library for representing a **single generic Petri-net model** shared by three frontends. It is library-only: no CLI, no execution language, no project-specific verification algorithms.

## The model

One generic net, three instantiations:

```rust
// src/net.rs — the generic model
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
| ConcPlanVerify (CVN) | `cvn::CvnNet` | `model::PlaceKind` | `model::TransitionKind` | `cvn::CvnArcKind` |

The marking is kept separate from the net; anything a net needs beyond token counts (a CVN variable store, a timed clock zone, …) lives in its own `State` `extra` payload.

## Analysis

Analysis is not part of the model. The [`analysis`](src/analysis/mod.rs) module provides the minimal firing contract [`NetLike`] plus `explore` (BFS/DFS reachability) and `find_deadlocks`. The explorer only reports *blocked* states; the caller decides what counts as a deadlock.

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
