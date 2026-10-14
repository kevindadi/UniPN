//! The state class of a dense-time TPN: a marking plus a firing domain.
//!
//! A class is `(M, D)` where `D` is a difference bound matrix over one clock
//! per *enabled instance* of a transition. Variables are keyed by
//! [`InstanceKey`] (transition id + instance index) from the start, so the
//! single-server case (instance index always 0) and the multi-server case
//! (Part 5) share one data structure.

use crate::net::{Marking, TransitionId};
use crate::tpn::{Interval, UpperBound};

use super::dbm::{Bound, Dbm};

/// A clock variable: one enabled instance of a transition.
///
/// The instance index is 0 under single-server semantics and the age rank
/// (0 = oldest) under multi-server semantics. Ordering is by `(transition,
/// instance)`, which is the canonical column order of a class's DBM.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstanceKey {
    pub transition: TransitionId,
    pub instance: usize,
}

/// A state class `(M, D)`: a marking and the firing domain of its enabled
/// instances.
///
/// `zone` has one column per enabled instance plus the reference clock at
/// column 0; `vars[i]` names the instance at column `i + 1`. The zone is kept
/// canonical (normalized), so [`Eq`] and [`Hash`] compare classes by marking
/// and canonical firing domain, exactly as the state-class method requires.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StateClass {
    pub marking: Marking,
    pub(crate) zone: Dbm,
    pub(crate) vars: Vec<InstanceKey>,
}

impl StateClass {
    pub(crate) fn new(marking: Marking, zone: Dbm, vars: Vec<InstanceKey>) -> Self {
        StateClass {
            marking,
            zone,
            vars,
        }
    }

    /// The enabled instances, in canonical `(transition, instance)` order.
    pub fn instances(&self) -> &[InstanceKey] {
        &self.vars
    }

    /// The DBM column of `key` (1-based; column 0 is the reference clock).
    pub(crate) fn column_of(&self, key: InstanceKey) -> Option<usize> {
        self.vars.iter().position(|&k| k == key).map(|i| i + 1)
    }

    /// The firing interval of `key` relative to the moment the class is
    /// entered, with open/closed endpoints preserved. `None` if `key` is not an
    /// enabled instance of this class.
    pub fn firing_interval(&self, key: InstanceKey) -> Option<Interval> {
        let col = self.column_of(key)?;
        // x_0 - x_i <= c  means  x_i >= -c  (lower bound).
        let lower = match self.zone.get(0, col) {
            Bound::Finite { value, strict } => {
                let v = u64::try_from(value.checked_neg()?).ok()?;
                (v, strict)
            }
            Bound::Infinity => return None,
        };
        // x_i - x_0 <= c  means  x_i <= c  (upper bound).
        let upper = match self.zone.get(col, 0) {
            Bound::Finite { value, strict } => UpperBound::Finite {
                value: u64::try_from(value).ok()?,
                open: strict,
            },
            Bound::Infinity => UpperBound::Infinity,
        };
        Some(Interval {
            lower: lower.0,
            lower_open: lower.1,
            upper,
        })
    }
}

impl AsRef<Marking> for StateClass {
    fn as_ref(&self) -> &Marking {
        &self.marking
    }
}
