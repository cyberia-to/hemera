// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Fast permutation kernel, generic over a [`Profile`] and a lane count.
//!
//! The state is laid out structure-of-arrays, `[[u64; L]; 16]`: element
//! `i` of the `L` independent permutations sits in `s[i][0..L]`, so every
//! step is a loop over lanes the compiler unrolls into `L` independent
//! dependency chains. `L = 1` is the single-permutation path.
//!
//! Field-equal to `reference::permute_generic` on every input; the output
//! is canonical. Differences from the reference structure (all exact in
//! the field):
//!
//! - external layer: accumulate in `u128` and reduce once per element
//!   (coefficients ≤ 80, so the high word is < 2⁷) instead of modular
//!   additions per step;
//! - partial rounds: the next S-box input is carried directly as
//!   `x' = (d₀+1)·y + (Σ_{i≥1} sᵢ + c_{r+1})`, one fused multiply-add after
//!   the S-box, so the serial path of a partial round is x⁷ (three dependent
//!   multiplications) plus that multiply-add; the fifteen rows
//!   `sᵢ' = dᵢ·sᵢ + (y + Σ)` are independent fused multiply-adds beside it.

#![allow(clippy::needless_range_loop)] // SoA: rows and lanes are indexed together

use crate::arith::{add_c, pow7, pow7_t, reduce_small};
use crate::backend::Arith;
use crate::profile::Profile;

/// `L` width-16 states, structure-of-arrays.
pub(crate) type Lanes<const L: usize> = [[u64; L]; 16];

/// Poseidon2 4×4 block `[[5,7,1,3],[4,6,1,1],[1,3,5,7],[1,1,4,6]]`, the
/// same addition schedule as `field::apply_mat4`, without reduction.
#[inline(always)]
fn mat4_wide(x: [u128; 4]) -> [u128; 4] {
    let t0 = x[0] + x[1];
    let t1 = x[2] + x[3];
    let t2 = 2 * x[1] + t1;
    let t3 = 2 * x[3] + t0;
    let t4 = 4 * t1 + t3;
    let t5 = 4 * t0 + t2;
    [t3 + t5, t5, t2 + t4, t4]
}

/// External layer `circ(2M, M, M, M)` on every lane. Each output is a
/// combination of 16 inputs with coefficients summing to ≤ 2·16 + 3·16 = 80
/// (M's row sums are 16, 12, 16, 12), so the `u128` accumulator stays below
/// 2^71 and `reduce_small` applies.
#[inline(always)]
fn external<const L: usize>(s: &mut Lanes<L>) {
    for l in 0..L {
        let mut m = [0u128; 16];
        for c in 0..4 {
            let y = mat4_wide(core::array::from_fn(|k| u128::from(s[4 * c + k][l])));
            m[4 * c..4 * c + 4].copy_from_slice(&y);
        }
        let sums: [u128; 4] = core::array::from_fn(|k| m[k] + m[4 + k] + m[8 + k] + m[12 + k]);
        for i in 0..16 {
            s[i][l] = reduce_small(m[i] + sums[i & 3]);
        }
    }
}

/// Add round constants, x⁷ on all 16 elements, external layer.
#[inline(always)]
fn full_round<A: Arith, const L: usize>(s: &mut Lanes<L>, rc: &[u64]) {
    for i in 0..16 {
        for l in 0..L {
            s[i][l] = pow7_t::<A>(add_c(s[i][l], rc[i]));
        }
    }
    external(s);
}

/// `Σ_{i≥1} s[i]` per lane (< 15·2^64, so `reduce_small` applies).
#[inline(always)]
fn sum_rest<const L: usize>(s: &Lanes<L>) -> [u64; L] {
    core::array::from_fn(|l| {
        let mut acc = 0u128;
        for row in &s[1..] {
            acc += u128::from(row[l]);
        }
        reduce_small(acc)
    })
}

/// The `rp` partial rounds: `s₀ += c; s₀ = s₀⁷; s = (1 + diag(d))·s`.
///
/// With `y = x⁷` and `rest = Σ_{i≥1} sᵢ`, round r's internal layer gives
/// `s₀' = (d₀+1)·y + rest` and `sᵢ' = dᵢ·sᵢ + (y + rest)` (i ≥ 1). The
/// next S-box input `x' = s₀' + c_{r+1}` is formed straight from `y` and
/// the old `rest`, so it does not wait for the fifteen rows; `s₀` itself
/// is written only after the last round.
#[inline(always)]
fn partial_rounds<A: Arith, const L: usize>(s: &mut Lanes<L>, p: &Profile<'_>) {
    if p.rp == 0 {
        return;
    }
    let d = p.diag;
    let d0p1 = A::add(d[0], 1);
    let c = p.internal;
    let mut x: [u64; L] = core::array::from_fn(|l| add_c(s[0][l], c[0]));
    let mut rest = sum_rest(s);
    for r in 0..p.rp {
        let y: [u64; L] = core::array::from_fn(|l| pow7::<A>(x[l]));
        let sum: [u64; L] = core::array::from_fn(|l| A::add(y[l], rest[l]));
        if r + 1 < p.rp {
            x = core::array::from_fn(|l| A::mul_add(y[l], d0p1, add_c(rest[l], c[r + 1])));
        } else {
            for l in 0..L {
                s[0][l] = A::mul_add(y[l], d0p1, rest[l]);
            }
        }
        for k in 1..16 {
            for l in 0..L {
                s[k][l] = A::mul_add_t(s[k][l], d[k], sum[l]);
            }
        }
        if r + 1 < p.rp {
            rest = sum_rest(s);
        }
    }
}

/// The full permutation on `L` independent states; output canonical.
///
/// The full-round loops iterate `chunks_exact(16)`: the `as_chunks` and
/// indexed forms measured ~20 % slower on the single-permutation path
/// (rustc 1.95, M4 Max, `examples/split.rs`), so the lint is allowed here.
#[inline(always)]
#[allow(unknown_lints, clippy::chunks_exact_to_as_chunks)]
pub(crate) fn permute_lanes<A: Arith, const L: usize>(s: &mut Lanes<L>, p: &Profile<'_>) {
    external(s);
    let (initial, terminal) = p.external.split_at(p.rf_half * 16);
    for rc in initial.chunks_exact(16) {
        full_round::<A, L>(s, rc);
    }
    partial_rounds::<A, L>(s, p);
    for rc in terminal.chunks_exact(16) {
        full_round::<A, L>(s, rc);
    }
    for row in s.iter_mut() {
        for v in row.iter_mut() {
            *v = A::canon(*v);
        }
    }
}
