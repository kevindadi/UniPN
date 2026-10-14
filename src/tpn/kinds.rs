//! Dense-time (Merlin–Farber) time Petri net kind payloads plus the
//! [`TpnNet`] alias.
//!
//! A transition carries a static firing [`Interval`]; places are plain
//! [`Tag`] labels. Time is *dense* here: an interval is a
//! subset of the non-negative reals, and the state-class analysis in
//! `analysis::tpn` interprets it with a difference bound matrix that keeps
//! open/closed endpoints distinct. This is the opposite of the PTPN frontend
//! ([`timed`](crate::timed)), whose integral clock domain folds open endpoints
//! into closed integer bounds before the DBM ever sees them.

use serde::{Deserialize, Serialize};

use crate::labeled::Tag;
use crate::net::{Labeled, Net};

/// A finite upper bound of a time interval: a value with an open or closed
/// endpoint.
///
/// `+∞` is a separate variant rather than a sentinel value so that "the upper
/// bound is open" is structural for infinity — there is no "closed infinity".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UpperBound {
    /// A finite upper bound, open (`value` excluded) or closed (included).
    Finite { value: u64, open: bool },
    /// `+∞`: always an open upper bound.
    Infinity,
}

/// A dense-time interval with integer endpoints.
///
/// Endpoints are non-negative `u64`s. The choice of `u64` over `i64` is
/// deliberate: a firing time is a non-negative quantity, so the type makes the
/// "lower bound ≥ 0" invariant structural instead of a runtime check, and it
/// matches the crate-wide convention that every count is `usize`. The
/// difference bound matrix in `analysis::tpn` uses signed `i64` internally
/// (clock *differences* can be negative) and checks the `u64 → i64` conversion
/// at the boundary.
///
/// A caller with rational endpoints scales them to a common denominator first
/// and passes the resulting integers; the analysis is exact for the scaled
/// net, and the scaling is the caller's responsibility (documented here rather
/// than guessed).
///
/// The lower bound is open or closed; the upper bound is a finite value (open
/// or closed) or `+∞`. Legality is judged in dense time: the interval is
/// non-empty iff `lower < upper`, or `lower == upper` with both endpoints
/// closed. So `(0, 1)` is legal, `[1, 1]` is legal, and `[1, 1)` is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Interval {
    pub lower: u64,
    pub lower_open: bool,
    pub upper: UpperBound,
}

/// Why an [`Interval`] was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntervalError {
    /// The interval has an empty dense-time domain (e.g. `[1, 1)`).
    Empty,
}

impl Interval {
    /// Validate and build an interval.
    ///
    /// Rejects an empty dense-time domain: `lower > upper`, or `lower == upper`
    /// with either endpoint open. `+∞` is always a legal upper bound.
    pub fn new(lower: u64, lower_open: bool, upper: UpperBound) -> Result<Self, IntervalError> {
        match upper {
            UpperBound::Finite { value, open } => {
                if lower < value || (lower == value && !lower_open && !open) {
                    Ok(Self {
                        lower,
                        lower_open,
                        upper,
                    })
                } else {
                    Err(IntervalError::Empty)
                }
            }
            UpperBound::Infinity => Ok(Self {
                lower,
                lower_open,
                upper,
            }),
        }
    }

    /// The closed interval `[a, b]`.
    pub fn closed(a: u64, b: u64) -> Self {
        Self {
            lower: a,
            lower_open: false,
            upper: UpperBound::Finite {
                value: b,
                open: false,
            },
        }
    }

    /// The open interval `(a, b)`.
    pub fn open(a: u64, b: u64) -> Self {
        Self {
            lower: a,
            lower_open: true,
            upper: UpperBound::Finite {
                value: b,
                open: true,
            },
        }
    }

    /// The half-open interval `(a, b]`.
    pub fn left_open(a: u64, b: u64) -> Self {
        Self {
            lower: a,
            lower_open: true,
            upper: UpperBound::Finite {
                value: b,
                open: false,
            },
        }
    }

    /// The half-open interval `[a, b)`.
    pub fn right_open(a: u64, b: u64) -> Self {
        Self {
            lower: a,
            lower_open: false,
            upper: UpperBound::Finite {
                value: b,
                open: true,
            },
        }
    }

    /// The unbounded interval `[a, +∞)`.
    pub fn unbounded(a: u64) -> Self {
        Self {
            lower: a,
            lower_open: false,
            upper: UpperBound::Infinity,
        }
    }

    /// Whether the interval has a non-empty dense-time domain.
    pub fn is_valid(&self) -> bool {
        match self.upper {
            UpperBound::Finite { value, open } => {
                self.lower < value || (self.lower == value && !self.lower_open && !open)
            }
            UpperBound::Infinity => true,
        }
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}, {}{}",
            if self.lower_open { "(" } else { "[" },
            self.lower,
            match self.upper {
                UpperBound::Finite { value, .. } => value.to_string(),
                UpperBound::Infinity => "∞".to_string(),
            },
            match self.upper {
                UpperBound::Finite { open, .. } if open => ")",
                UpperBound::Finite { .. } => "]",
                UpperBound::Infinity => ")",
            }
        )
    }
}

/// A transition kind for a dense-time TPN: a label plus a static firing
/// interval.
///
/// The label is the transition's classification (an enum, a `String`, `()` for
/// none); the interval is the firing window. The label is what
/// [`Labeled`] exposes, so the label-level analyses in
/// [`analysis::labels`](crate::analysis::labels) work on a `TpnNet` unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TpnTransition<L> {
    pub label: L,
    pub interval: Interval,
}

impl<L> TpnTransition<L> {
    pub fn new(label: L, interval: Interval) -> Self {
        Self { label, interval }
    }
}

impl<L> Labeled for TpnTransition<L> {
    type Label = L;

    fn label(&self) -> &L {
        &self.label
    }
}

/// A dense-time time Petri net: labeled places, interval-annotated
/// transitions, no arc payload.
///
/// The place kind is [`Tag<PL>`](crate::labeled::Tag) so a `TpnNet` is a *labeled*
/// net whose transitions additionally carry a firing interval. Its
/// [`Semantics`](crate::analysis::Semantics) is the untimed P/T skeleton (see
/// [`semantics`](super::semantics)): exploring a `TpnNet` directly yields the
/// untimed over-approximation, with time ignored.
pub type TpnNet<PL, TL> = Net<Tag<PL>, TpnTransition<TL>, ()>;
