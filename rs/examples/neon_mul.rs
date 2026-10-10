// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Is NEON worth it for Goldilocks multiplication on Apple Silicon?
//!
//! aarch64 NEON has no 64×64→128 multiply. A 2-lane Goldilocks product
//! has to be assembled from four 32×32→64 `umull`s plus carry and
//! reduction logic (~29 vector instructions per 2 lanes). This example
//! measures, ns per lane-multiplication (median of 21):
//!
//! - scalar `Goldilocks` multiplication (hemera's aarch64 asm form),
//!   16 independent chains — what the batched kernel uses;
//! - NEON emulation, 8 and 16 vectors (16 and 32 lanes) in flight;
//! - hybrids: scalar chains and NEON vectors interleaved in one loop,
//!   to see whether the NEON pipes add throughput beside the scalar ones;
//! - single-chain latency of both.
//!
//! The NEON product is checked against the scalar one on 10⁶ random
//! inputs first.
//!
//! ```text
//! cargo run --release -p cyber-hemera --example neon_mul
//! ```

#[cfg(target_arch = "aarch64")]
fn main() {
    use core::arch::aarch64::*;
    use cyber_hemera::field::{Goldilocks, P};
    use std::hint::black_box;
    use std::time::Instant;

    const E: u64 = 0xFFFF_FFFF;

    /// Two Goldilocks products in one 128-bit vector, any representatives
    /// in, a representative in `[0, 2^64)` out.
    #[inline(always)]
    fn neon_mul(a: uint64x2_t, b: uint64x2_t) -> uint64x2_t {
        // SAFETY: NEON is mandatory on aarch64; pure register arithmetic.
        unsafe {
            let e = vdupq_n_u64(E);
            let (al, ah) = (vmovn_u64(a), vshrn_n_u64(a, 32));
            let (bl, bh) = (vmovn_u64(b), vshrn_n_u64(b, 32));
            let ll = vmull_u32(al, bl);
            let lh = vmull_u32(al, bh);
            let hl = vmull_u32(ah, bl);
            let hh = vmull_u32(ah, bh);
            let mid = vaddq_u64(lh, hl);
            let cmid = vcltq_u64(mid, lh);
            let lo = vaddq_u64(ll, vshlq_n_u64(mid, 32));
            let clo = vcltq_u64(lo, ll);
            let mut hi = vaddq_u64(hh, vshrq_n_u64(mid, 32));
            hi = vsubq_u64(hi, clo);
            hi = vaddq_u64(hi, vshlq_n_u64(vshrq_n_u64(cmid, 63), 32));
            let hh32 = vshrq_n_u64(hi, 32);
            let borrow = vcltq_u64(lo, hh32);
            let t0 = vsubq_u64(vsubq_u64(lo, hh32), vandq_u64(borrow, e));
            let t1 = vsubq_u64(vshlq_n_u64(hi, 32), vandq_u64(hi, e));
            let s = vaddq_u64(t0, t1);
            let c = vcltq_u64(s, t0);
            vaddq_u64(s, vandq_u64(c, e))
        }
    }
    let pair = |x: u64, y: u64| unsafe { vcombine_u64(vcreate_u64(x), vcreate_u64(y)) };
    let lanes = |v: uint64x2_t| unsafe { [vgetq_lane_u64(v, 0), vgetq_lane_u64(v, 1)] };

    let mut s = 1u64;
    let mut rnd = move || {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    for i in 0..1_000_000u64 {
        let (a, b, c, d) = if i < 64 {
            (u64::MAX - i, P + i, i, u64::MAX)
        } else {
            (rnd(), rnd(), rnd(), rnd())
        };
        let got = lanes(neon_mul(pair(a, c), pair(b, d)));
        let want = [
            (Goldilocks::new(a) * Goldilocks::new(b)).as_canonical_u64(),
            (Goldilocks::new(c) * Goldilocks::new(d)).as_canonical_u64(),
        ];
        assert_eq!(got.map(|x| Goldilocks::new(x).as_canonical_u64()), want);
    }
    println!("NEON product = scalar product on 10^6 random pairs\n");

    const N: usize = 1_000_000;
    let bench = |name: &str, lanes: usize, f: &dyn Fn()| {
        let mut v: Vec<f64> = (0..21)
            .map(|_| {
                let t = Instant::now();
                f();
                t.elapsed().as_secs_f64() * 1e9 / (N * lanes) as f64
            })
            .collect();
        v.sort_by(f64::total_cmp);
        println!(
            "{name:<40} {:>6.3} ns per lane-mul (median), {:.3} min",
            v[10], v[0]
        );
    };
    let seed = |i: usize| i as u64 * 977 + 5;
    bench("scalar, 16 chains", 16, &|| {
        let mut x: [Goldilocks; 16] = black_box(core::array::from_fn(|i| Goldilocks::new(seed(i))));
        for _ in 0..N {
            for v in x.iter_mut() {
                *v = *v * *v;
            }
        }
        black_box(x);
    });
    bench("NEON, 8 vectors (16 lanes)", 16, &|| {
        let mut x: [uint64x2_t; 8] =
            black_box(core::array::from_fn(|i| pair(seed(i), seed(i + 8))));
        for _ in 0..N {
            for v in x.iter_mut() {
                *v = neon_mul(*v, *v);
            }
        }
        black_box(lanes(x[0]));
    });
    bench("NEON, 16 vectors (32 lanes)", 32, &|| {
        let mut x: [uint64x2_t; 16] =
            black_box(core::array::from_fn(|i| pair(seed(i), seed(i + 16))));
        for _ in 0..N {
            for v in x.iter_mut() {
                *v = neon_mul(*v, *v);
            }
        }
        black_box(lanes(x[0]));
    });
    bench("hybrid: 12 scalar + 2 vectors (16 lanes)", 16, &|| {
        let mut x: [Goldilocks; 12] = black_box(core::array::from_fn(|i| Goldilocks::new(seed(i))));
        let mut y: [uint64x2_t; 2] =
            black_box(core::array::from_fn(|i| pair(seed(i), seed(i + 2))));
        for _ in 0..N {
            for v in x.iter_mut() {
                *v = *v * *v;
            }
            for v in y.iter_mut() {
                *v = neon_mul(*v, *v);
            }
        }
        black_box((x, lanes(y[0])));
    });
    bench("hybrid: 16 scalar + 8 vectors (32 lanes)", 32, &|| {
        let mut x: [Goldilocks; 16] = black_box(core::array::from_fn(|i| Goldilocks::new(seed(i))));
        let mut y: [uint64x2_t; 8] =
            black_box(core::array::from_fn(|i| pair(seed(i), seed(i + 8))));
        for _ in 0..N {
            for v in x.iter_mut() {
                *v = *v * *v;
            }
            for v in y.iter_mut() {
                *v = neon_mul(*v, *v);
            }
        }
        black_box((x, lanes(y[0])));
    });
    bench("scalar latency (1 chain)", 1, &|| {
        let mut x = black_box(Goldilocks::new(12345));
        for _ in 0..N {
            x = x * x;
        }
        black_box(x);
    });
    bench("NEON latency (1 vector, per lane)", 2, &|| {
        let mut x = black_box(pair(12345, 678));
        for _ in 0..N {
            x = neon_mul(x, x);
        }
        black_box(lanes(x));
    });
}

#[cfg(not(target_arch = "aarch64"))]
fn main() {
    println!("aarch64 only");
}
