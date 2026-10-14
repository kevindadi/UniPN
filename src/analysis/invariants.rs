//! Structural invariants: P- and T-semiflows of the incidence matrix.
//!
//! A **P-semiflow** is a vector `y ≥ 0, y ≠ 0` over places with `yᵀ·C = 0`; a
//! **T-semiflow** is `x ≥ 0, x ≠ 0` over transitions with `C·x = 0`, where
//! `C[p, t] = w_post(p, t) − w_pre(p, t)` is the ordinary incidence matrix
//! (input and output arcs only — see [`net::incidence`](crate::net::incidence)).
//! Both are computed with the Farkas algorithm (Fourier–Motzkin elimination
//! over `[C | I]`), keeping only rows of minimal support at every step, so the
//! result is exactly the set of **minimal-support semiflows**, each scaled to
//! coprime integer coefficients. Every semiflow is a non-negative rational
//! combination of these.
//!
//! Nothing here depends on a kind: the matrix is the arc structure. Which of
//! the results are actually *invariants* of a net's behavior does depend on its
//! semantics, and that is what [`p_invariant_status`] answers.
//!
//! # When a P-semiflow is an invariant
//!
//! For a plain P/T net, `yᵀ·M = yᵀ·M₀` in every reachable marking — the
//! marking equation `M = M₀ + C·σ` times `yᵀ`. The arc kinds and capacity
//! policies of this crate change that in four ways:
//!
//! 1. **Read and inhibitor arcs** move no tokens and are not in `C`. They only
//!    forbid firings, and forbidding a firing cannot break an equation every
//!    firing preserves: P-semiflows stay invariants.
//! 2. **Reset arcs** empty a place regardless of `C`. A semiflow whose support
//!    contains a reset place is *not* an invariant in general.
//! 3. **Capacity clamping** (`PtNet`, `TimedNet`) drops the tokens that do not
//!    fit, which `C` does not see either. A semiflow whose support contains a
//!    bounded place a transition produces into is an invariant only if that
//!    place can never actually overflow. That follows from the semiflow itself
//!    when `⌊yᵀ·M₀ / y_p⌋ ≤ capacity(p)`: while the invariant holds, `y_p·M(p)`
//!    can never exceed `yᵀ·M₀`, so the unclamped successor never overfills `p`,
//!    so no clamping happens and the invariant holds in the successor too.
//! 4. **Guards and variable updates** (`CvnNet`) restrict firings and change
//!    `State.extra`, not the marking; the CVN *rejects* an over-capacity firing
//!    rather than clamping it. P-semiflows are invariants of the CVN's marking.
//!
//! [`p_invariant_status`] applies 2 and 3. For 3 it uses only what
//! [`PlaceCapacity`] says, not which policy the frontend chose, so on a
//! *rejecting* net (the CVN) it can answer [`InvariantStatus::CapacityUnproven`]
//! where the invariant in fact holds — conservative, never the other way round.
//! In practice the bound argument proves the common case (a mutex: `lock + its
//! holders = 1`, capacity 1).
//!
//! # T-semiflows
//!
//! `C·x = 0` says a firing sequence with Parikh vector `x` *if it can fire*
//! returns to the marking it started from. That is a structural fact; whether
//! such a sequence is fireable is a behavioral one (guards, inhibitors, the
//! marking). Reset arcs and clamping break even the structural reading, since a
//! sequence that resets or clamps a place does not return to where it began.
//!
//! # Cost
//!
//! The number of minimal-support semiflows can be exponential in the net, and
//! so can the intermediate rows. [`FarkasLimits::max_rows`] bounds the
//! intermediate row count; exceeding it is an error, never a truncated answer.
//! Arithmetic is checked `i128` throughout, and an overflow is an error too.

use std::fmt;

use crate::net::{ArcDir, Incidence, Marking, Net, Place, PlaceCapacity, PlaceId, TransitionId};

/// A minimal-support semiflow: strictly positive coefficients on its support,
/// sorted by id, with greatest common divisor 1.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Semiflow<I> {
    entries: Vec<(I, u64)>,
}

/// A P-semiflow: coefficients over places.
pub type PSemiflow = Semiflow<PlaceId>;

/// A T-semiflow: coefficients over transitions.
pub type TSemiflow = Semiflow<TransitionId>;

impl<I: Copy + Ord> Semiflow<I> {
    /// `(id, coefficient)` for every id in the support, in id order.
    pub fn entries(&self) -> &[(I, u64)] {
        &self.entries
    }

    /// The number of ids in the support.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Always `false`: a semiflow is non-zero by definition.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// `(id, coefficient)` pairs, in id order.
    pub fn iter(&self) -> impl Iterator<Item = &(I, u64)> {
        self.entries.iter()
    }

    /// The ids with a non-zero coefficient, in id order.
    pub fn support(&self) -> impl Iterator<Item = I> + '_ {
        self.entries.iter().map(|&(id, _)| id)
    }

    /// The coefficient of `id`; 0 outside the support.
    pub fn coefficient(&self, id: I) -> u64 {
        self.entries
            .binary_search_by_key(&id, |&(i, _)| i)
            .map_or(0, |at| self.entries[at].1)
    }
}

impl PSemiflow {
    /// `yᵀ·M`, or `None` if it does not fit in a `u128`.
    pub fn weighted_sum(&self, marking: &Marking) -> Option<u128> {
        self.entries.iter().try_fold(0u128, |sum, &(place, y)| {
            u128::from(y)
                .checked_mul(marking.tokens(place) as u128)
                .and_then(|term| sum.checked_add(term))
        })
    }
}

/// Bounds on the Farkas computation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FarkasLimits {
    /// The most rows the elimination may hold at once (counted before the
    /// minimal-support filter of each step). Each row stores one `i128` per
    /// place and per transition, so this is also the memory bound.
    pub max_rows: usize,
}

impl Default for FarkasLimits {
    fn default() -> Self {
        Self { max_rows: 100_000 }
    }
}

/// Why no semiflows were returned. There is never a partial answer: a missing
/// minimal semiflow would read as a missing invariant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemiflowError {
    /// A coefficient left `i128` during elimination, or a final coefficient
    /// does not fit in `u64`.
    Overflow,
    /// An elimination step would hold more than [`FarkasLimits::max_rows`]
    /// rows.
    RowLimit { limit: usize },
}

impl fmt::Display for SemiflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow => write!(f, "semiflow coefficient overflow"),
            Self::RowLimit { limit } => {
                write!(f, "Farkas elimination exceeded {limit} intermediate rows")
            }
        }
    }
}

impl std::error::Error for SemiflowError {}

/// The minimal-support P-semiflows of `net`.
pub fn p_semiflows<PK, TK, AK>(
    net: &Net<PK, TK, AK>,
    limits: FarkasLimits,
) -> Result<Vec<PSemiflow>, SemiflowError> {
    p_semiflows_within(net, |_| true, limits)
}

/// The minimal-support P-semiflows whose support lies inside the places `keep`
/// accepts.
///
/// These are semiflows of the whole net — `yᵀ·C = 0` over *every* transition —
/// that happen to be zero outside the selection. Selecting by label asks which
/// conservation laws live among one class of places: on a P/T net, keeping the
/// `Resource` places and the `BasicBlock` control points (leaving out function
/// starts and ends) finds, for each lock, the law "the lock plus the control
/// points that hold it is constant", without the per-thread laws that mention
/// every control place.
pub fn p_semiflows_within<PK, TK, AK>(
    net: &Net<PK, TK, AK>,
    keep: impl Fn(&Place<PK>) -> bool,
    limits: FarkasLimits,
) -> Result<Vec<PSemiflow>, SemiflowError> {
    let incidence = Incidence::of(net);
    let places: Vec<PlaceId> = net
        .places
        .iter()
        .filter(|p| keep(p))
        .map(|p| p.id)
        .collect();
    let transitions: Vec<TransitionId> = net.transition_ids().collect();
    let rows: Vec<Vec<i128>> = places
        .iter()
        .map(|&p| {
            transitions
                .iter()
                .map(|&t| entry(&incidence, p, t))
                .collect()
        })
        .collect();
    Ok(farkas(&rows, transitions.len(), limits)?
        .into_iter()
        .map(|flow| Semiflow {
            entries: flow.into_iter().map(|(i, y)| (places[i], y)).collect(),
        })
        .collect())
}

/// The minimal-support T-semiflows of `net`. See the module docs for what they
/// do and do not say about behavior.
pub fn t_semiflows<PK, TK, AK>(
    net: &Net<PK, TK, AK>,
    limits: FarkasLimits,
) -> Result<Vec<TSemiflow>, SemiflowError> {
    let incidence = Incidence::of(net);
    let places: Vec<PlaceId> = net.place_ids().collect();
    let rows: Vec<Vec<i128>> = net
        .transition_ids()
        .map(|t| places.iter().map(|&p| entry(&incidence, p, t)).collect())
        .collect();
    Ok(farkas(&rows, places.len(), limits)?
        .into_iter()
        .map(|flow| Semiflow {
            entries: flow
                .into_iter()
                .map(|(i, x)| (TransitionId(i), x))
                .collect(),
        })
        .collect())
}

/// `C[p, t]`, exact: weights are `usize`, so their difference always fits.
fn entry(incidence: &Incidence, place: PlaceId, transition: TransitionId) -> i128 {
    incidence.post_weight(place, transition) as i128
        - incidence.pre_weight(place, transition) as i128
}

/// Whether a P-semiflow is an invariant of the net's markings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvariantStatus {
    /// `yᵀ·M = yᵀ·M₀` in every reachable marking.
    Holds,
    /// Not an invariant in general: these support places are emptied by a
    /// reset arc, which the incidence matrix does not see.
    ResetInSupport(Vec<PlaceId>),
    /// These support places have a capacity, some transition produces into
    /// them, and the semiflow does not itself keep them below it — so a
    /// clamping net may drop tokens there. Holds along every run that never
    /// clamps them; and on a net that *rejects* over-capacity firings instead
    /// of clamping (the CVN) it holds outright.
    CapacityUnproven(Vec<PlaceId>),
}

/// Decide whether `semiflow` is an invariant of `net` from `initial`; see the
/// module docs for the rules. Reset arcs are checked first, since they break
/// the invariant whatever the capacities.
pub fn p_invariant_status<PK: PlaceCapacity, TK, AK>(
    net: &Net<PK, TK, AK>,
    semiflow: &PSemiflow,
    initial: &Marking,
) -> InvariantStatus {
    let reset: Vec<PlaceId> = semiflow
        .support()
        .filter(|&p| net.arcs_at(p).any(|a| a.direction == ArcDir::Reset))
        .collect();
    if !reset.is_empty() {
        return InvariantStatus::ResetInSupport(reset);
    }

    let total = semiflow.weighted_sum(initial);
    let unproven: Vec<PlaceId> = semiflow
        .entries()
        .iter()
        .filter(|&&(p, y)| {
            let Some(capacity) = net.capacity_of(p) else {
                return false;
            };
            if !net.arcs_at(p).any(|a| a.direction == ArcDir::Output) {
                return false;
            }
            total.is_none_or(|total| total / u128::from(y) > capacity as u128)
        })
        .map(|&(p, _)| p)
        .collect();
    if unproven.is_empty() {
        InvariantStatus::Holds
    } else {
        InvariantStatus::CapacityUnproven(unproven)
    }
}

/// One row of the elimination: `c` is the original row combination's entries
/// in the matrix, `y` its coefficients over the original rows, `support` the
/// non-zero positions of `y` as a bitset.
#[derive(Clone)]
struct Row {
    c: Vec<i128>,
    y: Vec<i128>,
    support: Vec<u64>,
}

impl Row {
    fn unit(index: usize, c: Vec<i128>, n: usize) -> Self {
        let mut y = vec![0; n];
        y[index] = 1;
        let mut support = vec![0u64; n.div_ceil(64)];
        support[index / 64] |= 1 << (index % 64);
        Self { c, y, support }
    }

    fn support_len(&self) -> u32 {
        self.support.iter().map(|w| w.count_ones()).sum()
    }

    fn support_within(&self, other: &Row) -> bool {
        self.support
            .iter()
            .zip(&other.support)
            .all(|(a, b)| a & !b == 0)
    }

    /// `a·self + b·other` with `a, b > 0`, scaled down by its gcd.
    fn combine(&self, a: i128, other: &Row, b: i128) -> Result<Row, SemiflowError> {
        let mix = |x: &[i128], z: &[i128]| -> Result<Vec<i128>, SemiflowError> {
            x.iter()
                .zip(z)
                .map(|(&x, &z)| {
                    a.checked_mul(x)
                        .zip(b.checked_mul(z))
                        .and_then(|(l, r)| l.checked_add(r))
                        .ok_or(SemiflowError::Overflow)
                })
                .collect()
        };
        let mut c = mix(&self.c, &other.c)?;
        let mut y = mix(&self.y, &other.y)?;
        // |i128::MIN| does not fit in an i128, which both the gcd below and any
        // later negation would need.
        if c.iter().chain(&y).any(|&v| v == i128::MIN) {
            return Err(SemiflowError::Overflow);
        }
        let g = c.iter().chain(&y).fold(0, |g, &v| gcd(g, v));
        if g > 1 {
            c.iter_mut().chain(y.iter_mut()).for_each(|v| *v /= g);
        }
        let support = self
            .support
            .iter()
            .zip(&other.support)
            .map(|(a, b)| a | b)
            .collect();
        Ok(Row { c, y, support })
    }
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.unsigned_abs(), b.unsigned_abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    // `combine` rejects i128::MIN, so every |entry| — and so the gcd — fits.
    a as i128
}

/// Fourier–Motzkin over `[A | I]` for the non-negative solutions of `yᵀ·A = 0`,
/// where `a` has one row per variable and `cols` columns. Returns each
/// minimal-support solution as sparse `(row index, coefficient)` pairs.
fn farkas(
    a: &[Vec<i128>],
    cols: usize,
    limits: FarkasLimits,
) -> Result<Vec<Vec<(usize, u64)>>, SemiflowError> {
    let n = a.len();
    let mut rows: Vec<Row> = a
        .iter()
        .enumerate()
        .map(|(i, c)| Row::unit(i, c.clone(), n))
        .collect();
    if rows.len() > limits.max_rows {
        return Err(SemiflowError::RowLimit {
            limit: limits.max_rows,
        });
    }
    let mut remaining: Vec<usize> = (0..cols).collect();

    while !remaining.is_empty() {
        // Eliminate the column that adds the fewest rows first (|pos|·|neg| new
        // rows, |pos|+|neg| removed) — a heuristic, the result does not depend
        // on the order.
        let (slot, column) = remaining
            .iter()
            .copied()
            .enumerate()
            .min_by_key(|&(_, j)| {
                let pos = rows.iter().filter(|r| r.c[j] > 0).count() as i128;
                let neg = rows.iter().filter(|r| r.c[j] < 0).count() as i128;
                pos * neg - pos - neg
            })
            .expect("remaining is not empty");
        remaining.swap_remove(slot);

        let (zero, nonzero): (Vec<Row>, Vec<Row>) =
            rows.into_iter().partition(|r| r.c[column] == 0);
        let (pos, neg): (Vec<Row>, Vec<Row>) = nonzero.into_iter().partition(|r| r.c[column] > 0);

        let generated = zero.len() + pos.len() * neg.len();
        if generated > limits.max_rows {
            return Err(SemiflowError::RowLimit {
                limit: limits.max_rows,
            });
        }

        let mut next = zero;
        for p in &pos {
            for q in &neg {
                let a = q.c[column].checked_neg().ok_or(SemiflowError::Overflow)?;
                next.push(p.combine(a, q, p.c[column])?);
            }
        }
        rows = minimal_supports(next);
    }

    rows.into_iter()
        .map(|row| {
            row.y
                .iter()
                .enumerate()
                .filter(|&(_, &v)| v != 0)
                .map(|(i, &v)| {
                    u64::try_from(v)
                        .map(|v| (i, v))
                        .map_err(|_| SemiflowError::Overflow)
                })
                .collect()
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|mut flows| {
            flows.sort();
            flows
        })
}

/// Keep only the rows whose support contains no other kept row's support.
///
/// After each elimination step the rows are a generating set of the cone of
/// non-negative solutions so far, containing (a positive multiple of) every
/// extreme ray. A ray is extreme exactly when it is the only ray, up to scale,
/// on its support; any other ray's support strictly contains an extreme ray's.
/// So discarding every row whose support contains another kept row's — equal
/// supports included, which are then multiples of one another — leaves exactly
/// one row per extreme ray, and the extreme rays of the final cone are the
/// minimal-support semiflows.
fn minimal_supports(mut rows: Vec<Row>) -> Vec<Row> {
    rows.sort_by_key(Row::support_len);
    let mut kept: Vec<Row> = Vec::with_capacity(rows.len());
    for row in rows {
        if !kept.iter().any(|k| k.support_within(&row)) {
            kept.push(row);
        }
    }
    kept
}
