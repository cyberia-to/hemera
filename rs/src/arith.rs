// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Raw Goldilocks arithmetic on `u64` representatives for the permutation
//! kernels.
//!
//! Every function takes any representative in `[0, 2^64)` and returns a
//! representative in `[0, 2^64)` of the correct field element; only
//! `canon` produces the canonical value in `[0, p)`. Functions whose name
//! ends in `_c` additionally require one argument to be canonical, which
//! the profile constructor checks for every round constant.
//!
//! The aarch64 assembly forms are selected on aarch64; the portable forms
//! everywhere else, or on aarch64 with `RUSTFLAGS="--cfg hemera_portable"`
//! (how the portable path is tested on Apple Silicon).

use crate::backend::Arith;

/// The Goldilocks prime p = 2^64 − 2^32 + 1.
pub(crate) const P: u64 = 0xFFFF_FFFF_0000_0001;

/// ε = 2^64 mod p = 2^32 − 1.
pub(crate) const E: u64 = P.wrapping_neg();

/// Canonical representative in `[0, p)`.
#[inline(always)]
pub(crate) fn canon(x: u64) -> u64 {
    if x >= P { x - P } else { x }
}

/// `a + b` for any representatives (identical to `Goldilocks::add`).
#[inline(always)]
pub(crate) fn add(a: u64, b: u64) -> u64 {
    let (sum, over) = a.overflowing_add(b);
    let (mut sum, over) = sum.overflowing_add(u64::from(over) * E);
    if over {
        sum += E;
    }
    sum
}

/// `a + c` where `c < p`. One wrap at most: `a + c − 2^64 + ε < 2^64`.
#[inline(always)]
pub(crate) fn add_c(a: u64, c: u64) -> u64 {
    let (sum, over) = a.overflowing_add(c);
    sum.wrapping_add(E & u64::from(over).wrapping_neg())
}

/// Reduce `lo + hi·2^64` with `hi < 2^32`: `lo + hi·ε` wraps at most once.
#[inline(always)]
pub(crate) fn reduce_small(v: u128) -> u64 {
    let lo = v as u64;
    let hi = (v >> 64) as u64;
    debug_assert!(hi <= E);
    add_c(lo, (hi << 32) - hi)
}

/// Reduce any `u128` (identical to `field::reduce128`).
#[cfg_attr(all(target_arch = "aarch64", not(hemera_portable)), allow(dead_code))]
#[inline(always)]
pub(crate) fn reduce_wide(x: u128) -> u64 {
    let x_lo = x as u64;
    let x_hi = (x >> 64) as u64;
    let x_hi_hi = x_hi >> 32;
    let x_hi_lo = x_hi & E;
    let (mut t0, borrow) = x_lo.overflowing_sub(x_hi_hi);
    if borrow {
        t0 = t0.wrapping_sub(E);
    }
    let t1 = x_hi_lo * E;
    let (res, carry) = t0.overflowing_add(t1);
    res.wrapping_add(E * u64::from(carry))
}

/// `a · b`, bit-identical to `reduce_wide(a as u128 * b as u128)`.
///
/// On aarch64 the reduction is scheduled by hand for latency, the cost
/// that bounds the inverse S-box chain:
///
/// - `t1 = hi_lo · ε` is formed as `(hi << 32) − (hi & ε)` (the shift drops
///   `hi_hi` for free), ready one cycle after `umulh`;
/// - `t1 + ε` is formed in parallel as `(hi << 32) + (ε & !hi)`, so both
///   candidate sums `t0 + t1` and `t0 + t1 + ε` exist at once and the
///   carry only drives the final `csel`;
/// - the borrow of `lo − hi_hi` is a branch, not a select: it fires only
///   when `lo < hi_hi < 2^32` (≈ 2⁻³² for products of field elements), so
///   the predicted branch takes the fix-up off the dependency chain.
///   Plonky3's Goldilocks reduction makes the same choice. The
///   constant-time `csel` form measured ~1.3 cycles more per dependent
///   multiplication (`audit/permutation-performance-2026-10.md`).
#[inline(always)]
pub(crate) fn mul(a: u64, b: u64) -> u64 {
    #[cfg(all(target_arch = "aarch64", not(hemera_portable)))]
    {
        let r: u64;
        // SAFETY: register-only arithmetic; flags are clobbered (the
        // default for `asm!`), no memory or stack is touched. `2` is a
        // numeric local label, unique per expansion.
        unsafe {
            core::arch::asm!(
                "mul {lo}, {a}, {b}",
                "umulh {hi}, {a}, {b}",
                "lsr {t}, {hi}, #32",
                "subs {lo}, {lo}, {t}",
                "b.cs 2f",
                "sub {lo}, {lo}, {e}",
                "2:",
                "lsl {u}, {hi}, #32",
                "and {t}, {hi}, {e}",
                "bic {hi}, {e}, {hi}",
                "sub {t}, {u}, {t}",
                "add {u}, {u}, {hi}",
                "add {u}, {lo}, {u}",
                "adds {r}, {lo}, {t}",
                "csel {r}, {u}, {r}, cs",
                a = in(reg) a,
                b = in(reg) b,
                e = in(reg) E,
                lo = out(reg) _,
                hi = out(reg) _,
                t = out(reg) _,
                u = out(reg) _,
                r = out(reg) r,
                options(pure, nomem, nostack),
            );
        }
        r
    }
    #[cfg(not(all(target_arch = "aarch64", not(hemera_portable))))]
    {
        reduce_wide(u128::from(a) * u128::from(b))
    }
}

/// `a · b + c` with one reduction (the 128-bit sum cannot overflow:
/// `(2^64−1)^2 + 2^64 − 1 < 2^128`).
#[inline(always)]
pub(crate) fn mul_add(a: u64, b: u64, c: u64) -> u64 {
    #[cfg(all(target_arch = "aarch64", not(hemera_portable)))]
    {
        let r: u64;
        // SAFETY: as in `mul`.
        unsafe {
            core::arch::asm!(
                "mul {lo}, {a}, {b}",
                "umulh {hi}, {a}, {b}",
                "adds {lo}, {lo}, {c}",
                "adc {hi}, {hi}, xzr",
                "lsr {t}, {hi}, #32",
                "subs {lo}, {lo}, {t}",
                "b.cs 2f",
                "sub {lo}, {lo}, {e}",
                "2:",
                "lsl {u}, {hi}, #32",
                "and {t}, {hi}, {e}",
                "bic {hi}, {e}, {hi}",
                "sub {t}, {u}, {t}",
                "add {u}, {u}, {hi}",
                "add {u}, {lo}, {u}",
                "adds {r}, {lo}, {t}",
                "csel {r}, {u}, {r}, cs",
                a = in(reg) a,
                b = in(reg) b,
                c = in(reg) c,
                e = in(reg) E,
                lo = out(reg) _,
                hi = out(reg) _,
                t = out(reg) _,
                u = out(reg) _,
                r = out(reg) r,
                options(pure, nomem, nostack),
            );
        }
        r
    }
    #[cfg(not(all(target_arch = "aarch64", not(hemera_portable))))]
    {
        reduce_wide(u128::from(a) * u128::from(b) + u128::from(c))
    }
}

/// `a · b` scheduled for throughput: 9 instructions on aarch64 (shifted-
/// and extended-register operands fold `hi_hi`, `hi_lo`), against 13 for
/// the latency form [`mul`]; 1–2 cycles more latency. Used where many
/// independent products are in flight (full-round S-boxes, the internal
/// layer). Same representative as [`mul`].
#[inline(always)]
pub(crate) fn mul_t(a: u64, b: u64) -> u64 {
    #[cfg(all(target_arch = "aarch64", not(hemera_portable)))]
    {
        let r: u64;
        // SAFETY: as in `mul`.
        unsafe {
            core::arch::asm!(
                "mul {lo}, {a}, {b}",
                "umulh {hi}, {a}, {b}",
                "subs {lo}, {lo}, {hi}, lsr #32",
                "b.cs 2f",
                "sub {lo}, {lo}, {e}",
                "2:",
                "lsl {u}, {hi}, #32",
                "sub {u}, {u}, {hi:w}, uxtw",
                "adds {r}, {lo}, {u}",
                "csel {u}, {e}, xzr, cs",
                "add {r}, {r}, {u}",
                a = in(reg) a,
                b = in(reg) b,
                e = in(reg) E,
                lo = out(reg) _,
                hi = out(reg) _,
                u = out(reg) _,
                r = out(reg) r,
                options(pure, nomem, nostack),
            );
        }
        r
    }
    #[cfg(not(all(target_arch = "aarch64", not(hemera_portable))))]
    {
        reduce_wide(u128::from(a) * u128::from(b))
    }
}

/// `a · b + c`, throughput form of [`mul_add`].
#[inline(always)]
pub(crate) fn mul_add_t(a: u64, b: u64, c: u64) -> u64 {
    #[cfg(all(target_arch = "aarch64", not(hemera_portable)))]
    {
        let r: u64;
        // SAFETY: as in `mul`.
        unsafe {
            core::arch::asm!(
                "mul {lo}, {a}, {b}",
                "umulh {hi}, {a}, {b}",
                "adds {lo}, {lo}, {c}",
                "adc {hi}, {hi}, xzr",
                "subs {lo}, {lo}, {hi}, lsr #32",
                "b.cs 2f",
                "sub {lo}, {lo}, {e}",
                "2:",
                "lsl {u}, {hi}, #32",
                "sub {u}, {u}, {hi:w}, uxtw",
                "adds {r}, {lo}, {u}",
                "csel {u}, {e}, xzr, cs",
                "add {r}, {r}, {u}",
                a = in(reg) a,
                b = in(reg) b,
                c = in(reg) c,
                e = in(reg) E,
                lo = out(reg) _,
                hi = out(reg) _,
                u = out(reg) _,
                r = out(reg) r,
                options(pure, nomem, nostack),
            );
        }
        r
    }
    #[cfg(not(all(target_arch = "aarch64", not(hemera_portable))))]
    {
        reduce_wide(u128::from(a) * u128::from(b) + u128::from(c))
    }
}

/// x⁷ in throughput form (full rounds: 16·L independent S-boxes).
#[inline(always)]
pub(crate) fn pow7_t<A: Arith>(x: u64) -> u64 {
    let x2 = A::mul_t(x, x);
    let x3 = A::mul_t(x2, x);
    let x4 = A::mul_t(x2, x2);
    A::mul_t(x3, x4)
}

/// x^7: depth 3 (x², then x³ ‖ x⁴, then x⁷), 4 multiplications.
#[inline(always)]
pub(crate) fn pow7<A: Arith>(x: u64) -> u64 {
    let x2 = A::mul(x, x);
    let x3 = A::mul(x2, x);
    let x4 = A::mul(x2, x2);
    A::mul(x3, x4)
}

/// Square `x` `n` times.
#[inline(always)]
fn sqn<A: Arith>(mut x: u64, n: usize) -> u64 {
    for _ in 0..n {
        x = A::mul(x, x);
    }
    x
}

/// Square `x` `n` times, calling `fill(k)` for the next pending task `k`
/// after every `every`-th squaring.
#[inline(always)]
fn sqn_fill<A: Arith, F: FnMut(usize)>(
    mut x: u64,
    n: usize,
    every: usize,
    fill: &mut F,
    next: &mut usize,
    tasks: usize,
) -> u64 {
    for k in 1..=n {
        x = A::mul(x, x);
        if k % every == 0 && *next < tasks {
            fill(*next);
            *next += 1;
        }
    }
    x
}

/// x^(p−2) for nonzero `x`, by an addition chain of depth 69.
///
/// With `a_k = x^(2^k − 1)`: `a_{j+k} = a_j^(2^k) · a_k`, and
/// `p − 2 = (2^31 − 1)·2^33 + (2^32 − 1)`, so `x^(p−2) = a_31^(2^33) · a_32`.
/// Depth (serial multiplications): a2 2 · a3 3 · a4 5 · a7 9 · a8 10 ·
/// a15 18 · a16 19 · a31 35 · result 35+33+1 = 69, against 71 for the
/// 75-multiplication chain in `field.rs` (85 multiplications here; the
/// extra ones run beside the critical path). Every serial multiplication
/// saved is saved 16 times per permutation.
///
/// `tasks` independent work items are interleaved into the chain:
/// `fill(0), …, fill(tasks − 1)` are each called exactly once, in order, spread over the long squaring runs (every 3rd squaring of
/// the last 48), and any not yet issued are run after the chain. The
/// chain is latency-bound (one multiplication in flight); placing
/// independent work between its steps in program order lets it execute
/// in the chain's shadow instead of after it — the out-of-order window
/// is too small to find work issued after a 69-deep chain.
#[inline(always)]
pub(crate) fn inv_nonzero_fill<A: Arith, F: FnMut(usize)>(
    x: u64,
    fill: &mut F,
    tasks: usize,
) -> u64 {
    let mut next = 0;
    let x2 = A::mul(x, x);
    let a2 = A::mul(x2, x); // x^3
    let x4 = A::mul(x2, x2);
    let a3 = A::mul(x4, a2); // x^7
    let a4 = A::mul(sqn::<A>(a2, 2), a2); // x^15
    let a7 = A::mul(sqn::<A>(a4, 3), a3);
    let a8 = A::mul(sqn::<A>(a4, 4), a4);
    let a15 = A::mul(sqn::<A>(a8, 7), a7);
    let a16 = A::mul(sqn::<A>(a8, 8), a8);
    let a31 = A::mul(sqn_fill::<A, F>(a16, 15, 3, fill, &mut next, tasks), a15);
    let a32 = A::mul(A::mul(a31, a31), x);
    let r = A::mul(sqn_fill::<A, F>(a31, 33, 3, fill, &mut next, tasks), a32);
    while next < tasks {
        fill(next);
        next += 1;
    }
    r
}
