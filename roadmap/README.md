# hemera roadmap

hemera is specified: x⁻¹ partial S-box, 16 partial rounds, 32-byte output, ~736 constraints per permutation. these proposals are OPTIMIZATIONS on top of the current spec — reducing constraint count further and shifting hemera's role from "hash for everything" to "trust anchor."

## implemented (in current spec)

| proposal | what it did |
|----------|-------------|
| [[inversion-sbox]] | x⁻¹ partial S-box, 16 rounds (was x⁷, 64 rounds): 36% fewer constraints, 2^1046 algebraic degree |
| [[compact-output]] | 32-byte output (was 64): 2× tree speed, single-perm binary nodes |

## optimization proposals

| proposal | status | target |
|----------|--------|--------|
| [[batched-proving]] | draft | N hemera calls → 1 sumcheck: 400× for N=1000 |
| [[partial-round-collapse]] | draft | precompute linear evolution: 4× prover wall-clock |
| [[constraint-free-mds]] | draft | absorb MDS into CCS wiring: 26% fewer constraints (~544) |
| [[folded-sponge]] | draft | fold multi-block absorption: 7× for 10-block inputs |
| [[algebraic-fiat-shamir]] | draft | algebraic challenge derivation: 8.7× fewer hemera calls |

## targets

```
                        hemera (current)      + optimizations (all)
constraints/perm:       ~736                  ~544 (wired MDS)
FS calls (20-round):    20 × 736 = 14,720    1 × 736 + 19 × 50 = 1,686
batch (N=1000):         N × 736 = 736K       736 + O(N)
10-block sponge:        10 × 736 = 7,360     736 + 300 = 1,036
```

## endgame role

hemera becomes the **trust anchor**: content identity, private records, initial seed, Fiat-Shamir binding. polynomial commitments ([[Brakedown]]) handle the high-volume work — proof binding and state verification with ZERO hemera calls.

```
always hemera:     H(particle) identity, H(cyberlink), Fiat-Shamir seed
algebraic:         proof challenges (algebraic FS), state verification (polynomial)
eliminated:        tree hashing (Brakedown is Merkle-free), DAS proofs (PCS openings)
```

key composition: folded-sponge + [[proof-carrying computation|proof-carrying]] = one continuous fold. hemera absorption is folded into the [[HyperNova]] accumulator (~30 field ops per block) instead of computed independently.

## cross-repo dependencies

| zheng proposal | hemera interaction |
|------------------|--------------------|
| [[folding-first]] | fold hemera calls during proof-carrying execution |
| [[proof-carrying]] | each hemera permutation = one fold step |
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
