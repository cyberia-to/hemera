---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera permutation, Poseidon2 permutation
---

# permutation specification

the Hemera permutation is a Poseidon2 instantiation over the [[Goldilocks field]]. it operates on a state of 16 field elements and applies 24 rounds of nonlinear substitution and linear diffusion.

## structure

```
input state [16 elements]
    │
    ▼
initial linear layer       M_E
    │
    ▼
4 full rounds              add_rc + S-box(all 16, x⁷) + M_E
    │
    ▼
16 partial rounds          add_rc(state[0]) + S-box(state[0], x⁻¹) + M_I
    │
    ▼
4 full rounds              add_rc + S-box(all 16, x⁷) + M_E
    │
    ▼
output state [16 elements]
```

total: 24 rounds (4 + 16 + 4). the initial linear layer is not a round — it is a one-time pre-mixing step applied before the first round.

## S-box

two S-boxes, applied in different round types:

### full-round S-box: x⁷

the power map x → x⁷ applied to all 16 state elements.

d = 7 is the minimum invertible exponent for this field. invertibility requires gcd(d, p−1) = 1. for the Goldilocks prime p−1 = 2³² × (2³² − 1), which has prime factors 2, 3, 5. d=3 fails (gcd=3). d=5 fails (gcd=5). d=7 is the smallest valid choice.

computation: 3-multiplication chain:

```
x² = x · x
x³ = x² · x
x⁴ = x² · x²
x⁷ = x³ · x⁴
```

STARK cost: 4 degree-2 constraints per element (decomposition through intermediates).

### partial-round S-box: x⁻¹

field inversion x → x⁻¹ = x^(p-2) applied to state[0] only, with 0⁻¹ = 0.
This is a bijection of F_p. The degree p-2 representation does not establish
security of the mixed permutation or justify reducing the partial-round count.
See `research/inverse-sbox-assessment.md` for reproducible checks and prior art.

For input x and witness y, require:

```
x × (x × y - 1) = 0
y × (x × y - 1) = 0
```

These are two degree-3 constraints. For x≠0 the first forces xy=1; for x=0
the second forces y=0. A quadratic backend needs an intermediate z and three
constraints: xy=z, x(z-1)=0, y(z-1)=0. The old xy(xy-1)=0 equation was
underconstrained: it admitted y=0 for nonzero x. `trace::inverse_residuals`
implements the corrected local relation; full-round circuit wiring is separate.

### application

in a **full round**, x⁷ is applied to all 16 state elements. in a **partial round**, x⁻¹ is applied only to state[0]. the combination provides: fast native nonlinearity in full rounds (where all elements need it) + cheap verified nonlinearity in partial rounds (where only one element needs it). x⁷ and x⁻¹ are the minimal forward and inverse permutations of the Goldilocks field.

## full round

a full round applies three operations in sequence:

```
for i in 0..16:
    state[i] += RC_FULL[round * 16 + i]     ── add round constant
    state[i]  = state[i]⁷                    ── S-box
M_E(state)                                   ── external linear layer
```

there are 8 full rounds total, split into two groups:
- rounds 0–3: initial full rounds (before partial rounds)
- rounds 4–7: terminal full rounds (after partial rounds)

each full round consumes 16 round constants from `RC_FULL[128]`.

## partial round

a partial round applies three operations:

```
state[0] += RC_PARTIAL[round]               ── add round constant (one element)
state[0]  = state[0]⁻¹                      ── S-box (field inversion, 0⁻¹ = 0)
M_I(state)                                   ── internal linear layer
```

there are 16 partial rounds, indexed 0–15. each consumes one round constant from `RC_PARTIAL[16]`.

## initial linear layer

before the first full round, the external matrix M_E is applied once to the input state:

```
M_E(state)                                   ── pre-mixing
```

this ensures all state elements are mixed before the first S-box application. without it, the first round's S-box would operate on unmixed input, reducing the effective security margin.

## linear layers

two matrices provide diffusion. see [[matrices]] for construction details and concrete values.

### external matrix M_E (full rounds)

a 16×16 circulant of 4×4 MDS sub-blocks:

```
M_E = circ(2·M4, M4, M4, M4)
```

where M4 is the circulant matrix:

```
[ 2 3 1 1 ]
[ 1 2 3 1 ]
[ 1 1 2 3 ]
[ 3 1 1 2 ]
```

algorithm:
1. apply M4 to each 4-element chunk of the state
2. compute column sums: `s[k] = Σ state[j+k]` for `j ∈ {0,4,8,12}`, `k ∈ {0,1,2,3}`
3. add the appropriate column sum to each element: `state[i] += s[i mod 4]`

### internal matrix M_I (partial rounds)

```
M_I = 1 + diag(d₀, d₁, ..., d₁₅)
```

where `1` is the all-ones matrix (every entry = 1). the multiplication is:

```
state'[i] = d[i] · state[i] + Σ state[j]   for j = 0..15
```

M_I is cheaper to compute than M_E (16 multiplications + 1 sum vs full matrix multiply) while still providing full diffusion across all 16 elements.

## round constants

144 Goldilocks field elements, partitioned as:

```
RC_FULL[128]    — 8 full rounds × 16 elements per round
RC_PARTIAL[16]  — 16 partial rounds × 1 element per round
```

indexing:
- full round `r` (0 ≤ r < 4, initial): constants at `RC_FULL[r*16 .. r*16+16]`
- full round `r` (4 ≤ r < 8, terminal): constants at `RC_FULL[r*16 .. r*16+16]`
- partial round `r` (0 ≤ r < 16): constant at `RC_PARTIAL[r]`

constants are generated by the self-bootstrap procedure (see [[bootstrap]]). no external PRNG is used.

## complete algorithm

```
function permute(state: [GoldilocksField; 16]):
    // initial linear layer
    state = M_E(state)

    // 4 initial full rounds
    for r in 0..4:
        for i in 0..16:
            state[i] += RC_FULL[r * 16 + i]
            state[i] = state[i]⁷
        state = M_E(state)

    // 16 partial rounds
    for r in 0..16:
        state[0] += RC_PARTIAL[r]
        state[0] = state[0]⁻¹          // field inversion, 0⁻¹ = 0
        state = M_I(state)

    // 4 terminal full rounds
    for r in 0..4:
        for i in 0..16:
            state[i] += RC_FULL[(r + 4) * 16 + i]
            state[i] = state[i]⁷
        state = M_E(state)

    return state
```

## security properties

The x⁷ S-box is bijective because gcd(7,p−1)=1. Total inversion is bijective,
including zero. For nonzero differences, the x⁷ S-box has differential
uniformity at most 6; total inversion over Goldilocks has uniformity exactly 4.
These are local S-box properties, not full-permutation security bounds.

RF=8/RP=16 is the implemented experimental parameter set. Its security is not
established by the unreduced expression 7⁸×(p−2)¹⁶. Neither 2^918 margin nor
immunity to algebraic attacks is claimed. The matrices do not both have the
all-minors-nonzero property; see matrices.md. Current checks, references and
remaining cryptanalysis are recorded in `research/inverse-sbox-assessment.md`.

## references

- [1] Grassi, Khovratovich, Rechberger, Roy, Schofnegger. "Poseidon2: A Faster Version of the Poseidon Hash Function." IACR ePrint 2023/323.
- [2] Grassi, Khovratovich, Rechberger, Roy, Schofnegger. "Poseidon: A New Hash Function for Zero-Knowledge Proof Systems." USENIX Security 2021.