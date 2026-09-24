//! Run: cargo run -p cyber-hemera --example structural_identity
//! Demonstrates commitments over extracted fields, not a `.cyb` parser.
use cyber_hemera::{commitment as c, cyb_commitment as cyb};

fn print_id(label: &str, bytes: &[u8]) {
    print!("{label} ");
    for byte in bytes {
        print!("{byte:02x}");
    }
    println!();
}

fn main() {
    let weights = [42; 8192];
    let entry = cyb::Entry {
        name: "weights",
        declaration: b"name = 'weights'\nelement = 32\nsize = 8192\n",
        content: &weights,
        element_size: 32,
    };
    let first = cyb::container(b"[cyb]\nname = 'first'\n", &[entry], &mut [""; 1]).unwrap();
    let second = cyb::container(b"[cyb]\nname = 'second'\n", &[entry], &mut [""; 1]).unwrap();
    let section = cyb::section(&weights, 32).unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(first.sequence, second.sequence);
    assert_eq!(cyb::entry(&entry).unwrap().content, section.id);
    let mut chunks = [c::blob(b""); 4];
    for (slot, range) in chunks
        .iter_mut()
        .zip(cyber_hemera::cdc::chunk_ranges(&weights, 32).unwrap())
    {
        *slot = c::blob(&weights[range]);
    }
    assert_eq!(chunks[0], chunks[1]);
    let (root, proof) = c::prove(&chunks, 1).unwrap();
    assert_eq!(root, section.sequence);
    assert!(c::verify(chunks[1], &proof, root));
    print_id("first container ", first.id.as_bytes());
    print_id("second container", second.id.as_bytes());
    print_id("shared section  ", section.id.as_bytes());
    print_id("shared chunk    ", chunks[0].as_bytes());
    println!("chunk 1 opening verified; {} siblings", proof.sibling_count);
}
