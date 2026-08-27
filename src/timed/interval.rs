//! Transition time intervals.
//!
//! An interval may have open endpoints; because the clock domain is integral,
//! [`TimeInterval::effective_earliest`] and
//! [`TimeInterval::effective_latest`] fold that openness into closed integer
//! bounds before the DBM ever sees it.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Sentinel for "no upper bound" (+∞) in time intervals and DBM matrices.
pub const INF: i32 = i32::MAX;

/// A time interval with optional open endpoints.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TimeInterval {
    pub earliest: i32,
    pub latest: i32,
    pub left_open: bool,
    pub right_open: bool,
}

impl TimeInterval {
    pub fn new(
        earliest: i32,
        latest: i32,
        left_open: bool,
        right_open: bool,
    ) -> Result<Self, String> {
        if earliest < 0 {
            return Err("earliest time must be non-negative".to_string());
        }
        if latest != INF && latest < earliest {
            return Err("latest time must be >= earliest time".to_string());
        }
        let interval = Self {
            earliest,
            latest,
            left_open,
            right_open,
        };
        if !interval.has_non_empty_integer_domain() {
            return Err("time interval has an empty integer domain".to_string());
        }
        Ok(interval)
    }

    pub fn closed(earliest: i32, latest: i32) -> Self {
        Self {
            earliest,
            latest,
            left_open: false,
            right_open: false,
        }
    }

    pub fn effective_earliest(&self) -> i32 {
        if self.left_open {
            self.earliest + 1
        } else {
            self.earliest
        }
    }

    pub fn effective_latest(&self) -> i32 {
        if self.latest == INF {
            return INF;
        }
        if self.right_open {
            self.latest - 1
        } else {
            self.latest
        }
    }

    pub fn has_non_empty_integer_domain(&self) -> bool {
        self.effective_latest() == INF || self.effective_earliest() <= self.effective_latest()
    }

    pub fn is_valid(&self) -> bool {
        self.earliest >= 0
            && (self.latest == INF || self.latest >= self.earliest)
            && self.has_non_empty_integer_domain()
    }

    pub fn contains(&self, time: i32) -> bool {
        time >= self.effective_earliest()
            && (self.effective_latest() == INF || time <= self.effective_latest())
    }
}

impl fmt::Display for TimeInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}, {}{}",
            if self.left_open { "(" } else { "[" },
            self.earliest,
            if self.latest == INF {
                "∞".to_string()
            } else {
                self.latest.to_string()
            },
            if self.right_open { ")" } else { "]" }
        )
    }
}
