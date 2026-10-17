// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! The constant-time kernel against the fast kernel and the oracle.
//!
//! - `permute_ct` equals `permute` bit for bit (raw limbs) on
//!   `HEMERA_DIFF_N` random states (default 10⁶ in release);
//! - the secret-input entry points (`hash_secret`, `keyed_hash`,
//!   `derive_key`, `Hasher::new_secret` XOF) equal a reference sponge built
//!   on `reference::permute`, on `HEMERA_DIFF_N / 10` random inputs each.
//!
//! ```text
//! cargo test --release -p cyber-hemera --test constant_time -- --nocapture
//! ```

use cyber_hemera::field::{Goldilocks, P};
use cyber_hemera::permutation::{permute, permute_ct};
use cyber_hemera::{Hasher, derive_key, hash, hash_secret, keyed_hash, reference};

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
    fn elem(&mut self) -> u64 {
        match self.next() % 16 {
            0 => [0, 1, P - 1, P, P + 1, u64::MAX, 1 << 32, (1 << 32) - 1]
                [(self.next() % 8) as usize],
            1 => P + self.next() % (u64::MAX - P + 1),
            2 => self.next() >> 32,
            _ => self.next(),
        }
    }
    fn bytes(&mut self, max: u64) -> Vec<u8> {
        let len = (self.next() % (max + 1)) as usize;
        (0..len).map(|_| self.next() as u8).collect()
    }
}

fn parallel(total: usize, f: impl Fn(u64, usize) + Sync) {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let per = total.div_ceil(threads);
    std::thread::scope(|sc| {
        for t in 0..threads {
            let f = &f;
            let count = per.min(total.saturating_sub(t * per));
            sc.spawn(move || f(t as u64 * 0x1000_0001 + 3, count));
        }
    });
}

#[test]
fn ct_kernel_equals_fast_kernel_bit_for_bit() {
    let total = n();
    parallel(total, |seed, count| {
        let mut r = Rng(seed);
        for _ in 0..count {
            let raw: [u64; 16] = match r.next() % 64 {
                0 => [0; 16],
                1 => [P; 16],
                _ => core::array::from_fn(|_| r.elem()),
            };
            let mut fast = raw.map(Goldilocks::new);
            let mut ct = fast;
            permute(&mut fast);
            permute_ct(&mut ct);
            assert_eq!(ct, fast, "permute_ct differs on {raw:x?}");
        }
    });
    println!("permute_ct == permute (raw limbs): {total} random states");
}

/// Reference sponge on `reference::permute`: `state` pre-initialised
/// (domain, seeding), `msg` absorbed, length `absorbed` in capacity[2].
fn ref_sponge(mut st: [u64; 16], msg: &[u8], absorbed: u64) -> [u64; 16] {
    let absorb = |st: &mut [u64; 16], block: &[u8]| {
        for i in 0..8 {
            let mut b = [0u8; 8];
            b[..7].copy_from_slice(&block[7 * i..7 * i + 7]);
            st[i] = reference::add(st[i], u64::from_le_bytes(b));
        }
    };
    let mut chunks = msg.chunks_exact(56);
    for c in &mut chunks {
        absorb(&mut st, c);
        reference::permute(&mut st);
    }
    let rem = chunks.remainder();
    let mut last = [0u8; 56];
    last[..rem.len()].copy_from_slice(rem);
    last[rem.len()] = 0x01;
    absorb(&mut st, &last);
    st[10] = absorbed;
    reference::permute(&mut st);
    st
}

fn digest(st: &[u64; 16]) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..4 {
        out[8 * i..8 * i + 8].copy_from_slice(&(st[i] % P).to_le_bytes());
    }
    out
}

fn domain(d: u64) -> [u64; 16] {
    let mut st = [0u64; 16];
    st[11] = d;
    st
}

#[test]
fn secret_entry_points_match_reference() {
    let total = n() / 10;
    parallel(total, |seed, count| {
        let mut r = Rng(seed ^ 0x5EC);
        for _ in 0..count {
            // hash_secret = hash = plain sponge
            let msg = r.bytes(200);
            let want = digest(&ref_sponge(domain(0), &msg, msg.len() as u64));
            assert_eq!(*hash_secret(&msg).as_bytes(), want);
            assert_eq!(*hash(&msg).as_bytes(), want);

            // keyed_hash: domain 1, key absorbed as the first 32 bytes
            let key: [u8; 32] = core::array::from_fn(|_| r.next() as u8);
            let data = r.bytes(150);
            let joined: Vec<u8> = key.iter().chain(&data).copied().collect();
            let want = digest(&ref_sponge(domain(1), &joined, joined.len() as u64));
            assert_eq!(*keyed_hash(&key, &data).as_bytes(), want);

            // derive_key: context phase (domain 2), then material phase
            // (domain 3, context digest in the rate, one permutation)
            let ctx: String = (0..(r.next() % 80))
                .map(|_| char::from(b'a' + (r.next() % 26) as u8))
                .collect();
            let material = r.bytes(150);
            let c = ref_sponge(domain(2), ctx.as_bytes(), ctx.len() as u64);
            let mut st = domain(3);
            for i in 0..4 {
                st[i] = c[i] % P;
            }
            reference::permute(&mut st);
            let want = digest(&ref_sponge(st, &material, material.len() as u64));
            assert_eq!(derive_key(&ctx, &material), want);

            // XOF from a secret hasher: first 32 bytes are the digest, the
            // next 32 come from one more permutation
            let mut h = Hasher::new_secret();
            h.update(&msg);
            let mut out = [0u8; 64];
            h.finalize_xof().fill(&mut out);
            let mut st = ref_sponge(domain(0), &msg, msg.len() as u64);
            assert_eq!(out[..32], digest(&st));
            reference::permute(&mut st);
            assert_eq!(out[32..], digest(&st));
        }
    });
    println!("hash_secret / keyed_hash / derive_key / secret XOF: {total} random inputs each");
}
