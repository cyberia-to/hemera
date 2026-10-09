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
//!   (coefficients ≤ 35, so the high word is < 64) instead of ~4 modular
//!   additions per element;
//! - partial rounds: the next S-box input is carried directly as
//!   `x' = (d₀+1)·y + (Σ_{i≥1} sᵢ + c_{r+1})`, one fused multiply-add after
//!   the S-box, the lane sum being computed beside the S-box;
//!   `sᵢ' = dᵢ·sᵢ + (y + Σ)` is one fused multiply-add per element;
//! - partial x⁻¹ with `L > 1`: Montgomery's batch inversion across lanes
//!   (one inversion + 3(L−1) multiplications instead of L inversions);
//!   zero lanes are replaced by 1 in the product and masked to 0 after.

#![allow(clippy::needless_range_loop)] // SoA: rows and lanes are indexed together

use crate::arith::{
    add, add_c, canon, inv_nonzero_fill, mul, mul_add, mul_add_t, pow7, pow7_t, reduce_small,
};
use crate::profile::{PartialSbox, Profile};

/// `L` width-16 states, structure-of-arrays.
pub(crate) type Lanes<const L: usize> = [[u64; L]; 16];

/// Poseidon2 4×4 block `[[2,3,1,1],[1,2,3,1],[1,1,2,3],[3,1,1,2]]`, the
/// same addition schedule as `field::apply_mat4`, without reduction.
#[inline(always)]
fn mat4_wide(x: [u128; 4]) -> [u128; 4] {
    let t01 = x[0] + x[1];
    let t23 = x[2] + x[3];
    let t0123 = t01 + t23;
    let t01123 = t0123 + x[1];
    let t01233 = t0123 + x[3];
    [
        t01123 + t01,
        t01123 + 2 * x[2],
        t01233 + t23,
        t01233 + 2 * x[0],
    ]
}

/// External layer `circ(2M, M, M, M)` on every lane. Each output is a
/// combination of 16 inputs with coefficients summing to ≤ 35, so the
/// `u128` accumulator stays below 2^70 and `reduce_small` applies.
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
fn full_round<const L: usize>(s: &mut Lanes<L>, rc: &[u64]) {
    for i in 0..16 {
        for l in 0..L {
            s[i][l] = pow7_t(add_c(s[i][l], rc[i]));
        }
    }
    external(s);
}

/// x⁻¹ (0 ↦ 0) of every lane — direct for one lane, Montgomery's trick
/// across lanes otherwise — with `tasks` work items interleaved into the
/// inversion chain (see `arith::inv_nonzero_fill`).
#[inline(always)]
fn inv_lanes_fill<const L: usize, F: FnMut(usize)>(
    x: [u64; L],
    fill: &mut F,
    tasks: usize,
) -> [u64; L] {
    let zero: [bool; L] = core::array::from_fn(|l| canon(x[l]) == 0);
    let a: [u64; L] = core::array::from_fn(|l| if zero[l] { 1 } else { x[l] });
    let mut prefix = [0u64; L];
    prefix[0] = a[0];
    for l in 1..L {
        prefix[l] = mul(prefix[l - 1], a[l]);
    }
    let mut t = inv_nonzero_fill(prefix[L - 1], fill, tasks);
    let mut out = [0u64; L];
    for l in (1..L).rev() {
        out[l] = mul(t, prefix[l - 1]);
        t = mul(t, a[l]);
    }
    out[0] = t;
    for l in 0..L {
        if zero[l] {
            out[l] = 0;
        }
    }
    out
}

/// Partial-round S-box on every lane, with `tasks` work items run in its
/// shadow (all of them, exactly once, in order).
#[inline(always)]
fn sbox_lanes_fill<const L: usize, F: FnMut(usize)>(
    x: [u64; L],
    kind: PartialSbox,
    fill: &mut F,
    tasks: usize,
) -> [u64; L] {
    match kind {
        PartialSbox::Inverse => inv_lanes_fill(x, fill, tasks),
        PartialSbox::Pow7 => {
            let y = core::array::from_fn(|l| pow7(x[l]));
            for k in 0..tasks {
                fill(k);
            }
            y
        }
    }
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

/// The `rp` partial rounds: `s₀ += c; s₀ = S(s₀); s = (1 + diag(d))·s`.
///
/// Only `x → S(x) → x'` is serial. With `y = S(x)` and
/// `rest = Σ_{i≥1} sᵢ`, the next S-box input is carried directly as
/// `x' = (d₀+1)·y + (rest + c_{r+1})` (one fused multiply-add after the
/// S-box), so the S-box of round r+1 starts before round r's internal
/// layer `sᵢ' = dᵢ·sᵢ + (y + rest)` (i ≥ 1) is done: those fifteen rows
/// are the work interleaved into round r+1's inversion chain, and
/// `rest' = Σ_{i≥1} sᵢ'` is summed after it, in time for `x''`.
#[inline(always)]
fn partial_rounds<const L: usize>(s: &mut Lanes<L>, p: &Profile<'_>) {
    if p.rp == 0 {
        return;
    }
    let d = p.diag;
    let d0p1 = add(d[0], 1);
    let c = p.internal;
    let x: [u64; L] = core::array::from_fn(|l| add_c(s[0][l], c[0]));
    let mut rest = sum_rest(s);
    let mut y = sbox_lanes_fill(x, p.partial, &mut |_| {}, 0);
    for r in 0..p.rp {
        let sum: [u64; L] = core::array::from_fn(|l| add(y[l], rest[l]));
        let mut row = |k: usize| {
            for l in 0..L {
                s[k + 1][l] = mul_add_t(s[k + 1][l], d[k + 1], sum[l]);
            }
        };
        if r + 1 < p.rp {
            let x: [u64; L] =
                core::array::from_fn(|l| mul_add(y[l], d0p1, add_c(rest[l], c[r + 1])));
            let y_next = sbox_lanes_fill(x, p.partial, &mut row, 15);
            rest = sum_rest(s);
            y = y_next;
        } else {
            for k in 0..15 {
                row(k);
            }
            for l in 0..L {
                s[0][l] = mul_add(y[l], d0p1, rest[l]);
            }
        }
    }
}

/// The full permutation on `L` independent states; output canonical.
#[inline(always)]
pub(crate) fn permute_lanes<const L: usize>(s: &mut Lanes<L>, p: &Profile<'_>) {
    external(s);
    let (initial, terminal) = p.external.split_at(p.rf_half * 16);
    for rc in initial.chunks_exact(16) {
        full_round(s, rc);
    }
    partial_rounds(s, p);
    for rc in terminal.chunks_exact(16) {
        full_round(s, rc);
    }
    for row in s.iter_mut() {
        for v in row.iter_mut() {
            *v = canon(*v);
        }
    }
}
