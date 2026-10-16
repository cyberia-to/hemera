// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Line-oriented oracle for cross-implementation agreement
//! (`audit/campaign/agree.py` drives it against the Python model).
//!
//! stdin, one request per line; stdout, one answer per line:
//! - `p <16 hex u64 limbs>` → the permuted state, 16 canonical hex limbs
//! - `h <hex bytes>` (or `h -` for empty) → `hash`, 32-byte hex digest
//! - `n <left hex32> <right hex32> <0|1>` → `hash_node(left, right, root)`
//!
//! ```text
//! cargo run --release -p cyber-hemera --example agree < requests
//! ```

use std::io::{BufRead, Write};

use cyber_hemera::field::Goldilocks;
use cyber_hemera::permutation::permute;
use cyber_hemera::tree::hash_node;
use cyber_hemera::{Hash, hash};

fn unhex(s: &str) -> Vec<u8> {
    if s == "-" {
        return Vec::new();
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex")).collect()
}

fn main() {
    let stdin = std::io::stdin();
    let mut out = std::io::BufWriter::new(std::io::stdout());
    for line in stdin.lock().lines() {
        let line = line.expect("stdin");
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("p") => {
                let limbs: Vec<u64> = parts.map(|x| u64::from_str_radix(x, 16).expect("limb")).collect();
                let mut s: [Goldilocks; 16] = core::array::from_fn(|i| Goldilocks::new(limbs[i]));
                permute(&mut s);
                let v: Vec<String> = s.iter().map(|e| format!("{:016x}", e.as_canonical_u64())).collect();
                writeln!(out, "{}", v.join(" ")).unwrap();
            }
            Some("h") => {
                writeln!(out, "{}", hash(&unhex(parts.next().unwrap_or("-")))).unwrap();
            }
            Some("n") => {
                let l = Hash::from_bytes(unhex(parts.next().unwrap()).try_into().unwrap());
                let r = Hash::from_bytes(unhex(parts.next().unwrap()).try_into().unwrap());
                let root = parts.next() == Some("1");
                writeln!(out, "{}", hash_node(&l, &r, root)).unwrap();
            }
            _ => writeln!(out, "?").unwrap(),
        }
    }
}
