//! Place types for the CVN.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a place in the CVN.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlaceId(pub String);

impl PlaceId {
    /// Create a new place ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for PlaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<S: Into<String>> From<S> for PlaceId {
    fn from(s: S) -> Self {
        Self(s.into())
    }
}

/// The kind of a place, determining its role in the net.
///
/// Places are partitioned into three disjoint sets: control, resource, and wait.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceKind {
    /// Control place: token here means a thread is at a specific statement.
    Control {
        /// Name of the function containing this control point.
        fn_name: String,
        /// Statement ID within the function.
        sid: String,
    },
    /// Resource place: token count represents available resource units.
    Resource {
        /// Name of the resource.
        res_name: String,
        /// Type of the resource.
        resource_type: ResourceType,
    },
    /// Wait place: token here means a thread is blocked at a condvar wait point.
    Wait {
        /// Name of the condition variable.
        cv_name: String,
        /// Name of the function containing this wait point.
        fn_name: String,
        /// Statement ID of the wait call.
        sid: String,
    },
}

/// Type of resource modeled by a resource place.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ResourceType {
    /// Mutual exclusion lock (initial tokens = 1).
    Mutex,
    /// Reader-writer lock (initial tokens = N, where N = number of concurrent entities).
    RwLock {
        /// Maximum concurrent readers (equals total concurrent entity count).
        max_readers: u32,
    },
    /// Counting semaphore (initial tokens = count).
    Semaphore {
        /// Initial permit count.
        count: u32,
    },
    /// Channel (initial tokens = 0).
    Channel,
    /// Condition variable (used with Wait places, not as a resource place itself).
    Condvar,
}

/// A place in the CVN.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    /// Unique identifier for this place.
    pub id: PlaceId,
    /// The kind/role of this place.
    pub kind: PlaceKind,
    /// Whether this is a return/terminal place (threads reaching here have completed).
    pub is_return: bool,
}

impl Place {
    /// Create a new place.
    pub fn new(id: impl Into<PlaceId>, kind: PlaceKind) -> Self {
        Self {
            id: id.into(),
            kind,
            is_return: false,
        }
    }

    /// Mark this place as a return/terminal place.
    pub fn with_return(mut self, is_return: bool) -> Self {
        self.is_return = is_return;
        self
    }

    /// Returns `true` if this is a control place.
    pub fn is_control(&self) -> bool {
        matches!(self.kind, PlaceKind::Control { .. })
    }

    /// Returns `true` if this is a resource place.
    pub fn is_resource(&self) -> bool {
        matches!(self.kind, PlaceKind::Resource { .. })
    }

    /// Returns `true` if this is a wait place.
    pub fn is_wait(&self) -> bool {
        matches!(self.kind, PlaceKind::Wait { .. })
    }

    /// Returns `true` if this is a control or wait place (i.e., a "control-flow" place).
    pub fn is_control_flow(&self) -> bool {
        self.is_control() || self.is_wait()
    }
}
