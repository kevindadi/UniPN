//! CSC 稀疏列 incidence 存储。
//!
//! `Pre/Post ∈ ℕ^{|P|×|T|}` 用**列优先稀疏（CSC）**表示：每个变迁一列，
//! 只存非零 `(place, weight)`。enabled/fire 热路径复杂度 O(弧数) 而非
//! O(|P|·|T|)。需要线性代数时（不变量等）才物化稠密 `C = Post − Pre`。

use crate::ids::{PlaceId, TransitionId, Weight};

/// 稀疏 incidence 矩阵（列优先）。
#[derive(Clone, Debug, Default)]
pub struct Incidence {
    /// 每变迁一列：`Vec<(place_index, weight)>`。
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

    /// 添加一条弧（累积权重）。
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

    /// 变迁 t 的输入/输出列（preset/postset）。
    pub fn column(&self, t: TransitionId) -> &[(usize, Weight)] {
        self.cols
            .get(t.index())
            .map_or(&[], |c| c.as_slice())
    }

    /// 遍历所有非零项 `(transition, place, weight)`。
    pub fn iter(&self) -> impl Iterator<Item = (TransitionId, PlaceId, Weight)> + '_ {
        self.cols
            .iter()
            .enumerate()
            .flat_map(|(t, col)| {
                col.iter()
                    .map(move |&(p, w)| (TransitionId(t), PlaceId(p), w))
            })
    }

    /// 物化稠密矩阵（行 = place，列 = transition，i64 便于差分）。
    pub fn dense(&self, places: usize) -> Vec<Vec<i64>> {
        let mut m = vec![vec![0i64; self.cols.len()]; places];
        for (t, p, w) in self.iter() {
            m[p.index()][t.index()] = w as i64;
        }
        m
    }
}

/// 效果矩阵 `C = Post − Pre`（稠密，|P|×|T|）。供不变量/结构分析。
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
