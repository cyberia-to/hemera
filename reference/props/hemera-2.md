---
title: "hemera-2: hybrid S-box Poseidon2, inversion partial rounds, 32-byte output"
status: draft
date: 2026-03-17
---

# hemera-2

a pre-genesis upgrade to the Hemera permutation. three changes: (1) replace the x^7 S-box in partial rounds with field inversion x^{-1}, (2) reduce partial rounds from 64 to 16, (3) reduce hash output from 64 bytes to 32 bytes. same native speed, 3× fewer rounds, 5× shallower MPC depth, 4× higher algebraic degree security margin, half the storage per hash.

## the insight

field inversion x^{-1} = x^{p-2} over the Goldilocks field is a permutation (gcd(p-2, p-1) = 1). it has algebraic degree p-2 ≈ 2^64 — versus degree 7 for x^7. each application of x^{-1} contributes 2^64 to the algebraic degree, while x^7 contributes only 2.8 (log₂ 7). this means x^{-1} achieves in one partial round what x^7 needs ~23 partial rounds to match.

the tradeoff: x^{-1} costs ~64 field multiplications to compute (via Fermat's little theorem: square-and-multiply chain for x^{p-2}), versus 3 multiplications for x^7. but partial rounds apply only ONE S-box (to state[0] only). the per-round cost difference is 64 vs 3 multiplications on a single element — 61 extra multiplications per round. with 4× fewer rounds, the total multiplication count is nearly identical.

the STARK constraint cost flips the other way: proving x × x^{-1} = 1 requires 1 degree-2 constraint (the prover provides x^{-1}, the verifier checks the product). proving x^7 requires 4 degree-2 constraints (decomposition through x², x⁴). inversion is 4× cheaper to prove.

## what changes

```
                    hemera-1                    hemera-2
S-box (full):       x^7                         x^7 (unchanged)
S-box (partial):    x^7                         x^{-1}
R_F:                8 (4+4)                     8 (4+4) (unchanged)
R_P:                64                          16
total rounds:       72                          24
state width:        16                          16 (unchanged)
capacity:           8                           8 (unchanged)
rate:               8                           8 (unchanged)
output:             8 elements (64 bytes)       4 elements (32 bytes)
```

three things change: the partial-round S-box, the partial-round count, and the output size. the Poseidon2 structure, MDS matrices M_E and M_I, sponge absorption, tree hashing, capacity layout, encoding, and API shape are identical. the sponge squeeze extracts 4 elements from the rate portion instead of 8.

## parameters

### S-box: full rounds

unchanged. x^7 applied to all 16 state elements.

```
full round:
  for i in 0..16:
    state[i] += RC_FULL[r * 16 + i]
    state[i] = state[i]^7                    ← unchanged
  state = M_E(state)
```

computation: 3 multiplications per element (x² → x⁴ → x⁷).
STARK cost: 4 degree-2 constraints per element, 64 per full round.

### S-box: partial rounds

changed. x^{-1} applied to state[0] only.

```
partial round:
  state[0] += RC_PARTIAL[r]
  state[0] = state[0]^{-1}                   ← changed from state[0]^7
  state = M_I(state)
```

computation: ~64 multiplications (square-and-multiply for x^{p-2}).
STARK cost: 1 degree-2 constraint (x × x^{-1} = 1).

### inversion S-box verification

the prover computes y = x^{-1} and provides y as a witness. the verifier checks a single constraint:

```
x × y - 1 = 0                                (degree 2)
```

this is the simplest possible nonlinear constraint. compare to x^7 verification:

```
a = x × x        (degree 2)     ← intermediate x²
b = a × a        (degree 2)     ← intermediate x⁴
c = a × x        (degree 2)     ← intermediate x³
y = b × c        (degree 2)     ← final x⁷
```

four constraints versus one. the inversion S-box is 4× cheaper to prove.

### zero handling

x^{-1} is undefined at x = 0. the Hemera permutation must handle this. two options:

**option A: define 0^{-1} = 0** (standard convention in algebraic hash literature). this makes the S-box a permutation over F_p (0 maps to 0, every non-zero element maps to its inverse). the constraint becomes:

```
x × y = x^2 × (something) ... no, simpler:
if x ≠ 0: x × y = 1
if x = 0: y = 0
```

combined as: `x × y × (x × y - 1) = 0 AND (1 - x × y) × y = 0`. this enforces y = x^{-1} when x ≠ 0 and y = 0 when x = 0. total: 2 constraints (still cheaper than 4 for x^7).

**option B: ensure state[0] is never zero after round constant addition.** the round constant RC_PARTIAL[r] is added before the S-box. if all round constants are non-zero and the state enters the partial-round phase from the full rounds (which mix all elements), the probability of state[0] + RC = 0 is 1/p ≈ 2^{-64} — negligible.

for permanence-grade safety: use option A. define 0^{-1} = 0. 2 constraints per partial round instead of 1, still 2× cheaper than x^7.

### partial round count: R_P = 16

the minimum partial round count for Hemera-1 (x^7) is R_P = 22 (from Poseidon2 security analysis for Goldilocks at 128-bit). Hemera-1 uses R_P = 64 (2.91× margin).

for x^{-1}, the algebraic degree after R_P rounds is (p-2)^{R_P} ≈ 2^{64 × R_P}.

```
R_P = 12: degree ≈ 2^768    (vs 2^180 for 64 rounds of x^7)
R_P = 16: degree ≈ 2^1024   (vs 2^180)
R_P = 22: degree ≈ 2^1408   (for comparison with Hemera-1 minimum)
```

for interpolation attacks: an attacker needs ~d evaluations to recover the permutation polynomial. with d = 2^1024, this is beyond any computation.

for Groebner basis attacks: the system of equations has 16 state variables × 24 rounds ≈ 384 variables with degree (p-2) per partial round relation. the Groebner basis complexity exceeds 2^{1000}.

security margin comparison:

```
                        algebraic degree    margin over 2^128
hemera-1 (R_P=64):     2^180               2^52 (52-bit margin)
hemera-2 (R_P=16):     2^1024              2^896 (896-bit margin)
```

hemera-2 with R_P=16 has 17× MORE margin in log-space than hemera-1 with R_P=64. the inversion S-box is so powerful that even a conservative R_P=16 provides vastly more algebraic security than the current design.

### why not even fewer partial rounds?

R_P = 8 would give degree 2^512. still far above 2^128. but:

1. differential/linear cryptanalysis has different bounds than interpolation. the full rounds (R_F=8) provide the main defense against these attacks. the partial rounds must provide enough algebraic complexity to prevent shortcut attacks that bypass the full-round defense.

2. future attacks may improve. the Graeffe transform (2025) gave 2^13× improvement for interpolation on Poseidon2. extrapolating 30 years at ~2× per year: 2^30 improvement factor. with R_P=16 and degree 2^1024: post-improvement degree = 2^994. still astronomical.

3. R_P = 16 = 2^4. a power of 2. consistent with Hemera's parameter aesthetic.

R_P = 16 is the sweet spot: massive security margin, power-of-2, 4× fewer rounds than Hemera-1.

## cost comparison

### native execution

```
                        hemera-1 (72 rounds)         hemera-2 (24 rounds)
full rounds:            8 × (16×3 + ~64) = 896      8 × (16×3 + ~64) = 896      (same)
partial S-box:          64 × 3 = 192                 16 × 64 = 1,024
partial MDS (M_I):      64 × 17 = 1,088             16 × 17 = 272
initial linear:         ~32                          ~32
total muls:             ~2,208                       ~2,224
ratio:                  1.00×                        0.99× (same speed)
```

the expensive x^{-1} (64 muls per application) is applied once per partial round. with 4× fewer partial rounds, the S-box budget increases from 192 to 1,024 muls — but the MDS budget drops from 1,088 to 272. net: same total.

native hash rate: ~53 MB/s (unchanged from Hemera-1).

### STARK constraints

```
                        hemera-1                     hemera-2
full rounds:            8 × (16×4 + 16) = 640       8 × (16×4 + 16) = 640       (same)
partial S-box:          64 × 4 = 256                 16 × 2 = 32                  (8×)
partial MDS:            64 × 4 = 256                 16 × 4 = 64                  (4×)
total:                  ~1,152                       ~736
ratio:                  1.00×                        1.56× fewer constraints
```

with the 0^{-1} = 0 handling (2 constraints per partial S-box instead of 4): 736 constraints per permutation. 36% fewer than Hemera-1.

### fold steps (zheng-2 proof-carrying)

```
                        hemera-1        hemera-2        improvement
rounds per hash:        72              24              3×
fold steps per hash:    72              24              3×
fold ops per hash:      2,160           720             3×
verifier hash calls:    ~400            ~400            (same count)
verifier fold total:    28,800          9,600           3×
```

### MPC multiplicative depth

```
                        hemera-1                     hemera-2
full-round depth:       8 × 3 = 24                  8 × 3 = 24                  (same)
partial-round depth:    64 × 3 = 192                16 × 1 = 16
total depth:            216                          40
improvement:            —                            5.4×
```

at 10 ms network latency per MPC round: hemera-1 takes 2.16 seconds per hash. hemera-2 takes 0.40 seconds. viable for threshold key management, multi-party signing, and distributed hash verification.

### FHE noise growth

FHE noise grows with multiplicative depth. noise budget per hash:

```
hemera-1: 216 sequential multiplications → noise ∝ 2^216
hemera-2: 40 sequential multiplications  → noise ∝ 2^40
```

5.4× reduction in depth = 5.4× reduction in noise exponent. this means:
- fewer bootstrapping operations needed per hash
- larger plaintext space per hash invocation
- hash-then-encrypt patterns become practical in TFHE over Goldilocks

## algebraic security analysis

### interpolation attacks

```
hemera-1: degree ≈ 7^{R_F + R_P} = 7^72 ≈ 2^202
hemera-2: degree ≈ 7^8 × (p-2)^16 ≈ 2^22 × 2^1024 = 2^1046
```

improvement: 5× more bits of algebraic security. interpolation requires ~2^1046 evaluations.

### Groebner basis attacks

hemera-2: 384 variables (16 × 24 rounds) + 16 auxiliary inversion variables. each inversion constraint (x × y = 1) has degree 2 but implicit degree p-2 per step. Groebner basis complexity exceeds 2^1000. at least as hard as hemera-1 with fewer variables.

### differential and linear cryptanalysis

full rounds unchanged (same x^7, same M_E, R_F=8). wide trail: 8 active S-boxes, differential probability (6/p)^8 ≈ 2^{-480}. exceeds 128-bit security by 352 bits. x^{-1} has differential uniformity 2 over F_p (optimal, vs 6 for x^7).

### recent algebraic attacks

the Graeffe transform (2025) improved interpolation by 2^13× for x^7 — exploits small exponents. x^{-1} = x^{p-2} has the largest possible exponent; Graeffe does not help. the resultant-based attack (ePrint 2026/150) broke reduced-round x^7 instances. for x^{-1}, resultant computation requires degree proportional to (p-2)^{rounds} — infeasible.

## design rationale

Poseidon2 chose x^7 to minimize native field multiplications (3 muls vs ~64 for x^{-1}). this optimized for hash throughput in zkVM systems. in cyber, the hash operates at three cost levels:

| level | operation | cost metric |
|---|---|---|
| identity | particle addressing, content hashing | native speed (muls/sec) |
| proving | Fiat-Shamir, WHIR, recursive verification | constraint count / fold steps |
| privacy | MPC threshold signing, FHE encrypted computation | multiplicative depth |

hemera-1 optimizes for level 1. hemera-2 maintains level 1 performance (same total muls) while dramatically improving levels 2 and 3. the key: in partial rounds (S-box applied to ONE element), per-S-box cost matters less than total round count. 21× more expensive S-box × 4× fewer rounds = same total cost, massively better algebraic properties.

### preservation of hemera properties

properties comparison:

| property | hemera-1 | hemera-2 | status |
|---|---|---|---|
| state/capacity/rate | 16/8/8 | 16/8/8 | unchanged |
| output | 64 bytes (8 elements) | 32 bytes (4 elements) | **changed** |
| endofunction | yes (64→64 bytes) | yes (32→32 bytes) | preserved |
| tree hashing | 2 perm calls/node | 1 perm call/node | **improved** |
| permanence-grade | R_P margin: 2.91× | algebraic margin: 2^896 | stronger |
| collision resistance | 256-bit classical | 128-bit classical | sufficient |
| quantum collision (BHT) | 2^170 | 2^85 | sufficient |

three changes: partial-round S-box (`state[0]^7` → `state[0]^{-1}`), R_P (64 → 16), output (64 → 32 bytes).

### the double seven becomes the seven-and-inverse

hemera-1's beauty: the number 7 appears twice (S-box exponent and input byte encoding), both forced by the Goldilocks prime.

hemera-2 preserves this and adds a new duality: x^7 (the minimal FORWARD power permutation) and x^{-1} (the INVERSE permutation) are algebraic complements. x^7 provides fast native nonlinearity in full rounds (where all 16 elements need it). x^{-1} provides cheap verified nonlinearity in partial rounds (where only one element needs it).

the two S-boxes are not arbitrary choices — they are the minimal forward and inverse permutations of the Goldilocks field.

## 32-byte output

hemera-2 squeezes 4 Goldilocks elements (32 bytes) instead of 8 (64 bytes). the sponge state is unchanged (16 elements, 512 bits). absorption rate is unchanged (8 elements per permutation call). only the output extraction changes.

### squeeze procedure

```
hemera-1 squeeze: output = state[0..8]     → 8 elements, 64 bytes
hemera-2 squeeze: output = state[0..4]     → 4 elements, 32 bytes
```

for multi-block output (when more than 32 bytes are needed): squeeze 4 elements, apply permutation, squeeze 4 more. same pattern as hemera-1 but with a smaller block.

### security

| property | hemera-1 (64 bytes) | hemera-2 (32 bytes) |
|---|---|---|
| classical collision resistance | 2^256 | 2^128 |
| classical preimage resistance | 2^256 | 2^256 (capacity is still 256 bits) |
| quantum collision (BHT) | 2^170 | 2^85 |
| quantum preimage (Grover) | 2^128 | 2^128 (capacity-limited) |
| quantum multi-target preimage (2^80 targets) | 2^88 | 2^88 (capacity-limited) |

128-bit classical collision resistance is the standard security level. every major hash function deployed today (SHA-256, BLAKE3, Keccak-256) operates at this level. 2^85 quantum collision resistance via the BHT algorithm exceeds all projected quantum computing capabilities.

birthday probability among 2^80 particles (planetary scale): 2^{80} × (2^{80} - 1) / (2 × 2^{256}) ≈ 2^{-98}. negligible.

preimage resistance is governed by the capacity (256 bits = 8 elements), not the output. an attacker cannot invert the sponge without finding a state that matches all 8 capacity elements. the 32-byte output does not weaken preimage security.

### storage savings

every hash in the system shrinks from 64 to 32 bytes:

| structure | hemera-1 | hemera-2 | savings |
|---|---|---|---|
| particle address | 64 bytes | 32 bytes | 2× |
| neuron identity | 64 bytes | 32 bytes | 2× |
| Merkle node | 128 bytes (2 children) | 64 bytes (2 children) | 2× |
| NMT node | 192 bytes (2 children + 2 namespaces) | 128 bytes | 1.5× |
| WHIR commitment | 64 bytes | 32 bytes | 2× |
| Fiat-Shamir transcript digest | 64 bytes | 32 bytes | 2× |
| edge hash | 64 bytes | 32 bytes | 2× |
| EdgeSet polynomial commitment | 64 bytes | 32 bytes | 2× |

at planetary scale (10^24 particles, 10^24 edges): 32 bytes × 2 × 10^24 = 6.4 × 10^16 bytes = 64 PB saved compared to 64-byte hashes.

### endofunction property

hemera-2 is an endofunction over 32-byte space: hash(32 bytes) → 32 bytes. the sponge absorbs 4 elements (one rate block of 4 elements, with 4 rate elements unused — padded), permutes, squeezes 4 elements. the output is a valid input. self-hashing is a fixed-point-free permutation over the reachable set.

### impact on Merkle trees

tree nodes store two children hashes. with 32-byte hashes:

```
hemera-1 node: child_left (64B) ‖ child_right (64B) = 128 bytes → hash → 64 bytes
hemera-2 node: child_left (32B) ‖ child_right (32B) = 64 bytes  → hash → 32 bytes
```

64 bytes = 8 Goldilocks elements = exactly one rate block. tree hashing absorbs both children in a single permutation call (no multi-block absorption needed for binary nodes). this is cleaner than hemera-1, where 128 bytes = 16 elements requires absorbing in two rate blocks (two permutation calls per node).

hemera-2 tree hashing is 2× faster: one permutation call per node instead of two.

### impact on nox

identity registers (r1-r4) in the nox trace hold particle/neuron hashes. with 32-byte hashes, each identity fits in 4 Goldilocks elements instead of 8. the trace width for identity-carrying operations halves.

### impact on zheng-2

WHIR commitments are 32 bytes. proof transcript digests are 32 bytes. the recursive verifier hashes 32-byte commitments instead of 64-byte ones — halving the hash work in the verification circuit.

```
zheng-2 + hemera-2 (32B):
  commitment size:    32 bytes (was 64)
  transcript digest:  32 bytes (was 64)
  Merkle path node:   32 bytes (was 64)
  proof size impact:  ~30% smaller (Merkle paths dominate proof bulk)
```

### impact on transcript format

the transcript wire format from the zheng specification changes:

```
Commitment := GoldilocksElement[0] ‖ ... ‖ GoldilocksElement[3]    // 4 × 8 = 32 bytes
```

all commitment fields in the proof format shrink from 64 to 32 bytes. WHIRProof Merkle path nodes shrink from 64 to 32 bytes per node. proof size reduction is proportional to the Merkle path depth.

## round constant generation

hemera-2 uses the same self-bootstrap procedure as hemera-1, with a different seed to produce different constants:

```
seed = [0x63, 0x79, 0x62, 0x65, 0x72, 0x32]    — bytes spelling "cyber2"

procedure:
  1. define Hemera2_0 = permutation with x^7 full rounds + x^{-1} partial rounds, all constants = 0
  2. feed seed through Hemera2_0 sponge
  3. squeeze 192 elements: 128 → RC_FULL, 64 → RC_PARTIAL (only first 16 used)
  4. Hemera-2 = Hemera2_0 + these constants. freeze.
```

only 16 partial round constants are needed (vs 64 for hemera-1). the generation procedure squeezes all 64 for uniformity but only the first 16 are loaded.

## nox integration

### pattern 15 (hash)

the hash pattern remains opcode 15. the focus cost changes from 300 to ~100 (proportional to round count reduction):

```
hemera-1: pattern 15 cost = 300 focus (72 rounds, accounting for multi-row patterns)
hemera-2: pattern 15 cost = 100 focus (24 rounds)
```

3× focus reduction per hash invocation. for hash-heavy computations (cyberlink creation, Merkle proofs, recursive verification), this is a 3× improvement in the computation budget.

### jet acceleration

the hash jet recognizes the hemera-2 formula hash and accelerates the full 24-round permutation. the jet decomposition:

```
hemera-2 jet:
  4 full rounds (x^7, M_E):  pattern 7 (mul) + pattern 5 (add)
  16 partial rounds (x^{-1}, M_I): pattern 8 (inv) + pattern 5 (add) + pattern 7 (mul)
  4 full rounds (x^7, M_E):  pattern 7 (mul) + pattern 5 (add)
```

the inverse pattern (pattern 8, cost 64) handles x^{-1} natively. each partial round is one inv (64 rows) + one M_I application (~17 rows) ≈ 81 rows. 16 partial rounds: ~1,296 rows. 8 full rounds: ~640 rows. total: ~1,936 rows.

with jet acceleration, the hash becomes a single opcode at cost 100, regardless of internal row count.

### trace row count

without jet acceleration (pure Layer 1):

```
hemera-1: ~5,000 trace rows per hash (72 rounds × ~70 rows)
hemera-2: ~2,000 trace rows per hash (24 rounds × ~80 rows)
```

the partial rounds with inversion have more rows per round (64 for inv vs ~3 for x^7) but there are 4× fewer of them. full round rows are the same. net: ~2.5× fewer total rows.

## interaction with zheng-2

hemera-2 composes multiplicatively with the zheng-2 improvements:

### proof-carrying fold reduction

```
zheng-1 + hemera-1:
  hash fold cost: 72 fold steps × 30 ops = 2,160 ops per hash
  verifier (400 calls): 400 × 2,160 = 864,000 ops

zheng-2 + hemera-1:
  same: 72 fold steps per hash, 28,800 total for verifier

zheng-2 + hemera-2:
  hash fold cost: 24 fold steps × 30 ops = 720 ops per hash
  verifier (400 calls): 400 × 720 = 288,000 ops
  improvement over zheng-1 + hemera-1: 3× from fewer rounds
```

### recursive verification

the zheng-2 verifier performs ~400 hemera calls. with hemera-2, each call is 3× cheaper in fold steps. the recursive verification cost drops:

```
zheng-2 verifier + hemera-1:
  ~5,500 constraints (from zheng-2.md)
  hash-related: ~300 constraints × (fraction of calls that are hashes)

zheng-2 verifier + hemera-2:
  hash constraints: 736 vs 1,152 per hash (1.56× fewer)
  fold steps for hashes: 3× fewer
  combined verifier cost: ~4,000 constraints (estimated)
```

### end-to-end composition

```
operation                  zheng-1+hemera-1    zheng-2+hemera-2    improvement
proof size:                157 KiB             2-8 KiB             20-78×
verification:              1.0 ms              30-100 μs           10-33×
hash fold cost:            2,160 ops           720 ops             3×
MPC hash depth:            216 rounds          40 rounds           5.4×
recursive step:            70K constraints      ~30 field ops       2,300×
prover memory:             O(N)                O(√N)               √N ×
hash native speed:         53 MB/s             53 MB/s             1× (same)
tree hash speed:           1 perm/node         1 perm/node         same calls, but...
tree hash throughput:      ~26 MB/s            ~53 MB/s            2× (single-block nodes)
hash output:               64 bytes            32 bytes            2× storage
```

## comparison with alternative hash designs

| property | hemera-2 | Anemoi | Griffin | Tip5 | Rescue-Prime |
|---|---|---|---|---|---|
| rounds | 24 | ~12 | ~30 | ~5 | ~12 |
| native speed | ~53 MB/s | ~40 MB/s | ~35 MB/s | ~60 MB/s | ~15 MB/s |
| STARK constraints | ~736 | ~500 | ~900 | ~400 | ~600 |
| MPC depth | 40 | ~24 | ~60 | impossible | ~72 |
| FHE compatible | yes | yes | yes | no | yes |
| trinity compliant | yes | partial | partial | no | partial |
| maturity | Poseidon2 + standard inversion | novel (2023) | novel (2022) | lookup-based | well-studied |
| permanence-grade | yes | uncertain | uncertain | no (MPC/FHE fail) | yes |

Tip5 wins on raw STARK constraints but fails MPC/FHE. Anemoi has fewer rounds but less cryptanalytic history. Rescue-Prime has the algebraic benefits of inversion but applies it to ALL 16 elements per round — 21× more expensive natively. hemera-2 applies x^{-1} to one element in partial rounds only, getting the algebraic strength of Rescue at Poseidon2's native speed.

## open questions

1. **mixed S-box formal analysis**: x^7 full rounds + x^{-1} partial rounds has not been analyzed together in the Poseidon2 security framework. the degree argument is strong but the interaction via MDS mixing needs formal verification.

2. **constant generation quality**: the self-bootstrap with zero-constant hemera-2 permutation has different algebraic structure from hemera-1. constant pseudo-randomness should be verified.

3. **hardware acceleration**: GFP p2r pipeline needs to support field inversion efficiently. likely maps to the fma unit via square-and-multiply.

4. **bounty program**: Poseidon2 bounties (~$130K) do not cover the hybrid S-box. a separate bounty for the mixed design would strengthen confidence.

5. **32-byte output collision bound**: 128-bit classical CR is standard. the decision accepts 2^85 quantum BHT as sufficient. document this as an explicit security target, not an accident.

## recommendation

adopt hemera-2 as the pre-genesis permutation upgrade. three changes: S-box substitution in partial rounds (x^7 → x^{-1}), partial round reduction (64 → 16), output reduction (64 → 32 bytes). the security margin is strictly improved for algebraic attacks. the collision resistance moves from 256-bit to 128-bit classical — the standard security level used by every deployed hash function.

the upgrade preserves the Hemera specification's structure: sponge, capacity, encoding, API shape, endofunction property. output size and tree node format change. the 32-byte output makes tree hashing 2× faster (single-block binary nodes) and halves storage for every hash in the system.

the interaction with zheng-2 is multiplicative: 3× fewer fold steps per hash × 20-78× smaller proofs × 5.4× shallower MPC depth × 2× tree throughput × same native speed. the combined system is qualitatively different from the current architecture — proofs small enough for on-chain storage, verification fast enough for real-time, MPC depth shallow enough for threshold operations, FHE noise low enough for practical encrypted computation, storage halved at every layer.

see [[hemera]] for the base specification, [[zheng-2]] for the proof system upgrade, [[nox]] for VM integration, [[trinity]] for the multi-domain requirement
