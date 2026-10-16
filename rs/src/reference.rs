// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Frozen reference permutation — the definitional oracle.
//!
//! The hemera 0.4 permutation written from its definition over raw `u64`:
//! the external layer as the explicit product of the 16×16 matrix
//! `circ(2·M4, M4, M4, M4)` built from the entries of M4 (no addition
//! schedule), the internal layer as `1 + diag(d)`, x⁷ by plain
//! multiplication, reductions through one 128-bit reduction. It shares no
//! code with the optimised field arithmetic and kernels and must never be
//! optimised: every fast path in `field.rs` / `kernel.rs` /
//! `permutation.rs` is checked against it, element by element on
//! canonical values, over millions of random inputs
//! (`rs/tests/differential.rs`).

use crate::constants::ROUND_CONSTANTS_U64;

const P: u64 = 0xFFFF_FFFF_0000_0001;
const NEG_ORDER: u64 = P.wrapping_neg();

/// Internal diagonal, identical to `field::MATRIX_DIAG_16` (raw values):
/// the Poseidon2 reference `MAT_DIAG16_M_1` for Goldilocks.
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

/// The 4×4 block of the external layer (Poseidon2 reference).
pub const M4: [[u64; 4]; 4] = [[5, 7, 1, 3], [4, 6, 1, 1], [1, 3, 5, 7], [1, 1, 4, 6]];

/// `y = M_E · x` with `M_E[i][j] = (2 if i/4 == j/4 else 1) · M4[i%4][j%4]`.
fn external(state: &mut [u64; 16]) {
    let x = *state;
    for (i, e) in state.iter_mut().enumerate() {
        let mut acc = 0u128;
        for (j, &xj) in x.iter().enumerate() {
            let k = if i / 4 == j / 4 { 2 } else { 1 };
            acc += u128::from(k * M4[i % 4][j % 4]) * u128::from(xj);
        }
        *e = (acc % u128::from(P)) as u64;
    }
}

/// `y = (1 + diag(d)) · x`: `y_i = d_i x_i + Σ_j x_j`.
fn internal(state: &mut [u64; 16], diag: &[u64; 16]) {
    let wide: u128 = state.iter().map(|&x| u128::from(x)).sum();
    let sum = reduce128(wide);
    for i in 0..16 {
        state[i] = add(mul(state[i], diag[i]), sum);
    }
}

/// Generic reference permutation: initial M_E, `rf_half` full rounds,
/// `rp` partial rounds (x⁷ on `state[0]`, then M_I), the other `rf_half`
/// full rounds. `constants` = `2·rf_half·16` external then `rp` internal
/// constants. Output canonical.
pub fn permute_generic(
    state: &mut [u64; 16],
    rf_half: usize,
    rp: usize,
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
        state[0] = pow7(add(state[0], *c));
        internal(state, diag);
    }
    for r in rf_half..2 * rf_half {
        full(state, &ext[r * 16..(r + 1) * 16]);
    }
    for e in state.iter_mut() {
        *e = canon(*e);
    }
}

/// The hemera 0.4 permutation (RF 6+6, RP 48, x⁷ in every round).
pub fn permute(state: &mut [u64; 16]) {
    permute_generic(state, 6, 48, &ROUND_CONSTANTS_U64, &DIAG);
}
