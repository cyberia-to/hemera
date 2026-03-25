# hemera roadmap

hemera is specified: x⁻¹ partial S-box, 16 partial rounds, 32-byte output, ~736 constraints per permutation. these proposals are OPTIMIZATIONS on top of the current spec — reducing constraint count further and shifting hemera's role from "hash for everything" to "trust anchor."

## status: in reference = proposal is now the canonical spec

## core (implemented in reference)

| proposal | in reference? | what it did |
|----------|--------------|-------------|
| [[inversion-sbox]] | **yes** → reference/permutation.md | x⁻¹ S-box, 16 partial rounds, 2^1046 algebraic degree |
| [[compact-output]] | **yes** → reference/sponge.md, reference/tree.md, reference/api.md | 32-byte output, 1-perm binary nodes |

## optimization proposals

| proposal | in reference? | target |
|----------|--------------|--------|
| [[partial-round-collapse]] | no | precompute linear evolution: 4× prover wall-clock |
| [[constraint-free-mds]] | no | absorb MDS into CCS wiring: 26% fewer constraints (~544) |
| [[algebraic-fiat-shamir]] | no | algebraic challenge derivation: 8.7× fewer hemera calls |

batched-proving and folded-sponge removed — polynomial nouns reduce hemera to ~3 calls per execution, making batch/fold optimizations unnecessary.

## targets

```
                        hemera (current)      + optimizations (all)
constraints/perm:       ~736                  ~544 (wired MDS)
FS calls (20-round):    20 × 736 = 14,720    1 × 736 + 19 × 50 = 1,686
```

## endgame role

hemera becomes the **trust anchor**: content identity, private records, initial seed, Fiat-Shamir binding. polynomial commitments ([[Brakedown]]) handle the high-volume work — proof binding and state verification with ZERO hemera calls.

```
always hemera:     H(particle) identity, H(cyberlink), Fiat-Shamir seed
algebraic:         proof challenges (algebraic FS), state verification (polynomial)
eliminated:        tree hashing (Brakedown is Merkle-free), DAS proofs (PCS openings)
```

key composition: with ~3 hemera calls per execution, each permutation folds into the [[HyperNova]] accumulator (~30 field ops) during [[proof-carrying computation|proof-carrying]] execution.

## cross-repo dependencies

| zheng proposal | hemera interaction |
|------------------|--------------------|
| [[proof-carrying]] | each hemera permutation (~3 per execution) = one fold step |
| [[brakedown-pcs]] | Merkle-free PCS eliminates hemera tree overhead entirely |

| bbg proposal | hemera interaction |
|--------------|-------------------|
| [[algebraic-nmt]] | polynomial state reduces hemera state calls from 144K to 0 per block |
| [[signal-first]] | signals content-addressed via hemera; hemera identity IS signal identity |

## lifecycle

| status | meaning |
|--------|---------|
| draft | idea captured, open for discussion |
| accepted | approved — ready to implement |
| implemented | done — migrated to relevant spec file |
