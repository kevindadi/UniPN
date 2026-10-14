//! State-class reachability for dense-time TPNs (Berthomieu–Menasche).
//!
//! [`StateClassSpace`] wraps a [`TpnNet`] and a [`TpnSemantics`] and implements
//! [`NetLike`] with [`StateClass`] as its state, so the generic
//! [`explore`](crate::analysis::explore), [`find_deadlocks`](crate::analysis::find_deadlocks),
//! and the label-level analyses in [`analysis::labels`](crate::analysis::labels)
//! all work on it unchanged. [`state_class_graph`] is the one-line convenience
//! that builds the initial class and explores.

use crate::analysis::{AnalysisConfig, NetLike, ReachabilityGraph, explore};
use crate::net::{Marking, TransitionId};
use crate::tpn::{Interval, TpnNet, UpperBound};

use super::dbm::{Bound, Dbm, Overflow};
use super::state_class::{InstanceKey, StateClass};

/// Which marking the "newly enabled" test is measured against.
///
/// Both rules reset the fired transition `t_f` whenever it is still enabled in
/// the successor marking; they differ only in the marking used to decide
/// whether some *other* transition `u` was already enabled before the firing.
/// See Bérard, Cassez, Haddad, Lime, Roux, "Comparison of Different Semantics
/// for Time Petri Nets", ATVA 2005 (LNCS 3707), where these are the
/// *intermediate* (`I`) and *atomic* (`A`) semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NewlyEnabled {
    /// `u` is newly enabled iff `u ∈ En(M')` and (`u = t_f` or
    /// `u ∉ En(M − Pre(t_f))`): the memory test uses the intermediate marking
    /// after consuming `t_f`'s inputs. This is the classical Merlin–Farber
    /// semantics.
    Intermediate,
    /// `u` is newly enabled iff `u ∈ En(M')` and (`u = t_f` or `u ∉ En(M)`):
    /// the memory test uses the full marking before the firing.
    Atomic,
}

/// How many instances of a transition may be enabled at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Servers {
    /// Single-server: one instance per transition, regardless of the enabling
    /// degree. The classical TPN semantics.
    Single,
}

/// The semantic parameters of a dense-time TPN analysis.
///
/// The default is [`NewlyEnabled::Atomic`] + [`Servers::Single`]. Atomic is
/// the default because it matches the crate's existing PTPN analysis (which
/// keeps a transition's clock while it stays enabled across firings) and
/// because Bérard et al. argue the atomic family is the more appropriate
/// memory policy for discrete-event modelling; single-server is the default
/// because it is the classical Merlin–Farber semantics, with multi-server an
/// extension (Boucheneb, Lime, Roux, PETRI NETS 2013).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TpnSemantics {
    pub newly_enabled: NewlyEnabled,
    pub servers: Servers,
}

impl Default for TpnSemantics {
    fn default() -> Self {
        TpnSemantics {
            newly_enabled: NewlyEnabled::Atomic,
            servers: Servers::Single,
        }
    }
}

/// A [`TpnNet`] explored under a [`TpnSemantics`], with [`StateClass`] as the
/// state.
///
/// This is the adapter that lets the generic explorer and the label-level
/// analyses run over the state-class graph without a second explorer.
pub struct StateClassSpace<'a, PL, TL> {
    net: &'a TpnNet<PL, TL>,
    semantics: TpnSemantics,
}

impl<'a, PL, TL> StateClassSpace<'a, PL, TL> {
    pub fn new(net: &'a TpnNet<PL, TL>, semantics: TpnSemantics) -> Self {
        StateClassSpace { net, semantics }
    }

    /// The static firing interval of a transition.
    fn interval_of(&self, t: TransitionId) -> Interval {
        self.net
            .transition(t)
            .map_or_else(|| Interval::unbounded(0), |tr| tr.kind.interval)
    }

    /// The enabled instances of `marking`, in canonical `(transition,
    /// instance)` order. Single-server: one instance per structurally enabled
    /// transition.
    fn enabled_instances(&self, marking: &Marking) -> Vec<InstanceKey> {
        self.net
            .transition_ids()
            .filter(|&t| self.net.structurally_enabled(marking, t))
            .map(|t| InstanceKey {
                transition: t,
                instance: 0,
            })
            .collect()
    }

    /// The initial class: every enabled instance constrained to its static
    /// interval.
    pub fn initial_class(&self, marking: &Marking) -> Result<StateClass, Overflow> {
        let vars = self.enabled_instances(marking);
        let mut zone = Dbm::new(1 + vars.len());
        for (i, key) in vars.iter().enumerate() {
            let interval = self.interval_of(key.transition);
            let col = i + 1;
            zone.set(col, 0, upper_bound(interval)?);
            zone.set(0, col, lower_bound(interval)?);
        }
        zone.normalize()?;
        Ok(StateClass::new(marking.clone(), zone, vars))
    }

    /// Whether `key` can fire from `class`: the firing domain intersected with
    /// `θ_key ≤ θ_u` for every other enabled instance `u` is non-empty.
    fn is_firable(&self, class: &StateClass, key: InstanceKey) -> bool {
        let Some(fired_col) = class.column_of(key) else {
            return false;
        };
        let mut zone = class.zone.clone();
        for (col, _) in class.vars.iter().enumerate() {
            let u_col = col + 1;
            if u_col == fired_col {
                continue;
            }
            zone.set(fired_col, u_col, Bound::le(0));
        }
        zone.normalize().is_ok() && !zone.has_negative_diagonal()
    }

    /// Whether `u` is newly enabled by firing `t_f` from `marking` to
    /// `marking_prime`, under the configured [`NewlyEnabled`] rule.
    fn newly_enabled(
        &self,
        u: TransitionId,
        marking: &Marking,
        t_f: TransitionId,
        marking_prime: &Marking,
    ) -> bool {
        if !self.net.structurally_enabled(marking_prime, u) {
            return false;
        }
        if u == t_f {
            return true;
        }
        let memory = match self.semantics.newly_enabled {
            NewlyEnabled::Intermediate => {
                let mut m = marking.clone();
                self.net.consume_inputs(&mut m, t_f);
                m
            }
            NewlyEnabled::Atomic => marking.clone(),
        };
        !self.net.structurally_enabled(&memory, u)
    }

    /// Fire instance `key` from `class`, returning the successor class, or
    /// `None` if `key` is not firable (or arithmetic overflows).
    fn fire_instance(&self, class: &StateClass, key: InstanceKey) -> Option<StateClass> {
        let fired_col = class.column_of(key)?;

        // Step 1: the firing domain D_f = D ∧ { θ_key ≤ θ_u : u ≠ key }.
        let mut d_f = class.zone.clone();
        for (col, _) in class.vars.iter().enumerate() {
            let u_col = col + 1;
            if u_col == fired_col {
                continue;
            }
            d_f.set(fired_col, u_col, Bound::le(0));
        }
        d_f.normalize().ok()?;
        if d_f.has_negative_diagonal() {
            return None;
        }

        // Step 2: the discrete token shuffle.
        let marking = NetLike::fire(self.net, &class.marking, key.transition)?;

        // Step 3: the successor's enabled instances, and which are persistent.
        let new_vars = self.enabled_instances(&marking);
        let mut zone = Dbm::new(1 + new_vars.len());
        for (i, &u) in new_vars.iter().enumerate() {
            let new_col = i + 1;
            let persistent = u.transition != key.transition
                && !self.newly_enabled(u.transition, &class.marking, key.transition, &marking);
            if persistent {
                // θ'_u = θ_u − θ_key: differences between persistent clocks are
                // preserved, and the bounds relative to the reference become
                // the bounds relative to θ_key.
                let old_col = class.column_of(u)?;
                zone.set(new_col, 0, d_f.get(old_col, fired_col));
                zone.set(0, new_col, d_f.get(fired_col, old_col));
                for (j, &v) in new_vars.iter().enumerate() {
                    let new_col2 = j + 1;
                    let persistent2 = v.transition != key.transition
                        && !self.newly_enabled(
                            v.transition,
                            &class.marking,
                            key.transition,
                            &marking,
                        );
                    if persistent2 {
                        let old_col2 = class.column_of(v)?;
                        zone.set(new_col, new_col2, d_f.get(old_col, old_col2));
                    }
                }
            } else {
                // Newly enabled: the static interval, open/closed preserved.
                let interval = self.interval_of(u.transition);
                zone.set(new_col, 0, upper_bound(interval).ok()?);
                zone.set(0, new_col, lower_bound(interval).ok()?);
            }
        }
        zone.normalize().ok()?;
        if zone.has_negative_diagonal() {
            return None;
        }
        Some(StateClass::new(marking, zone, new_vars))
    }
}

impl<PL, TL> NetLike for StateClassSpace<'_, PL, TL> {
    type State = StateClass;

    fn num_places(&self) -> usize {
        self.net.num_places()
    }

    fn num_transitions(&self) -> usize {
        self.net.num_transitions()
    }

    fn enabled(&self, state: &StateClass) -> Vec<TransitionId> {
        state
            .instances()
            .iter()
            .filter(|key| self.is_firable(state, **key))
            .map(|key| key.transition)
            .collect()
    }

    fn fire(&self, state: &StateClass, transition: TransitionId) -> Option<StateClass> {
        self.fire_instance(
            state,
            InstanceKey {
                transition,
                instance: 0,
            },
        )
    }
}

/// Build the state-class reachability graph of `net` from `initial` under
/// `semantics`, reusing the generic explorer.
///
/// Returns [`Overflow`] if the initial class cannot be represented (an
/// interval endpoint exceeds `i64`); a firing-time overflow during exploration
/// is treated as "that transition cannot fire", exactly as [`NetLike::fire`]
/// returning `None`.
pub fn state_class_graph<PL, TL>(
    net: &TpnNet<PL, TL>,
    initial: Marking,
    semantics: TpnSemantics,
    config: &AnalysisConfig,
) -> Result<ReachabilityGraph<StateClass>, Overflow> {
    let space = StateClassSpace::new(net, semantics);
    let initial_class = space.initial_class(&initial)?;
    Ok(explore(&space, initial_class, config))
}

/// The DBM upper bound of an interval: `θ ≤ value` (or `< value`), or `+∞`.
fn upper_bound(interval: Interval) -> Result<Bound, Overflow> {
    match interval.upper {
        UpperBound::Finite { value, open } => Ok(Bound::Finite {
            value: i64::try_from(value).map_err(|_| Overflow)?,
            strict: open,
        }),
        UpperBound::Infinity => Ok(Bound::Infinity),
    }
}

/// The DBM lower bound of an interval: `θ ≥ lower` (or `> lower`), encoded as
/// `x_0 − θ ≤ −lower`.
fn lower_bound(interval: Interval) -> Result<Bound, Overflow> {
    let value = i64::try_from(interval.lower).map_err(|_| Overflow)?;
    Ok(Bound::Finite {
        value: -value,
        strict: interval.lower_open,
    })
}
