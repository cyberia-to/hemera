// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Goldilocks prime field (p = 2^64 - 2^32 + 1).
//!
//! Minimal implementation covering the operations hemera needs:
//! addition, subtraction, multiplication, and the x^7 S-box.

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
        let mut c = self.value;
        if c >= P {
            c -= P;
        }
        c
    }

    /// Compute x^2.
    #[inline]
    fn square(self) -> Self {
        self * self
    }

    /// Compute x^7 (the Poseidon2 S-box for Goldilocks).
    #[inline]
    pub fn pow7(self) -> Self {
        let x2 = self.square();
        let x3 = x2 * self;
        let x4 = x2.square();
        x3 * x4
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
        let (mut sum, over) = sum.overflowing_add(u64::from(over) * NEG_ORDER);
        if over {
            sum += NEG_ORDER;
        }
        Self::new(sum)
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
        let (mut diff, under) = diff.overflowing_sub(u64::from(under) * NEG_ORDER);
        if under {
            diff -= NEG_ORDER;
        }
        Self::new(diff)
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
        reduce128(u128::from(self.value) * u128::from(rhs.value))
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

/// Apply the 4x4 MDS matrix used in Poseidon2 external rounds:
/// ```text
/// [ 2 3 1 1 ]
/// [ 1 2 3 1 ]
/// [ 1 1 2 3 ]
/// [ 3 1 1 2 ]
/// ```
#[inline(always)]
pub fn apply_mat4(x: &mut [Goldilocks; 4]) {
    let t01 = x[0] + x[1];
    let t23 = x[2] + x[3];
    let t0123 = t01 + t23;
    let t01123 = t0123 + x[1];
    let t01233 = t0123 + x[3];
    x[3] = t01233 + x[0].double();
    x[1] = t01123 + x[2].double();
    x[0] = t01123 + t01;
    x[2] = t01233 + t23;
}

/// Apply the external MDS layer for a width-16 state.
///
/// Multiplies by the 16×16 circulant-of-4×4 matrix:
/// `[[2M M ... M], [M 2M ... M], ..., [M M ... 2M]]`
/// where M is the 4×4 MDS matrix.
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

/// Diagonal elements of the internal diffusion matrix for Goldilocks t=16.
///
/// The full matrix is M_I = 1 + diag(d), where 1 is the all-ones matrix.
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
