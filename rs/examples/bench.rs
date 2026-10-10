// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Permutation benchmark harness — median of N timed batches.
//!
//! ```text
//! cargo run --release -p cyber-hemera --example bench [-- runs]
//! ```
//!
//! Rows:
//! - `mul latency`: one dependent chain of Goldilocks multiplications
//!   (each result feeds the next) — the cost that bounds the inverse chain.
//! - `mul throughput`: 8 independent chains interleaved.
//! - `permute (latency)`: each permutation's output is the next input.
//! - `permute_ct (latency)`: the constant-time kernel, chained.
//! - `hash 64 B`: `hemera::hash` of a 64-byte message (2 permutations);
//!   `hash_secret` (same digest, constant time) and `keyed_hash` (key ‖ 64 B = 96 B,
//!   2 permutations).
//! - `hash_node`: the 2-to-1 Merkle compression lens verifies with
//!   (`tree::hash_node`), chained.
//! - `1024 × permute (loop)`: 1024 independent states, one `permute` each.
//! - `1024 × permute_batch`: the same 1024 states through the batched,
//!   lane-interleaved API.
//! - `1024 × hash_node_batch`: 1024 independent node compressions.
//! - `1024 × hash_leaf 144 B`: leaf digests (3 sponge permutations + 1
//!   re-derivation each), per leaf and through `hash_leaf_batch`.
//!
//! Every row prints ns per operation (median over runs).

use std::hint::black_box;
use std::time::Instant;

use cyber_hemera::field::Goldilocks;
use cyber_hemera::permutation::{permute, permute_batch, permute_ct};
use cyber_hemera::tree::{hash_leaf, hash_leaf_batch, hash_node, hash_node_batch};
use cyber_hemera::{Hash, WIDTH};

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Run `f` (which performs `ops` operations) `runs` times; median ns/op.
fn time(runs: usize, ops: usize, mut f: impl FnMut()) -> f64 {
    f(); // warm-up
    let samples = (0..runs)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1e9 / ops as f64
        })
        .collect();
    median(samples)
}

fn rng(seed: u64) -> impl FnMut() -> u64 {
    let mut s = seed;
    move || {
        s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn main() {
    let runs: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(31);
    let mut r = rng(7);

    println!("{:<30} {:>10}", "operation", "ns/op");
    let row = |name: &str, ns: f64| println!("{name:<30} {ns:>10.1}");

    // ── field multiply: latency and throughput ──────────────────────
    const CHAIN: usize = 100_000;
    let x0 = Goldilocks::new(r() % cyber_hemera::field::P);
    row(
        "mul latency",
        time(runs, CHAIN, || {
            let mut x = black_box(x0);
            for _ in 0..CHAIN {
                x = x * x;
            }
            black_box(x);
        }),
    );
    row(
        "mul throughput (8 chains)",
        time(runs, CHAIN * 8, || {
            let mut x: [Goldilocks; 8] = black_box([x0; 8]);
            for (i, v) in x.iter_mut().enumerate() {
                *v *= Goldilocks::new(i as u64 + 3);
            }
            for _ in 0..CHAIN {
                for v in x.iter_mut() {
                    *v = *v * *v;
                }
            }
            black_box(x);
        }),
    );

    // ── single permutation, chained ─────────────────────────────────
    const PERMS: usize = 10_000;
    let s0: [Goldilocks; WIDTH] =
        core::array::from_fn(|_| Goldilocks::new(r() % cyber_hemera::field::P));
    row(
        "permute (latency)",
        time(runs, PERMS, || {
            let mut s = black_box(s0);
            for _ in 0..PERMS {
                permute(&mut s);
            }
            black_box(s);
        }),
    );

    row(
        "permute_ct (latency)",
        time(runs, PERMS, || {
            let mut s = black_box(s0);
            for _ in 0..PERMS {
                permute_ct(&mut s);
            }
            black_box(s);
        }),
    );

    // ── hash of 64 bytes ────────────────────────────────────────────
    let msg: [u8; 64] = core::array::from_fn(|i| (r() as u8).wrapping_add(i as u8));
    row(
        "hash 64 B",
        time(runs, PERMS, || {
            for _ in 0..PERMS {
                black_box(cyber_hemera::hash(black_box(&msg)));
            }
        }),
    );

    row(
        "hash_secret 64 B (CT)",
        time(runs, PERMS, || {
            for _ in 0..PERMS {
                black_box(cyber_hemera::hash_secret(black_box(&msg)));
            }
        }),
    );
    let key = [7u8; 32];
    row(
        "keyed_hash 64 B (CT)",
        time(runs, PERMS, || {
            for _ in 0..PERMS {
                black_box(cyber_hemera::keyed_hash(&key, black_box(&msg)));
            }
        }),
    );

    // ── 2-to-1 node compression, chained ────────────────────────────
    let (a, b) = (cyber_hemera::hash(b"a"), cyber_hemera::hash(b"b"));
    row(
        "hash_node (latency)",
        time(runs, PERMS, || {
            let (mut l, rr) = (black_box(a), black_box(b));
            for _ in 0..PERMS {
                l = hash_node(&l, &rr, false);
            }
            black_box(l);
        }),
    );

    // ── 1024 independent permutations ───────────────────────────────
    const BATCH: usize = 1024;
    let states: Vec<[Goldilocks; WIDTH]> = (0..BATCH)
        .map(|_| core::array::from_fn(|_| Goldilocks::new(r() % cyber_hemera::field::P)))
        .collect();
    let mut work = states.clone();
    row(
        "1024 × permute (loop)",
        time(runs, BATCH, || {
            work.copy_from_slice(&states);
            for s in work.iter_mut() {
                permute(s);
            }
            black_box(&work);
        }),
    );
    row(
        "1024 × permute_batch",
        time(runs, BATCH, || {
            work.copy_from_slice(&states);
            permute_batch(&mut work);
            black_box(&work);
        }),
    );

    // ── 1024 independent node compressions ──────────────────────────
    let lefts: Vec<Hash> = (0..BATCH as u64)
        .map(|i| cyber_hemera::hash(&i.to_le_bytes()))
        .collect();
    let rights: Vec<Hash> = (0..BATCH as u64)
        .map(|i| cyber_hemera::hash(&(i + 7).to_le_bytes()))
        .collect();
    let pairs: Vec<(Hash, Hash)> = lefts.iter().copied().zip(rights.iter().copied()).collect();
    let mut out = vec![Hash::from_bytes([0; 32]); BATCH];
    row(
        "1024 × hash_node (loop)",
        time(runs, BATCH, || {
            for (o, (l, rr)) in out.iter_mut().zip(&pairs) {
                *o = hash_node(l, rr, false);
            }
            black_box(&out);
        }),
    );
    row(
        "1024 × hash_node_batch",
        time(runs, BATCH, || {
            hash_node_batch(&pairs, false, &mut out);
            black_box(&out);
        }),
    );

    // ── 1024 independent 144-byte leaves (4 permutations each) ──────
    let leaf_bytes: Vec<[u8; 144]> = (0..BATCH)
        .map(|_| core::array::from_fn(|_| r() as u8))
        .collect();
    let leaves: Vec<(&[u8], u64)> = leaf_bytes
        .iter()
        .enumerate()
        .map(|(i, b)| (b.as_slice(), i as u64))
        .collect();
    row(
        "1024 × hash_leaf 144 B (loop)",
        time(runs, BATCH, || {
            for (o, (b, i)) in out.iter_mut().zip(&leaves) {
                *o = hash_leaf(b, *i, false);
            }
            black_box(&out);
        }),
    );
    row(
        "1024 × hash_leaf_batch 144 B",
        time(runs, BATCH, || {
            hash_leaf_batch(&leaves, false, &mut out);
            black_box(&out);
        }),
    );
}
