//! Strict Eidos checking plus separately reported Rust/model checks.
use cyber_eidos::{
    elab::ElabState,
    reduce::nf,
    stdlib::{self, std_env},
    surface::{DeclKind, check_strict_source},
    term::Term,
};
use cyber_hemera::{
    field::{self, Goldilocks},
    permutation, trace,
};
use std::{fs, path::Path};

fn checked(src: &str) -> Result<(ElabState, Vec<String>), String> {
    let module = check_strict_source(src)?;
    let theorems: Vec<String> = module
        .declarations
        .into_iter()
        .filter(|d| d.kind == DeclKind::Theorem)
        .map(|d| d.name)
        .collect();
    if theorems.is_empty() {
        return Err("no theorems checked".into());
    }
    Ok((module.state, theorems))
}

fn rejected(label: &str, source: &str) {
    assert!(
        checked(source).is_err(),
        "negative control accepted: {label}"
    );
    println!("REJECT {label}");
}

fn negative_controls(sbox: &str, matrix: &str, inverse: &str) {
    let wrong = inverse.replace(
        "(constraint : Eq F (mul x (mul x y)) x)",
        "(constraint : Eq F x x)",
    );
    assert_ne!(wrong, inverse);
    rejected("missing nonzero inverse constraint", &wrong);
    let wrong = inverse.replace(
        "(constraint : Eq F (mul y (mul zero y)) y)",
        "(constraint : Eq F y y)",
    );
    assert_ne!(wrong, inverse);
    rejected("missing zero inverse constraint", &wrong);
    rejected("false equality", "theorem bad : Eq Nat 0 1 := by { rfl }");
    rejected("sorry", "theorem bad : Eq Nat 0 1 := by { sorry }");
    rejected("axiom", "axiom bad : Eq Nat 0 1");
    rejected("unresolved hole", "theorem bad : Eq Nat 0 1 := _");
    let wrong = sbox.replace("  mul x3 x4", "  mul x3 x3");
    assert_ne!(wrong, sbox);
    rejected("S-box x^6 substituted for x^7", &wrong);
    let wrong = matrix.replace(
        "(add (add (add (add a b) (add c d)) b) (add a b))",
        "(add (add (add (add a b) (add c d)) b) (add a a))",
    );
    assert_ne!(wrong, matrix);
    rejected("M4 row-0 coefficient changed", &wrong);
    let wrong = sbox.replace("(hy : Eq F y (mul x3 (mul x2 x2)))", "(hy : Eq F y y)");
    assert_ne!(wrong, sbox);
    rejected("missing S-box output constraint", &wrong);
}

fn app(f: Term, x: Term) -> Term {
    Term::App(Box::new(f), Box::new(x))
}

/// Evaluate a proved row model over Nat on basis vectors, then compare
/// coefficients with the actual Rust implementation. This is a model bridge
/// test, not a proof of Rust field arithmetic or an extraction theorem.
fn matrix_bridge(state: &ElabState) {
    let add = Term::Lam(
        Box::new(stdlib::nat()),
        Box::new(Term::Lam(
            Box::new(stdlib::nat()),
            Box::new(stdlib::nat_add(Term::Var(1), Term::Var(0))),
        )),
    );
    let env = std_env();
    for col in 0..4 {
        let mut basis = [Goldilocks::ZERO; 4];
        basis[col] = Goldilocks::new(1);
        field::apply_mat4(&mut basis);
        for (row, actual) in basis.iter().enumerate() {
            let mut term = state.globals[&format!("mat4_row{row}")].0.clone();
            term = app(app(term, stdlib::nat()), add.clone());
            for i in 0..4 {
                term = app(term, stdlib::nat_lit(u64::from(i == col)));
            }
            let expected = stdlib::nat_lit(actual.as_canonical_u64());
            assert_eq!(nf(&env, &vec![], term), expected, "M4 row={row} col={col}");
        }
    }
    println!("BRIDGE M4: all 16 coefficients agree with the Eidos models");
}

fn rust_checks() {
    struct WitnessCheck;
    impl trace::RoundVisitor for WitnessCheck {
        fn full_round(&mut self, _: u8, _: &[Goldilocks; 16], ws: &trace::FullRoundWitnesses) {
            for &[x2, x3] in ws {
                // Necessary consequence of x2=x*x, x3=x2*x in a field.
                assert_eq!(
                    (x3 * x3).as_canonical_u64(),
                    (x2 * x2 * x2).as_canonical_u64()
                );
            }
        }
        fn partial_round(&mut self, _: u8, _: &[Goldilocks; 16], _: Goldilocks) {}
    }
    let mut raw = 0x9e3779b97f4a7c15u64;
    for i in 0..128 {
        raw = raw.wrapping_mul(6364136223846793005).wrapping_add(1);
        let n = [0, 1, 2, field::P - 1, field::P, u64::MAX]
            .get(i)
            .copied()
            .unwrap_or(raw);
        let x = Goldilocks::new(n);
        let mut naive = Goldilocks::new(1);
        for _ in 0..7 {
            naive *= x;
        }
        assert_eq!(x.pow7().as_canonical_u64(), naive.as_canonical_u64());
        let mut a = [x; 16];
        let mut b = a;
        permutation::permute(&mut a);
        permutation::permute_traced(&mut b, &mut WitnessCheck);
        for (a, b) in a.iter().zip(b) {
            assert_eq!(a.as_canonical_u64(), b.as_canonical_u64());
        }
    }
    println!("BRIDGE S-box/trace: 128 inputs including noncanonical field representatives");

    for (name, transform, rows, cols, entry) in [
        (
            "internal",
            field::matmul_internal as fn(&mut [Goldilocks; 16]),
            [0, 1],
            [2, 3],
            1,
        ),
        (
            "external",
            field::mds_light_permutation as fn(&mut [Goldilocks; 16]),
            [0, 4],
            [8, 12],
            2,
        ),
    ] {
        let mut minor = [[Goldilocks::ZERO; 2]; 2];
        for (j, col) in cols.into_iter().enumerate() {
            let mut basis = [Goldilocks::ZERO; 16];
            basis[col] = Goldilocks::new(1);
            transform(&mut basis);
            for (i, row) in rows.into_iter().enumerate() {
                minor[i][j] = basis[row];
            }
        }
        for x in minor.iter().flatten() {
            assert_eq!(x.as_canonical_u64(), entry);
        }
        let det = minor[0][0] * minor[1][1] - minor[0][1] * minor[1][0];
        assert_eq!(det.as_canonical_u64(), 0);
        println!(
            "COUNTEREXAMPLE {name} 16x16 matrix: rows {rows:?}, columns {cols:?}, determinant 0"
        );
    }
}

fn main() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let sbox = fs::read_to_string(root.join("Sbox.ei")).map_err(|e| e.to_string())?;
    let matrix = fs::read_to_string(root.join("Matrix.ei")).map_err(|e| e.to_string())?;
    let inverse = fs::read_to_string(root.join("Inverse.ei")).map_err(|e| e.to_string())?;
    let mut total = 0;
    for (name, src) in [
        ("Sbox.ei", &sbox),
        ("Matrix.ei", &matrix),
        ("Inverse.ei", &inverse),
    ] {
        let (state, proofs) = checked(src)?;
        total += proofs.len();
        for theorem in proofs {
            println!("PROVED {name}: {theorem}");
        }
        if name == "Matrix.ei" {
            matrix_bridge(&state);
        }
    }
    negative_controls(&sbox, &matrix, &inverse);
    rust_checks();
    println!(
        "{total} theorems rechecked; assumptions are explicit parameters; Rust checks reported separately"
    );
    Ok(())
}
