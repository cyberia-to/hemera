// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Arithmetic backends of the permutation kernel: [`Fast`] for public
//! data, [`Ct`] (constant time) for secret data.
//!
//! Both return the same representative for every operation (the
//! differential test checks the full permutation bit for bit); they differ
//! only in how the two rare reduction fix-ups are taken:
//!
//! - [`Fast`]: the borrow fix-up of the multiplication reduction is a
//!   predicted branch (taken with probability ≈ 2⁻³²), off the critical
//!   path — `arith::mul`, `arith::mul_t`;
//! - [`Ct`]: every fix-up is a mask or a `csel`. No branch and no memory
//!   access depends on a field value: the kernel's only branches are loop
//!   counters over the public round structure, and its only memory
//!   accesses are the state and the round constants at public indices.
//!
//! Which entry points run [`Ct`] is specified in `specs/api.md`
//! (§ constant time); callers do not choose a backend.

use crate::arith::{E, P};

/// Field operations the kernel is generic over.
pub(crate) trait Arith {
    /// `a·b`, latency-scheduled (serial chains).
    fn mul(a: u64, b: u64) -> u64;
    /// `a·b`, throughput-scheduled (many independent products).
    fn mul_t(a: u64, b: u64) -> u64;
    /// `a·b + c`, latency-scheduled.
    fn mul_add(a: u64, b: u64, c: u64) -> u64;
    /// `a·b + c`, throughput-scheduled.
    fn mul_add_t(a: u64, b: u64, c: u64) -> u64;
    /// `a + b`, any representatives.
    fn add(a: u64, b: u64) -> u64;
    /// Canonical representative in `[0, p)`.
    fn canon(x: u64) -> u64;
}

/// Public-data backend.
#[derive(Debug)]
pub(crate) struct Fast;

impl Arith for Fast {
    #[inline(always)]
    fn mul(a: u64, b: u64) -> u64 {
        crate::arith::mul(a, b)
    }
    #[inline(always)]
    fn mul_t(a: u64, b: u64) -> u64 {
        crate::arith::mul_t(a, b)
    }
    #[inline(always)]
    fn mul_add(a: u64, b: u64, c: u64) -> u64 {
        crate::arith::mul_add(a, b, c)
    }
    #[inline(always)]
    fn mul_add_t(a: u64, b: u64, c: u64) -> u64 {
        crate::arith::mul_add_t(a, b, c)
    }
    #[inline(always)]
    fn add(a: u64, b: u64) -> u64 {
        crate::arith::add(a, b)
    }
    #[inline(always)]
    fn canon(x: u64) -> u64 {
        crate::arith::canon(x)
    }
}

/// Constant-time backend.
#[derive(Debug)]
pub(crate) struct Ct;

/// All-ones if `b`, else zero.
#[inline(always)]
fn mask(b: bool) -> u64 {
    u64::from(b).wrapping_neg()
}

/// `lo + hi·2^64 mod p`, constant time, same representative as
/// `arith::reduce_wide`.
#[cfg(not(all(target_arch = "aarch64", not(hemera_portable))))]
#[inline(always)]
fn reduce_ct(lo: u64, hi: u64) -> u64 {
    let (t0, borrow) = lo.overflowing_sub(hi >> 32);
    let t0 = t0.wrapping_sub(E & mask(borrow));
    let t1 = (hi << 32).wrapping_sub(hi & E);
    let (r, carry) = t0.overflowing_add(t1);
    r.wrapping_add(E & mask(carry))
}

/// `a·b`, constant time (no `adds`/`adc` of an addend on the `hi` path).
///
/// aarch64: 13 instructions; the borrow selects `ε` or 0 by `csel`, and the
/// carry selects between `t0 + t1` and `t0 + t1 + ε`, both computed.
#[inline(always)]
fn mul_ct(a: u64, b: u64) -> u64 {
    #[cfg(all(target_arch = "aarch64", not(hemera_portable)))]
    {
        let r: u64;
        // SAFETY: register-only arithmetic; flags clobbered, no memory.
        unsafe {
            core::arch::asm!(
                "mul {lo}, {a}, {b}",
                "umulh {hi}, {a}, {b}",
                "subs {lo}, {lo}, {hi}, lsr #32",
                "csel {t}, {e}, xzr, cc",
                "sub {lo}, {lo}, {t}",
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
        let x = u128::from(a) * u128::from(b);
        reduce_ct(x as u64, (x >> 64) as u64)
    }
}

/// `a·b + c`, constant time.
///
/// aarch64: 12 instructions, both fix-ups by `csel` on the flags of the
/// preceding `subs`/`adds` — no branch, by construction.
#[inline(always)]
fn mul_add_ct(a: u64, b: u64, c: u64) -> u64 {
    #[cfg(all(target_arch = "aarch64", not(hemera_portable)))]
    {
        let r: u64;
        // SAFETY: register-only arithmetic; flags are clobbered (the
        // default for `asm!`), no memory or stack is touched.
        unsafe {
            core::arch::asm!(
                "mul {lo}, {a}, {b}",
                "umulh {hi}, {a}, {b}",
                "adds {lo}, {lo}, {c}",
                "adc {hi}, {hi}, xzr",
                "subs {lo}, {lo}, {hi}, lsr #32",
                "csel {u}, {e}, xzr, cc",
                "sub {lo}, {lo}, {u}",
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
        let x = u128::from(a) * u128::from(b) + u128::from(c);
        reduce_ct(x as u64, (x >> 64) as u64)
    }
}

impl Arith for Ct {
    #[inline(always)]
    fn mul(a: u64, b: u64) -> u64 {
        mul_ct(a, b)
    }
    #[inline(always)]
    fn mul_t(a: u64, b: u64) -> u64 {
        mul_ct(a, b)
    }
    #[inline(always)]
    fn mul_add(a: u64, b: u64, c: u64) -> u64 {
        mul_add_ct(a, b, c)
    }
    #[inline(always)]
    fn mul_add_t(a: u64, b: u64, c: u64) -> u64 {
        mul_add_ct(a, b, c)
    }
    #[inline(always)]
    fn add(a: u64, b: u64) -> u64 {
        let (s, o1) = a.overflowing_add(b);
        let (s, o2) = s.overflowing_add(E & mask(o1));
        s.wrapping_add(E & mask(o2))
    }
    #[inline(always)]
    fn canon(x: u64) -> u64 {
        x.wrapping_sub(P & mask(x >= P))
    }
}
