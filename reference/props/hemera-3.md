---
tags: cyber, cip
crystal-type: process
crystal-domain: cyber
status: draft
date: 2026-03-20
---
# hemera-3 — moonshot optimizations beyond hemera-2

hemera-2 achieved the x⁻¹ S-box breakthrough: same native speed, 3× fewer rounds, 5.4× shallower MPC depth, 17× more algebraic security. these proposals push further — optimizing hemera for the full stack (nox, zheng-2, bbg) and for hybrid field/bitwise proving.

## where hemera-2 stands

```
hemera-2 (current target):
  rounds:         24 (8 full + 16 partial)
  S-box:          x⁷ (full) + x⁻¹ (partial)
  output:         32 bytes (4 elements)
  native speed:   ~53 MB/s
  constraints:    ~736 per permutation
  MPC depth:      40
  tree node:      1 permutation (64 bytes = 1 rate block)
```

what's left to optimize? three cost centers:

```
hemera-2 constraint breakdown:
  full-round S-boxes:    8 × 16 × 4 = 512 constraints    (69%)
  partial-round S-boxes: 16 × 2 = 32 constraints           (4%)
  linear layers (MDS):   ~192 constraints                  (26%)
  total:                 ~736
```

the full rounds dominate. the partial rounds are already cheap (x⁻¹ at 2 constraints each). the MDS layers are linear.

---

## proposal 1: batched hemera proving

when a block contains N hemera calls (Merkle paths, Fiat-Shamir, content addressing, cyberlink hashing), prove them all with one batched argument instead of N independent constraint sets.

### the observation

N independent hemera calls produce N × 24-round constraint sets. each set is structurally identical — same round constants, same MDS matrices, same S-box. only the input differs.

### the construction

express all N hemera calls as evaluations of a single "multi-instance permutation polynomial":

```
P(x, instance) where:
  x encodes the round index (0..23)
  instance encodes which of the N calls (0..N-1)

constraint: P satisfies the hemera round relation at all (x, instance) pairs
proof: one sumcheck over the product domain {0..23} × {0..N-1}
```

```
current:    N × 736 = 736N constraints (independent)
batched:    736 + O(N) field ops (shared structure, per-instance input binding)

for N = 1000 (typical block):
  current: 736,000 constraints
  batched: ~1,736 constraints + sumcheck over 24,000 points
  savings: ~400×
```

### why it works

the round constants and MDS matrices are the same across all N calls. the sumcheck amortizes the structural verification. only the input/output binding is per-instance (one constraint per call to bind the input).

### interaction with proof-carrying

with proof-carrying computation ([[proof-carrying]]), hemera calls during nox execution fold individually into the accumulator. batched proving applies to BLOCK-LEVEL aggregation: after folding, the decider verifies all hemera calls in the accumulated proof with one batch argument.

---

## proposal 2: partial-round collapse

in hemera-2, states [1..15] evolve LINEARLY during all 16 partial rounds. only state[0] gets the nonlinear x⁻¹ S-box. the linear evolution of states [1..15] across all 16 partial rounds can be precomputed.

### the construction

define the composed linear map across all 16 partial rounds:

```
partial round i:
  state[0] = (state[0] + RC[i])⁻¹
  state = M_I × state

for states [1..15], this is:
  state'[j] = d[j] × state[j] + Σ state[k]   (M_I application)
  = linear function of all state elements

across 16 rounds, the composition for [1..15] is:
  state_final[1..15] = L₁₆ × state_initial + f(state[0] values at each round)

where L₁₆ = M_I^16 restricted to indices [1..15]
and f encodes the contribution of state[0] at each round
```

### constraint savings

```
current partial rounds: 16 rounds × (2 S-box + ~12 MDS) = ~224 constraints

with collapse:
  16 × x⁻¹ on state[0]:     16 × 2 = 32 constraints (sequential, unchanged)
  L₁₆ on states [1..15]:     15 × 16 = 240 constraints (one matrix multiply)
  f contribution:             16 × 15 = 240 constraints (state[0] injection per round)
  total:                      ~512 constraints
```

no improvement in constraint count (actually slightly worse). BUT the prover advantage is significant:

```
prover work:
  current: 16 sequential rounds, each touching all 16 elements
  collapsed: precompute L₁₆ (constant matrix), apply once

  L₁₆ is a 15×15 matrix over Goldilocks — precomputed at compile time
  prover evaluates ONE matrix-vector multiply instead of 16 sequential rounds
  prover speedup: ~4× for partial round evaluation
```

the constraint count doesn't improve, but the PROVER WALL-CLOCK drops because the sequential dependency is broken for 15 of 16 state elements.

---

## proposal 3: full-round reduction (R_F = 6)

hemera-2 keeps 8 full rounds (4+4) unchanged from hemera-1. the x⁻¹ partial rounds provide 2^1024 algebraic degree — so much margin that the full rounds could potentially be reduced.

### security analysis

full rounds defend against differential and linear cryptanalysis. the Poseidon2 security analysis requires:

```
R_F ≥ 2 × ceil(log_d(min(2^M, p))) / (t-1) + 2

for hemera (t=16, d=7, M=128, p=2^64):
  ceil(log_7(2^64)) / 15 + 2 ≈ ceil(22.75) / 15 + 2 ≈ 3.5
  minimum R_F = 6 (with safety margin)
```

hemera-1 and hemera-2 use R_F = 8 (1.33× margin over R_F = 6).

### what R_F = 6 saves

```
                    R_F = 8 (hemera-2)       R_F = 6 (proposed)
total rounds:       24                        22
full-round S-boxes: 8 × 16 × 4 = 512        6 × 16 × 4 = 384
partial S-boxes:    16 × 2 = 32              16 × 2 = 32
linear layers:      ~192                     ~176
total constraints:  ~736                     ~592
improvement:        —                        20% fewer constraints
```

### risk assessment

reducing R_F from 8 to 6 is within the proven security bound but removes the safety margin. hemera is permanence-grade — the hash must survive for the lifetime of the cybergraph. reducing the safety margin is a tradeoff between efficiency and permanence.

**recommendation**: keep R_F = 8 for the main hash. offer R_F = 6 as an optional "fast mode" for internal operations (Fiat-Shamir, intermediate Merkle nodes) where the permanence requirement is weaker. Fiat-Shamir hashes are ephemeral — they don't need to survive 100 years.

```
hemera-2 (permanence):  R_F = 8, 24 rounds, ~736 constraints
hemera-2-fast (ephemeral): R_F = 6, 22 rounds, ~592 constraints
```

domain separation: capacity[11] distinguishes permanence mode from fast mode. the two modes produce different hashes for the same input — cross-mode collisions are impossible.

---

## proposal 4: constraint-free linear layers via wiring

MDS matrices (M_E, M_I) are PUBLIC constants. in the STARK constraint system, linear operations can be encoded as WIRING rather than explicit constraints. wiring is free — it's the prover's bookkeeping, not a constraint the verifier checks.

### the insight

a constraint `a = 2b + 3c` is degree 1 — it's a linear relation. in CCS, linear constraints can be absorbed into the wiring matrix rather than the constraint polynomial. the verifier doesn't check them directly — the sumcheck implicitly verifies them through the committed trace polynomial.

### savings

```
hemera-2 constraint breakdown:
  S-box constraints:     544 (degree 2, cannot be wired)
  MDS constraints:       ~192 (degree 1, CAN be wired)
  total:                 ~736

with wired MDS:
  S-box constraints:     544
  MDS constraints:       0 (absorbed into CCS wiring)
  total:                 ~544
  improvement:           26% fewer explicit constraints
```

### caveats

wiring absorption depends on the CCS encoding strategy in zheng. standard CCS encodes linear constraints as matrix rows. absorbing them into wiring requires a modified CCS layout. this is a zheng optimization, not a hemera change — but it's hemera-specific because hemera's linear layers are the largest linear constraint block in the system.

---

## proposal 5: hemera-in-hemera (recursive sponge)

hemera's sponge absorbs data block by block. for long inputs (e.g., large particles, batch cyberlink encoding), multiple permutation calls are needed. currently each call is independent.

### the observation

consecutive sponge absorption calls share state. the output of permutation N is the input of permutation N+1 (after absorbing the next block). this sequential dependency means the prover must evaluate the permutations in order.

### the construction

express the entire multi-block sponge as ONE extended permutation:

```
single-block:  input → 24 rounds → output  (1 permutation, ~736 constraints)

K-block sponge: input₁ → 24 rounds → absorb input₂ → 24 rounds → ... → output
               = 24K rounds total
               = K permutations sequentially

with folding:
  fold each permutation into accumulator: K × 30 field ops
  one decider at end: ~736 constraints
  total: ~736 + 30K field ops (vs K × 736 constraints)
```

for K=10 (10-block sponge, ~560 bytes of input):

```
current:    10 × 736 = 7,360 constraints
folded:     736 + 300 = 1,036 constraints equivalent
savings:    7.1×
```

this composes with proof-carrying: the nox VM folds each hemera permutation call into the running accumulator as it executes. a 10-block hash is 10 fold steps during execution + 1 decider in the proof.

---

## proposal 6: algebraic Fiat-Shamir

hemera's Fiat-Shamir role: absorb proof transcript → squeeze challenge. the challenge must be unpredictable from the transcript. currently this requires a full hemera permutation per challenge.

### the observation

the zheng-2 IOP (SuperSpartan + sumcheck) already has algebraic binding. the sumcheck transcript is committed polynomially. an algebraic challenge can be derived from the polynomial commitment itself — no separate hash needed.

### the construction

```
current Fiat-Shamir:
  commit round polynomial → hemera(commitment) → squeeze challenge
  cost: 1 hemera permutation per round = ~20 hemera calls for a typical proof
  total: 20 × 736 = ~14,720 constraints

algebraic Fiat-Shamir:
  commit round polynomial → derive challenge from commitment algebraically
  challenge = polynomial evaluation at a secret point (derived from initial seed)
  cost: 1 polynomial evaluation per round = ~20 poly_eval calls
  total: 20 × ~50 = ~1,000 constraints
```

### security

algebraic Fiat-Shamir requires the commitment scheme to be binding — which it is (WHIR/Brakedown are binding PCS). the challenge derivation must be unforgeable — which it is if the evaluation point is unpredictable (derived from the initial hemera-seeded random oracle).

the first challenge still uses hemera (bootstrapping the random oracle from the instance). subsequent challenges derive algebraically from the committed transcript.

```
hybrid approach:
  initial seed: hemera(instance)                   ~736 constraints (one-time)
  subsequent challenges: poly_eval(commitment, seed_i)  ~50 constraints each
  total for 20 rounds: 736 + 19 × 50 = ~1,686 constraints
  vs current: 20 × 736 = 14,720 constraints
  savings: ~8.7×
```

### interaction with hemera role

hemera remains the trust anchor (initial seed). algebraic derivation handles the bulk of challenge generation. this REDUCES hemera's constraint footprint in proofs by ~8× while keeping hemera as the cryptographic foundation.

---

## how zheng-2 hybrid proving improves the trinity

the trinity: nebu (field) + hemera (hash) + zheng (proof). zheng-2 introduces dual-algebra proving (Goldilocks + F₂). how does this affect hemera?

### hemera is pure Goldilocks

hemera operates entirely in F_p. no bitwise operations. the permutation is: field additions (round constants), field multiplications (S-box), field multiply-accumulate (MDS). all native Goldilocks.

this means hemera NEVER runs in Binius. hemera constraints are always WHIR/Brakedown constraints. the binary prover is irrelevant for hemera itself.

### the context around hemera benefits from F₂

hemera is called from contexts that DO involve bitwise operations:

```
1. Merkle path index tracking
   current: bit decomposition of path index costs ~32 F_p constraints
   with Binius: index bit extraction is 1 F₂ constraint
   savings per Merkle level: ~31 constraints

2. domain separation flags
   capacity flags (root/parent/chunk) are bit fields
   setting/checking flags: ~5 F_p constraints → 5 F₂ constraints
   savings per hash call: ~25 constraints

3. byte encoding/decoding
   hemera absorbs bytes → field elements (7-byte packing)
   packing involves bit manipulation: ~20 F_p constraints per block
   with Binius: ~20 F₂ constraints
   savings per absorption: ~600 constraints for a large input

4. Fiat-Shamir transcript management
   byte-level manipulation of proof elements: bitwise
   with Binius: native
```

### quantitative impact on Merkle verification

```
merkle_verify (32 levels):
  hemera constraint per level:    ~736 (WHIR, unchanged)
  index bit extraction per level: ~32 → 1 (Binius)
  left/right selection per level: ~5 → 1 (Binius)

  current:   32 × (736 + 37) = ~24,736 constraints (all F_p)
  hybrid:    32 × 736 (WHIR) + 32 × 2 (Binius) = 23,616 + 64
  savings:   ~5% (the surrounding ops are small relative to hemera itself)
```

the savings from hybrid proving around hemera are modest (~5%). hemera dominates its own cost. the bitwise context is a small fraction.

### where hybrid proving REALLY helps hemera

the big win is not making hemera cheaper — it's making hemera LESS NEEDED:

```
1. algebraic extraction (zheng-2)
   replaces Merkle auth paths with batch algebraic openings
   Merkle paths are 77% of proof size, 71% of recursive verification
   hemera tree hashing: ELIMINATED for proof verification
   remaining hemera role: Fiat-Shamir + content addressing

2. algebraic NMT (bbg-2)
   replaces NMT trees with polynomial commitments
   9 independent NMT trees with hemera paths → 1 polynomial
   hemera NMT hashing: ELIMINATED for public indexes
   remaining hemera role: private state + signals + content addressing

3. polynomial state (bbg-2)
   replaces ALL 13 BBG sub-roots with polynomial commitments
   hemera Merkle trees: ELIMINATED for state commitment
   remaining hemera role: Fiat-Shamir + content addressing + private records

4. algebraic Fiat-Shamir (proposal 6 above)
   replaces most Fiat-Shamir hemera calls with algebraic derivation
   remaining hemera role: initial seed + content addressing + private records
```

the trajectory: hemera evolves from "hash for everything" to "hash for trust anchoring." algebraic commitments handle the bulk of binding and verification. hemera provides the cryptographic foundation that bootstraps the algebraic system.

### tri-kernel improvement

the three kernels (diffusion, springs, heat) for π computation:

```
diffusion:  SpMV (matrix-vector multiply)
  field version: pure Goldilocks arithmetic → WHIR
  quantized version: binary matmul → Binius (1,400× cheaper)
  hemera role: commit intermediate state per iteration

springs:    Newton iterations
  pure Goldilocks arithmetic → WHIR
  hemera role: commit iteration state

heat:       exponential diffusion via NTT
  NTT over Goldilocks → WHIR (Wav language)
  hemera role: commit spectral coefficients
```

with proof-carrying computation, each iteration carries its proof. hemera provides the commitment (32 bytes) that links iterations. the cost:

```
per tri-kernel iteration:
  kernel computation:     millions of constraints (dominates)
  hemera commitment:      ~736 constraints (negligible)
  fold into accumulator:  ~30 field ops (trivial)

total overhead from hemera: < 0.1% of kernel cost
```

hemera's overhead in the tri-kernel is already negligible. the improvement from hemera-2 (736 vs 1,152 constraints) doesn't materially change tri-kernel performance.

the REAL improvement for tri-kernel comes from zheng-2:
- quantized SpMV in Binius: 1,400× cheaper constraints
- folding-first composition: 1,000× cheaper iteration composition
- proof-carrying: zero proving latency

hemera enables these improvements (it's the commitment primitive) but doesn't limit them.

## summary table

| proposal | target | improvement | risk |
|----------|--------|-------------|------|
| batched proving | N hemera calls per block | ~400× for N=1000 | low (sumcheck is standard) |
| partial-round collapse | prover wall-clock | ~4× prover speedup | none (mathematical identity) |
| R_F = 6 (fast mode) | ephemeral hashes | 20% fewer constraints | medium (reduced safety margin) |
| constraint-free MDS | explicit constraints | 26% fewer constraints | low (CCS encoding change) |
| hemera-in-hemera (folded) | multi-block sponge | 7× for 10-block inputs | none (folding is standard) |
| algebraic Fiat-Shamir | challenge derivation | 8.7× fewer hemera calls | medium (needs security proof) |

## the trajectory

```
hemera-1: hash for everything          ~1,152 constraints, 72 rounds
hemera-2: faster hash for everything   ~736 constraints, 24 rounds
hemera-3: hash for trust anchoring     ~544 constraints (wired MDS)
          + algebraic for everything else

hemera's role evolves:
  binding:         hemera → polynomial commitments (algebraic NMT, polynomial state)
  Merkle proofs:   hemera → algebraic extraction (batch openings)
  Fiat-Shamir:     hemera → algebraic derivation (polynomial challenges)
  content identity: hemera (permanent — the anchor)
  private records:  hemera (permanent — privacy requires hashing)

the endgame: hemera is the root of trust. algebraic machinery handles the scale.
```

see [[hemera-2]] for the current upgrade, [[algebraic-extraction]] for Merkle elimination, [[algebraic-nmt]] for NMT polynomial replacement, [[zheng-2]] for dual-algebra architecture
