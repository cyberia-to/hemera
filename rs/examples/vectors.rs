// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Emit `vectors/hemera.json`: the known-answer vectors of the current
//! profile at the top level, and the frozen 0.3 record under `"0.3"`.
//!
//! ```text
//! cargo run -p cyber-hemera --example vectors > vectors/hemera.json
//! ```
//!
//! Every entry is checked by `rs/tests/vectors.rs` (this crate) and
//! independently by `audit/campaign/hemera_ref.py` (the Python model).

use cyber_hemera::field::{Goldilocks, P};
use cyber_hemera::permutation::{HEMERA, Profile, permute, permute_profile};
use cyber_hemera::tree::{fixed_chunk_root, hash_leaf, hash_node, root_hash};
use cyber_hemera::{Hash, Hasher, derive_key, hash, keyed_hash};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn q(s: &str) -> String {
    format!("\"{s}\"")
}

/// Deterministic message bytes: `data[i] = (i·mul + add) mod 256`.
fn pattern(len: usize, mul: usize, add: usize) -> Vec<u8> {
    (0..len).map(|i| ((i * mul + add) & 0xFF) as u8).collect()
}

/// The 0.3 vectors as published with hemera 0.3.1 (the record).
const V03: &str = r#"{
    "version": "0.3.1",
    "hash": {
      "empty": "a67a71b221e6bdd6442a20432bf5d74c885d89e5dfbeec3ec4e334cb806d563c",
      "hello": "e1b19b8235443e9fac8f1d6a1203de66e9a58c53e36cbbc1f71a031c3d13ce77",
      "hemera": "94341ea38ac105378d9e8ce04ac889fdbcb952c7877d9ab9225ecc022b66c82a"
    },
    "tree": {
      "empty": "ea57b2e6b1ec7d2de11b15cb6d7060dd61d247fe0fbf5f7d3fb97a7be9328552",
      "hello": "626fa46e4e7bd5c87d630eef8333931a0b9198587400a0191eae7821692880d7",
      "4k_zeros": "66a2b0f7356486c81cadeb735ed5dda43a5842040e73ea2583ca2b4c63edccf0"
    },
    "derive_key": {
      "test": "8c001612a68bc3c3b7caf998c361bfff6dddb3ff64a5cd1a0ba855277fd28e13"
    }
  }"#;

fn main() {
    let mut out = Vec::new();
    out.push(format!("  \"version\": {},", q(env!("CARGO_PKG_VERSION"))));
    out.push(
        "  \"profile\": \"hemera 0.4: Poseidon2, Goldilocks, t 16, rate 8, capacity 8, x^7 every round, R_F 6+6, R_P 48, M4 [[5,7,1,3],[4,6,1,1],[1,3,5,7],[1,1,4,6]], internal diag MAT_DIAG16_M_1, 240 constants bootstrapped from seed 6379626572 at counter 0, digest 4 limbs (32 bytes)\",".into(),
    );
    out.push("  \"notes\": {\"tree\": \"fixed_chunk_root (4096-byte chunks)\", \"root_hash\": \"content-defined chunking tree, data[i] = (i*mul + add) mod 256\", \"fixed_chunk_root\": \"fill byte repeated len times\", \"permutation\": \"canonical u64 limbs, hex\", \"hash_secret\": \"equals hash on every input\"},".into());

    // permutation
    let mut rng = 0x243F_6A88_85A3_08D3u64;
    let mut next = || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng % P
    };
    let random: [u64; 16] = core::array::from_fn(|_| next());
    let perms: [(&str, [u64; 16]); 4] = [
        ("zeros", [0; 16]),
        ("iota", core::array::from_fn(|i| i as u64)),
        ("p_minus_1", [P - 1; 16]),
        ("random", random),
    ];
    let mut rows = Vec::new();
    for (name, input) in perms {
        let mut s = input.map(Goldilocks::new);
        permute(&mut s);
        let fmt = |v: &[u64]| {
            v.iter()
                .map(|x| q(&format!("{x:016x}")))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let outv: Vec<u64> = s.iter().map(|e| e.as_canonical_u64()).collect();
        rows.push(format!(
            "    {{\"name\": {}, \"input\": [{}], \"output\": [{}]}}",
            q(name),
            fmt(&input),
            fmt(&outv)
        ));
    }
    out.push(format!("  \"permutation\": [\n{}\n  ],", rows.join(",\n")));

    // hash, with the block-boundary lengths
    let extra: [(&str, Vec<u8>); 5] = [
        ("zeros_55", vec![0; 55]),
        ("zeros_56", vec![0; 56]),
        ("zeros_57", vec![0; 57]),
        ("pattern_112", pattern(112, 1, 0)),
        ("pattern_1000", pattern(1000, 31, 7)),
    ];
    let mut h = vec![
        format!("    \"empty\": {}", q(&hash(b"").to_string())),
        format!("    \"hello\": {}", q(&hash(b"hello").to_string())),
        format!("    \"hemera\": {}", q(&hash(b"hemera").to_string())),
    ];
    for (name, msg) in &extra {
        h.push(format!("    {}: {}", q(name), q(&hash(msg).to_string())));
    }
    out.push(format!("  \"hash\": {{\n{}\n  }},", h.join(",\n")));
    let inputs: Vec<String> = extra
        .iter()
        .map(|(n, m)| format!("    {}: {}", q(n), q(&hex(m))))
        .collect();
    out.push(format!("  \"inputs\": {{\n{}\n  }},", inputs.join(",\n")));

    // tree (fixed chunks), same names as 0.3
    out.push(format!(
        "  \"tree\": {{\n    \"empty\": {},\n    \"hello\": {},\n    \"4k_zeros\": {}\n  }},",
        q(&fixed_chunk_root(b"").to_string()),
        q(&fixed_chunk_root(b"hello").to_string()),
        q(&fixed_chunk_root(&[0u8; 4096]).to_string())
    ));

    // derive_key
    let dk = derive_key("test context", b"test material");
    out.push(format!(
        "  \"derive_key\": {{\n    \"test\": {}\n  }},",
        q(&hex(&dk))
    ));
    let long_ctx = "hemera 0.4 derive-key context longer than one 56-byte rate block";
    let mat = pattern(100, 3, 1);
    out.push(format!(
        "  \"derive_key_cases\": [\n    {{\"name\": \"long_context\", \"context\": {}, \"material\": {}, \"output\": {}}}\n  ],",
        q(long_ctx),
        q(&hex(&mat)),
        q(&hex(&derive_key(long_ctx, &mat)))
    ));

    // keyed_hash
    let key: [u8; 32] = core::array::from_fn(|i| i as u8);
    let mut k = Vec::new();
    for (name, msg) in [
        ("empty", Vec::new()),
        ("hello", b"hello".to_vec()),
        ("pattern_100", pattern(100, 5, 9)),
    ] {
        k.push(format!(
            "    {{\"name\": {}, \"key\": {}, \"input\": {}, \"output\": {}}}",
            q(name),
            q(&hex(&key)),
            q(&hex(&msg)),
            q(&keyed_hash(&key, &msg).to_string())
        ));
    }
    out.push(format!("  \"keyed_hash\": [\n{}\n  ],", k.join(",\n")));

    // xof: 96 bytes (three squeezes)
    let mut x = Vec::new();
    for (name, msg) in [("empty", Vec::new()), ("hello", b"hello".to_vec())] {
        let mut buf = [0u8; 96];
        Hasher::new().update(&msg).finalize_xof().fill(&mut buf);
        x.push(format!(
            "    {{\"name\": {}, \"input\": {}, \"output\": {}}}",
            q(name),
            q(&hex(&msg)),
            q(&hex(&buf))
        ));
    }
    out.push(format!("  \"xof\": [\n{}\n  ],", x.join(",\n")));

    // hash_leaf
    let mut l = Vec::new();
    for (name, msg, ctr, root) in [
        ("empty_root", Vec::new(), 0u64, true),
        ("hello_ctr7", b"hello".to_vec(), 7, false),
        ("pattern_144_ctr_max", pattern(144, 13, 1), u64::MAX, false),
    ] {
        l.push(format!(
            "    {{\"name\": {}, \"input\": {}, \"counter\": {}, \"root\": {}, \"output\": {}}}",
            q(name),
            q(&hex(&msg)),
            ctr,
            root,
            q(&hash_leaf(&msg, ctr, root).to_string())
        ));
    }
    out.push(format!("  \"hash_leaf\": [\n{}\n  ],", l.join(",\n")));

    // hash_node (incl. non-canonical limbs 2^64 - 1)
    let a = hash(b"left");
    let b = hash(b"right");
    let ff = Hash::from_bytes([0xFF; 32]);
    let zero = Hash::from_bytes([0; 32]);
    let mut n = Vec::new();
    for (name, left, right, root) in [
        ("interior", a, b, false),
        ("root", a, b, true),
        ("zeros", zero, zero, false),
        ("all_ff", ff, ff, false),
    ] {
        n.push(format!(
            "    {{\"name\": {}, \"left\": {}, \"right\": {}, \"root\": {}, \"output\": {}}}",
            q(name),
            q(&left.to_string()),
            q(&right.to_string()),
            root,
            q(&hash_node(&left, &right, root).to_string())
        ));
    }
    out.push(format!("  \"hash_node\": [\n{}\n  ],", n.join(",\n")));

    // fixed_chunk_root, multi-chunk
    let mut f = Vec::new();
    for (name, fill, len) in [
        ("ab_3_chunks", 0xABu8, 3 * 4096usize),
        ("cd_5_chunks_partial", 0xCD, 4 * 4096 + 100),
    ] {
        f.push(format!(
            "    {{\"name\": {}, \"fill\": {}, \"len\": {}, \"output\": {}}}",
            q(name),
            fill,
            len,
            q(&fixed_chunk_root(&vec![fill; len]).to_string())
        ));
    }
    out.push(format!(
        "  \"fixed_chunk_root\": [\n{}\n  ],",
        f.join(",\n")
    ));

    // root_hash (CDC)
    let mut r = Vec::new();
    for (name, len, mul, add) in [
        ("hello_like_5", 5usize, 1usize, 104usize),
        ("pattern_10000", 10_000, 7, 3),
        ("pattern_20000", 20_000, 131, 17),
    ] {
        r.push(format!(
            "    {{\"name\": {}, \"len\": {}, \"mul\": {}, \"add\": {}, \"output\": {}}}",
            q(name),
            len,
            mul,
            add,
            q(&root_hash(&pattern(len, mul, add)).to_string())
        ));
    }
    out.push(format!("  \"root_hash\": [\n{}\n  ],", r.join(",\n")));

    // reduced-round instances of the campaign (audit/campaign.md §4.1):
    // first rf/2 rows of the initial and of the terminal external group,
    // first rp internal constants; input = zero state.
    let mut c = Vec::new();
    for (rf, rp) in [
        (4usize, 8usize),
        (6, 16),
        (8, 24),
        (12, 24),
        (8, 22),
        (12, 32),
        (12, 48),
    ] {
        let h = rf / 2;
        let half = HEMERA.external.len() / 2;
        let mut ext = HEMERA.external[..h * 16].to_vec();
        ext.extend_from_slice(&HEMERA.external[half..half + h * 16]);
        let prof = Profile::new(h, rp, &ext, &HEMERA.internal[..rp], HEMERA.diag);
        let mut s = [Goldilocks::new(0); 16];
        permute_profile(&mut s, &prof);
        let limbs: Vec<String> = s
            .iter()
            .map(|e| q(&format!("{:016x}", e.as_canonical_u64())))
            .collect();
        c.push(format!(
            "    {{\"rf\": {rf}, \"rp\": {rp}, \"input\": \"zeros\", \"output\": [{}]}}",
            limbs.join(", ")
        ));
    }
    out.push(format!(
        "  \"campaign_instances\": [\n{}\n  ],",
        c.join(",\n")
    ));

    out.push(format!("  \"0.3\": {V03}"));
    println!("{{\n{}\n}}", out.join("\n"));
}
