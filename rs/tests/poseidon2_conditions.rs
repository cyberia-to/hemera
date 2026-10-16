// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! The Poseidon2 security conditions on the 0.4 linear layers, checked on
//! the layers the crate actually computes (`specs/matrices.md`):
//!
//! - the matrices are read off the implementation (`mds_light_permutation`
//!   and `matmul_internal` applied to unit vectors) and equal their
//!   definitions: M_E = circ(2·M4, M4, M4, M4), M_I = 1 + diag(d);
//! - M4 is MDS (all 69 square minors nonzero), M_E and M_I are invertible;
//! - for i = 1 … 2t = 32 the characteristic polynomial of M_I^i is
//!   irreducible over F_p, so its minimal polynomial has full degree 16 and
//!   M_I^i has no nontrivial invariant subspace — the sufficient condition
//!   of the Poseidon2 parameter script against infinitely long subspace
//!   trails (Grassi, Rechberger, Schofnegger, ePrint 2020/500);
//! - the round constants pass the bootstrap checks.
//!
//! Arithmetic on plain `u128` remainders — no code shared with the field
//! implementation under test. The same conditions are checked by
//! `audit/campaign/poseidon2_params.py`.

#![allow(clippy::needless_range_loop)] // matrix code indexes rows and columns together

use cyber_hemera::bootstrap::constant_checks;
use cyber_hemera::constants::ROUND_CONSTANTS_U64;
use cyber_hemera::field::{Goldilocks, MATRIX_DIAG_16, P, matmul_internal, mds_light_permutation};

const T: usize = 16;
type Mat = [[u64; T]; T];

fn mul(a: u64, b: u64) -> u64 {
    (u128::from(a) * u128::from(b) % u128::from(P)) as u64
}
fn add(a: u64, b: u64) -> u64 {
    ((u128::from(a) + u128::from(b)) % u128::from(P)) as u64
}
fn sub(a: u64, b: u64) -> u64 {
    add(a, P - b % P)
}
fn inv(a: u64) -> u64 {
    let (mut r, mut base, mut e) = (1u64, a, P - 2);
    while e > 0 {
        if e & 1 == 1 {
            r = mul(r, base);
        }
        base = mul(base, base);
        e >>= 1;
    }
    r
}

/// Column j of a linear layer = the layer applied to e_j.
fn read_layer(layer: fn(&mut [Goldilocks; T])) -> Mat {
    let mut m = [[0u64; T]; T];
    for j in 0..T {
        let mut e = [Goldilocks::new(0); T];
        e[j] = Goldilocks::new(1);
        layer(&mut e);
        for i in 0..T {
            m[i][j] = e[i].as_canonical_u64();
        }
    }
    m
}

fn det(rows: &[Vec<u64>]) -> u64 {
    let mut m: Vec<Vec<u64>> = rows.to_vec();
    let n = m.len();
    let mut d = 1u64;
    for c in 0..n {
        let Some(piv) = (c..n).find(|&r| m[r][c] != 0) else {
            return 0;
        };
        if piv != c {
            m.swap(piv, c);
            d = sub(0, d);
        }
        d = mul(d, m[c][c]);
        let ic = inv(m[c][c]);
        for r in c + 1..n {
            let f = mul(m[r][c], ic);
            for k in c..n {
                m[r][k] = sub(m[r][k], mul(f, m[c][k]));
            }
        }
    }
    d
}

fn matmul(a: &Mat, b: &Mat) -> Mat {
    let mut c = [[0u64; T]; T];
    for i in 0..T {
        for j in 0..T {
            let mut acc = 0u64;
            for k in 0..T {
                acc = add(acc, mul(a[i][k], b[k][j]));
            }
            c[i][j] = acc;
        }
    }
    c
}

/// Coefficients c_0 … c_16 (c_16 = 1) of det(xI − A), Faddeev–LeVerrier.
fn charpoly(a: &Mat) -> [u64; T + 1] {
    let mut c = [0u64; T + 1];
    c[T] = 1;
    let mut m = [[0u64; T]; T];
    for k in 1..=T {
        let mut am = matmul(a, &m);
        for (i, row) in am.iter_mut().enumerate() {
            row[i] = add(row[i], c[T - k + 1]);
        }
        m = am;
        let am = matmul(a, &m);
        let tr = (0..T).fold(0, |acc, i| add(acc, am[i][i]));
        c[T - k] = sub(0, mul(tr, inv(k as u64)));
    }
    c
}

/// Product of two residues modulo the monic degree-16 `f`.
fn mulmod(a: &[u64; T], b: &[u64; T], f: &[u64; T + 1]) -> [u64; T] {
    let mut r = [0u64; 2 * T - 1];
    for i in 0..T {
        for j in 0..T {
            r[i + j] = add(r[i + j], mul(a[i], b[j]));
        }
    }
    for i in (T..2 * T - 1).rev() {
        let q = r[i];
        for j in 0..=T {
            r[i - T + j] = sub(r[i - T + j], mul(q, f[j]));
        }
    }
    core::array::from_fn(|i| r[i])
}

fn powmod(mut a: [u64; T], mut e: u64, f: &[u64; T + 1]) -> [u64; T] {
    let mut r = [0u64; T];
    r[0] = 1;
    while e > 0 {
        if e & 1 == 1 {
            r = mulmod(&r, &a, f);
        }
        a = mulmod(&a, &a, f);
        e >>= 1;
    }
    r
}

fn trim(mut v: Vec<u64>) -> Vec<u64> {
    while v.last() == Some(&0) {
        v.pop();
    }
    v
}

fn gcd(a: Vec<u64>, b: Vec<u64>) -> Vec<u64> {
    let (mut a, mut b) = (trim(a), trim(b));
    while !b.is_empty() {
        let ib = inv(*b.last().unwrap());
        while a.len() >= b.len() {
            let q = mul(*a.last().unwrap(), ib);
            let s = a.len() - b.len();
            for (j, &bj) in b.iter().enumerate() {
                a[s + j] = sub(a[s + j], mul(q, bj));
            }
            a = trim(a);
        }
        core::mem::swap(&mut a, &mut b);
    }
    a
}

/// Rabin: monic f of degree 16 is irreducible over F_p iff x^(p^16) ≡ x
/// (mod f) and gcd(x^(p^8) − x, f) = 1 (2 is the only prime dividing 16).
fn irreducible(f: &[u64; T + 1]) -> bool {
    let mut x = [0u64; T];
    x[1] = 1;
    let mut frob = x;
    let mut frob8 = x;
    for k in 1..=T {
        frob = powmod(frob, P, f);
        if k == T / 2 {
            frob8 = frob;
        }
    }
    if frob != x {
        return false;
    }
    let d: Vec<u64> = (0..T).map(|i| sub(frob8[i], x[i])).collect();
    gcd(f.to_vec(), d).len() == 1
}

const M4: [[u64; 4]; 4] = [[5, 7, 1, 3], [4, 6, 1, 1], [1, 3, 5, 7], [1, 1, 4, 6]];

#[test]
fn layers_are_the_specified_matrices() {
    let me = read_layer(mds_light_permutation);
    for i in 0..T {
        for j in 0..T {
            let k = if i / 4 == j / 4 { 2 } else { 1 };
            assert_eq!(me[i][j], k * M4[i % 4][j % 4], "M_E[{i}][{j}]");
        }
    }
    let mi = read_layer(matmul_internal);
    for i in 0..T {
        for j in 0..T {
            let want = if i == j {
                add(MATRIX_DIAG_16[i].as_canonical_u64(), 1)
            } else {
                1
            };
            assert_eq!(mi[i][j], want, "M_I[{i}][{j}]");
        }
    }
}

#[test]
fn m4_is_mds() {
    let mut minors = 0;
    for k in 1..=4usize {
        for rows in (0..16u32).filter(|m| m.count_ones() as usize == k) {
            for cols in (0..16u32).filter(|m| m.count_ones() as usize == k) {
                let sub: Vec<Vec<u64>> = (0..4)
                    .filter(|r| rows >> r & 1 == 1)
                    .map(|r| {
                        (0..4)
                            .filter(|c| cols >> c & 1 == 1)
                            .map(|c| M4[r][c])
                            .collect()
                    })
                    .collect();
                assert_ne!(
                    det(&sub),
                    0,
                    "singular minor rows {rows:04b} cols {cols:04b}"
                );
                minors += 1;
            }
        }
    }
    assert_eq!(minors, 69);
}

#[test]
fn external_and_internal_layers_are_invertible() {
    let rows = |m: &Mat| m.iter().map(|r| r.to_vec()).collect::<Vec<_>>();
    assert_ne!(det(&rows(&read_layer(mds_light_permutation))), 0);
    assert_ne!(det(&rows(&read_layer(matmul_internal))), 0);
}

#[test]
fn internal_powers_have_irreducible_characteristic_polynomials() {
    let mi = read_layer(matmul_internal);
    let mut power = mi;
    for i in 1..=2 * T {
        assert!(
            irreducible(&charpoly(&power)),
            "charpoly(M_I^{i}) is reducible"
        );
        power = matmul(&mi, &power);
    }
}

/// Negative control: the irreducibility test rejects a matrix with an
/// invariant subspace (the all-ones matrix plus the identity: the vector
/// (1, …, 1) is an eigenvector).
#[test]
fn irreducibility_test_rejects_a_reducible_case() {
    let mut m = [[1u64; T]; T];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 2;
    }
    assert!(!irreducible(&charpoly(&m)));
}

#[test]
fn round_constants_pass_the_bootstrap_checks() {
    assert_eq!(constant_checks(&ROUND_CONSTANTS_U64), [true; 3]);
}
