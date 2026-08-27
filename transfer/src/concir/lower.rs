//! Walking a ConcIR program and building the CVN.
//!
//! # The shape of the net
//!
//! One control place per ConcIR statement, holding the token *before* that
//! statement runs, plus a `FunctionStart` and a `FunctionEnd` per function. A
//! statement becomes one transition from its own place to its successor's — or
//! several, when one ConcIR operation is several net events (a `branch` is two
//! transitions, a `condvar_wait` is three).
//!
//! Threads need no machinery of their own: `spawn` and `scope` target ordinary
//! functions, so "start a thread" is a token on a `FunctionStart` and "the
//! thread ended" is a token on the matching `FunctionEnd`. That is the whole
//! reason [`ControlSub`] has no `ThreadEnd`.
//!
//! # Variable names carry the scoping
//!
//! The CVN store is one flat map, so the lowering qualifies every name it puts
//! there: a shared variable is `module::name`, a function slot is
//! `module::function::name`. Expressions are rewritten to match, and a `return`
//! drops that function's slots via [`CvnArcKind::DropVars`] — a dead local left
//! behind would keep splitting states that are otherwise identical.
//!
//! # Where it is deliberately approximate
//!
//! - **One net per function, not per invocation.** Two live invocations of the
//!   same function share its places, so their tokens merge. Fine for `scope`
//!   over distinct functions; a recursive or repeatedly-spawned function is
//!   modelled as though the instances were one.
//! - **A condvar is a waiter count plus a signal count.** `notify_all` cannot
//!   release *all* waiters with a fixed arc weight, so it releases one. With
//!   several waiters on one condvar that under-approximates wakeups and can
//!   report a wait that the real program would have woken.
//! - **A notification is only produced when somebody is waiting** (a read arc on
//!   the waiter count), and lost otherwise. Both outcomes are explored, which is
//!   what makes a missed notification visible instead of assumed away.

use std::collections::{BTreeMap, BTreeSet};

use unipn::cvn::expr::{BoolExpr, Expr, Val, VarUpdate};
use unipn::cvn::{
    ControlSub, CvnArcKind, CvnBuilder, CvnTransition, PlaceKind, ResourceType, TransitionKind,
};
use unipn::net::{ArcDir, PlaceId, TransitionId};

use super::ast::{BaseType, Function, Module, Op, Program, Resource, split_fqn};
use super::expr::{parse_expr, parse_guard};
use crate::{Degraded, Lowered, LoweringConfig, LoweringReport, TransferError};

/// Lower a whole program.
pub fn lower(program: &Program, config: &LoweringConfig) -> Result<Lowered, TransferError> {
    let mut lowering = Lowering {
        program,
        config,
        builder: CvnBuilder::new(),
        report: LoweringReport::default(),
        sync: BTreeMap::new(),
        functions: BTreeMap::new(),
    };

    lowering.declare_resources();
    lowering.declare_functions();
    lowering.lower_bodies()?;
    lowering.place_entry_token()?;

    let (net, initial) = lowering.builder.build();
    Ok(Lowered {
        net,
        initial,
        report: lowering.report,
    })
}

/// The places a synchronization resource turns into.
#[derive(Clone, Copy, Debug)]
enum Sync {
    /// A mutex, an rwlock, a semaphore, or a channel: one place whose tokens are
    /// the free permits (or the buffered messages).
    Counted(PlaceId),
    /// A condvar needs two counts, because one place cannot say both "somebody
    /// is waiting" and "a notification is pending".
    Condvar { waiters: PlaceId, signals: PlaceId },
}

impl Sync {
    fn counted(&self) -> Option<PlaceId> {
        match self {
            Self::Counted(place) => Some(*place),
            Self::Condvar { .. } => None,
        }
    }

    fn condvar(&self) -> Option<(PlaceId, PlaceId)> {
        match self {
            Self::Condvar { waiters, signals } => Some((*waiters, *signals)),
            Self::Counted(_) => None,
        }
    }
}

/// The places of one lowered function.
struct FnPlaces {
    start: PlaceId,
    end: PlaceId,
    /// Statement sid → the place holding the token before that statement.
    stmts: BTreeMap<String, PlaceId>,
    /// Unqualified names of the `modeled` params and locals.
    slots: BTreeSet<String>,
}

struct Lowering<'a> {
    program: &'a Program,
    config: &'a LoweringConfig,
    builder: CvnBuilder,
    report: LoweringReport,
    /// `module::resource` → its places.
    sync: BTreeMap<String, Sync>,
    /// `module::function` → its places.
    functions: BTreeMap<String, FnPlaces>,
}

/// One statement's context: enough to name things and to report errors against
/// the right source location.
struct At<'a> {
    module: &'a str,
    scope: String,
    sid: &'a str,
}

impl At<'_> {
    /// The name of the statement's own place, and the prefix of its transitions.
    fn label(&self) -> String {
        format!("{}@{}", self.scope, self.sid)
    }
}

impl<'a> Lowering<'a> {
    // ── Declarations ──

    /// Split the resources: `sync` becomes places, `var` becomes variables.
    ///
    /// This is the one place the two halves of ConcIR's resource list diverge.
    /// A lock is a token — its whole behavior is "somebody holds it or somebody
    /// does not". A shared variable is a value, and putting values in tokens is
    /// what makes a colored net's state space explode, so it goes in the store.
    fn declare_resources(&mut self) {
        for module in &self.program.modules {
            for resource in &module.resources {
                if resource.is_sync() {
                    self.declare_sync(&module.name, resource);
                } else if resource.is_var() {
                    self.declare_var(&module.name, resource);
                }
            }
        }
    }

    fn declare_sync(&mut self, module: &str, resource: &Resource) {
        let name = fq(module, &resource.name);
        let places = match resource.res_type.as_str() {
            "Condvar" => {
                let waiters = self.builder.add_place(
                    format!("{name}@waiters"),
                    PlaceKind::Resource(ResourceType::Condvar),
                );
                let signals = self.builder.add_place(
                    format!("{name}@signals"),
                    PlaceKind::Resource(ResourceType::Condvar),
                );
                Sync::Condvar { waiters, signals }
            }
            "Semaphore" => {
                let count = resource.permits(1);
                let place = self.builder.add_marked_place(
                    &name,
                    PlaceKind::Resource(ResourceType::Semaphore { count }),
                    count,
                );
                Sync::Counted(place)
            }
            "Channel" => {
                // The place holds messages, not permits, so it starts empty.
                let capacity = resource.slots(0);
                let place = self.builder.add_place(
                    &name,
                    PlaceKind::Resource(ResourceType::Channel { capacity }),
                );
                Sync::Counted(place)
            }
            "RwLock" => {
                let max_readers = self.config.default_max_readers;
                self.report.defaulted_rwlock_readers.push(name.clone());
                let place = self.builder.add_marked_place(
                    &name,
                    PlaceKind::Resource(ResourceType::RwLock { max_readers }),
                    max_readers,
                );
                Sync::Counted(place)
            }
            // Mutex, and anything else that behaves like one permit.
            _ => {
                let place = self.builder.add_marked_place(
                    &name,
                    PlaceKind::Resource(ResourceType::Mutex),
                    1,
                );
                Sync::Counted(place)
            }
        };
        self.sync.insert(name, places);
    }

    fn declare_var(&mut self, module: &str, resource: &Resource) {
        let name = fq(module, &resource.name);
        let initial = resource.init.as_ref().map_or(Val::Unknown, json_to_val);
        self.builder.add_variable(&name, initial);
        if let Some((lo, hi)) = resource.base.as_ref().and_then(BaseType::bounded_int) {
            self.builder.set_variable_domain(&name, lo, hi);
        }
    }

    /// Create every function's places up front, so a `goto` or a `spawn` can
    /// point at a function or a statement the walk has not reached yet.
    fn declare_functions(&mut self) {
        for module in &self.program.modules {
            for function in &module.functions {
                let scope = fq(&module.name, &function.name);
                let start = self.builder.add_place(
                    format!("{scope}@start"),
                    PlaceKind::Control(ControlSub::FunctionStart),
                );
                let end = self.builder.add_place(
                    format!("{scope}@end"),
                    PlaceKind::Control(ControlSub::FunctionEnd),
                );

                let mut stmts = BTreeMap::new();
                for stmt in &function.body {
                    let place = self.builder.add_place(
                        format!("{scope}@{}", stmt.sid),
                        PlaceKind::Control(ControlSub::BasicBlock),
                    );
                    stmts.insert(stmt.sid.clone(), place);
                }

                let slots = function.modeled_slots().map(str::to_owned).collect();
                self.declare_slots(&scope, function);

                self.functions.insert(
                    scope,
                    FnPlaces {
                        start,
                        end,
                        stmts,
                        slots,
                    },
                );
            }
        }
    }

    /// Put the function's modeled params and locals in the store.
    ///
    /// They are declared once, qualified by function, and dropped again on
    /// `return`. A slot is therefore present before its function runs, which
    /// costs nothing: it holds its declared initial value (or `Unknown`), and no
    /// transition reads it until control is inside the function.
    fn declare_slots(&mut self, scope: &str, function: &Function) {
        for param in &function.params {
            if param.modeled {
                self.builder
                    .add_variable(fq(scope, &param.name), Val::Unknown);
            } else {
                self.report.unmodeled_slots.push(fq(scope, &param.name));
            }
        }
        for local in &function.locals {
            if !local.modeled {
                self.report.unmodeled_slots.push(fq(scope, &local.name));
                continue;
            }
            let name = fq(scope, &local.name);
            let initial = local.init.as_ref().map_or(Val::Unknown, json_to_val);
            self.builder.add_variable(&name, initial);
            if let Some((lo, hi)) = local.local_type.bounded_int() {
                self.builder.set_variable_domain(&name, lo, hi);
            }
        }
    }

    fn place_entry_token(&mut self) -> Result<(), TransferError> {
        let (module, name) = split_fqn("", &self.program.entry);
        let scope = fq(module, name);
        let start = self
            .functions
            .get(&scope)
            .ok_or_else(|| TransferError::UnknownEntry {
                entry: self.program.entry.clone(),
            })?
            .start;
        self.builder.set_initial_tokens(start, 1);
        Ok(())
    }

    // ── Bodies ──

    fn lower_bodies(&mut self) -> Result<(), TransferError> {
        let program = self.program;
        for module in &program.modules {
            for function in &module.functions {
                self.lower_function(module, function)?;
            }
        }
        Ok(())
    }

    fn lower_function(
        &mut self,
        module: &Module,
        function: &Function,
    ) -> Result<(), TransferError> {
        let scope = fq(&module.name, &function.name);
        let places = &self.functions[&scope];
        let (start, end) = (places.start, places.end);

        // Entering is its own transition, so a spawned thread's start is an
        // event the explorer can interleave rather than something that has
        // already happened.
        let first = function
            .body
            .first()
            .map_or(end, |stmt| self.functions[&scope].stmts[&stmt.sid]);
        let enter = self.transition(
            format!("{scope}@enter"),
            TransitionKind::FunctionEnter,
            &scope,
            "enter",
        );
        self.builder.add_input_arc(start, enter, 1, BoolExpr::True);
        self.builder.add_output_arc(enter, first, 1, None);

        for (index, stmt) in function.body.iter().enumerate() {
            let at = At {
                module: &module.name,
                scope: scope.clone(),
                sid: &stmt.sid,
            };
            // Fallthrough: a non-control statement continues at the next entry,
            // and a body that runs off the end lands on the function's exit.
            let next = match function.body.get(index + 1) {
                Some(following) => self.functions[&scope].stmts[&following.sid],
                None => end,
            };
            self.lower_stmt(&at, &stmt.op, next)?;
        }

        Ok(())
    }

    fn lower_stmt(&mut self, at: &At, op: &Op, next: PlaceId) -> Result<(), TransferError> {
        let pre = self.functions[&at.scope].stmts[at.sid];

        match op {
            Op::Nop => {
                self.step(at, "", TransitionKind::Sequential, pre, next, None);
            }

            Op::AssignLocal { target, expr } => {
                let value = self.value(at, expr);
                let update = self.single_update(at, target, value);
                self.step(at, "", TransitionKind::Sequential, pre, next, Some(update));
            }

            Op::ReadShared { resource, dst } => {
                let source = self.var_ref(at, resource)?;
                let update = dst
                    .as_ref()
                    .map(|dst| self.single_update(at, dst, Expr::Ref(source)));
                self.step(at, "", TransitionKind::Sequential, pre, next, update);
            }

            Op::WriteShared { resource, expr } => {
                let target = self.var_ref(at, resource)?;
                let value = self.value(at, expr);
                let update = VarUpdate::from([(target, value)]);
                self.step(at, "", TransitionKind::Sequential, pre, next, Some(update));
            }

            Op::AbstractStep { writes, desc, .. } => {
                // An opaque step says what it touches, not what it computes, so
                // every written slot becomes Unknown.
                let mut update = VarUpdate::new();
                for write in writes {
                    let name = self.var_ref(at, write).unwrap_or_else(|_| write.clone());
                    update.insert(name, Expr::Lit(Val::Unknown));
                }
                if !writes.is_empty() {
                    self.report.degraded.push(Degraded {
                        scope: at.scope.clone(),
                        sid: at.sid.to_owned(),
                        role: "abstract_step",
                        source: desc.clone(),
                    });
                }
                self.step(at, "", TransitionKind::Sequential, pre, next, Some(update));
            }

            Op::MutexLock { resource } => {
                let lock = self.counted_place(at, resource)?;
                let t = self.step(at, "", TransitionKind::Lock, pre, next, None);
                self.builder.add_input_arc(lock, t, 1, BoolExpr::True);
            }

            Op::MutexUnlock { resource } => {
                let lock = self.counted_place(at, resource)?;
                let t = self.step(at, "", TransitionKind::Unlock, pre, next, None);
                self.builder.add_output_arc(t, lock, 1, None);
            }

            Op::CondvarWait { condvar, lock } => {
                self.lower_condvar_wait(at, condvar, lock, pre, next)?;
            }

            Op::CondvarNotify { condvar } => {
                self.lower_notify(at, condvar, pre, next, false)?;
            }

            Op::CondvarNotifyAll { condvar } => {
                self.lower_notify(at, condvar, pre, next, true)?;
            }

            Op::Goto { target } => {
                let target = self.stmt_place(at, target)?;
                self.step(at, "", TransitionKind::Goto, pre, target, None);
            }

            Op::Branch {
                cond,
                then,
                else_target,
            } => {
                let guard = self.condition(at, cond);
                let then_place = self.stmt_place(at, then)?;
                let else_place = self.stmt_place(at, else_target)?;

                let t = self.transition(
                    format!("{}#true", at.label()),
                    TransitionKind::BranchTrue,
                    &at.scope,
                    at.sid,
                );
                self.builder.add_input_arc(pre, t, 1, guard.clone());
                self.builder.add_output_arc(t, then_place, 1, None);

                let f = self.transition(
                    format!("{}#false", at.label()),
                    TransitionKind::BranchFalse,
                    &at.scope,
                    at.sid,
                );
                self.builder
                    .add_input_arc(pre, f, 1, BoolExpr::Not(Box::new(guard)));
                self.builder.add_output_arc(f, else_place, 1, None);
            }

            Op::Return { .. } => {
                let end = self.functions[&at.scope].end;
                let t = self.transition(at.label(), TransitionKind::Return, &at.scope, at.sid);
                self.builder.add_input_arc(pre, t, 1, BoolExpr::True);

                // The function's slots die here. Leaving them in the store would
                // make two runs that differ only in a dead local look like two
                // different states.
                let dying: Vec<String> = self.functions[&at.scope]
                    .slots
                    .iter()
                    .map(|slot| fq(&at.scope, slot))
                    .collect();
                if dying.is_empty() {
                    self.builder.add_output_arc(t, end, 1, None);
                } else {
                    self.builder.add_scope_end_arc(t, end, 1, dying);
                }
            }

            Op::Scope { funcs } => {
                self.lower_scope(at, funcs, pre, next)?;
            }

            _ => {
                return Err(TransferError::UnsupportedOp {
                    scope: at.scope.clone(),
                    sid: at.sid.to_owned(),
                    kind: op.kind_name(),
                });
            }
        }

        Ok(())
    }

    /// `condvar_wait`: release the lock, register as a waiter, and take the lock
    /// back after a notification.
    ///
    /// Three transitions, because those are three separately observable events:
    /// a thread that has released the lock but not yet been notified is in a
    /// different situation from one that has been notified but cannot get the
    /// lock back.
    fn lower_condvar_wait(
        &mut self,
        at: &At,
        condvar: &str,
        lock: &str,
        pre: PlaceId,
        next: PlaceId,
    ) -> Result<(), TransferError> {
        let (waiters, signals) = self.condvar_places(at, condvar)?;
        let lock = self.counted_place(at, lock)?;

        let waiting = self.builder.add_place(
            format!("{}:waiting", at.label()),
            PlaceKind::Control(ControlSub::WaitPoint),
        );
        let holding = self.builder.add_place(
            format!("{}:reacquire", at.label()),
            PlaceKind::Control(ControlSub::BasicBlock),
        );

        let enter = self.transition(
            format!("{}#enter", at.label()),
            TransitionKind::CondvarWaitEnter,
            &at.scope,
            at.sid,
        );
        self.builder.add_input_arc(pre, enter, 1, BoolExpr::True);
        self.builder.add_output_arc(enter, waiting, 1, None);
        self.builder.add_output_arc(enter, waiters, 1, None);
        self.builder.add_output_arc(enter, lock, 1, None);

        // The net keeps one signal count, so it cannot tell a `notify` from a
        // `notify_all`; waking is one transition either way.
        let wake = self.transition(
            format!("{}#wake", at.label()),
            TransitionKind::CondvarWakeByNotify,
            &at.scope,
            at.sid,
        );
        self.builder.add_input_arc(waiting, wake, 1, BoolExpr::True);
        self.builder.add_input_arc(signals, wake, 1, BoolExpr::True);
        self.builder.add_input_arc(waiters, wake, 1, BoolExpr::True);
        self.builder.add_output_arc(wake, holding, 1, None);

        let reacquire = self.transition(
            format!("{}#reacquire", at.label()),
            TransitionKind::CondvarReacquire,
            &at.scope,
            at.sid,
        );
        self.builder
            .add_input_arc(holding, reacquire, 1, BoolExpr::True);
        self.builder
            .add_input_arc(lock, reacquire, 1, BoolExpr::True);
        self.builder.add_output_arc(reacquire, next, 1, None);

        Ok(())
    }

    /// `condvar_notify` / `condvar_notify_all`: two transitions, one for a
    /// notification that reaches a waiter and one for a notification that had
    /// nobody to reach.
    ///
    /// The read arc on the waiter count is what keeps a signal from outliving
    /// its waiter; the inhibitor arc on the lost variant is its exact
    /// complement, so control always continues either way.
    fn lower_notify(
        &mut self,
        at: &At,
        condvar: &str,
        pre: PlaceId,
        next: PlaceId,
        all: bool,
    ) -> Result<(), TransferError> {
        let (waiters, signals) = self.condvar_places(at, condvar)?;
        let (delivered, lost) = if all {
            (
                TransitionKind::CondvarNotifyAll,
                TransitionKind::CondvarNotifyAllLost,
            )
        } else {
            (
                TransitionKind::CondvarNotify,
                TransitionKind::CondvarNotifyLost,
            )
        };

        let t = self.step(at, "", delivered, pre, next, None);
        self.builder
            .add_arc(waiters, t, ArcDir::Read, 1, CvnArcKind::Plain);
        self.builder.add_output_arc(t, signals, 1, None);

        let l = self.step(at, "#lost", lost, pre, next, None);
        self.builder
            .add_arc(waiters, l, ArcDir::Inhibitor, 1, CvnArcKind::Plain);

        Ok(())
    }

    /// `scope`: run every listed function and join them all.
    ///
    /// One fan-out transition and one fan-in transition, which is exactly
    /// `thread::scope` plus `join_all`. Nothing is lost by starting the threads
    /// in a single step: entering a function is its own transition, so the
    /// explorer still interleaves "one thread runs before the other begins".
    fn lower_scope(
        &mut self,
        at: &At,
        funcs: &[String],
        pre: PlaceId,
        next: PlaceId,
    ) -> Result<(), TransferError> {
        if funcs.is_empty() {
            return Err(TransferError::EmptyScope {
                scope: at.scope.clone(),
                sid: at.sid.to_owned(),
            });
        }

        let mut spawned = Vec::with_capacity(funcs.len());
        for name in funcs {
            let (module, function) = self.program.function(at.module, name).ok_or_else(|| {
                TransferError::UnknownFunction {
                    scope: at.scope.clone(),
                    sid: at.sid.to_owned(),
                    name: name.clone(),
                }
            })?;
            let places = &self.functions[&fq(&module.name, &function.name)];
            spawned.push((places.start, places.end));
        }

        let joining = self.builder.add_place(
            format!("{}:joining", at.label()),
            PlaceKind::Control(ControlSub::BasicBlock),
        );

        let spawn = self.transition(
            format!("{}#spawn", at.label()),
            TransitionKind::Spawn,
            &at.scope,
            at.sid,
        );
        self.builder.add_input_arc(pre, spawn, 1, BoolExpr::True);
        self.builder.add_output_arc(spawn, joining, 1, None);
        for (start, _) in &spawned {
            self.builder.add_output_arc(spawn, *start, 1, None);
        }

        let join = self.transition(
            format!("{}#join", at.label()),
            TransitionKind::Join,
            &at.scope,
            at.sid,
        );
        self.builder.add_input_arc(joining, join, 1, BoolExpr::True);
        for (_, end) in &spawned {
            self.builder.add_input_arc(*end, join, 1, BoolExpr::True);
        }
        self.builder.add_output_arc(join, next, 1, None);

        Ok(())
    }

    // ── Building blocks ──

    /// A single transition from `pre` to `next`, optionally updating variables.
    fn step(
        &mut self,
        at: &At,
        suffix: &str,
        kind: TransitionKind,
        pre: PlaceId,
        next: PlaceId,
        update: Option<VarUpdate>,
    ) -> TransitionId {
        let name = format!("{}{suffix}", at.label());
        let t = self.transition(name, kind, &at.scope, at.sid);
        self.builder.add_input_arc(pre, t, 1, BoolExpr::True);
        self.builder.add_output_arc(t, next, 1, update);
        t
    }

    fn transition(
        &mut self,
        name: String,
        kind: TransitionKind,
        scope: &str,
        sid: &str,
    ) -> TransitionId {
        let t = self.builder.add_transition(name, CvnTransition::new(kind));
        self.builder.set_scope(t, scope);
        self.builder.set_anchor(t, sid);
        t
    }

    // ── Name resolution ──

    fn stmt_place(&self, at: &At, sid: &str) -> Result<PlaceId, TransferError> {
        self.functions[&at.scope]
            .stmts
            .get(sid)
            .copied()
            .ok_or_else(|| TransferError::UnknownTarget {
                scope: at.scope.clone(),
                sid: at.sid.to_owned(),
                target: sid.to_owned(),
            })
    }

    fn counted_place(&self, at: &At, name: &str) -> Result<PlaceId, TransferError> {
        self.sync_places(at, name, "counted")?
            .counted()
            .ok_or_else(|| TransferError::UnknownResource {
                scope: at.scope.clone(),
                sid: at.sid.to_owned(),
                name: name.to_owned(),
                expected: "counted",
            })
    }

    fn condvar_places(&self, at: &At, name: &str) -> Result<(PlaceId, PlaceId), TransferError> {
        self.sync_places(at, name, "condvar")?
            .condvar()
            .ok_or_else(|| TransferError::UnknownResource {
                scope: at.scope.clone(),
                sid: at.sid.to_owned(),
                name: name.to_owned(),
                expected: "condvar",
            })
    }

    fn sync_places(
        &self,
        at: &At,
        name: &str,
        expected: &'static str,
    ) -> Result<Sync, TransferError> {
        let (module, resource) = self.program.resource(at.module, name).ok_or_else(|| {
            TransferError::UnknownResource {
                scope: at.scope.clone(),
                sid: at.sid.to_owned(),
                name: name.to_owned(),
                expected,
            }
        })?;
        self.sync
            .get(&fq(&module.name, &resource.name))
            .copied()
            .ok_or_else(|| TransferError::UnknownResource {
                scope: at.scope.clone(),
                sid: at.sid.to_owned(),
                name: name.to_owned(),
                expected,
            })
    }

    /// The store key of a shared variable resource.
    fn var_ref(&self, at: &At, name: &str) -> Result<String, TransferError> {
        let (module, resource) = self.program.resource(at.module, name).ok_or_else(|| {
            TransferError::UnknownResource {
                scope: at.scope.clone(),
                sid: at.sid.to_owned(),
                name: name.to_owned(),
                expected: "var",
            }
        })?;
        Ok(fq(&module.name, &resource.name))
    }

    /// The store key a bare ConcIR name refers to at this statement: a function
    /// slot if the function declares one, else a shared variable, else the name
    /// as written (which evaluates to `Unknown`, the safe answer).
    fn store_key(&self, at: &At, name: &str) -> String {
        if self.functions[&at.scope].slots.contains(name) {
            return fq(&at.scope, name);
        }
        match self.program.resource(at.module, name) {
            Some((module, resource)) => fq(&module.name, &resource.name),
            None => name.to_owned(),
        }
    }

    fn single_update(&mut self, at: &At, target: &str, value: Expr) -> VarUpdate {
        VarUpdate::from([(self.store_key(at, target), value)])
    }

    // ── Expressions ──

    fn value(&mut self, at: &At, source: &str) -> Expr {
        let parsed = parse_expr(source);
        if parsed.degraded {
            self.note_degraded(at, "expr", source);
        }
        self.rebind_expr(at, parsed.tree)
    }

    fn condition(&mut self, at: &At, source: &str) -> BoolExpr {
        let parsed = parse_guard(source);
        if parsed.degraded {
            self.note_degraded(at, "cond", source);
        }
        self.rebind_guard(at, parsed.tree)
    }

    fn note_degraded(&mut self, at: &At, role: &'static str, source: &str) {
        self.report.degraded.push(Degraded {
            scope: at.scope.clone(),
            sid: at.sid.to_owned(),
            role,
            source: source.to_owned(),
        });
    }

    /// Rewrite every reference to the qualified name it has in the flat store.
    fn rebind_expr(&self, at: &At, expr: Expr) -> Expr {
        match expr {
            Expr::Ref(name) => Expr::Ref(self.store_key(at, &name)),
            Expr::BinOp { op, lhs, rhs } => Expr::BinOp {
                op,
                lhs: Box::new(self.rebind_expr(at, *lhs)),
                rhs: Box::new(self.rebind_expr(at, *rhs)),
            },
            Expr::Lit(_) => expr,
        }
    }

    fn rebind_guard(&self, at: &At, guard: BoolExpr) -> BoolExpr {
        match guard {
            BoolExpr::Cmp { op, lhs, rhs } => BoolExpr::Cmp {
                op,
                lhs: Box::new(self.rebind_expr(at, *lhs)),
                rhs: Box::new(self.rebind_expr(at, *rhs)),
            },
            BoolExpr::And(a, b) => BoolExpr::And(
                Box::new(self.rebind_guard(at, *a)),
                Box::new(self.rebind_guard(at, *b)),
            ),
            BoolExpr::Or(a, b) => BoolExpr::Or(
                Box::new(self.rebind_guard(at, *a)),
                Box::new(self.rebind_guard(at, *b)),
            ),
            BoolExpr::Not(inner) => BoolExpr::Not(Box::new(self.rebind_guard(at, *inner))),
            BoolExpr::True => BoolExpr::True,
        }
    }
}

fn fq(prefix: &str, name: &str) -> String {
    format!("{prefix}::{name}")
}

/// A JSON initializer as a store value. An object or an array has no scalar
/// counterpart, so it becomes `Unknown` rather than a guess.
fn json_to_val(json: &serde_json::Value) -> Val {
    match json {
        serde_json::Value::Bool(b) => Val::bool(*b),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Val::int)
            .or_else(|| n.as_f64().map(Val::float))
            .unwrap_or(Val::Unknown),
        serde_json::Value::String(s) => Val::string(s),
        _ => Val::Unknown,
    }
}
