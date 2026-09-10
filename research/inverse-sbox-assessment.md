# Inverse S-box assessment — 2026-09-10

The candidate remains x^7 full rounds and total inverse (0 -> 0) partial
rounds, RF=8, RP=16, Goldilocks, width 16. No hash parameters were changed.
The hypothesis is worth studying for arithmetization cost. The claim that it
strongly increases security and justifies 64 -> 16 rounds is not established.

## Reproduction

`python3 research/inverse_sbox.py` (Python 3, SymPy 1.14.0) produces
`inverse-sbox-results.json`. It reads the actual Rust addition chain and
internal matrix constants, pins the source fingerprint in the report, and
fails on unrecognized chain syntax. This is executable algebraic analysis,
not an Eidos proof or universal verification of Rust machine arithmetic.

`cargo test -p cyber-hemera --test inverse_constraints` checks the actual
field implementation against the corrected local witness relation, including
noncanonical representatives. `proofs/Inverse.ei` proves conditional lemmas
for the zero and cancellable-input cases in Eidos.

## 1. A real constraint bug, with a repair

The old spec used `xy(xy-1)=0` and `(1-xy)y=0`. Every `(x,0)` satisfies
these equations, including x != 0. Their degrees are 4 and 3, not 2.
An implementation of that circuit would permit a false inverse witness.
The Rust inverse computation itself is not broken by this specification bug.

Correct total inverse constraints over a field are:

```
x(xy-1) = 0
y(xy-1) = 0
```

If x != 0, cancellation gives xy=1. If x=0, the second equation gives y=0.
Conversely the actual inverse satisfies both equations. This is two **cubic**
constraints. For a quadratic/R1CS backend, introduce z and use three:
`xy=z`, `x(z-1)=0`, `y(z-1)=0`. No extra boolean constraint is needed.
The script exhaustively checks the relation on five small prime fields.

This establishes a useful local cost advantage over the four multiplication
constraints of x^7 in a quadratic backend, conditional on surrounding wiring.
It does not establish the old total of 736 constraints or a backend benchmark.

## 2. Inversion is a permutation with a good local differential bound

For nonzero input difference a, away from x=0,-a, the derivative equation
`inv(x+a)-inv(x)=b` becomes `b*x*(x+a)=-a` (b cannot be zero).
There are at most two such roots. The two exceptional inputs can add two more.
Thus differential uniformity is at most 4 for any odd prime field.

For Goldilocks it is exactly 4: for a=b=1 the four solutions are
0, 4294967295, 18446744065119617025, 18446744069414584320.
The script checks all four using modular exponentiation. The x^7 derivative
has polynomial degree 6, yielding a local upper bound of 6 solutions.
These local bounds do not compare the security of the complete permutations;
active S-box counts, trails, matrices and attack models matter.

## 3. The enormous-degree argument is not a security argument

The expression `7^8*(p-2)^16` is an unreduced composition-degree upper bound,
not a lower bound on attack complexity. As a function on F_p, a univariate
polynomial reduces modulo X^p-X to degree at most p-1. A function on F_p^16
has a representative of degree at most p-1 in each variable, at most
16*(p-1) total. The univariate bound must not be misreported as a multivariate
total-degree bound.

There is also a much smaller rational representation on the branch where all
inverse inputs are nonzero. Write the state as `[N0/D,...,N15/D]`, with
homogeneous numerator/denominator degree at most d. A partial inversion maps
this to `[D^2, N1*N0, ..., N15*N0] / (D*N0)`, so the degree at most doubles.
Constants and linear layers do not increase this bound. A full x^7 layer
multiplies it by at most 7. Consequently the nonzero branch has projective
degree at most `7^8*2^16 = 377801998336`, about 2^38.46.

This is not a 2^38 attack estimate: solving a multivariate rational system is
not proportional to its degree, and zero branches require separate handling.
It shows why the p-2 representation cannot justify the claimed 2^918 margin.

The original [Poseidon paper, §1.4](https://www.usenix.org/system/files/sec21-grassi.pdf)
explicitly discusses abandoning inverse S-boxes in early design exploration
because their degree growth was insufficiently fast. Its §6.2.1 counts three
quadratic constraints for a total inverse. This is directly relevant prior art,
not an analysis of Hemera's mixed S-box configuration. The
[Poseidon2 paper](https://eprint.iacr.org/2023/323.pdf) must likewise not be cited
as proving security for this modified round function.

## 4. Actual linear-layer checks

For M_I read from field.rs, the script finds rank 16 and one irreducible
degree-16 characteristic-polynomial factor over Goldilocks. The ranks of the
first k rows of `[e0; e0*M_I; ...; e0*M_I^15]` are k for k=1..16.
Thus no nonzero difference can stay inactive at coordinate 0 through all
16 steps of this linear evolution. It also leaves a dimension-1 family for
15 constraints; it does not establish a full-round differential bound.

These are preliminary checks. They do not test all powers of M_I, all
active subspace trails, or interaction with full-round layers and constants.
See the linear-layer criteria in Poseidon2 §5.3 for the distinction.

## 5. Native cost, proof cost and digest size are distinct

The actual Rust addition chain has exponent p-2, 75 multiplications including
squares and multiplication depth 71. A proof circuit verifying a supplied
inverse may be cheap while computing the inverse under MPC/FHE remains costly.
The old one-multiplication inversion depth and 5.4x MPC/FHE claim do not follow
without a separately specified protocol.

A 32-byte digest has a generic classical collision ceiling around 128 bits,
regardless of S-box choice. Four canonical field elements have p^4 possible
outputs, slightly fewer than 2^256. Capacity protects the sponge interior;
it does not remove the output birthday bound. See the
[Keccak team's sponge security explanation](https://keccak.team/sponge_duplex.html).
Hemera already outputs 32 bytes. This size does not require an inverse S-box.

## Remaining security work

The next cryptanalysis must model the exact matrices/constants and compare
matched cost/security configurations: x^7 partial rounds, inverse partial
rounds with varying RP, and the current hybrid. Build quadratic systems with
zero branches, test reduced-round CICO/preimage instances, study solver scaling
and subspace/differential attacks, then commission independent review. Small
fields, avalanche tests and failure to find an attack cannot certify RF=8/RP=16.

Conclusion: confirmed compact constraints and a favorable local differential
bound; refuted witness equations and degree-based security rationale; no
full-round break found or full-round security claim established by this work.
