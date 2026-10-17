// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Frozen reference permutation — the definitional oracle.
//!
//! The hemera 0.3.1 arithmetic (same reduction, same matrices, same round
//! order) with the inverse computed by plain square-and-multiply over the
//! bits of p−2, i.e. the definition rather than any addition chain. This
//! module is the oracle for the differential tests and must never be
//! optimised: every fast path
//! in `field.rs` / `permutation.rs` is checked against it, element by
//! element on canonical values, over millions of random inputs
//! (`rs/tests/differential.rs`). It is written over raw `u64` so it shares
//! no code with the optimised field arithmetic.

use crate::constants::ROUND_CONSTANTS_U64;

const P: u64 = 0xFFFF_FFFF_0000_0001;
const NEG_ORDER: u64 = P.wrapping_neg();

/// Internal diagonal, identical to `field::MATRIX_DIAG_16` (raw values).
pub const DIAG: [u64; 16] = [
    0xde9b91a467d6afc0,
    0xc5f16b9c76a9be17,
    0x0ab0fef2d540ac55,
    0x3001d27009d05773,
    0xed23b1f906d3d9eb,
    0x5ce73743cba97054,
    0x1c3bab944af4ba24,
    0x2faa105854dbafae,
    0x53ffb3ae6d421a10,
    0xbcda9df8884ba396,
    0xfc1273e4a31807bb,
    0xc77952573d5142c0,
    0x56683339a819b85e,
    0x328fcbd8f0ddc8eb,
    0xb5101e303fce9cb7,
    0x774487b8c40089bb,
];

#[inline]
fn canon(x: u64) -> u64 {
    if x >= P { x - P } else { x }
}

/// Field addition (any representatives).
#[inline]
pub fn add(a: u64, b: u64) -> u64 {
    let (sum, over) = a.overflowing_add(b);
    let (mut sum, over) = sum.overflowing_add(u64::from(over) * NEG_ORDER);
    if over {
        sum += NEG_ORDER;
    }
    sum
}

#[inline]
fn reduce128(x: u128) -> u64 {
    let x_lo = x as u64;
    let x_hi = (x >> 64) as u64;
    let x_hi_hi = x_hi >> 32;
    let x_hi_lo = x_hi & NEG_ORDER;
    let (mut t0, borrow) = x_lo.overflowing_sub(x_hi_hi);
    if borrow {
        t0 -= NEG_ORDER;
    }
    let t1 = x_hi_lo * NEG_ORDER;
    let (res, carry) = t0.overflowing_add(t1);
    res + NEG_ORDER * u64::from(carry)
}

#[inline]
fn mul(a: u64, b: u64) -> u64 {
    reduce128(u128::from(a) * u128::from(b))
}

#[inline]
fn pow7(x: u64) -> u64 {
    let x2 = mul(x, x);
    let x3 = mul(x2, x);
    let x4 = mul(x2, x2);
    mul(x3, x4)
}

/// x^(p-2) by plain square-and-multiply over the bits of p-2 (the
/// definition, not the addition chain), with 0 -> 0.
pub fn inv(x: u64) -> u64 {
    if canon(x) == 0 {
        return 0;
    }
    let e = P - 2;
    let mut acc = 1u64;
    for bit in (0..64).rev() {
        acc = mul(acc, acc);
        if (e >> bit) & 1 == 1 {
            acc = mul(acc, x);
        }
    }
    acc
}

fn mat4(x: &mut [u64]) {
    let t01 = add(x[0], x[1]);
    let t23 = add(x[2], x[3]);
    let t0123 = add(t01, t23);
    let t01123 = add(t0123, x[1]);
    let t01233 = add(t0123, x[3]);
    x[3] = add(t01233, add(x[0], x[0]));
    x[1] = add(t01123, add(x[2], x[2]));
    x[0] = add(t01123, t01);
    x[2] = add(t01233, t23);
}

fn external(state: &mut [u64; 16]) {
    for chunk in state.chunks_exact_mut(4) {
        mat4(chunk);
    }
    let mut sums = [0u64; 4];
    for (k, s) in sums.iter_mut().enumerate() {
        let wide: u128 = (0..4).map(|j| u128::from(state[4 * j + k])).sum();
        *s = reduce128(wide);
    }
    for (i, e) in state.iter_mut().enumerate() {
        *e = add(*e, sums[i % 4]);
    }
}

fn internal(state: &mut [u64; 16], diag: &[u64; 16]) {
    let wide: u128 = state.iter().map(|&x| u128::from(x)).sum();
    let sum = reduce128(wide);
    for i in 0..16 {
        state[i] = add(mul(state[i], diag[i]), sum);
    }
}

/// Generic reference permutation: `rf_half` full rounds, `rp` partial
/// rounds with S-box `x^-1` (`inverse = true`) or `x^7`, then the other
/// `rf_half` full rounds. `constants` = `2·rf_half·16` external then `rp`
/// internal constants. Output canonical.
pub fn permute_generic(
    state: &mut [u64; 16],
    rf_half: usize,
    rp: usize,
    inverse: bool,
    constants: &[u64],
    diag: &[u64; 16],
) {
    assert_eq!(constants.len(), 2 * rf_half * 16 + rp);
    let (ext, int) = constants.split_at(2 * rf_half * 16);
    let full = |state: &mut [u64; 16], rc: &[u64]| {
        for i in 0..16 {
            state[i] = pow7(add(state[i], rc[i]));
        }
        external(state);
    };
    external(state);
    for r in 0..rf_half {
        full(state, &ext[r * 16..(r + 1) * 16]);
    }
    for c in int.iter().take(rp) {
        let x = add(state[0], *c);
        state[0] = if inverse { inv(x) } else { pow7(x) };
        internal(state, diag);
    }
    for r in rf_half..2 * rf_half {
        full(state, &ext[r * 16..(r + 1) * 16]);
    }
    for e in state.iter_mut() {
        *e = canon(*e);
    }
}

/// The hemera 0.3.1 permutation (RF 4+4, RP 16, partial S-box x^-1).
pub fn permute(state: &mut [u64; 16]) {
    permute_generic(state, 4, 16, true, &ROUND_CONSTANTS_U64, &DIAG);
}
