//! P- and T-semiflows: checked against the definition, against an independent
//! brute-force enumeration on small random nets, and against the reachable
//! markings of nets where the arc kinds and capacities matter.

use unipn::analysis::invariants::{
    FarkasLimits, InvariantStatus, PSemiflow, SemiflowError, p_invariant_status, p_semiflows,
    p_semiflows_within, t_semiflows,
};
use unipn::analysis::{AnalysisConfig, NetLike, explore};
use unipn::cvn::{CvnBuilder, CvnTransition, PlaceKind, ResourceType, TransitionKind};
use unipn::net::{ArcDir, ControlSub, Labeled, Net, PlaceCapacity, PlaceClass, PlaceId};
use unipn::pt::{PtNet, PtPlaceKind, PtTransitionKind, TransitionType};
use unipn::{BoolExpr, LabeledBuilder, LabeledNet, Marking, Tag};

fn limits() -> FarkasLimits {
    FarkasLimits::default()
}

/// `yᵀ·C = 0` and `C·x = 0`, straight from the incidence matrix.
fn assert_semiflow_definitions<PK, TK, AK>(net: &Net<PK, TK, AK>) {
    let c = net.incidence_matrix();
    for y in p_semiflows(net, limits()).unwrap() {
        for t in net.transition_ids() {
            let dot: i128 = y
                .entries()
                .iter()
                .map(|&(p, w)| i128::from(c.get(p, t)) * w as i128)
                .sum();
            assert_eq!(dot, 0, "yᵀC ≠ 0 at {t:?} for {y:?}");
        }
    }
    for x in t_semiflows(net, limits()).unwrap() {
        for p in net.place_ids() {
            let dot: i128 = x
                .entries()
                .iter()
                .map(|&(t, w)| i128::from(c.get(p, t)) * w as i128)
                .sum();
            assert_eq!(dot, 0, "Cx ≠ 0 at {p:?} for {x:?}");
        }
    }
}

/// Every reachable marking has the same `yᵀ·M` for every semiflow `p_invariant_status`
/// calls an invariant. Returns the statuses so tests can also assert on them.
fn assert_invariants_hold<N, PK, TK, AK>(
    net: &N,
    as_net: &Net<PK, TK, AK>,
    initial: N::State,
    marking_of: impl Fn(&N::State) -> &Marking,
) -> Vec<(PSemiflow, InvariantStatus)>
where
    N: NetLike,
    PK: PlaceCapacity,
{
    let m0 = marking_of(&initial).clone();
    let graph = explore(
        net,
        initial,
        &AnalysisConfig {
            max_states: 1_000_000,
            ..AnalysisConfig::default()
        },
    );
    assert!(!graph.truncated);
    let mut out = Vec::new();
    for y in p_semiflows(as_net, limits()).unwrap() {
        let status = p_invariant_status(as_net, &y, &m0);
        if status == InvariantStatus::Holds {
            let expected = y.weighted_sum(&m0).unwrap();
            for state in &graph.states {
                assert_eq!(
                    y.weighted_sum(marking_of(state)).unwrap(),
                    expected,
                    "{y:?}"
                );
            }
        }
        out.push((y, status));
    }
    out
}

fn varies(net: &impl NetLike<State = Marking>, initial: Marking, y: &PSemiflow) -> bool {
    let expected = y.weighted_sum(&initial).unwrap();
    let graph = explore(net, initial, &AnalysisConfig::default());
    graph
        .states
        .iter()
        .any(|m| y.weighted_sum(m).unwrap() != expected)
}

// ── Dining philosophers: a textbook count ────────────────────────────────

/// n philosophers who take both forks at once: places think_i, eat_i, fork_i;
/// transitions take_i (think_i + fork_i + fork_{i+1} → eat_i) and release_i
/// (the reverse).
///
/// The count follows from the matrix directly: a vector y is a P-semiflow iff,
/// for every i, y(eat_i) = y(think_i) + y(fork_i) + y(fork_{i+1}) (the take_i
/// column; release_i is its negation). So y is free on the 2n think/fork
/// places and determined on the eat places: the cone of semiflows is a copy of
/// ℝ≥0^{2n}, whose extreme rays are the 2n unit vectors — think_i + eat_i and
/// fork_i + eat_i + eat_{i−1}. These are the two families of invariants usually
/// quoted for this model ("a philosopher thinks or eats", "a fork is free or
/// held by one of its two neighbours"). Likewise C·x = 0
/// iff x(take_i) = x(release_i) for each i: n T-semiflows.
fn philosophers(n: usize) -> (LabeledNet<&'static str, &'static str>, Marking) {
    let mut b = LabeledBuilder::new();
    let think: Vec<_> = (0..n)
        .map(|i| b.add_marked_place(format!("think{i}"), Tag("think"), 1))
        .collect();
    let eat: Vec<_> = (0..n)
        .map(|i| b.add_place(format!("eat{i}"), Tag("eat")))
        .collect();
    let fork: Vec<_> = (0..n)
        .map(|i| b.add_marked_place(format!("fork{i}"), Tag("fork"), 1))
        .collect();
    for i in 0..n {
        let next = (i + 1) % n;
        let take = b.add_transition(format!("take{i}"), Tag("take"));
        b.add_input_arc(think[i], take, 1);
        b.add_input_arc(fork[i], take, 1);
        b.add_input_arc(fork[next], take, 1);
        b.add_output_arc(eat[i], take, 1);
        let release = b.add_transition(format!("release{i}"), Tag("release"));
        b.add_input_arc(eat[i], release, 1);
        b.add_output_arc(think[i], release, 1);
        b.add_output_arc(fork[i], release, 1);
        b.add_output_arc(fork[next], release, 1);
    }
    b.build()
}

#[test]
fn philosophers_have_two_n_p_semiflows_and_n_t_semiflows() {
    for n in 2..=4 {
        let (net, initial) = philosophers(n);
        assert_semiflow_definitions(&net);

        let ps = p_semiflows(&net, limits()).unwrap();
        assert_eq!(ps.len(), 2 * n, "n = {n}");
        let ts = t_semiflows(&net, limits()).unwrap();
        assert_eq!(ts.len(), n, "n = {n}");
        for x in &ts {
            assert_eq!(x.entries().len(), 2);
            assert!(x.entries().iter().all(|&(_, w)| w == 1));
        }

        let eat0 = net.places.iter().find(|p| p.name == "eat0").unwrap().id;
        let think0 = net.places.iter().find(|p| p.name == "think0").unwrap().id;
        let think_eat = ps
            .iter()
            .find(|y| y.coefficient(think0) == 1)
            .expect("think0 + eat0");
        assert_eq!(think_eat.entries(), [(think0, 1), (eat0, 1)]);

        for (y, status) in assert_invariants_hold(&net, &net, initial, |m| m) {
            assert_eq!(status, InvariantStatus::Holds, "{y:?}");
        }
    }
}

// ── Brute force: every minimal support, found independently ──────────────

#[derive(Clone, Copy, Debug, PartialEq)]
struct Q {
    n: i128,
    d: i128,
}

impl Q {
    fn int(n: i128) -> Self {
        Q { n, d: 1 }
    }
    fn norm(n: i128, d: i128) -> Self {
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Q {
            n: s * n / g,
            d: s * d / g,
        }
    }
    fn sub(self, o: Q) -> Q {
        Q::norm(self.n * o.d - o.n * self.d, self.d * o.d)
    }
    fn mul(self, o: Q) -> Q {
        Q::norm(self.n * o.n, self.d * o.d)
    }
    fn div(self, o: Q) -> Q {
        Q::norm(self.n * o.d, self.d * o.n)
    }
}

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// A basis of `{ v : M·v = 0 }` by reduced row echelon form over ℚ.
fn nullspace(m: &[Vec<i128>], cols: usize) -> Vec<Vec<Q>> {
    let mut a: Vec<Vec<Q>> = m
        .iter()
        .map(|r| r.iter().map(|&v| Q::int(v)).collect())
        .collect();
    let mut pivots = Vec::new();
    let mut row = 0;
    for col in 0..cols {
        let Some(p) = (row..a.len()).find(|&r| a[r][col].n != 0) else {
            continue;
        };
        a.swap(row, p);
        let lead = a[row][col];
        for v in a[row].iter_mut() {
            *v = v.div(lead);
        }
        let pivot_row = a[row].clone();
        for (r, other) in a.iter_mut().enumerate() {
            if r != row && other[col].n != 0 {
                let f = other[col];
                for (v, &p) in other.iter_mut().zip(&pivot_row) {
                    *v = v.sub(f.mul(p));
                }
            }
        }
        pivots.push(col);
        row += 1;
    }
    (0..cols)
        .filter(|c| !pivots.contains(c))
        .map(|free| {
            let mut v = vec![Q::int(0); cols];
            v[free] = Q::int(1);
            for (r, &pc) in pivots.iter().enumerate() {
                v[pc] = Q::int(0).sub(a[r][free]);
            }
            v
        })
        .collect()
}

/// The minimal-support non-negative solutions of `yᵀ·A = 0` (rows of `a` are
/// the variables), by trying every support: S is a minimal support iff the
/// solutions living on S form a one-dimensional space spanned by a vector that
/// is non-zero and of one sign everywhere on S.
fn brute_force(a: &[Vec<i128>], cols: usize) -> Vec<Vec<(usize, u64)>> {
    let n = a.len();
    let mut found = Vec::new();
    for mask in 1u32..(1 << n) {
        let support: Vec<usize> = (0..n).filter(|i| mask & (1 << i) != 0).collect();
        // Columns of Mᵀ are the selected rows of A.
        let m: Vec<Vec<i128>> = (0..cols)
            .map(|j| support.iter().map(|&i| a[i][j]).collect())
            .collect();
        let basis = nullspace(&m, support.len());
        if basis.len() != 1 {
            continue;
        }
        let v = &basis[0];
        if v.iter().any(|q| q.n == 0) {
            continue;
        }
        let positive = v[0].n > 0;
        if v.iter().any(|q| (q.n > 0) != positive) {
            continue;
        }
        let lcm = v.iter().fold(1i128, |l, q| l / gcd(l, q.d) * q.d);
        let ints: Vec<i128> = v.iter().map(|q| (q.n * (lcm / q.d)).abs()).collect();
        let g = ints.iter().fold(0, |g, &x| gcd(g, x));
        found.push(
            support
                .iter()
                .zip(&ints)
                .map(|(&i, &x)| (i, (x / g) as u64))
                .collect(),
        );
    }
    found.sort();
    found
}

/// A small deterministic pseudo-random net, as a `LabeledNet<(), ()>`.
fn random_net(seed: u64) -> LabeledNet<(), ()> {
    let mut state = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut next = move |bound: u64| {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) % bound
    };
    let places = 1 + next(5) as usize;
    let transitions = 1 + next(5) as usize;
    let mut b = LabeledBuilder::new();
    let ps: Vec<_> = (0..places)
        .map(|i| b.add_place(format!("p{i}"), Tag(())))
        .collect();
    for j in 0..transitions {
        let t = b.add_transition(format!("t{j}"), Tag(()));
        for &p in &ps {
            let weight = |w| [0, 0, 0, 1, 1, 2][w as usize];
            let (pre, post) = (weight(next(6)), weight(next(6)));
            if pre > 0 {
                b.add_input_arc(p, t, pre);
            }
            if post > 0 {
                b.add_output_arc(p, t, post);
            }
        }
    }
    b.build().0
}

#[test]
fn farkas_finds_exactly_the_minimal_supports_on_random_nets() {
    // Guard against the comparison passing vacuously: these nets must produce
    // plenty of semiflows, including multi-place ones with coefficients > 1.
    let (mut total, mut weighted) = (0, 0);
    for seed in 0..400 {
        let net = random_net(seed);
        assert_semiflow_definitions(&net);
        let c = net.incidence_matrix();
        let by_place: Vec<Vec<i128>> = net
            .place_ids()
            .map(|p| {
                net.transition_ids()
                    .map(|t| i128::from(c.get(p, t)))
                    .collect()
            })
            .collect();
        let by_transition: Vec<Vec<i128>> = net
            .transition_ids()
            .map(|t| net.place_ids().map(|p| i128::from(c.get(p, t))).collect())
            .collect();

        let p: Vec<Vec<(usize, u64)>> = p_semiflows(&net, limits())
            .unwrap()
            .iter()
            .map(|y| y.entries().iter().map(|&(p, w)| (p.index(), w)).collect())
            .collect();
        assert_eq!(
            p,
            brute_force(&by_place, net.num_transitions()),
            "P, seed {seed}"
        );
        total += p.len();
        weighted += p
            .iter()
            .filter(|y| y.len() > 1 && y.iter().any(|&(_, w)| w > 1))
            .count();

        let t: Vec<Vec<(usize, u64)>> = t_semiflows(&net, limits())
            .unwrap()
            .iter()
            .map(|x| x.entries().iter().map(|&(t, w)| (t.index(), w)).collect())
            .collect();
        assert_eq!(
            t,
            brute_force(&by_transition, net.num_places()),
            "T, seed {seed}"
        );
    }
    assert!(total > 200, "only {total} P-semiflows over the corpus");
    assert!(
        weighted > 20,
        "only {weighted} multi-place semiflows with a coefficient > 1"
    );
}

// ── Arc kinds and capacities ─────────────────────────────────────────────

#[test]
fn read_and_inhibitor_arcs_leave_p_semiflows_invariant() {
    // a → b, gated by a read arc on `gate` and an inhibitor on `stop`.
    let mut b = LabeledBuilder::new();
    let a = b.add_marked_place("a", Tag(()), 2);
    let bb = b.add_place("b", Tag(()));
    let gate = b.add_marked_place("gate", Tag(()), 1);
    let stop = b.add_place("stop", Tag(()));
    let t = b.add_transition("t", Tag(()));
    b.add_input_arc(a, t, 1);
    b.add_output_arc(bb, t, 1);
    b.add_arc(gate, t, ArcDir::Read, 1, ());
    b.add_arc(stop, t, ArcDir::Inhibitor, 1, ());
    let (net, initial) = b.build();

    let statuses = assert_invariants_hold(&net, &net, initial, |m| m);
    // a + b, and the untouched gate and stop on their own.
    assert_eq!(statuses.len(), 3);
    assert!(statuses.iter().all(|(_, s)| *s == InvariantStatus::Holds));
}

#[test]
fn a_reset_arc_in_the_support_breaks_the_invariant() {
    // t: a → b; v: c → d, and v empties b.
    let mut b = LabeledBuilder::new();
    let a = b.add_marked_place("a", Tag(()), 1);
    let bb = b.add_place("b", Tag(()));
    let c = b.add_marked_place("c", Tag(()), 1);
    let d = b.add_place("d", Tag(()));
    let t = b.add_transition("t", Tag(()));
    b.add_input_arc(a, t, 1);
    b.add_output_arc(bb, t, 1);
    let v = b.add_transition("v", Tag(()));
    b.add_input_arc(c, v, 1);
    b.add_output_arc(d, v, 1);
    b.add_arc(bb, v, ArcDir::Reset, 1, ());
    let (net, initial) = b.build();

    let statuses = assert_invariants_hold(&net, &net, initial.clone(), |m| m);
    let ab = &statuses
        .iter()
        .find(|(y, _)| y.coefficient(a) == 1)
        .unwrap();
    assert_eq!(ab.1, InvariantStatus::ResetInSupport(vec![bb]));
    assert!(
        varies(&net, initial, &ab.0),
        "the reset really does break a + b"
    );
    let cd = &statuses
        .iter()
        .find(|(y, _)| y.coefficient(c) == 1)
        .unwrap();
    assert_eq!(cd.1, InvariantStatus::Holds);
}

fn pt_place(capacity: Option<usize>) -> PtPlaceKind {
    PtPlaceKind {
        capacity,
        ..PtPlaceKind::control(ControlSub::BasicBlock)
    }
}

#[test]
fn clamping_breaks_an_invariant_the_bound_cannot_prove() {
    // p holds 2; t moves one token from p into q, which holds at most 1. The
    // second firing clamps q and loses a token.
    let mut net = PtNet::new();
    let p = net.add_place("p", pt_place(None));
    let q = net.add_place("q", pt_place(Some(1)));
    let t = net.add_transition("t", PtTransitionKind::new(TransitionType::Normal));
    net.add_arc(p, t, ArcDir::Input, 1, ());
    net.add_arc(q, t, ArcDir::Output, 1, ());
    let initial = Marking::new(vec![2, 0]);

    let statuses = assert_invariants_hold(&net, &net, initial.clone(), |m| m);
    assert_eq!(statuses.len(), 1);
    let (y, status) = &statuses[0];
    assert_eq!(y.entries(), [(p, 1), (q, 1)]);
    assert_eq!(status, &InvariantStatus::CapacityUnproven(vec![q]));
    assert!(
        varies(&net, initial, y),
        "clamping really does lose a token"
    );
}

/// Two workers and a lock, as a P/T net: `idle_i`, `crit_i` (control) and
/// `lock` (resource, capacity `lock_capacity`).
fn pt_mutex(lock_capacity: Option<usize>) -> (PtNet, Marking) {
    let mut net = PtNet::new();
    let lock = net.add_place(
        "lock",
        PtPlaceKind {
            capacity: lock_capacity,
            ..PtPlaceKind::resource()
        },
    );
    for i in 0..2 {
        let idle = net.add_place(format!("idle{i}"), pt_place(None));
        let crit = net.add_place(format!("crit{i}"), pt_place(None));
        let enter = net.add_transition(
            format!("enter{i}"),
            PtTransitionKind::new(TransitionType::Lock(0)),
        );
        net.add_arc(idle, enter, ArcDir::Input, 1, ());
        net.add_arc(lock, enter, ArcDir::Input, 1, ());
        net.add_arc(crit, enter, ArcDir::Output, 1, ());
        let leave = net.add_transition(
            format!("leave{i}"),
            PtTransitionKind::new(TransitionType::Unlock(0)),
        );
        net.add_arc(crit, leave, ArcDir::Input, 1, ());
        net.add_arc(idle, leave, ArcDir::Output, 1, ());
        net.add_arc(lock, leave, ArcDir::Output, 1, ());
    }
    (net, Marking::new(vec![1, 1, 0, 1, 0]))
}

#[test]
fn a_bounded_place_the_semiflow_keeps_below_capacity_is_proven() {
    let (net, initial) = pt_mutex(Some(1));
    let statuses = assert_invariants_hold(&net, &net, initial, |m| m);
    // idle_i + crit_i for each worker, and lock + crit0 + crit1.
    assert_eq!(statuses.len(), 3);
    assert!(statuses.iter().all(|(_, s)| *s == InvariantStatus::Holds));
}

#[test]
fn a_rejecting_net_holds_an_invariant_the_conservative_check_cannot_prove() {
    // A CVN worker and a mutex. The CVN *rejects* a firing that would overfill
    // a resource place, so no P-semiflow is ever broken by capacity; the check
    // reads only `PlaceCapacity` and so can be more cautious than that.
    let mut b = CvnBuilder::new();
    let lock = b.add_marked_place("m", PlaceKind::Resource(ResourceType::Mutex), 1);
    let idle = b.add_marked_place("idle", PlaceKind::Control(ControlSub::BasicBlock), 1);
    let crit = b.add_place("crit", PlaceKind::Control(ControlSub::BasicBlock));
    let enter = b.add_transition("enter", CvnTransition::new(TransitionKind::Lock));
    b.add_input_arc(idle, enter, 1, BoolExpr::True);
    b.add_input_arc(lock, enter, 1, BoolExpr::True);
    b.add_output_arc(enter, crit, 1, None);
    let leave = b.add_transition("leave", CvnTransition::new(TransitionKind::Unlock));
    b.add_input_arc(crit, leave, 1, BoolExpr::True);
    b.add_output_arc(leave, idle, 1, None);
    b.add_output_arc(leave, lock, 1, None);
    let (net, mut initial) = b.build();

    // idle + crit = 1 and m + crit = 1: the second touches the capacity-1
    // mutex, and the bound (1 / 1 ≤ 1) proves it.
    let statuses = assert_invariants_hold(&net, &net, initial.clone(), |s| &s.marking);
    assert_eq!(statuses.len(), 2);
    assert!(statuses.iter().all(|(_, s)| *s == InvariantStatus::Holds));

    // Start with two tokens on the mutex place. m + crit = 2 now exceeds what
    // the bound can vouch for, so the check says "unproven" — yet the CVN
    // rejects the `leave` that would push m to 2, and the sum never changes.
    initial.marking.set(lock, 2);
    let statuses = assert_invariants_hold(&net, &net, initial.clone(), |s| &s.marking);
    let (law, status) = statuses
        .iter()
        .find(|(y, _)| y.coefficient(lock) == 1)
        .unwrap();
    assert_eq!(status, &InvariantStatus::CapacityUnproven(vec![lock]));
    let expected = law.weighted_sum(&initial.marking).unwrap();
    let graph = explore(&net, initial, &AnalysisConfig::default());
    assert!(
        graph
            .states
            .iter()
            .all(|s| law.weighted_sum(&s.marking).unwrap() == expected)
    );
}

// ── Restricting the support by label ─────────────────────────────────────

/// Two threads taking two locks in opposite orders.
/// Places: x, y, then a0..a3 and b0..b3 (start, holding one, holding both, end).
fn lock_order() -> (PtNet, Marking) {
    let mut net = PtNet::new();
    let x = net.add_place("x", PtPlaceKind::resource());
    let y = net.add_place("y", PtPlaceKind::resource());
    for (name, first, second) in [("a", x, y), ("b", y, x)] {
        let subs = [
            ControlSub::FunctionStart,
            ControlSub::BasicBlock,
            ControlSub::BasicBlock,
            ControlSub::FunctionEnd,
        ];
        let at: Vec<PlaceId> = subs
            .iter()
            .enumerate()
            .map(|(i, &sub)| net.add_place(format!("{name}{i}"), PtPlaceKind::control(sub)))
            .collect();
        let steps = [(0, Some(first)), (1, Some(second)), (2, None)];
        for (i, takes) in steps {
            let t = net.add_transition(
                format!("{name}{i}"),
                PtTransitionKind::new(TransitionType::Normal),
            );
            net.add_arc(at[i], t, ArcDir::Input, 1, ());
            net.add_arc(at[i + 1], t, ArcDir::Output, 1, ());
            match takes {
                Some(lock) => net.add_arc(lock, t, ArcDir::Input, 1, ()),
                None => {
                    net.add_arc(first, t, ArcDir::Output, 1, ());
                    net.add_arc(second, t, ArcDir::Output, 1, ());
                }
            }
        }
    }
    (net, Marking::new(vec![1, 1, 1, 0, 0, 0, 1, 0, 0, 0]))
}

#[test]
fn restricting_to_resources_and_inner_control_points_finds_one_law_per_lock() {
    let (net, initial) = lock_order();
    let all = p_semiflows(&net, limits()).unwrap();
    // One per thread (a0+a1+a2+a3, b0+…+b3) and one per lock.
    assert_eq!(all.len(), 4);

    let inner = p_semiflows_within(
        &net,
        |p| {
            !matches!(
                p.label(),
                PlaceClass::Control(ControlSub::FunctionStart | ControlSub::FunctionEnd)
            )
        },
        limits(),
    )
    .unwrap();
    assert_eq!(inner.len(), 2);
    for resource in [PlaceId(0), PlaceId(1)] {
        let law = inner
            .iter()
            .find(|y| y.coefficient(resource) == 1)
            .expect("every lock is covered");
        assert!(
            law.support()
                .any(|p| net.place(p).unwrap().kind.place_type.control().is_some()),
            "and its law involves the control points that hold it"
        );
        assert_eq!(
            p_invariant_status(&net, law, &initial),
            InvariantStatus::Holds
        );
        assert!(
            all.contains(law),
            "a restricted semiflow is a semiflow of the net"
        );
    }
    let names = |y: &PSemiflow| -> Vec<String> {
        y.support()
            .map(|p| net.place(p).unwrap().name.clone())
            .collect()
    };
    assert_eq!(names(&inner[0]), ["x", "a1", "a2", "b2"]);
    assert_eq!(names(&inner[1]), ["y", "a2", "b1", "b2"]);
}

// ── Limits ───────────────────────────────────────────────────────────────

#[test]
fn coefficients_that_do_not_fit_are_an_error_not_a_wrong_answer() {
    // Column k forces y(p_{k+1}) = 2^62 · y(p_k): the coefficients grow past
    // i128 by the third place.
    let mut b = LabeledBuilder::new();
    let ps: Vec<_> = (0..4)
        .map(|i| b.add_place(format!("p{i}"), Tag(())))
        .collect();
    for k in 0..3 {
        let t = b.add_transition(format!("t{k}"), Tag(()));
        b.add_input_arc(ps[k], t, 1 << 62);
        b.add_output_arc(ps[k + 1], t, 1);
    }
    let (net, _) = b.build();
    assert_eq!(p_semiflows(&net, limits()), Err(SemiflowError::Overflow));
}

#[test]
fn exceeding_the_row_limit_is_an_error_not_a_truncated_answer() {
    let (net, _) = philosophers(4);
    assert_eq!(
        p_semiflows(&net, FarkasLimits { max_rows: 5 }),
        Err(SemiflowError::RowLimit { limit: 5 })
    );
}
