//! 矩阵底层 `Net`：CSC incidence + 可选 guard/update/容量/变量域。

use rustc_hash::FxHashMap;

use crate::expr::{BoolExpr, ConcreteVal, Val, VarUpdate, eval_expr, eval_guard};
use crate::ids::{PlaceId, TransitionId, Weight};
use crate::model::{ControlSub, Place, PlaceKind, ResourceType, Transition, TransitionKind};
use crate::netlike::{FireError, NetLike};
use crate::state::{Marking, State, VarStore};
use crate::storage::Incidence;

/// 统一的矩阵存储网。
#[derive(Clone, Debug)]
pub struct Net {
    places: Vec<Place>,
    transitions: Vec<Transition>,
    pre: Incidence,
    post: Incidence,
    /// 输入弧守卫（稀疏，仅带数据网才有）。
    pre_guards: FxHashMap<(TransitionId, PlaceId), BoolExpr>,
    /// 输出弧变量更新（稀疏）。
    post_updates: FxHashMap<(TransitionId, PlaceId), VarUpdate>,
    initial_marking: Marking,
    initial_vars: Option<VarStore>,
    /// 有界 Int 域：更新越界禁用变迁（可判定性）。
    var_domains: FxHashMap<String, (i64, i64)>,
}

impl Net {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        places: Vec<Place>,
        transitions: Vec<Transition>,
        pre: Incidence,
        post: Incidence,
        pre_guards: FxHashMap<(TransitionId, PlaceId), BoolExpr>,
        post_updates: FxHashMap<(TransitionId, PlaceId), VarUpdate>,
        initial_marking: Marking,
        initial_vars: Option<VarStore>,
        var_domains: FxHashMap<String, (i64, i64)>,
    ) -> Self {
        Self {
            places,
            transitions,
            pre,
            post,
            pre_guards,
            post_updates,
            initial_marking,
            initial_vars,
            var_domains,
        }
    }

    pub fn place(&self, p: PlaceId) -> Option<&Place> {
        self.places.get(p.index())
    }

    pub fn transition(&self, t: TransitionId) -> Option<&Transition> {
        self.transitions.get(t.index())
    }

    pub fn places(&self) -> &[Place] {
        &self.places
    }

    pub fn transitions(&self) -> &[Transition] {
        &self.transitions
    }

    pub fn pre(&self) -> &Incidence {
        &self.pre
    }

    pub fn post(&self) -> &Incidence {
        &self.post
    }

    pub fn var_domain(&self, var: &str) -> Option<(i64, i64)> {
        self.var_domains.get(var).copied()
    }
}

impl NetLike for Net {
    fn num_places(&self) -> usize {
        self.places.len()
    }

    fn num_transitions(&self) -> usize {
        self.transitions.len()
    }

    fn place_label(&self, p: PlaceId) -> String {
        self.places
            .get(p.index())
            .map_or_else(|| format!("p{}", p.index()), |pl| pl.name.clone())
    }

    fn place_kind(&self, p: PlaceId) -> Option<PlaceKind> {
        self.places.get(p.index()).map(|pl| pl.kind.clone())
    }

    fn transition_label(&self, t: TransitionId) -> String {
        self.transitions
            .get(t.index())
            .map_or_else(|| format!("t{}", t.index()), |tr| tr.name.clone())
    }

    fn transition_kind(&self, t: TransitionId) -> Option<TransitionKind> {
        self.transitions.get(t.index()).map(|tr| tr.kind.clone())
    }

    fn transition_anchors(&self, t: TransitionId) -> Vec<String> {
        self.transitions
            .get(t.index())
            .map_or_else(Vec::new, |tr| tr.anchors.clone())
    }

    fn transition_family(&self, t: TransitionId) -> Option<&str> {
        self.transitions
            .get(t.index())
            .and_then(|tr| tr.family.as_deref())
    }

    fn pre_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)> {
        self.pre
            .column(t)
            .iter()
            .map(|&(p, w)| (PlaceId(p), w))
            .collect()
    }

    fn post_arcs(&self, t: TransitionId) -> Vec<(PlaceId, Weight)> {
        self.post
            .column(t)
            .iter()
            .map(|&(p, w)| (PlaceId(p), w))
            .collect()
    }

    fn is_thread_terminal(&self, p: PlaceId) -> bool {
        matches!(
            self.place_kind(p),
            Some(PlaceKind::Control(
                ControlSub::ThreadEnd | ControlSub::FunctionEnd
            ))
        )
    }

    fn is_wait_point(&self, p: PlaceId) -> bool {
        matches!(
            self.place_kind(p),
            Some(PlaceKind::Control(ControlSub::WaitPoint))
        )
    }

    fn is_resource(&self, p: PlaceId) -> bool {
        matches!(self.place_kind(p), Some(PlaceKind::Resource(_)))
    }

    fn initial_state(&self) -> State {
        State::new(self.initial_marking.clone(), self.initial_vars.clone())
    }

    fn enabled_transitions(&self, s: &State) -> Vec<TransitionId> {
        let mut out = Vec::new();
        for t in 0..self.transitions.len() {
            let tid = TransitionId(t);
            if self.is_enabled(tid, s) {
                out.push(tid);
            }
        }
        out
    }

    fn fire(&self, t: TransitionId, s: &State) -> Result<State, FireError> {
        if t.index() >= self.transitions.len() {
            return Err(FireError::OutOfBounds(t));
        }
        if !self.is_enabled(t, s) {
            return Err(FireError::NotEnabled(t));
        }

        let mut next = s.clone();
        for &(p, w) in self.pre.column(t) {
            let pid = PlaceId(p);
            let tokens = next.marking.tokens(pid);
            next.marking.set(pid, tokens - w);
        }
        for &(p, w) in self.post.column(t) {
            let pid = PlaceId(p);
            let after = next.marking.tokens(pid) + w;
            if let Some(cap) = self.capacity_of(pid) {
                if after > cap {
                    return Err(FireError::Capacity {
                        place: pid,
                        after,
                        capacity: cap,
                    });
                }
            }
            next.marking.set(pid, after);
        }

        // 变量更新：对原状态求值后写入新状态。
        let mut applied = false;
        let mut store = next.vars.clone().unwrap_or_default();
        for (&(tt, _), update) in &self.post_updates {
            if tt == t {
                applied = true;
                for (var, expr) in update {
                    store.insert(var.clone(), eval_expr(expr, s.vars()));
                }
            }
        }
        if applied {
            next.vars = Some(store);
        }

        Ok(next)
    }
}

impl Net {
    fn capacity_of(&self, p: PlaceId) -> Option<u32> {
        let place = self.places.get(p.index())?;
        if let Some(cap) = place.capacity {
            return Some(cap);
        }
        match &place.kind {
            PlaceKind::Resource(ResourceType::Mutex) => Some(1),
            PlaceKind::Resource(ResourceType::RwLock { max_readers }) => Some(*max_readers),
            PlaceKind::Resource(ResourceType::Semaphore { count }) => Some(*count),
            _ => None,
        }
    }

    fn is_enabled(&self, t: TransitionId, s: &State) -> bool {
        for &(p, w) in self.pre.column(t) {
            let pid = PlaceId(p);
            if s.marking.tokens(pid) < w {
                return false;
            }
            if let Some(guard) = self.pre_guards.get(&(t, pid)) {
                if !eval_guard(guard, s.vars()).is_not_false() {
                    return false;
                }
            }
        }
        // 有界 Int 域：更新越界则禁用。
        for (&(tt, _), update) in &self.post_updates {
            if tt != t {
                continue;
            }
            for (var, expr) in update {
                if let Some((lo, hi)) = self.var_domains.get(var) {
                    if let Val::Concrete(ConcreteVal::Int(v)) = eval_expr(expr, s.vars())
                        && (v < *lo || v > *hi)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}
