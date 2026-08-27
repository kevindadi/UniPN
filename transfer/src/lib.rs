//! Converters from external formats into UniPN nets.
//!
//! [`unipn`] itself is the model and the analyses; this crate is the layer that
//! reads somebody else's file and produces one of its nets. It lives in the same
//! workspace so a change to a kind and the converter that depends on it compile
//! together, and it is a *separate* crate so `unipn` does not carry
//! `serde_json` or anyone else's schema.
//!
//! Today there is one converter: [`concir`], which lowers a
//! [ConcIR](https://github.com/kevindadi/ConcIR) program to a
//! [`CvnNet`]. PNML is next.
//!
//! ```no_run
//! let json = std::fs::read_to_string("program.json").unwrap();
//! let lowered = unipn_transfer::cvn_from_concir_json(&json).unwrap();
//! for note in &lowered.report.degraded {
//!     eprintln!("{note}");
//! }
//! ```
//!
//! # What the conversion promises
//!
//! Precision is lost in one direction only: the net admits **at least** the runs
//! the program has. A condition the expression parser cannot read becomes an
//! `Unknown` guard, which the CVN's three-valued evaluation treats as satisfied,
//! so both arms of a branch stay open. Nothing is quietly narrowed, and every
//! place precision was lost is listed in [`LoweringReport`].
//!
//! An operation this crate does not lower yet is a hard
//! [`TransferError::UnsupportedOp`], never a silent skip: dropping a `join` or a
//! `channel_recv` would delete exactly the blocking behavior the analysis exists
//! to find.
//!
//! # Why ConcIR's structs are mirrored rather than imported
//!
//! ConcIR's own `ast` module describes the same JSON, so the obvious move is to
//! depend on it. Three things stand in the way, and none of them is the size of
//! that crate (it is small):
//!
//! 1. ConcIR marks its structs `deny_unknown_fields` — correct for a validator,
//!    wrong for a reader. A field added upstream would turn every conversion
//!    into a hard error while this crate's submodule pin lags behind.
//! 2. A `path` dependency on a git submodule makes this crate unpublishable and
//!    forces the submodule on every consumer. Only the tests need it now.
//! 3. Putting `concir::ast::Program` in the public API would pin the *caller's*
//!    ConcIR revision to ours. Two revisions in one binary are two incompatible
//!    `Program` types; JSON as the only interface makes that impossible.
//!
//! The cost of a mirror is drift, so `tests/schema_sync.rs` keeps `concir` as a
//! **dev-dependency** and checks both ASTs against ConcIR's own example corpus.

pub mod concir;

use std::fmt;

use unipn::cvn::{CvnNet, CvnState};
use unipn::net::{PlaceId, TransitionId};

pub use concir::ast::Program;

/// Knobs for facts a ConcIR program does not state.
#[derive(Clone, Debug)]
pub struct LoweringConfig {
    /// Reader capacity for an `RwLock`. ConcIR has no field for it, so the net
    /// needs a number from somewhere; every use is recorded in
    /// [`LoweringReport::defaulted_rwlock_readers`].
    pub default_max_readers: usize,
}

impl Default for LoweringConfig {
    fn default() -> Self {
        Self {
            default_max_readers: 2,
        }
    }
}

/// Where the lowering had to be less precise than the source.
///
/// This is not decoration. The lowering over-approximates on purpose, and an
/// over-approximation that nobody can see is indistinguishable from a bug: a
/// verifier that reports "no deadlock" over a net whose every guard degraded to
/// `Unknown` has proved nothing.
#[derive(Clone, Debug, Default)]
pub struct LoweringReport {
    /// Expressions and conditions that became `Unknown`.
    pub degraded: Vec<Degraded>,
    /// `RwLock` resources that took [`LoweringConfig::default_max_readers`].
    pub defaulted_rwlock_readers: Vec<String>,
    /// Declarations left out of the variable store because `modeled` is false.
    pub unmodeled_slots: Vec<String>,
}

/// One expression the parser could not read, and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Degraded {
    /// The enclosing function as `module::function`.
    pub scope: String,
    /// The ConcIR statement id.
    pub sid: String,
    /// Which slot degraded: `"expr"`, `"cond"`, or `"abstract_step"`.
    pub role: &'static str,
    /// The source text, verbatim.
    pub source: String,
}

impl fmt::Display for Degraded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{}: {} `{}` became Unknown",
            self.scope, self.sid, self.role, self.source
        )
    }
}

/// A lowered program: the net, the state to start exploring from, and what was
/// lost on the way.
#[derive(Clone, Debug)]
pub struct Lowered {
    pub net: CvnNet,
    pub initial: CvnState,
    pub report: LoweringReport,
}

impl Lowered {
    pub fn place_named(&self, name: &str) -> Option<PlaceId> {
        self.net
            .places
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.id)
    }

    pub fn transition_named(&self, name: &str) -> Option<TransitionId> {
        self.net
            .transitions
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.id)
    }

    /// Every transition lowered from `scope` (a `module::function` FQN), in id
    /// order.
    pub fn transitions_in(&self, scope: &str) -> Vec<TransitionId> {
        self.net
            .transitions
            .iter()
            .filter(|t| t.kind.scope.as_deref() == Some(scope))
            .map(|t| t.id)
            .collect()
    }
}

/// Everything that can stop a conversion.
#[derive(Debug)]
pub enum TransferError {
    /// The JSON did not match ConcIR's shape. An op *kind* this crate has never
    /// heard of lands here too, because a new operation changes what the program
    /// means.
    Json(serde_json::Error),
    /// A recognized operation that is not lowered yet. Distinct from
    /// [`TransferError::Json`] on purpose: the file is fine, this crate is not
    /// finished.
    UnsupportedOp {
        scope: String,
        sid: String,
        kind: &'static str,
    },
    UnknownFunction {
        scope: String,
        sid: String,
        name: String,
    },
    UnknownResource {
        scope: String,
        sid: String,
        name: String,
        expected: &'static str,
    },
    /// A `goto` / `branch` / `switch` naming a sid that its function does not
    /// contain.
    UnknownTarget {
        scope: String,
        sid: String,
        target: String,
    },
    /// `entry` does not name a function of any module.
    UnknownEntry { entry: String },
    /// `scope` with no functions to run (ConcIR E410).
    EmptyScope { scope: String, sid: String },
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "not a ConcIR program: {e}"),
            Self::UnsupportedOp { scope, sid, kind } => {
                write!(f, "{scope}@{sid}: `{kind}` is not lowered yet")
            }
            Self::UnknownFunction { scope, sid, name } => {
                write!(f, "{scope}@{sid}: no function named `{name}`")
            }
            Self::UnknownResource {
                scope,
                sid,
                name,
                expected,
            } => write!(f, "{scope}@{sid}: no {expected} resource named `{name}`"),
            Self::UnknownTarget { scope, sid, target } => {
                write!(f, "{scope}@{sid}: no statement `{target}` in this function")
            }
            Self::UnknownEntry { entry } => write!(f, "entry `{entry}` is not a known function"),
            Self::EmptyScope { scope, sid } => {
                write!(f, "{scope}@{sid}: `scope` lists no functions")
            }
        }
    }
}

impl std::error::Error for TransferError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for TransferError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// Parse a ConcIR program and lower it with the default configuration.
pub fn cvn_from_concir_json(json: &str) -> Result<Lowered, TransferError> {
    let program: Program = serde_json::from_str(json)?;
    cvn_from_concir(&program, &LoweringConfig::default())
}

/// Lower an already-parsed ConcIR program.
pub fn cvn_from_concir(
    program: &Program,
    config: &LoweringConfig,
) -> Result<Lowered, TransferError> {
    concir::lower::lower(program, config)
}
