//! ConcIR's JSON shape, re-declared here and deliberately lenient.
//!
//! These structs mirror [ConcIR](https://github.com/kevindadi/ConcIR)'s own
//! `ast.rs` closely enough to deserialize a program, and nothing more. Two
//! differences are on purpose:
//!
//! - **No `deny_unknown_fields`.** ConcIR uses it; we must not. A field added
//!   upstream should not turn every conversion into a hard error, because this
//!   crate is meant to keep reading programs written for a newer ConcIR than it
//!   was built against.
//! - **No dependency on the ConcIR crate.** The wire format is the contract, not
//!   its Rust types. Depending on the crate would drag its validator and its
//!   error codes in, and pin the two repositories to one revision.
//!
//! An op *kind* this file does not know is still a deserialization error: a new
//! operation changes what a program means, so failing loudly is right. The
//! softer case — a kind we can parse but do not lower yet — is
//! [`TransferError::UnsupportedOp`](crate::TransferError::UnsupportedOp), which
//! is why [`Op::kind_name`] exists.

use std::collections::BTreeMap;

use serde::Deserialize;

fn default_version() -> String {
    "3.4.0".to_owned()
}

fn default_form() -> String {
    "function".to_owned()
}

/// A complete ConcIR program: modules plus one entry FQN (`module::function`).
#[derive(Clone, Debug, Deserialize)]
pub struct Program {
    pub program: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub modules: Vec<Module>,
    pub entry: String,
}

/// One module: its resources, its lock/variable protection table, and its
/// functions.
#[derive(Clone, Debug, Deserialize)]
pub struct Module {
    pub name: String,
    #[serde(default)]
    pub resources: Vec<Resource>,
    #[serde(default)]
    pub protection: Vec<Protection>,
    #[serde(default)]
    pub functions: Vec<Function>,
}

/// A resource declaration. `kind` splits the two worlds this crate cares about:
/// `"sync"` becomes a resource place, `"var"` becomes an entry in the CVN
/// variable store.
#[derive(Clone, Debug, Deserialize)]
pub struct Resource {
    pub name: String,
    pub kind: String,
    #[serde(rename = "type")]
    pub res_type: String,
    #[serde(default)]
    pub mode: Option<String>,
    /// Semaphore permits. Signed because the wire format is: ConcIR accepts a
    /// negative here and rejects it in its own validator (E001).
    #[serde(default)]
    pub count: Option<i64>,
    #[serde(default)]
    pub base: Option<BaseType>,
    #[serde(default)]
    pub init: Option<serde_json::Value>,
    /// Channel only: in-flight payload slots. `0` is a rendezvous.
    #[serde(default)]
    pub capacity: Option<i64>,
}

impl Resource {
    pub fn is_sync(&self) -> bool {
        self.kind == "sync"
    }

    pub fn is_var(&self) -> bool {
        self.kind == "var"
    }

    /// The semaphore permit count as a token count.
    pub fn permits(&self, default: usize) -> usize {
        clamp_count(self.count, default)
    }

    /// The channel's slot count as a token capacity.
    pub fn slots(&self, default: usize) -> usize {
        clamp_count(self.capacity, default)
    }
}

/// A wire count as a token count. A negative value is invalid ConcIR, and
/// reading it as zero is the blocking-safe direction: fewer permits means more
/// waiting, so the net can only report a stall that is not there, never miss
/// one. ConcIR's validator is where a negative gets a proper diagnosis.
fn clamp_count(value: Option<i64>, default: usize) -> usize {
    value.map_or(default, |n| usize::try_from(n).unwrap_or(0))
}

/// A value's type. `"Int"` and friends are primitives; the compound forms
/// (`{"Int": [lo, hi]}`, `{"Enum": [..]}`, …) arrive as an object, and the only
/// one that changes the net is the bounded Int.
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum BaseType {
    Primitive(String),
    Complex(serde_json::Value),
}

impl BaseType {
    /// The `[lo, hi]` of a `{"Int": [lo, hi]}` domain.
    ///
    /// This is the one type detail the CVN needs: a declared domain is what
    /// makes a counter loop finite, since an update leaving it disables the
    /// transition.
    pub fn bounded_int(&self) -> Option<(i64, i64)> {
        let Self::Complex(value) = self else {
            return None;
        };
        let bounds = value.get("Int")?.as_array()?;
        let [lo, hi] = bounds.as_slice() else {
            return None;
        };
        Some((lo.as_i64()?, hi.as_i64()?))
    }
}

/// Which lock protects which shared variable.
#[derive(Clone, Debug, Deserialize)]
pub struct Protection {
    pub var: String,
    pub lock: String,
}

/// A function parameter. `modeled` decides whether the value reaches the CVN
/// variable store at all; unmodeled declarations are codegen placeholders.
#[derive(Clone, Debug, Deserialize)]
pub struct ParamDecl {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: BaseType,
    #[serde(default)]
    pub modeled: bool,
}

/// A function local: the same projection flag, plus an optional initializer.
#[derive(Clone, Debug, Deserialize)]
pub struct LocalDecl {
    pub name: String,
    #[serde(rename = "type")]
    pub local_type: BaseType,
    #[serde(default)]
    pub modeled: bool,
    #[serde(default)]
    pub init: Option<serde_json::Value>,
}

/// A function. A thread is one of these too — `spawn` and `scope` target
/// ordinary functions, which is why the CVN has no thread-specific place kind.
#[derive(Clone, Debug, Deserialize)]
pub struct Function {
    pub name: String,
    /// Body / execution: `"normal"` or `"async"`. Required upstream; defaulted
    /// here, because a missing field should not stop us reading the rest.
    #[serde(default)]
    pub kind: String,
    #[serde(default = "default_form")]
    pub form: String,
    #[serde(default)]
    pub params: Vec<ParamDecl>,
    #[serde(default)]
    pub returns: Option<ParamDecl>,
    #[serde(default)]
    pub locals: Vec<LocalDecl>,
    /// Statement list. A non-control statement falls through to the next entry.
    #[serde(default)]
    pub body: Vec<Stmt>,
}

impl Function {
    /// The declarations that enter the variable store, in declaration order.
    pub fn modeled_slots(&self) -> impl Iterator<Item = &str> {
        let params = self.params.iter().filter(|p| p.modeled).map(|p| &*p.name);
        let locals = self.locals.iter().filter(|l| l.modeled).map(|l| &*l.name);
        params.chain(locals)
    }
}

/// One CFG node: a `sid` plus the flattened, `kind`-tagged operation.
#[derive(Clone, Debug, Deserialize)]
pub struct Stmt {
    pub sid: String,
    #[serde(flatten)]
    pub op: Op,
}

/// A ConcIR operation. Control transfer lives here too; there is no separate
/// terminator.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum Op {
    #[serde(rename = "nop")]
    Nop,
    #[serde(rename = "assign_local")]
    AssignLocal { target: String, expr: String },
    #[serde(rename = "read_shared")]
    ReadShared {
        resource: String,
        #[serde(default)]
        dst: Option<String>,
    },
    #[serde(rename = "write_shared")]
    WriteShared { resource: String, expr: String },
    #[serde(rename = "abstract_step")]
    AbstractStep {
        #[serde(default)]
        reads: Vec<String>,
        #[serde(default)]
        writes: Vec<String>,
        #[serde(default)]
        desc: String,
    },
    #[serde(rename = "atomic_load")]
    AtomicLoad { resource: String, dst: String },
    #[serde(rename = "atomic_store")]
    AtomicStore { resource: String, value: String },
    #[serde(rename = "atomic_cas")]
    AtomicCas {
        resource: String,
        expected: String,
        desired: String,
        dst: String,
    },
    #[serde(rename = "mutex_lock")]
    MutexLock { resource: String },
    #[serde(rename = "mutex_unlock")]
    MutexUnlock { resource: String },
    #[serde(rename = "rwlock_read")]
    RwLockRead { resource: String },
    #[serde(rename = "rwlock_write")]
    RwLockWrite { resource: String },
    #[serde(rename = "rwlock_unlock")]
    RwLockUnlock { resource: String },
    #[serde(rename = "channel_send")]
    ChannelSend { channel: String, value: String },
    #[serde(rename = "channel_recv")]
    ChannelRecv { channel: String, dst: String },
    #[serde(rename = "condvar_wait")]
    CondvarWait { condvar: String, lock: String },
    #[serde(rename = "condvar_notify")]
    CondvarNotify { condvar: String },
    #[serde(rename = "condvar_notify_all")]
    CondvarNotifyAll { condvar: String },
    #[serde(rename = "semaphore_acquire")]
    SemaphoreAcquire {
        resource: String,
        #[serde(default)]
        count: Option<usize>,
    },
    #[serde(rename = "semaphore_release")]
    SemaphoreRelease {
        resource: String,
        #[serde(default)]
        count: Option<usize>,
    },
    #[serde(rename = "call")]
    Call {
        func: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        dst: Option<String>,
    },
    #[serde(rename = "spawn")]
    Spawn {
        func: String,
        #[serde(default)]
        args: Vec<String>,
        handle: String,
    },
    /// Run every function in `funcs` together and join them all before falling
    /// through — one `thread::scope`.
    #[serde(rename = "scope")]
    Scope { funcs: Vec<String> },
    #[serde(rename = "join")]
    Join { handle: String },
    #[serde(rename = "async_call")]
    AsyncCall {
        func: String,
        #[serde(default)]
        args: Vec<String>,
        handle: String,
    },
    #[serde(rename = "await")]
    Await { handle: String },
    #[serde(rename = "goto")]
    Goto { target: String },
    #[serde(rename = "branch")]
    Branch {
        cond: String,
        then: String,
        #[serde(rename = "else")]
        else_target: String,
    },
    #[serde(rename = "switch")]
    Switch {
        var: String,
        cases: BTreeMap<String, String>,
        default: String,
    },
    #[serde(rename = "return")]
    Return {
        #[serde(default)]
        value: Option<String>,
    },
    #[serde(rename = "select")]
    Select {
        branches: Vec<SelectBranch>,
        #[serde(default)]
        default: Option<String>,
    },
}

impl Op {
    /// The ConcIR `kind` string, for diagnostics.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Nop => "nop",
            Self::AssignLocal { .. } => "assign_local",
            Self::ReadShared { .. } => "read_shared",
            Self::WriteShared { .. } => "write_shared",
            Self::AbstractStep { .. } => "abstract_step",
            Self::AtomicLoad { .. } => "atomic_load",
            Self::AtomicStore { .. } => "atomic_store",
            Self::AtomicCas { .. } => "atomic_cas",
            Self::MutexLock { .. } => "mutex_lock",
            Self::MutexUnlock { .. } => "mutex_unlock",
            Self::RwLockRead { .. } => "rwlock_read",
            Self::RwLockWrite { .. } => "rwlock_write",
            Self::RwLockUnlock { .. } => "rwlock_unlock",
            Self::ChannelSend { .. } => "channel_send",
            Self::ChannelRecv { .. } => "channel_recv",
            Self::CondvarWait { .. } => "condvar_wait",
            Self::CondvarNotify { .. } => "condvar_notify",
            Self::CondvarNotifyAll { .. } => "condvar_notify_all",
            Self::SemaphoreAcquire { .. } => "semaphore_acquire",
            Self::SemaphoreRelease { .. } => "semaphore_release",
            Self::Call { .. } => "call",
            Self::Spawn { .. } => "spawn",
            Self::Scope { .. } => "scope",
            Self::Join { .. } => "join",
            Self::AsyncCall { .. } => "async_call",
            Self::Await { .. } => "await",
            Self::Goto { .. } => "goto",
            Self::Branch { .. } => "branch",
            Self::Switch { .. } => "switch",
            Self::Return { .. } => "return",
            Self::Select { .. } => "select",
        }
    }
}

/// One arm of a `select`: a blocking guard and where control goes when it fires.
#[derive(Clone, Debug, Deserialize)]
pub struct SelectBranch {
    pub guard: SelectGuard,
    pub target: String,
}

/// The blocking operations legal as `select` guards. Each uses the same tagged
/// JSON object as the corresponding [`Op`].
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum SelectGuard {
    #[serde(rename = "channel_recv")]
    ChannelRecv { channel: String, dst: String },
    #[serde(rename = "condvar_wait")]
    CondvarWait { condvar: String, lock: String },
    #[serde(rename = "semaphore_acquire")]
    SemaphoreAcquire { resource: String },
}

impl Program {
    pub fn module(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.name == name)
    }

    /// Resolve a function named from inside `from_module`, accepting a bare name
    /// or a `module::function` FQN.
    pub fn function(&self, from_module: &str, name: &str) -> Option<(&Module, &Function)> {
        let (module, entity) = split_fqn(from_module, name);
        let module = self.module(module)?;
        let function = module.functions.iter().find(|f| f.name == entity)?;
        Some((module, function))
    }

    /// Resolve a resource named from inside `from_module`, same naming rules.
    pub fn resource(&self, from_module: &str, name: &str) -> Option<(&Module, &Resource)> {
        let (module, entity) = split_fqn(from_module, name);
        let module = self.module(module)?;
        let resource = module.resources.iter().find(|r| r.name == entity)?;
        Some((module, resource))
    }
}

/// Split `module::entity`, defaulting the module to `from_module`.
pub fn split_fqn<'a>(from_module: &'a str, name: &'a str) -> (&'a str, &'a str) {
    match name.split_once("::") {
        Some((module, entity)) => (module, entity),
        None => (from_module, name),
    }
}
