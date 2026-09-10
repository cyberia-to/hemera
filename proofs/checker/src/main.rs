//! Strict Eidos checking plus separately reported Rust/model checks.
use cyber_eidos::{
    elab::ElabState,
    kernel,
    reduce::nf,
    stdlib::{self, std_env},
    surface::{DeclKind, Token, check_file, lex, parse_file},
    term::Term,
};
use cyber_hemera::{
    field::{self, Goldilocks},
    permutation, trace,
};
use std::{fs, path::Path};

fn checked(src: &str) -> Result<(ElabState, Vec<String>), String> {
    let tokens = lex(src)?;
    for token in &tokens {
        if matches!(
            token,
            Token::KwAxiom | Token::KwSorry | Token::KwImport | Token::KwInductive | Token::Hash
        ) {
            return Err(format!("excluded declaration/token: {token:?}"));
        }
    }
    let decls = parse_file(&tokens)?;
    let mut state = ElabState::new();
    state.add_stdlib();
    let mut env = std_env();
    let results = check_file(&decls, &mut state, &mut env)
        .map_err(|(name, error)| format!("{name}: {error}"))?;

    // Definitions and proofs are fully expanded by the frontend. Recheck
    // closed terms against a fresh environment with no user declarations.
    let fresh = std_env();
    let mut theorems = Vec::new();
    for result in results {
        let (body, ty) = state.globals.get(&result.name).ok_or("missing body")?;
        clean(body)?;
        clean(ty)?;
        let sort = kernel::infer(&fresh, &vec![], ty).map_err(|e| format!("type: {e:?}"))?;
        kernel::check(&fresh, &vec![], body, ty)
            .map_err(|e| format!("closed {}: {e:?}", result.name))?;
        if result.kind == DeclKind::Theorem {
            if nf(&fresh, &vec![], sort) != Term::Sort(0) {
                return Err(format!("{} is not a proposition", result.name));
            }
            theorems.push(result.name);
        }
    }
    if theorems.is_empty() {
        return Err("no theorems checked".into());
    }
    Ok((state, theorems))
}

/// Exclude all opaque constants, metas and unused/axiomatic stdlib facilities.
fn clean(t: &Term) -> Result<(), String> {
    match t {
        Term::Const(_) | Term::Meta(_) => return Err("constant or metavariable".into()),
        Term::Var(_) | Term::Sort(_) => {}
        Term::Pi(a, b) | Term::Lam(a, b) | Term::App(a, b) => {
            clean(a)?;
            clean(b)?;
        }
        Term::Let(a, b, c) | Term::EqSubst(a, b, c) => {
            clean(a)?;
            clean(b)?;
            clean(c)?;
        }
        Term::Ind(id, xs) | Term::Ctor(id, _, xs) => {
            allowed_ind(*id)?;
            for x in xs {
                clean(x)?;
            }
        }
        Term::Elim(id, m, cs, t) => {
            allowed_ind(*id)?;
            clean(m)?;
            clean(t)?;
            for c in cs {
                clean(c)?;
            }
        }
    }
    Ok(())
}

fn allowed_ind(id: u64) -> Result<(), String> {
    if [stdlib::NAT_ID, stdlib::BOOL_ID, stdlib::EQ_ID].contains(&id) {
        Ok(())
    } else {
        Err(format!("inductive outside proof fragment: {id}"))
    }
}

fn rejected(label: &str, source: &str) {
    assert!(
        checked(source).is_err(),
        "negative control accepted: {label}"
    );
    println!("REJECT {label}");
}

fn negative_controls(sbox: &str, matrix: &str) {
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
    let mut total = 0;
    for (name, src) in [("Sbox.ei", &sbox), ("Matrix.ei", &matrix)] {
        let (state, proofs) = checked(src)?;
        total += proofs.len();
        for theorem in proofs {
            println!("PROVED {name}: {theorem}");
        }
        if name == "Matrix.ei" {
            matrix_bridge(&state);
        }
    }
    negative_controls(&sbox, &matrix);
    rust_checks();
    println!(
        "{total} theorems rechecked; assumptions are explicit parameters; Rust checks reported separately"
    );
    Ok(())
}
