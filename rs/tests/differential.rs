// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Randomized differential test: every optimised path against the frozen
//! oracle in `cyber_hemera::reference` (definitional square-and-multiply
//! inverse, 0.3.1 arithmetic), compared on canonical values / bytes.
//!
//! Sizes: `HEMERA_DIFF_N` inputs per family (default 1 000 000 in release
//! builds, 10 000 in debug). Families: permutation (single and batched),
//! hash (random lengths 0..=200 bytes), node compression (`hash_node`,
//! single and batched, both root flags), leaf hashing, and a profile with
//! the shape of the x⁷ RF 6+6 / RP 48 proposal.
//!
//! ```text
//! cargo test --release -p cyber-hemera --test differential -- --nocapture
//! ```

use cyber_hemera::field::{Goldilocks, P};
use cyber_hemera::permutation::{
    PartialSbox, Profile, permute, permute_batch, permute_batch_profile, permute_profile,
};
use cyber_hemera::reference;
use cyber_hemera::tree::{hash_leaf, hash_leaf_batch, hash_node, hash_node_batch};
use cyber_hemera::{Hash, hash};

fn n() -> usize {
    std::env::var("HEMERA_DIFF_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) {
            10_000
        } else {
            1_000_000
        })
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Raw state element: mostly uniform u64 (≈2⁻³² non-canonical), with
    /// a share of edge values and non-canonical representatives.
    fn elem(&mut self) -> u64 {
        match self.next() % 16 {
            0 => [0, 1, P - 1, P, P + 1, u64::MAX, 1 << 32, (1 << 32) - 1]
                [(self.next() % 8) as usize],
            1 => P + self.next() % (u64::MAX - P + 1), // non-canonical
            2 => self.next() >> 32,
            _ => self.next(),
        }
    }
    fn state(&mut self) -> [u64; 16] {
        match self.next() % 64 {
            0 => [0; 16],
            1 => [P; 16],
            _ => core::array::from_fn(|_| self.elem()),
        }
    }
}

/// Run `f(seed_offset, count)` over all cores; panics propagate.
fn parallel(total: usize, f: impl Fn(u64, usize) + Sync) {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let per = total.div_ceil(threads);
    std::thread::scope(|sc| {
        for t in 0..threads {
            let f = &f;
            let count = per.min(total.saturating_sub(t * per));
            sc.spawn(move || f(t as u64 * 0x1000_0001 + 1, count));
        }
    });
}

fn canon(s: [Goldilocks; 16]) -> [u64; 16] {
    s.map(|e| e.as_canonical_u64())
}

#[test]
fn permutation_matches_reference() {
    let total = n();
    parallel(total, |seed, count| {
        let mut r = Rng(seed);
        let mut batch_in = Vec::new();
        let mut batch_ref = Vec::new();
        let mut flush_at = 1 + (r.next() % 40) as usize;
        for k in 0..count {
            let raw = r.state();
            let mut expect = raw;
            reference::permute(&mut expect);
            let mut s = raw.map(Goldilocks::new);
            permute(&mut s);
            assert_eq!(canon(s), expect, "permute differs on {raw:x?}");
            // the representative itself is canonical (raw equality)
            assert!(
                s.iter()
                    .all(|e| *e == Goldilocks::new(e.as_canonical_u64()))
            );
            batch_in.push(raw.map(Goldilocks::new));
            batch_ref.push(expect);
            // batched path: flush at random sizes 1..=40 to cover every
            // lane-group cascade (16, 8, 4, 2, 1)
            if batch_in.len() == flush_at || k + 1 == count {
                flush_at = 1 + (r.next() % 40) as usize;
                permute_batch(&mut batch_in);
                for (got, want) in batch_in.iter().zip(&batch_ref) {
                    assert_eq!(canon(*got), *want, "permute_batch differs");
                }
                batch_in.clear();
                batch_ref.clear();
            }
        }
    });
    println!("permutation: {total} random states (single + batched)");
}

fn ref_sponge_hash(msg: &[u8]) -> [u8; 32] {
    let mut st = [0u64; 16];
    st[11] = 0; // DOMAIN_HASH
    let absorb = |st: &mut [u64; 16], block: &[u8; 56]| {
        for i in 0..8 {
            let mut b = [0u8; 8];
            b[..7].copy_from_slice(&block[7 * i..7 * i + 7]);
            st[i] = reference::add(st[i], u64::from_le_bytes(b));
        }
    };
    let mut chunks = msg.chunks_exact(56);
    for c in &mut chunks {
        absorb(&mut st, c.try_into().unwrap());
        reference::permute(&mut st);
    }
    let rem = chunks.remainder();
    let mut last = [0u8; 56];
    last[..rem.len()].copy_from_slice(rem);
    last[rem.len()] = 0x01;
    absorb(&mut st, &last);
    st[10] = msg.len() as u64;
    reference::permute(&mut st);
    out_bytes(&st)
}

fn out_bytes(st: &[u64; 16]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..4 {
        out[8 * i..8 * i + 8].copy_from_slice(&(st[i] % P).to_le_bytes());
    }
    out
}

fn cv(h: &Hash) -> [u64; 4] {
    core::array::from_fn(|i| u64::from_le_bytes(h.as_bytes()[8 * i..8 * i + 8].try_into().unwrap()))
}

fn ref_node(l: &Hash, r: &Hash, root: bool) -> [u8; 32] {
    let mut st = [0u64; 16];
    st[9] = 2 | u64::from(root);
    st[..4].copy_from_slice(&cv(l));
    st[4..8].copy_from_slice(&cv(r));
    reference::permute(&mut st);
    out_bytes(&st)
}

fn ref_leaf(chunk: &[u8], counter: u64, root: bool) -> [u8; 32] {
    let base = Hash::from_bytes(ref_sponge_hash(chunk));
    let mut st = [0u64; 16];
    st[..4].copy_from_slice(&cv(&base));
    st[8] = counter;
    st[9] = 4 | u64::from(root);
    reference::permute(&mut st);
    out_bytes(&st)
}

#[test]
fn hash_matches_reference() {
    let total = n();
    parallel(total, |seed, count| {
        let mut r = Rng(seed ^ 0xA5A5);
        let mut msg = Vec::with_capacity(200);
        for _ in 0..count {
            let len = (r.next() % 201) as usize;
            msg.clear();
            msg.extend((0..len).map(|_| r.next() as u8));
            assert_eq!(
                *hash(&msg).as_bytes(),
                ref_sponge_hash(&msg),
                "hash differs, len {len}"
            );
        }
    });
    println!("hash: {total} random messages, 0..=200 bytes");
}

#[test]
fn node_and_leaf_match_reference() {
    let total = n();
    parallel(total, |seed, count| {
        let mut r = Rng(seed ^ 0x5A5A);
        let mut pairs = Vec::new();
        let mut want = Vec::new();
        for k in 0..count {
            // Children: canonical digests (what trees carry), plus raw
            // 32-byte strings that decode to non-canonical limbs.
            let mut child = || {
                let mut b = [0u8; 32];
                for i in 0..4 {
                    let v = if r.next().is_multiple_of(8) {
                        r.elem()
                    } else {
                        r.next() % P
                    };
                    b[8 * i..8 * i + 8].copy_from_slice(&v.to_le_bytes());
                }
                Hash::from_bytes(b)
            };
            let (a, b) = (child(), child());
            let root = k % 5 == 0;
            let expect = ref_node(&a, &b, root);
            assert_eq!(
                *hash_node(&a, &b, root).as_bytes(),
                expect,
                "hash_node differs"
            );
            if !root {
                pairs.push((a, b));
                want.push(expect);
            }
            if pairs.len() == 61 || k + 1 == count {
                let mut out = vec![Hash::from_bytes([0; 32]); pairs.len()];
                hash_node_batch(&pairs, false, &mut out);
                for (o, w) in out.iter().zip(&want) {
                    assert_eq!(o.as_bytes(), w, "hash_node_batch differs");
                }
                pairs.clear();
                want.clear();
            }
            if k % 8 == 0 {
                let len = (r.next() % 130) as usize;
                let chunk: Vec<u8> = (0..len).map(|_| r.next() as u8).collect();
                let ctr = r.next() >> (r.next() % 64);
                assert_eq!(
                    *hash_leaf(&chunk, ctr, root).as_bytes(),
                    ref_leaf(&chunk, ctr, root)
                );
            }
        }
    });
    println!(
        "hash_node: {total} random pairs (single + batched), hash_leaf: {}",
        total / 8
    );
}

/// A profile with the shape of the profile-v2 proposal (x⁷ everywhere,
/// RF 6+6, RP 48). Constants are pseudo-random placeholders: the test
/// checks that the kernels are generic in round counts and S-box kind,
/// not any parameter choice.
#[test]
fn v2_shaped_profile_matches_reference() {
    let mut r = Rng(0xC0FFEE);
    let consts: Vec<u64> = (0..12 * 16 + 48).map(|_| r.next() % P).collect();
    let diag = reference::DIAG;
    let (ext, int) = consts.split_at(12 * 16);
    let profile = Profile::new(6, 48, PartialSbox::Pow7, ext, int, &diag);
    let inv_profile = Profile::new(6, 48, PartialSbox::Inverse, ext, int, &diag);
    let total = n() / 10;
    for (prof, inverse) in [(profile, false), (inv_profile, true)] {
        let mut batch = Vec::new();
        let mut want = Vec::new();
        for k in 0..total {
            let raw = r.state();
            let mut expect = raw;
            reference::permute_generic(&mut expect, 6, 48, inverse, &consts, &diag);
            let mut s = raw.map(Goldilocks::new);
            permute_profile(&mut s, &prof);
            assert_eq!(canon(s), expect);
            batch.push(raw.map(Goldilocks::new));
            want.push(expect);
            if batch.len() == 29 || k + 1 == total {
                permute_batch_profile(&mut batch, &prof);
                for (g, w) in batch.iter().zip(&want) {
                    assert_eq!(canon(*g), *w);
                }
                batch.clear();
                want.clear();
            }
        }
    }
    println!("v2-shaped profiles (x^7 and x^-1 partial): {total} states each");
}

#[test]
fn leaf_batch_matches_reference() {
    let total = n() / 4;
    parallel(total, |seed, count| {
        let mut r = Rng(seed ^ 0x77);
        let mut k = 0;
        while k < count {
            // Groups of equal length (lockstep path), sometimes with one
            // odd length (fallback path); lengths cross the 56-byte rate.
            let group = 1 + (r.next() % 40) as usize;
            let len = (r.next() % 200) as usize;
            let odd = r.next().is_multiple_of(4);
            let chunks: Vec<Vec<u8>> = (0..group)
                .map(|g| {
                    let l = if odd && g == group / 2 { len + 1 } else { len };
                    (0..l).map(|_| r.next() as u8).collect()
                })
                .collect();
            let leaves: Vec<(&[u8], u64)> = chunks
                .iter()
                .map(|c| (c.as_slice(), r.next() >> 20))
                .collect();
            let root = r.next().is_multiple_of(9);
            let mut out = vec![Hash::from_bytes([0; 32]); group];
            hash_leaf_batch(&leaves, root, &mut out);
            for ((c, ctr), o) in leaves.iter().zip(&out) {
                assert_eq!(
                    *o.as_bytes(),
                    ref_leaf(c, *ctr, root),
                    "hash_leaf_batch differs"
                );
            }
            k += group;
        }
    });
    println!("hash_leaf_batch: {total} random leaves in groups of 1..=40");
}

/// The traced and stepped permutations end bit-identical (raw limbs, not
/// only canonical values) to `permute`.
#[test]
fn traced_and_stepped_equal_permute() {
    use cyber_hemera::StepSponge;
    use cyber_hemera::permutation::permute_traced;
    use cyber_hemera::trace::{FullRoundWitnesses, RoundVisitor};
    struct Nop;
    impl RoundVisitor for Nop {
        fn full_round(&mut self, _: u8, _: &[Goldilocks; 16], _: &FullRoundWitnesses) {}
        fn partial_round(&mut self, _: u8, _: &[Goldilocks; 16], _: Goldilocks) {}
    }
    let mut r = Rng(0x7EACE);
    for _ in 0..n() / 10 {
        let raw = r.state();
        let mut plain = raw.map(Goldilocks::new);
        permute(&mut plain);
        let mut traced = raw.map(Goldilocks::new);
        permute_traced(&mut traced, &mut Nop);
        assert_eq!(traced, plain);
        let rate: [Goldilocks; 8] = core::array::from_fn(|i| Goldilocks::new(raw[i]));
        let mut atomic = [Goldilocks::new(0); 16];
        atomic[..8].copy_from_slice(&rate);
        permute(&mut atomic);
        assert_eq!(StepSponge::absorb(&rate).last().unwrap(), atomic);
    }
}
