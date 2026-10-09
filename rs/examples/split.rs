// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Where a permutation's time goes: the live profile against the same
//! kernel with only its full rounds (RP = 0) and only its partial rounds
//! (RF = 0), single-state and batched; a profile with the shape of the
//! x⁷ RF 6+6 / RP 48 proposal; batch tails of 1..31 states. Median and
//! min of 41 runs over 1024 states, ns per permutation.
//!
//! ```text
//! cargo run --release -p cyber-hemera --example split
//! ```

use cyber_hemera::field::Goldilocks;
use cyber_hemera::permutation::{
    HEMERA, PartialSbox, Profile, permute_batch_profile, permute_profile,
};
use std::hint::black_box;
use std::time::Instant;
fn t(name: &str, p: &Profile, batch: bool) {
    let mut v: Vec<[Goldilocks; 16]> = (0..1024u64)
        .map(|i| core::array::from_fn(|j| Goldilocks::new(i * 31 + j as u64 * 7 + 1)))
        .collect();
    let mut best = Vec::new();
    for _ in 0..41 {
        let t0 = Instant::now();
        if batch {
            permute_batch_profile(&mut v, p)
        } else {
            for s in v.iter_mut() {
                permute_profile(s, p)
            }
        }
        black_box(&v);
        best.push(t0.elapsed().as_secs_f64() * 1e9 / 1024.0);
    }
    best.sort_by(f64::total_cmp);
    println!(
        "{name:<30} median {:.0} ns  min {:.0} ns",
        best[20], best[0]
    );
}
fn tail(n: usize) {
    let mut v: Vec<[Goldilocks; 16]> = (0..n as u64)
        .map(|i| core::array::from_fn(|j| Goldilocks::new(i * 31 + j as u64 * 7 + 1)))
        .collect();
    let mut best = Vec::new();
    for _ in 0..201 {
        let t0 = Instant::now();
        cyber_hemera::permutation::permute_batch(&mut v);
        black_box(&v);
        best.push(t0.elapsed().as_secs_f64() * 1e9 / n as f64);
    }
    best.sort_by(f64::total_cmp);
    println!(
        "permute_batch of {n:<3}               median {:.0} ns  min {:.0} ns (per permutation)",
        best[100], best[0]
    );
}

fn main() {
    for n in [1, 2, 4, 7, 8, 15, 16, 20, 31] {
        tail(n);
    }
    // Shape of the profile-v2 proposal (x^7 partial, RF 6+6, RP 48).
    // Timing does not depend on constant values; these are placeholders
    // (the live constants repeated), so only the cost is meaningful.
    let ext: Vec<u64> = HEMERA
        .external
        .iter()
        .cycle()
        .take(12 * 16)
        .copied()
        .collect();
    let int: Vec<u64> = HEMERA.internal.iter().cycle().take(48).copied().collect();
    let v2 = Profile::new(6, 48, PartialSbox::Pow7, &ext, &int, HEMERA.diag);
    for batch in [false, true] {
        t(
            &format!("v2-shaped x^7 RF12/RP48 batch={batch}"),
            &v2,
            batch,
        );
    }
    let full_only = Profile::new(4, 0, HEMERA.partial, HEMERA.external, &[], HEMERA.diag);
    let part_only = Profile::new(0, 16, HEMERA.partial, &[], HEMERA.internal, HEMERA.diag);
    for batch in [false, true] {
        t(&format!("hemera batch={batch}"), &HEMERA, batch);
        t(
            &format!("full rounds only batch={batch}"),
            &full_only,
            batch,
        );
        t(
            &format!("partial rounds only batch={batch}"),
            &part_only,
            batch,
        );
    }
}
