# Full-round investigation, 2026-09-11

The user's acceptance condition is a justified production hash: establish the
required security of the exact inverse-16 design, or demonstrate a concrete
failure of that requirement. **Neither result has been obtained.** The previous
engineering recommendation to move to a different candidate does not satisfy
this condition and is not a finalized parameter decision.

## Exact system, including input encoding and zero branches

`python3 -B research/full_round_system.py` models the actual hash of every
seven-byte input: 56 boolean input variables, real constants and matrices,
initial diffusion, four x^7 rounds, sixteen inverse rounds, four x^7 rounds,
padding word 1, length word 7, and all four output coordinates.

There are 1161 variables, 1177 equations, and 616 quadratic equations (560
nonlinear permutation constraints plus 56 input-bit constraints). A variable
for the constant one is explicitly constrained. Inverse constraints include
zero: `z=xy`, `x(z-1)=0`, `y(z-1)=0`. Linear equations describe every other wire.

The script validates four complete witnesses and rejects a +1 perturbation of
every individual wire of each witness. Additional controls exercise inverse
inputs 0, 1, 2 and p-1. The four digests agree with the actual Rust hash in
`cargo test -p cyber-hemera --test full_round_model`.

Export the entire system and its zero-message witness using:

```
python3 -B research/full_round_system.py --system /tmp/hemera-full-system.json
```

This is a model with concrete bridge tests, not a formal Rust extraction.
Witness validation is not a search for alternative satisfying assignments.
No solver run or attack complexity is claimed. Seven-byte messages cover the
single-permutation case and are a deliberately restricted input domain; longer
messages and two-message collision systems need separate models.
`research/model-sources.json` pins the reviewed Rust sources; changes fail
closed until the translation is reviewed and the manifest is updated.

## Closing the zero-branch gap in the rational relation

Let X be the integer encoded by the first seven bytes, viewed as a field
element. All other initial state coordinates are constant. Use common-denominator
polynomials `[N_i(X)/D(X)]`. Full x^7 rounds multiply a degree upper bound by
seven. A partial inverse maps the active numerator U=N_0+cD to D^2, the other
numerators to N_i U, and the denominator to DU, before the linear layer.
This doubles the bound. Thus d=7^8*2^16=377801998336 bounds each final numerator
and denominator on the nonzero branch.

At the j-th partial round (j=0,...,15), deg(U_j) is at most 7^4*2^j.
Define E as the product of these sixteen U_j polynomials. Then
deg(E) <= 7^4*(2^16-1)=157349535. Whenever any actual inverse input first becomes
zero, that U_j vanishes, so E vanishes. For every X, including exceptional
branches, each actual output y_i therefore satisfies

```
E(X) D(X) y_i(X) - E(X) N_i(X) = 0.
```

The coefficient polynomials have degree at most **377959347871**. The model
checks that at X=0 every actual partial inverse input is nonzero, hence E and
D are not identically zero for this line. This supplies a nontrivial relation
over the whole seven-byte domain, not just the nonzero branch. This derivation
is mathematical reasoning over the field model; it is not yet checked in Eidos.

The [full Poseidon paper, appendix D.2.1](https://eips.ethereum.org/assets/eip-5988/papers/poseidon_paper.pdf)
develops rational interpolation for inverse S-boxes and identifies the factor-two
growth of a partial inverse layer. Its round inequalities concern its own
configurations. They cannot be copied as a security theorem for the mixed
Hemera/Poseidon2 layers.

**A low-degree relation does not by itself produce a collision or preimage.**
In particular d is not an attack time. Turning it into a claimed distinguisher
requires an explicit experiment, interpolation algorithm, exceptional-point
handling and advantage/work/memory analysis. Turning a distinguisher into a
break of the required hash security needs an additional argument. None is
silently substituted for the missing production-security conclusion here.

## Eidos dependency delivered in this slice

Canonical BNat extends Pos with zero. CIC terms implement addition,
multiplication and comparison. Concrete Euclidean-division certificates check
both `q*p+r=x` and `r<p`; proposed host division is untrusted. Hemera's
`Reduction.ei` exercises 0, 2^64 and 2^128-1. The checker also certifies two
actual Rust multiplication outputs. These are concrete arithmetic facts,
not universal field refinement and not collision/preimage proofs.

## Universal arithmetic result, with a separate trusted solver

`python3 -B research/reduce128_lia.py` verifies a manually translated integer
model of `field.rs::reduce128` using Z3 4.15.4, QF_LIA. Negating the conjunction
of nine obligations is UNSAT. The checked range is all 128-bit inputs, decomposed
into lo in [0,2^64), hi_lo and hi_hi in [0,2^32). Besides the canonical range,
the obligations include every correction's absence of unintended overflow.

Let b be the initial borrow, c the addition carry, and k the final canonical
subtraction indicator. The proof uses the exact quotient

```
q = hi_lo + hi_hi*(2^32+1) - b + c + k
x = p*q + canonical_result
```

All expressions are linear integer arithmetic; no bitvector division or
random sampling substitutes for this identity. A wrong quotient formula is
refuted with a concrete model. The SMT-LIB obligation and result are saved as
`reduce128-lia.smt2` and `reduce128-results.json`. Z3 and the manual Rust
translation remain trusted; no Z3 proof is imported into Eidos. In particular,
this result does not establish cryptographic security.

Outstanding: universal machine-word/`reduce128` refinement; full-round attack
search and justified bounds in explicitly stated security games. Hash outputs
and production parameters have not been changed.
