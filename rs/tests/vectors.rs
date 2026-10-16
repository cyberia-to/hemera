// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Every known-answer vector of the current profile in
//! `vectors/hemera.json` (top level), checked against this crate. The same
//! file is checked independently by `audit/campaign/hemera_ref.py`, which
//! also checks the frozen `"0.3"` section against its 0.3 model.

use cyber_hemera::field::{Goldilocks, P};
use cyber_hemera::permutation::{HEMERA, Profile, permute, permute_ct, permute_profile};
use cyber_hemera::tree::{fixed_chunk_root, hash_leaf, hash_node, root_hash};
use cyber_hemera::{Hash, Hasher, derive_key, hash, hash_secret, keyed_hash};
use serde_json::Value;

fn vectors() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../vectors/hemera.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("vectors/hemera.json")).expect("json")
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
}

fn digest(s: &str) -> Hash {
    Hash::from_bytes(unhex(s).try_into().unwrap())
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn pattern(len: usize, mul: usize, add: usize) -> Vec<u8> {
    (0..len).map(|i| ((i * mul + add) & 0xFF) as u8).collect()
}

#[test]
fn version_matches_crate() {
    assert_eq!(text(&vectors()["version"]), env!("CARGO_PKG_VERSION"));
}

#[test]
fn permutation_vectors() {
    let v = vectors();
    let cases = v["permutation"].as_array().unwrap();
    assert!(cases.len() >= 4);
    for case in cases {
        let limbs = |k: &str| -> [u64; 16] {
            let a = case[k].as_array().unwrap();
            core::array::from_fn(|i| u64::from_str_radix(text(&a[i]), 16).unwrap())
        };
        let (input, want) = (limbs("input"), limbs("output"));
        assert!(input.iter().all(|&x| x < P));
        let mut s = input.map(Goldilocks::new);
        permute(&mut s);
        assert_eq!(s.map(|e| e.as_canonical_u64()), want, "{}", case["name"]);
        let mut c = input.map(Goldilocks::new);
        permute_ct(&mut c);
        assert_eq!(c.map(|e| e.as_canonical_u64()), want, "ct {}", case["name"]);
        let mut r = input;
        cyber_hemera::reference::permute(&mut r);
        assert_eq!(r, want, "reference {}", case["name"]);
    }
}

#[test]
fn hash_vectors() {
    let v = vectors();
    for (name, want) in v["hash"].as_object().unwrap() {
        let msg = match name.as_str() {
            "empty" => Vec::new(),
            "hello" => b"hello".to_vec(),
            "hemera" => b"hemera".to_vec(),
            other => unhex(text(&v["inputs"][other])),
        };
        assert_eq!(hash(&msg).to_string(), text(want), "hash {name}");
        assert_eq!(hash_secret(&msg).to_string(), text(want), "hash_secret {name}");
    }
}

#[test]
fn tree_vectors() {
    let v = vectors();
    for (name, want) in v["tree"].as_object().unwrap() {
        let msg = match name.as_str() {
            "empty" => Vec::new(),
            "hello" => b"hello".to_vec(),
            "4k_zeros" => vec![0u8; 4096],
            other => panic!("unknown tree vector {other}"),
        };
        assert_eq!(fixed_chunk_root(&msg).to_string(), text(want), "tree {name}");
    }
    for case in v["fixed_chunk_root"].as_array().unwrap() {
        let data = vec![case["fill"].as_u64().unwrap() as u8; case["len"].as_u64().unwrap() as usize];
        assert_eq!(fixed_chunk_root(&data).to_string(), text(&case["output"]), "{}", case["name"]);
    }
    for case in v["root_hash"].as_array().unwrap() {
        let n = |k: &str| case[k].as_u64().unwrap() as usize;
        let data = pattern(n("len"), n("mul"), n("add"));
        assert_eq!(root_hash(&data).to_string(), text(&case["output"]), "{}", case["name"]);
    }
}

#[test]
fn leaf_and_node_vectors() {
    let v = vectors();
    for case in v["hash_leaf"].as_array().unwrap() {
        let h = hash_leaf(
            &unhex(text(&case["input"])),
            case["counter"].as_u64().unwrap(),
            case["root"].as_bool().unwrap(),
        );
        assert_eq!(h.to_string(), text(&case["output"]), "{}", case["name"]);
    }
    for case in v["hash_node"].as_array().unwrap() {
        let h = hash_node(
            &digest(text(&case["left"])),
            &digest(text(&case["right"])),
            case["root"].as_bool().unwrap(),
        );
        assert_eq!(h.to_string(), text(&case["output"]), "{}", case["name"]);
    }
}

#[test]
fn keyed_derive_xof_vectors() {
    let v = vectors();
    let dk = derive_key("test context", b"test material");
    assert_eq!(hexs(&dk), text(&v["derive_key"]["test"]));
    for case in v["derive_key_cases"].as_array().unwrap() {
        let k = derive_key(text(&case["context"]), &unhex(text(&case["material"])));
        assert_eq!(hexs(&k), text(&case["output"]), "{}", case["name"]);
    }
    for case in v["keyed_hash"].as_array().unwrap() {
        let key: [u8; 32] = unhex(text(&case["key"])).try_into().unwrap();
        let h = keyed_hash(&key, &unhex(text(&case["input"])));
        assert_eq!(h.to_string(), text(&case["output"]), "{}", case["name"]);
    }
    for case in v["xof"].as_array().unwrap() {
        let want = unhex(text(&case["output"]));
        let mut got = vec![0u8; want.len()];
        Hasher::new().update(&unhex(text(&case["input"]))).finalize_xof().fill(&mut got);
        assert_eq!(got, want, "{}", case["name"]);
        let mut ct = vec![0u8; want.len()];
        Hasher::new_secret().update(&unhex(text(&case["input"]))).finalize_xof().fill(&mut ct);
        assert_eq!(ct, want, "secret {}", case["name"]);
    }
}

/// Reduced-round instances of the campaign: the first rf/2 rows of each
/// external group and the first rp internal constants of the frozen set.
#[test]
fn campaign_instance_vectors() {
    let v = vectors();
    let cases = v["campaign_instances"].as_array().unwrap();
    assert_eq!(cases.len(), 7);
    for case in cases {
        let rf = case["rf"].as_u64().unwrap() as usize;
        let rp = case["rp"].as_u64().unwrap() as usize;
        let half = HEMERA.external.len() / 2;
        let mut ext = HEMERA.external[..rf / 2 * 16].to_vec();
        ext.extend_from_slice(&HEMERA.external[half..half + rf / 2 * 16]);
        let prof = Profile::new(rf / 2, rp, &ext, &HEMERA.internal[..rp], HEMERA.diag);
        let mut s = [Goldilocks::new(0); 16];
        permute_profile(&mut s, &prof);
        let want: Vec<u64> = case["output"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| u64::from_str_radix(text(x), 16).unwrap())
            .collect();
        assert_eq!(s.map(|e| e.as_canonical_u64()).to_vec(), want, "instance ({rf}, {rp})");
        let mut r = [0u64; 16];
        let mut consts = ext.clone();
        consts.extend_from_slice(&HEMERA.internal[..rp]);
        cyber_hemera::reference::permute_generic(&mut r, rf / 2, rp, &consts, HEMERA.diag);
        assert_eq!(r.to_vec(), want, "reference ({rf}, {rp})");
    }
    // the full instance is the permutation itself
    let mut z = [Goldilocks::new(0); 16];
    permute(&mut z);
    let full = cases.last().unwrap()["output"].as_array().unwrap();
    assert_eq!(format!("{:016x}", z[0].as_canonical_u64()), text(&full[0]));
}

/// The 0.3 record stays in the file, untouched, beside the current vectors.
#[test]
fn record_of_0_3_is_kept() {
    let v = vectors();
    assert_eq!(text(&v["0.3"]["version"]), "0.3.1");
    assert_eq!(
        text(&v["0.3"]["hash"]["empty"]),
        "a67a71b221e6bdd6442a20432bf5d74c885d89e5dfbeec3ec4e334cb806d563c"
    );
    assert_ne!(text(&v["hash"]["empty"]), text(&v["0.3"]["hash"]["empty"]));
}
