# hemera proposals

design proposals for hemera hash function evolution.

## hemera-2 (pre-genesis upgrade)

| proposal | status | target |
|----------|--------|--------|
| [[inversion-sbox]] | draft | x⁻¹ partial S-box + 16 rounds: 36% fewer constraints, 5.4× MPC depth, 2^1046 algebraic degree |
| [[compact-output]] | draft | 32-byte output: 2× tree speed, 2× storage, single-perm binary nodes |

## hemera-3 (stack-aware optimizations)

| proposal | status | target |
|----------|--------|--------|
| [[batched-proving]] | draft | N hemera calls → 1 sumcheck: 400× for N=1000 |
| [[partial-round-collapse]] | draft | precompute linear evolution: 4× prover wall-clock |
| [[constraint-free-mds]] | draft | absorb MDS into CCS wiring: 26% fewer constraints |
| [[folded-sponge]] | draft | fold multi-block absorption: 7× for 10-block inputs |
| [[algebraic-fiat-shamir]] | draft | algebraic challenge derivation: 8.7× fewer hemera calls |

## combined targets

```
                        hemera-1            hemera-2            hemera-3 (all)
constraints/perm:       ~1,152              ~736                ~544 (wired MDS)
rounds:                 72                  24                  24
MPC depth:              216                 40                  40
output:                 64 bytes            32 bytes            32 bytes
tree node perms:        2                   1                   1
FS calls (20-round):    20 × 1,152         20 × 736            1 × 736 + 19 × 50
FS total constraints:   23,040              14,720              1,686
batch (N=1000):         N × 1,152           N × 736             736 + O(N)
10-block sponge:        10 × 1,152          10 × 736            736 + 300
```

## trajectory

```
hemera-1: hash for everything              ~1,152 constraints/perm
hemera-2: faster hash for everything       ~736 constraints/perm
hemera-3: hash for trust anchoring         ~544 constraints (wired MDS)
          + algebraic for the rest

endgame role: cryptographic root of trust — initial seed, content identity,
private records. algebraic commitments handle binding and verification at scale.
```

## structural sync impact

hemera serves two [[structural sync|structural-sync]] layers:

| layer | hemera role | optimization |
|-------|------------|--------------|
| 1. validity | zheng proof generation uses hemera for Fiat-Shamir | algebraic-fiat-shamir: 8.7× fewer hemera calls |
| 3. completeness | NMT tree hashing for completeness proofs | batched-proving: 400× for N=1000 NMT updates per block |

hemera-3's endgame: hemera becomes the **trust anchor** (content identity, private records, initial seed) while polynomial commitments handle the high-volume work (proof binding, state verification). this mirrors structural sync's architecture — hemera secures the root, algebra handles the scale.

key composition: folded-sponge + proof-carrying = one continuous fold. hemera absorption is folded into the accumulator (30 field ops per block) instead of computed independently. the sponge fold IS the proof fold.

## cross-repo dependencies

| zheng-2 proposal | hemera interaction |
|------------------|--------------------|
| [[algebraic-extraction]] | eliminates Merkle paths — hemera tree hashing reduced |
| [[folding-first]] | fold hemera calls during proof-carrying execution |
| [[proof-carrying]] | each hemera permutation = one fold step; folded-sponge composes naturally |
| [[brakedown-pcs]] | Merkle-free PCS eliminates hemera tree overhead entirely |

| bbg proposal | hemera interaction |
|--------------|-------------------|
| [[algebraic-nmt]] | NMT → polynomial reduces hemera calls from 144K to 0 per block |
| [[signal-first]] | signals are content-addressed via hemera; hemera identity IS signal identity |

## lifecycle

| status | meaning |
|--------|---------|
| draft | idea captured, open for discussion |
| accepted | approved — ready to implement |
| rejected | decided against, kept for rationale |
| implemented | done — migrated to relevant spec file |
