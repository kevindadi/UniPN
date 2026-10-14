//! Difference bound matrix with strict bounds, for dense-time TPN analysis.
//!
//! A DBM represents a conjunction of constraints `x_i − x_j ≤ c` (non-strict)
//! or `x_i − x_j < c` (strict), where `x_0` is the reference clock (value 0)
//! and each other `x_i` is a clock. A bound is either `+∞` (no constraint) or a
//! finite `(value, strict)` pair. This is a *new* DBM, separate from the PTPN
//! one in [`analysis::timed::dbm`](crate::analysis::timed::dbm): the PTPN DBM
//! folds open endpoints into closed integer bounds (its clock domain is
//! integral), while this one keeps strictness so that `(0, 1)` and `[1, 1]`
//! stay distinct in dense time.
//!
//! All arithmetic is checked: an `i64` overflow is an [`Overflow`] error, never
//! a panic and never a wrap. [`Eq`] and [`Hash`] are meaningful on the
//! *canonical* (normalized) form; call [`normalize`](Dbm::normalize) before
//! comparing.

use std::fmt;

/// A single difference bound: `x_i − x_j ≤ value` or `x_i − x_j < value`, or
/// `+∞` for "no constraint".
///
/// The ordering is the DBM's "tighter is smaller": `+∞` is the loosest, and at
/// equal `value` a strict bound is tighter than a non-strict one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bound {
    Finite { value: i64, strict: bool },
    Infinity,
}

impl Bound {
    /// The non-strict bound `x ≤ value`.
    pub fn le(value: i64) -> Self {
        Bound::Finite {
            value,
            strict: false,
        }
    }

    /// The strict bound `x < value`.
    pub fn lt(value: i64) -> Self {
        Bound::Finite {
            value,
            strict: true,
        }
    }

    /// `+∞`: no constraint.
    pub fn infinity() -> Self {
        Bound::Infinity
    }

    /// The tighter of two bounds (the smaller in the DBM order).
    pub fn min(self, other: Self) -> Self {
        if other < self { other } else { self }
    }

    /// The sum of two bounds, for composing a two-edge path in Floyd–Warshall.
    ///
    /// `(a, s1) + (b, s2) = (a + b, s1 ∨ s2)`: a strict bound on either edge
    /// makes the composed bound strict. `+∞` absorbs. Returns `None` on `i64`
    /// overflow.
    pub fn compose(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Bound::Infinity, _) | (_, Bound::Infinity) => Some(Bound::Infinity),
            (
                Bound::Finite {
                    value: a,
                    strict: s1,
                },
                Bound::Finite {
                    value: b,
                    strict: s2,
                },
            ) => Some(Bound::Finite {
                value: a.checked_add(b)?,
                strict: s1 || s2,
            }),
        }
    }

    /// Whether this bound makes a diagonal constraint `x_i − x_i ≤ c` (or
    /// `< c`) unsatisfiable: `c < 0`, or `c = 0` and strict.
    pub fn is_negative(self) -> bool {
        match self {
            Bound::Finite { value, strict } => value < 0 || (value == 0 && strict),
            Bound::Infinity => false,
        }
    }
}

impl PartialOrd for Bound {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Bound {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Bound::Infinity, Bound::Infinity) => Ordering::Equal,
            (Bound::Infinity, _) => Ordering::Greater,
            (_, Bound::Infinity) => Ordering::Less,
            (
                Bound::Finite {
                    value: a,
                    strict: s1,
                },
                Bound::Finite {
                    value: b,
                    strict: s2,
                },
            ) => a.cmp(b).then_with(|| s2.cmp(s1)),
        }
    }
}

impl fmt::Display for Bound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bound::Infinity => write!(f, "∞"),
            Bound::Finite { value, strict } => {
                write!(f, "{}{}", if *strict { "<" } else { "≤" }, value)
            }
        }
    }
}

/// Arithmetic overflow in a DBM operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overflow;

/// A difference bound matrix over [`Bound`]s.
///
/// `matrix[i * size + j]` is the bound on `x_i − x_j`. Index 0 is the
/// reference clock by convention, but the DBM itself is generic over the
/// meaning of the indices.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dbm {
    matrix: Vec<Bound>,
    size: usize,
}

impl Dbm {
    /// An unconstrained DBM of `size` clocks: every off-diagonal bound is `+∞`
    /// and every diagonal is `0 ≤ 0`.
    pub fn new(size: usize) -> Self {
        let mut matrix = vec![Bound::Infinity; size * size];
        for i in 0..size {
            matrix[i * size + i] = Bound::le(0);
        }
        Dbm { matrix, size }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn get(&self, i: usize, j: usize) -> Bound {
        self.matrix[i * self.size + j]
    }

    pub fn set(&mut self, i: usize, j: usize, bound: Bound) {
        self.matrix[i * self.size + j] = bound;
    }

    /// Floyd–Warshall all-pairs shortest-path closure, making the DBM canonical.
    ///
    /// Returns [`Overflow`] if any path sum overflows `i64`.
    pub fn normalize(&mut self) -> Result<(), Overflow> {
        let n = self.size;
        for k in 0..n {
            for i in 0..n {
                let ik = self.matrix[i * n + k];
                if ik == Bound::Infinity {
                    continue;
                }
                for j in 0..n {
                    let kj = self.matrix[k * n + j];
                    if kj == Bound::Infinity {
                        continue;
                    }
                    let sum = ik.compose(kj).ok_or(Overflow)?;
                    let ij = &mut self.matrix[i * n + j];
                    if sum < *ij {
                        *ij = sum;
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether the zone is empty. Normalizes first, so it is always sound.
    pub fn is_empty(&self) -> Result<bool, Overflow> {
        let mut copy = self.clone();
        copy.normalize()?;
        Ok(copy.has_negative_diagonal())
    }

    /// Tighten `(i, j)` to `bound` and re-normalize; `Ok(false)` if the
    /// tightening empties the zone.
    pub fn tighten(&mut self, i: usize, j: usize, bound: Bound) -> Result<bool, Overflow> {
        let idx = i * self.size + j;
        if bound < self.matrix[idx] {
            self.matrix[idx] = bound;
        }
        self.normalize()?;
        Ok(!self.has_negative_diagonal())
    }

    /// Element-wise minimum of two same-sized DBMs, then closure.
    pub fn intersection(&self, other: &Dbm) -> Result<Dbm, Overflow> {
        assert_eq!(self.size, other.size, "DBM sizes must match");
        let mut result = Dbm::new(self.size);
        for i in 0..self.size {
            for j in 0..self.size {
                result.matrix[i * self.size + j] =
                    self.matrix[i * self.size + j].min(other.matrix[i * self.size + j]);
            }
        }
        result.normalize()?;
        Ok(result)
    }

    /// Whether any diagonal constraint is unsatisfiable. Sound only after
    /// [`normalize`](Dbm::normalize); the state-class graph always normalizes
    /// before calling this.
    pub fn has_negative_diagonal(&self) -> bool {
        (0..self.size).any(|i| self.matrix[i * self.size + i].is_negative())
    }
}

impl fmt::Display for Dbm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.size == 0 {
            return write!(f, "Dbm(empty)");
        }
        writeln!(f, "Dbm(size={}):", self.size)?;
        for i in 0..self.size {
            write!(f, "x{i} ")?;
            for j in 0..self.size {
                write!(f, "{:>8}", self.matrix[i * self.size + j])?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-clock DBM (x0 = reference, x1 = the clock) with `x0 - x1 <= lo`
    /// (i.e. `x1 >= -lo`) and `x1 - x0 <= hi` (i.e. `x1 <= hi`).
    fn single_clock(lo: Bound, hi: Bound) -> Dbm {
        let mut d = Dbm::new(2);
        d.set(1, 0, hi);
        d.set(0, 1, lo);
        d
    }

    #[test]
    fn strict_and_non_strict_combine() {
        // (a, <) + (b, <=) = (a+b, <).
        assert_eq!(Bound::lt(1).compose(Bound::le(2)), Some(Bound::lt(3)));
        // (a, <=) + (b, <=) = (a+b, <=).
        assert_eq!(Bound::le(1).compose(Bound::le(2)), Some(Bound::le(3)));
        // Infinity absorbs.
        assert_eq!(Bound::lt(1).compose(Bound::Infinity), Some(Bound::Infinity));
        // Overflow is reported, not wrapped.
        assert_eq!(Bound::le(i64::MAX).compose(Bound::le(1)), None);
    }

    #[test]
    fn strict_is_tighter_than_non_strict_at_equal_value() {
        assert!(Bound::lt(1) < Bound::le(1));
        assert!(Bound::le(0) < Bound::lt(1));
        assert!(Bound::Infinity > Bound::le(i64::MAX));
    }

    #[test]
    fn emptiness_distinguishes_strict_from_non_strict() {
        // x < 1 ∧ x > 1 is empty.
        let d = single_clock(Bound::lt(-1), Bound::lt(1));
        assert!(d.is_empty().unwrap());
        // x <= 1 ∧ x >= 1 is the single point x = 1: non-empty.
        let d = single_clock(Bound::le(-1), Bound::le(1));
        assert!(!d.is_empty().unwrap());
        // x < 1 ∧ x >= 1 is empty (no value is both < 1 and >= 1).
        let d = single_clock(Bound::le(-1), Bound::lt(1));
        assert!(d.is_empty().unwrap());
    }

    #[test]
    fn normalization_is_idempotent() {
        // A triangle that needs two passes of tightening: x1 - x2 <= 1,
        // x2 - x3 <= 2, x1 - x3 <= 10 (loose), so closure gives x1 - x3 <= 3.
        let mut d = Dbm::new(4);
        d.set(1, 2, Bound::le(1));
        d.set(2, 3, Bound::le(2));
        d.set(1, 3, Bound::le(10));
        d.normalize().unwrap();
        assert_eq!(d.get(1, 3), Bound::le(3));

        let once = d.clone();
        d.normalize().unwrap();
        assert_eq!(d, once, "normalizing a canonical DBM changes nothing");
    }

    #[test]
    fn strictness_propagates_through_closure() {
        // x1 - x2 < 1 and x2 - x3 <= 2 imply x1 - x3 < 3.
        let mut d = Dbm::new(4);
        d.set(1, 2, Bound::lt(1));
        d.set(2, 3, Bound::le(2));
        d.normalize().unwrap();
        assert_eq!(d.get(1, 3), Bound::lt(3));
    }
}
