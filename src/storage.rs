//! CSC sparse-column incidence storage.
//!
//! `Pre/Post ∈ ℕ^{|P|×|T|}` is stored column-major sparse (CSC): one column
//! per transition holding only the non-zero `(place, weight)` entries. The
//! enabled/fire hot path is O(|arcs|) instead of O(|P|·|T|). The dense
//! `C = Post − Pre` matrix is only materialized when linear algebra is needed
//! (e.g. invariants).

use crate::ids::{PlaceId, TransitionId, Weight};

/// Sparse incidence matrix (column-major).
#[derive(Clone, Debug, Default)]
pub struct Incidence {
    /// One column per transition: `Vec<(place_index, weight)>`.
    cols: Vec<Vec<(usize, Weight)>>,
}

impl Incidence {
    pub fn with_transitions(transitions: usize) -> Self {
        Self {
            cols: vec![Vec::new(); transitions],
        }
    }

    pub fn transitions(&self) -> usize {
        self.cols.len()
    }

    /// Add an arc (accumulates weight).
    pub fn add(&mut self, t: TransitionId, p: PlaceId, weight: Weight) {
        if weight == 0 {
            return;
        }
        let col = &mut self.cols[t.index()];
        match col.iter_mut().find(|(pi, _)| *pi == p.index()) {
            Some((_, w)) => *w += weight,
            None => col.push((p.index(), weight)),
        }
    }

    /// The column of transition `t` (its preset/postset).
    pub fn column(&self, t: TransitionId) -> &[(usize, Weight)] {
        self.cols
            .get(t.index())
            .map_or(&[], |c| c.as_slice())
    }

    /// Iterate over all non-zero entries `(transition, place, weight)`.
    pub fn iter(&self) -> impl Iterator<Item = (TransitionId, PlaceId, Weight)> + '_ {
        self.cols
            .iter()
            .enumerate()
            .flat_map(|(t, col)| {
                col.iter()
                    .map(move |&(p, w)| (TransitionId(t), PlaceId(p), w))
            })
    }

    /// Materialize a dense matrix (row = place, column = transition, i64 for
    /// differencing).
    pub fn dense(&self, places: usize) -> Vec<Vec<i64>> {
        let mut m = vec![vec![0i64; self.cols.len()]; places];
        for (t, p, w) in self.iter() {
            m[p.index()][t.index()] = w as i64;
        }
        m
    }
}

/// Effect matrix `C = Post − Pre` (dense, |P|×|T|). Used for invariants and
/// structural analysis.
pub fn effect_matrix(pre: &Incidence, post: &Incidence, places: usize) -> Vec<Vec<i64>> {
    let pre_d = pre.dense(places);
    let post_d = post.dense(places);
    let t = pre.transitions().max(post.transitions());
    let mut c = vec![vec![0i64; t]; places];
    for p in 0..places {
        for tt in 0..t {
            c[p][tt] = post_d[p][tt] - pre_d[p][tt];
        }
    }
    c
}
