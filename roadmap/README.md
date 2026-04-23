# hemera roadmap

hemera is specified: x⁻¹ partial S-box, 16 partial rounds, 32-byte output, ~736 constraints per permutation. these proposals are OPTIMIZATIONS on top of the current spec — reducing constraint count further and shifting hemera's role from "hash for everything" to "trust anchor."

## status: implemented = shipped in code and spec

## implemented (0.3)

| proposal | what it does |
|----------|-------------|
| [[inversion-sbox]] | x⁻¹ S-box in partial rounds, 16 partial rounds, 2^1046 algebraic degree |
| [[compact-output]] | 32-byte output, single-permutation binary tree nodes |
| [[verified-streaming]] | pre-order encode/decode with incremental hash verification |
| [[async-streaming]] | O(log n) memory async FSM decoder, tokio-compatible |
| [[sparse-merkle]] | 256-bit key sparse Merkle tree with compressed proofs |
| [[batch-proofs]] | deduplicated multi-leaf inclusion proofs |
| [[gpu-backend]] | wgpu compute shaders, u64 emulation in WGSL, batch dispatch |
| [[zero-alloc]] | no_std core, fixed-size buffers, rs-edition compliant |

## optimization proposals

| proposal | in reference? | breaks hash? | target |
|----------|--------------|:------------:|--------|
| [[partial-round-collapse]] | no | no | precompute linear evolution: 4× prover wall-clock |
| [[constraint-free-mds]] | no | no | absorb MDS into CCS wiring: 26% fewer constraints (~544) |
| [[algebraic-fiat-shamir]] | no | no | algebraic challenge derivation: 8.7× fewer hemera calls |

batched-proving and folded-sponge removed — polynomial nouns reduce hemera to ~3 calls per execution, making batch/fold optimizations unnecessary.

## scope expansion proposals

| proposal | in reference? | breaks hash? | target |
|----------|--------------|:------------:|--------|
| [[erasure-coding]] | no | no | Reed-Solomon erasure coding over Goldilocks: same field, same NTT, data availability codec |
| [[capacity-typing]] | no | no | type tags in reserved capacity slot state[14]: type-integrated hashing, type confusion prevention |
| [[semantic-hashing]] | no | **yes** | section tree identity for .cyb containers: flat hash → section tree, changes particle_id for structured files |

## targets

```
                        hemera (current)      + optimizations (all)
constraints/perm:       ~736                  ~544 (wired MDS)
FS calls (20-round):    20 × 736 = 14,720    1 × 736 + 19 × 50 = 1,686
```

## endgame role

hemera becomes the identity layer: content identity (hash), content typing (capacity), content availability (erasure). polynomial commitments ([[Brakedown]]) handle the high-volume proof work — proof binding and state verification with ZERO hemera calls.

```
always hemera:     H(particle) identity, H(cyberlink), Fiat-Shamir seed
                   type-integrated hashing (capacity slot → type IS identity)
                   erasure encoding (RS over Goldilocks → availability codec)
algebraic:         proof challenges (algebraic FS), state verification (polynomial)
eliminated:        tree hashing (Brakedown is Merkle-free), DAS proofs (Lens openings)
```

key composition: with ~3 hemera calls per execution, each permutation folds into the [[HyperNova]] accumulator (~30 field ops) during [[proof-carrying computation|proof-carrying]] execution.

## cross-repo dependencies

| zheng proposal | hemera interaction |
|------------------|--------------------|
| [[proof-carrying]] | each hemera permutation (~3 per execution) = one fold step |
| [[brakedown-pcs]] | Merkle-free Lens eliminates hemera tree overhead entirely |

| bbg proposal | hemera interaction |
|--------------|-------------------|
| [[algebraic-nmt]] | polynomial state reduces hemera state calls from 144K to 0 per block |
| [[signal-first]] | signals content-addressed via hemera; hemera identity IS signal identity |

## lifecycle

| status | meaning |
|--------|---------|
| draft | idea captured, open for discussion |
| accepted | approved — ready to implement |
| implemented | done — shipped in code |
