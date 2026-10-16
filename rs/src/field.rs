// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Goldilocks prime field (p = 2^64 - 2^32 + 1).
//!
//! Minimal implementation covering the operations hemera needs:
//! addition, subtraction, multiplication, the x^7 S-box and the two
//! linear layers of the permutation.

use core::ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign};

/// The Goldilocks prime: p = 2^64 - 2^32 + 1.
pub const P: u64 = 0xFFFF_FFFF_0000_0001;

/// Two's complement of P modulo 2^64: 2^32 - 1.
const NEG_ORDER: u64 = P.wrapping_neg(); // 0xFFFF_FFFF

/// A Goldilocks field element.
///
/// Internal value may be non-canonical (in `[0, 2^64)`).
/// Use `as_canonical_u64()` to reduce to `[0, p)`.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Goldilocks {
    value: u64,
}

impl Goldilocks {
    pub const ZERO: Self = Self { value: 0 };

    #[inline]
    pub const fn new(value: u64) -> Self {
        Self { value }
    }

    /// Reduce to canonical form in [0, p).
    #[inline]
    pub fn as_canonical_u64(self) -> u64 {
        // Mask, not branch: hash outputs of secret inputs pass through here.
        self.value.wrapping_sub(P & u64::from(self.value >= P).wrapping_neg())
    }

    /// Compute x^2.
    #[inline]
    fn square(self) -> Self {
        self * self
    }

    /// Compute x^7 (the hemera S-box, every round).
    #[inline]
    pub fn pow7(self) -> Self {
        let x2 = self.square();
        let x3 = x2 * self;
        let x4 = x2.square();
        x3 * x4
    }

    /// The raw representative in `[0, 2^64)` (not necessarily canonical).
    #[inline(always)]
    pub(crate) const fn raw(self) -> u64 {
        self.value
    }

    /// Double this element.
    #[inline]
    fn double(self) -> Self {
        self + self
    }
}

impl core::fmt::Debug for Goldilocks {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Goldilocks({})", self.as_canonical_u64())
    }
}

// ── Arithmetic ──────────────────────────────────────────────────────

impl Add for Goldilocks {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        let (sum, over) = self.value.overflowing_add(rhs.value);
        let (sum, over) = sum.overflowing_add(u64::from(over) * NEG_ORDER);
        // Mask, not branch: the sponge absorbs secret input with this add.
        Self::new(sum.wrapping_add(NEG_ORDER & u64::from(over).wrapping_neg()))
    }
}

impl AddAssign for Goldilocks {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Goldilocks {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        let (diff, under) = self.value.overflowing_sub(rhs.value);
        let (diff, under) = diff.overflowing_sub(u64::from(under) * NEG_ORDER);
        Self::new(diff.wrapping_sub(NEG_ORDER & u64::from(under).wrapping_neg()))
    }
}

impl SubAssign for Goldilocks {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul for Goldilocks {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        // Same representative as `reduce128(a·b)`; latency-scheduled on aarch64.
        Self::new(crate::arith::mul(self.value, rhs.value))
    }
}

impl MulAssign for Goldilocks {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl core::iter::Sum for Goldilocks {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        reduce128(iter.map(|x| x.value as u128).sum::<u128>())
    }
}

/// Reduce a 128-bit product to a Goldilocks element.
///
/// Uses the identity 2^64 ≡ 2^32 - 1 (mod p).
#[inline]
fn reduce128(x: u128) -> Goldilocks {
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
    Goldilocks::new(res + NEG_ORDER * u64::from(carry))
}

// ── MDS and diffusion matrices ──────────────────────────────────────

/// Apply the 4×4 MDS block of the Poseidon2 external layer (Poseidon2
/// paper, Appendix B; the Horizen Labs reference matrix):
/// ```text
/// [ 5 7 1 3 ]
/// [ 4 6 1 1 ]
/// [ 1 3 5 7 ]
/// [ 1 1 4 6 ]
/// ```
/// Eight additions and four doublings (the paper's schedule).
#[inline(always)]
pub fn apply_mat4(x: &mut [Goldilocks; 4]) {
    let t0 = x[0] + x[1];
    let t1 = x[2] + x[3];
    let t2 = x[1].double() + t1;
    let t3 = x[3].double() + t0;
    let t4 = t1.double().double() + t3;
    let t5 = t0.double().double() + t2;
    let t6 = t3 + t5;
    let t7 = t2 + t4;
    x[0] = t6;
    x[1] = t5;
    x[2] = t7;
    x[3] = t4;
}

/// Apply the external linear layer M_E for a width-16 state.
///
/// Multiplies by the 16×16 block matrix
/// `[[2M M M M], [M 2M M M], [M M 2M M], [M M M 2M]]`
/// where M is the 4×4 MDS block of [`apply_mat4`] (Poseidon2 §5.1). M_E is
/// invertible and MDS block by block — see `specs/matrices.md`.
#[inline]
pub fn mds_light_permutation(state: &mut [Goldilocks; 16]) {
    // Apply M4 to each consecutive 4-element chunk.
    for chunk in state.chunks_exact_mut(4) {
        apply_mat4(chunk.try_into().unwrap());
    }

    // Compute column sums (one per M4 column position).
    let sums: [Goldilocks; 4] = core::array::from_fn(|k| {
        (0..16).step_by(4).map(|j| state[j + k]).sum()
    });

    // Add the appropriate column sum to each element.
    for (i, elem) in state.iter_mut().enumerate() {
        *elem += sums[i % 4];
    }
}

/// Diagonal of the internal matrix M_I = 1 + diag(d) for Goldilocks t = 16,
/// where 1 is the all-ones matrix.
///
/// The Poseidon2 reference instance (Horizen Labs `MAT_DIAG16_M_1`, also
/// Plonky3 `MATRIX_DIAG_16_GOLDILOCKS`): the 47th diagonal proposed by the
/// Grain LFSR of the Poseidon2 parameter script (field 1, S-box 0, n 64,
/// t 16, R_F 8, R_P 22), the first whose powers M_I^1 … M_I^32 all have an
/// irreducible characteristic polynomial. Derivation and conditions:
/// `specs/matrices.md`, `audit/campaign/poseidon2_params.py`,
/// `rs/tests/poseidon2_conditions.rs`.
pub const MATRIX_DIAG_16: [Goldilocks; 16] = [
    Goldilocks::new(0xde9b91a467d6afc0),
    Goldilocks::new(0xc5f16b9c76a9be17),
    Goldilocks::new(0x0ab0fef2d540ac55),
    Goldilocks::new(0x3001d27009d05773),
    Goldilocks::new(0xed23b1f906d3d9eb),
    Goldilocks::new(0x5ce73743cba97054),
    Goldilocks::new(0x1c3bab944af4ba24),
    Goldilocks::new(0x2faa105854dbafae),
    Goldilocks::new(0x53ffb3ae6d421a10),
    Goldilocks::new(0xbcda9df8884ba396),
    Goldilocks::new(0xfc1273e4a31807bb),
    Goldilocks::new(0xc77952573d5142c0),
    Goldilocks::new(0x56683339a819b85e),
    Goldilocks::new(0x328fcbd8f0ddc8eb),
    Goldilocks::new(0xb5101e303fce9cb7),
    Goldilocks::new(0x774487b8c40089bb),
];

/// Apply the internal diffusion matrix: M_I = 1 + diag(d).
///
/// Computes `state'[i] = d[i] * state[i] + sum(state)`.
#[inline]
pub fn matmul_internal(state: &mut [Goldilocks; 16]) {
    let sum: Goldilocks = state.iter().copied().sum();
    for i in 0..16 {
        state[i] *= MATRIX_DIAG_16[i];
        state[i] += sum;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_add_basic() {
        let a = Goldilocks::new(1);
        let b = Goldilocks::new(2);
        assert_eq!((a + b).as_canonical_u64(), 3);
    }

    #[test]
    fn field_add_wrap() {
        let a = Goldilocks::new(P - 1);
        let b = Goldilocks::new(1);
        assert_eq!((a + b).as_canonical_u64(), 0);
    }

    #[test]
    fn field_sub_basic() {
        let a = Goldilocks::new(5);
        let b = Goldilocks::new(3);
        assert_eq!((a - b).as_canonical_u64(), 2);
    }

    #[test]
    fn field_sub_wrap() {
        let a = Goldilocks::new(0);
        let b = Goldilocks::new(1);
        assert_eq!((a - b).as_canonical_u64(), P - 1);
    }

    #[test]
    fn field_mul_basic() {
        let a = Goldilocks::new(3);
        let b = Goldilocks::new(7);
        assert_eq!((a * b).as_canonical_u64(), 21);
    }

    #[test]
    fn field_mul_large() {
        // (P-1) * (P-1) mod P = 1
        let a = Goldilocks::new(P - 1);
        assert_eq!((a * a).as_canonical_u64(), 1);
    }

    /// The addition schedule of `apply_mat4` is the matrix
    /// [[5,7,1,3],[4,6,1,1],[1,3,5,7],[1,1,4,6]] applied to the vector.
    #[test]
    fn mat4_schedule_is_the_reference_matrix() {
        const M4: [[u64; 4]; 4] = [[5, 7, 1, 3], [4, 6, 1, 1], [1, 3, 5, 7], [1, 1, 4, 6]];
        let inputs = [[1u64, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1], [3, P - 1, 1 << 40, 7]];
        for v in inputs {
            let mut x = v.map(Goldilocks::new);
            apply_mat4(&mut x);
            for r in 0..4 {
                let want = (0..4).fold(0u128, |acc, c| acc + u128::from(M4[r][c]) * u128::from(v[c]));
                assert_eq!(x[r].as_canonical_u64(), (want % u128::from(P)) as u64);
            }
        }
    }

    #[test]
    fn pow7_basic() {
        let x = Goldilocks::new(2);
        assert_eq!(x.pow7().as_canonical_u64(), 128);
    }

    #[test]
    fn pow7_zero() {
        assert_eq!(Goldilocks::ZERO.pow7().as_canonical_u64(), 0);
    }

    #[test]
    fn pow7_one() {
        assert_eq!(Goldilocks::new(1).pow7().as_canonical_u64(), 1);
    }

    #[test]
    fn canonical_reduces() {
        // A non-canonical value >= P
        let a = Goldilocks::new(P);
        assert_eq!(a.as_canonical_u64(), 0);
        let b = Goldilocks::new(P + 1);
        assert_eq!(b.as_canonical_u64(), 1);
    }
}
