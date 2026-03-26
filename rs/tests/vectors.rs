// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Test that hemera-rs matches the pinned test vectors in vectors/hemera.json.

use cyber_hemera::{hash, derive_key, tree::root_hash};

#[test]
fn hash_empty() {
    assert_eq!(
        hash(b"").to_string(),
        "a67a71b221e6bdd6442a20432bf5d74c885d89e5dfbeec3ec4e334cb806d563c"
    );
}

#[test]
fn hash_hello() {
    assert_eq!(
        hash(b"hello").to_string(),
        "e1b19b8235443e9fac8f1d6a1203de66e9a58c53e36cbbc1f71a031c3d13ce77"
    );
}

#[test]
fn hash_hemera() {
    assert_eq!(
        hash(b"hemera").to_string(),
        "94341ea38ac105378d9e8ce04ac889fdbcb952c7877d9ab9225ecc022b66c82a"
    );
}

#[test]
fn tree_empty() {
    assert_eq!(
        root_hash(b"").to_string(),
        "ea57b2e6b1ec7d2de11b15cb6d7060dd61d247fe0fbf5f7d3fb97a7be9328552"
    );
}

#[test]
fn tree_hello() {
    assert_eq!(
        root_hash(b"hello").to_string(),
        "626fa46e4e7bd5c87d630eef8333931a0b9198587400a0191eae7821692880d7"
    );
}

#[test]
fn derive_key_test() {
    let k = derive_key("test context", b"test material");
    let hex: String = k.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex,
        "8c001612a68bc3c3b7caf998c361bfff6dddb3ff64a5cd1a0ba855277fd28e13"
    );
}
