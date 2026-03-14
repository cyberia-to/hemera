//! Test that hemera-rs matches the pinned test vectors in vectors/hemera.json.

use cyber_hemera::{hash, derive_key, tree::root_hash};

#[test]
fn hash_empty() {
    assert_eq!(
        hash(b"").to_string(),
        "6cea59b721c719bd156fecf46a4cdb13f7708eecaed94ec40c69037f7a872f1a8d544ace7f339656dcac635d0c787ed77d7f156ccee8551862a01683211b8bd7"
    );
}

#[test]
fn hash_hello() {
    assert_eq!(
        hash(b"hello").to_string(),
        "287411eea25b1bee2a6147ba32d22b48cc5d59dcf19bf8b19650107d16cd59a9efb2b5169ed336d096f815a468b74cf4c91973abcd8c5358383928f29d7644ac"
    );
}

#[test]
fn hash_hemera() {
    assert_eq!(
        hash(b"hemera").to_string(),
        "2d9ea21f9e2e42c42babe7567f7db0a79c7f6ccaa80941760391c86f53beaffc1771dfa002ba97c8cb9ad8c93eb6fbe7987d374e3b33bc0c75e5b19614059c6f"
    );
}

#[test]
fn tree_empty() {
    assert_eq!(
        root_hash(b"").to_string(),
        "6036c37455619f225ce9bd112fb9f245af1ef99d61c0fa4d023bec91dc8fd4df94c97c9eaab4ea3e2f6a526021a014eecc9f5b831d297252457d2c8d8e87d9c4"
    );
}

#[test]
fn tree_hello() {
    assert_eq!(
        root_hash(b"hello").to_string(),
        "d68dfd8b91edb2594013eb237048ee614fe31caee0d466168bbf289965056c602c32056719eb3dbeda6d602ba6302cb60537953223cc227e5979fc69b7dad1a2"
    );
}

#[test]
fn derive_key_test() {
    let k = derive_key("test context", b"test material");
    let hex: String = k.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex,
        "7d6cd5c816efd1f20585f3b8dd2b67afd0c00d3784a636783cff892d689cb2cbde7331e541d912216ac53a11897302922c427e1d10597f5cbdbfed12ee95d7f6"
    );
}
