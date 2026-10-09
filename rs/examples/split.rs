// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Where a permutation's time goes: the live profile against the same
//! kernel with only its full rounds (RP = 0) and only its partial rounds
//! (RF = 0), single-state and batched. Median and min of 41 runs over
//! 1024 states, ns per permutation.
//!
//! ```text
//! cargo run --release -p cyber-hemera --example split
//! ```

use cyber_hemera::field::Goldilocks;
use cyber_hemera::permutation::{HEMERA, Profile, permute_batch_profile, permute_profile};
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
fn main() {
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
