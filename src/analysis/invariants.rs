//! 库位/变迁不变量（feature `invariants`）。
//!
//! 基于效果矩阵 `C = Post − Pre` 的零空间计算（Gaussian elimination，
//! BigRational 精确 + 整数规范化）。移植自 ConcBugDect 的 proven 实现。
//!
//! - 库位不变量：`y·C = 0`（守恒量，如"互斥锁 token + 持有者 = 常量"）。
//! - 变迁不变量：`C·x = 0`（T-不变量，如循环净效果为零）。
#![allow(clippy::needless_range_loop, clippy::collapsible_if)]

use num_bigint::BigInt;
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{One, Signed, Zero};

use crate::net::Net;
use crate::netlike::NetLike;
use crate::storage::effect_matrix;

/// 库位不变量（行向量，长度 = |P|）。
pub fn place_invariants(net: &Net) -> Vec<Vec<BigInt>> {
    let c = effect_matrix(net.pre(), net.post(), net.num_places());
    let places = net.num_places();
    let transitions = net.num_transitions();

    // 转置：行 = transition，列 = place。
    let mut transposed = vec![vec![BigInt::from(0); places]; transitions];
    for (place, row) in c.iter().enumerate() {
        for (t, v) in row.iter().enumerate() {
            transposed[t][place] = BigInt::from(*v);
        }
    }
    compute_nullspace(&transposed, places)
}

/// 变迁不变量（列向量，长度 = |T|）。
pub fn transition_invariants(net: &Net) -> Vec<Vec<BigInt>> {
    let c = effect_matrix(net.pre(), net.post(), net.num_places());
    let rows: Vec<Vec<BigInt>> = c
        .iter()
        .map(|row| row.iter().map(|v| BigInt::from(*v)).collect())
        .collect();
    compute_nullspace(&rows, net.num_transitions())
}

fn compute_nullspace(matrix: &[Vec<BigInt>], cols: usize) -> Vec<Vec<BigInt>> {
    if cols == 0 {
        return Vec::new();
    }
    let rows = matrix.len();
    if rows == 0 {
        return (0..cols)
            .map(|free_col| {
                let mut vector = vec![BigInt::from(0); cols];
                vector[free_col] = BigInt::from(1);
                vector
            })
            .collect();
    }

    let mut rref: Vec<Vec<BigRational>> = matrix
        .iter()
        .map(|row| {
            (0..cols)
                .map(|idx| {
                    let v = row.get(idx).cloned().unwrap_or_else(BigInt::zero);
                    BigRational::from_integer(v)
                })
                .collect()
        })
        .collect();

    let mut pivot_cols = Vec::new();
    let mut pivot_row = 0usize;

    for col in 0..cols {
        if pivot_row >= rows {
            break;
        }
        let mut pivot = None;
        for row in pivot_row..rows {
            if !rref[row][col].is_zero() {
                pivot = Some(row);
                break;
            }
        }
        let Some(row_idx) = pivot else {
            continue;
        };

        if row_idx != pivot_row {
            rref.swap(row_idx, pivot_row);
        }

        let pivot_value = rref[pivot_row][col].clone();
        for value in rref[pivot_row].iter_mut() {
            *value /= pivot_value.clone();
        }

        for row in 0..rows {
            if row == pivot_row {
                continue;
            }
            let factor = rref[row][col].clone();
            if factor.is_zero() {
                continue;
            }
            for inner_col in col..cols {
                let adjustment = rref[pivot_row][inner_col].clone() * factor.clone();
                rref[row][inner_col] -= adjustment;
            }
        }

        pivot_cols.push(col);
        pivot_row += 1;
    }

    let mut pivot_flags = vec![false; cols];
    for &col in &pivot_cols {
        pivot_flags[col] = true;
    }
    let free_cols: Vec<usize> = (0..cols).filter(|&c| !pivot_flags[c]).collect();
    if free_cols.is_empty() {
        return Vec::new();
    }

    let mut basis = Vec::new();
    for &free_col in &free_cols {
        let mut vector = vec![BigRational::zero(); cols];
        vector[free_col] = BigRational::one();
        for (pivot_index, &pivot_col) in pivot_cols.iter().enumerate() {
            let coeff = rref[pivot_index][free_col].clone();
            if !coeff.is_zero() {
                vector[pivot_col] = -coeff;
            }
        }
        basis.push(rational_vector_to_integer(vector));
    }

    basis.into_iter().map(normalize_integer_vector).collect()
}

fn rational_vector_to_integer(vector: Vec<BigRational>) -> Vec<BigInt> {
    let mut lcm = BigInt::one();
    for value in &vector {
        let denom = value.denom();
        if denom.is_zero() {
            continue;
        }
        let d = denom.clone();
        lcm = lcm.lcm(&d);
    }
    vector
        .into_iter()
        .map(|value| {
            let numer = value.numer().clone();
            let denom = value.denom().clone();
            if denom.is_zero() {
                BigInt::zero()
            } else {
                let scale = &lcm / &denom;
                numer * scale
            }
        })
        .collect()
}

fn normalize_integer_vector(mut vector: Vec<BigInt>) -> Vec<BigInt> {
    let mut gcd = BigInt::zero();
    for value in &vector {
        if value.is_zero() {
            continue;
        }
        let abs = value.abs();
        gcd = if gcd.is_zero() { abs } else { gcd.gcd(&abs) };
    }
    if !gcd.is_zero() && gcd != BigInt::one() {
        for value in &mut vector {
            *value /= gcd.clone();
        }
    }
    vector
}
