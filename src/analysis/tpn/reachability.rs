//! State-class reachability for dense-time TPNs (Berthomieu–Menasche).
//!
//! [`StateClassSpace`] wraps a [`TpnNet`] and a [`TpnSemantics`] and implements
//! [`NetLike`] with [`StateClass`] as its state, so the generic
//! [`explore`](crate::analysis::explore), [`find_deadlocks`](crate::analysis::find_deadlocks),
//! and the label-level analyses in [`analysis::labels`](crate::analysis::labels)
//! all work on it unchanged. [`state_class_graph`] is the one-line convenience
//! that builds the initial class and explores.
//!
//! # Multi-server semantics
//!
//! Under [`Servers::Multiple`] a transition has one clock per *instance*, where
//! the enabling degree is `deg(t, M) = min over input places p of ⌊M(p) / w(p,t)⌋`.
//! Read and inhibitor arcs only gate (an unsatisfied one makes the degree 0);
//! a transition with no input arc has degree 1, documented here because the
//! min-over-an-empty-set reading would leave it undefined. Instances of one
//! transition are ordered by age (oldest first) — the canonical column order,
//! which is what makes class equality well-defined.
//!
//! Two policy parameters, each following Boucheneb, Lime, Roux, "On
//! multi-enabledness in time Petri nets", PETRI NETS 2013 (LNCS 7927):
//!
//! - **Firing**: FEFF (First Enabled First Fired, [`FiringPolicy::OldestFirst`])
//!   fires the oldest instance; the non-deterministic choice
//!   ([`FiringPolicy::Any`]) lets any firable instance fire. Under the
//!   canonical age-ordered representation, firing any firable instance at time
//!   τ yields the same successor class as firing the oldest at τ (the surviving
//!   instances are the same multiset of clocks), so this implementation fires
//!   the oldest instance for both policies. The reachable markings are
//!   identical — the paper proves the two semantics simulate each other with
//!   strong timed simulations, hence generate the same timed traces — while the
//!   underlying timed transition systems differ (they are not bisimilar).
//! - **Disabling**: when the enabling degree of a transition drops (a
//!   conflicting firing consumed tokens), FEFD (First Enabled First Disabled,
//!   [`DisablingPolicy::OldestFirst`]) discards the oldest instances; the
//!   alternative [`DisablingPolicy::NewestFirst`] discards the newest. This
//!   choice genuinely changes the reachable markings (see `tests/tpn.rs`).

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
    /// Multi-server: `deg(t, M)` instances, one clock each (Boucheneb, Lime,
    /// Roux, PETRI NETS 2013).
    Multiple(MultiServerConfig),
}

/// The instance policies of the multi-server semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MultiServerConfig {
    pub firing: FiringPolicy,
    pub disabling: DisablingPolicy,
}

impl Default for MultiServerConfig {
    fn default() -> Self {
        MultiServerConfig {
            // FEFF + FEFD: the policy combination Boucheneb et al. adopt.
            firing: FiringPolicy::OldestFirst,
            disabling: DisablingPolicy::OldestFirst,
        }
    }
}

/// Which instance of a transition fires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FiringPolicy {
    /// Any firable instance may fire (the non-deterministic choice). Same
    /// reachable markings as [`OldestFirst`] (strong timed simulation,
    /// Boucheneb et al.); see the module docs for why the implementation fires
    /// the oldest instance for both policies.
    Any,
    /// Only the oldest instance fires (First Enabled First Fired, FEFF).
    OldestFirst,
}

/// Which instances are disabled when the enabling degree drops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DisablingPolicy {
    /// The oldest instances are disabled first (First Enabled First Disabled,
    /// FEFD — Boucheneb et al.'s policy: "the first clock in `enab` is
    /// discarded").
    OldestFirst,
    /// The newest instances are disabled first.
    NewestFirst,
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

    /// The enabling degree of `t` at `marking`: the minimum over its input
    /// places of `⌊M(p) / w(p,t)⌋`. Read and inhibitor arcs only gate — an
    /// unsatisfied one makes the degree 0 — and a transition with no input arc
    /// has degree 1 (there is no empty minimum to take).
    fn enabling_degree(&self, marking: &Marking, t: TransitionId) -> usize {
        if !self.net.structurally_enabled(marking, t) {
            return 0;
        }
        let demand = self.net.input_demand(t);
        if demand.is_empty() {
            return 1;
        }
        demand
            .iter()
            .map(|&(p, w)| marking.tokens(p) / w)
            .min()
            .unwrap_or(0)
    }

    /// The enabled instances of `marking`, in canonical `(transition,
    /// instance)` order with instance 0 the oldest.
    fn enabled_instances(&self, marking: &Marking) -> Vec<InstanceKey> {
        let mut vars = Vec::new();
        for t in self.net.transition_ids() {
            let degree = match self.semantics.servers {
                Servers::Single => {
                    if self.net.structurally_enabled(marking, t) {
                        1
                    } else {
                        0
                    }
                }
                Servers::Multiple(_) => self.enabling_degree(marking, t),
            };
            for i in 0..degree {
                vars.push(InstanceKey {
                    transition: t,
                    instance: i,
                });
            }
        }
        vars
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

    /// The number of instances of `u` that survive firing `t_f`, under the
    /// configured [`NewlyEnabled`] rule. `None` if `u` is the fired
    /// transition, which is handled separately (only the fired instance is
    /// removed).
    fn survivor_count(
        &self,
        u: TransitionId,
        class: &StateClass,
        t_f: TransitionId,
        marking_prime: &Marking,
    ) -> Option<usize> {
        if u == t_f {
            return None;
        }
        let deg_old = class.vars.iter().filter(|k| k.transition == u).count();
        if deg_old == 0 {
            return Some(0);
        }
        let memory = match self.semantics.newly_enabled {
            NewlyEnabled::Intermediate => {
                let mut m = class.marking.clone();
                self.net.consume_inputs(&mut m, t_f);
                m
            }
            NewlyEnabled::Atomic => class.marking.clone(),
        };
        match self.semantics.servers {
            // Single-server: `u` persists iff it is enabled in M' and in the
            // memory marking.
            Servers::Single => Some(
                if self.net.structurally_enabled(marking_prime, u)
                    && self.net.structurally_enabled(&memory, u)
                {
                    1
                } else {
                    0
                },
            ),
            Servers::Multiple(_) => {
                let deg_mem = self.enabling_degree(&memory, u);
                let deg_new = self.enabling_degree(marking_prime, u);
                Some(match self.semantics.newly_enabled {
                    // Intermediate: the consumption step reduces the degree to
                    // deg(u, M − Pre(t_f)); those instances persist.
                    NewlyEnabled::Intermediate => deg_mem.min(deg_old),
                    // Atomic: one step; the persistent instances are those
                    // enabled in both M and M'.
                    NewlyEnabled::Atomic => deg_old.min(deg_new),
                })
            }
        }
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

        // Step 3: the successor's instances, and where each comes from.
        let disabling = match self.semantics.servers {
            Servers::Multiple(config) => config.disabling,
            Servers::Single => DisablingPolicy::OldestFirst,
        };
        let mut new_vars: Vec<InstanceKey> = Vec::new();
        // `Some(old_key)` for a persistent instance, `None` for a fresh one.
        let mut origins: Vec<Option<InstanceKey>> = Vec::new();

        for u in self.net.transition_ids() {
            let deg_old = class.vars.iter().filter(|k| k.transition == u).count();
            let deg_new = match self.semantics.servers {
                Servers::Single => {
                    if self.net.structurally_enabled(&marking, u) {
                        1
                    } else {
                        0
                    }
                }
                Servers::Multiple(_) => self.enabling_degree(&marking, u),
            };
            let survivors: Vec<InstanceKey> = if u == key.transition {
                // The fired instance is removed (and reset if re-enabled);
                // every other old instance survives.
                class
                    .vars
                    .iter()
                    .filter(|k| k.transition == u && *k != &key)
                    .copied()
                    .collect()
            } else {
                let count = self
                    .survivor_count(u, class, key.transition, &marking)
                    .unwrap_or(deg_old)
                    .min(deg_old);
                let old: Vec<InstanceKey> = class
                    .vars
                    .iter()
                    .filter(|k| k.transition == u)
                    .copied()
                    .collect();
                // The instances are age-ordered (oldest first); disabling the
                // oldest keeps the youngest, and vice versa.
                let start = match disabling {
                    DisablingPolicy::OldestFirst => deg_old - count,
                    DisablingPolicy::NewestFirst => 0,
                };
                old[start..start + count].to_vec()
            };
            let kept = survivors.len();
            let fresh_count = deg_new - kept;
            for (j, old_key) in survivors.into_iter().enumerate() {
                new_vars.push(InstanceKey {
                    transition: u,
                    instance: j,
                });
                origins.push(Some(old_key));
            }
            for j in 0..fresh_count {
                new_vars.push(InstanceKey {
                    transition: u,
                    instance: kept + j,
                });
                origins.push(None);
            }
        }

        // Step 4: the successor zone.
        let mut zone = Dbm::new(1 + new_vars.len());
        for (i, &u) in new_vars.iter().enumerate() {
            let new_col = i + 1;
            match origins[i] {
                Some(old_key) => {
                    // θ'_u = θ_u − θ_key: differences between persistent
                    // instances are preserved, and the bounds relative to the
                    // reference become the bounds relative to θ_key.
                    let old_col = class.column_of(old_key)?;
                    zone.set(new_col, 0, d_f.get(old_col, fired_col));
                    zone.set(0, new_col, d_f.get(fired_col, old_col));
                    for (j, _) in new_vars.iter().enumerate() {
                        let new_col2 = j + 1;
                        if let Some(old_key2) = origins[j] {
                            let old_col2 = class.column_of(old_key2)?;
                            zone.set(new_col, new_col2, d_f.get(old_col, old_col2));
                        }
                    }
                }
                None => {
                    // Newly enabled: the static interval, open/closed preserved.
                    let interval = self.interval_of(u.transition);
                    zone.set(new_col, 0, upper_bound(interval).ok()?);
                    zone.set(0, new_col, lower_bound(interval).ok()?);
                }
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
            .filter(|key| key.instance == 0 && self.is_firable(state, **key))
            .map(|key| key.transition)
            .collect()
    }

    fn fire(&self, state: &StateClass, transition: TransitionId) -> Option<StateClass> {
        // The oldest instance fires, for both firing policies (see the module
        // docs for why `Any` and `OldestFirst` share this implementation).
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
