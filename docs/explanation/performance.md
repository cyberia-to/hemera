---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera performance, hash rate, proving cost"
---

# Performance Characteristics

## Native Hash Rate

hemera 0.4 against the Poseidon2 reference instance for Goldilocks t = 12 (Plonky3, R_F 8, R_P 22), counted the same way (x⁷ = 4 multiplications, one multiplication per diagonal entry of M_I):

| Metric                       | Hemera 0.4 | Goldilocks t=12 (8, 22) | Ratio |
|------------------------------|------------|-------------------------|-------|
| State width                  | 16 elements | 12 elements            | 1.33x |
| Total rounds                 | 60 (12 + 48) | 30 (8 + 22)           | 2.00x |
| Permutation field muls       | 1,728       | 736                    | 2.35x |
| Input bytes per permutation  | 56          | 32 (rate 4 × 8 B)      | 1.75x |

measured on an Apple M4 Max, one core, shared machine (`audit/profile-0.4-2026-10.md`):

| operation | time | throughput |
|---|---|---|
| one permutation, latency | 1.35 µs | 0.74 M permutations/s |
| permutations batched 16 at a time | 1.23 µs each | 0.81 M permutations/s |
| sponge, 56 bytes per permutation | | ≈ 41–46 MB/s |

## Proving Cost

the trace API emits one row per round: 60 rows per permutation, 240 S-box witnesses (`[x², x³]` each). in hemera's constraint accounting a permutation costs ~1,440 constraints (960 for the S-boxes, ~480 for the linear layers), against ~736 for the 0.3 profile. the system-level overhead is the hashing share of a proof times that factor; the roadmap (`roadmap/README.md`) moves bulk commitment away from hemera, which shrinks that share.

## Steady-State Adequacy

At scale: 10²⁴ cyberlinks with 1% annual update rate.

- 10^24 x 0.01 / (365.25 x 86,400) ≈ 3.2 × 10^14 cyberlinks/sec required
- Each particle = 32 bytes = ~1 permutation
- Single core at ~0.8 million permutations/sec (hemera 0.4, measured): ≈ 4 × 10^8 core-equivalents at that scale

Burst scenarios (bulk import, migration, recovery) benefit from
parallelism. The permutation is independently computable per chunk,
scaling linearly with core count. A 64-core machine sustains
~50 million permutations/sec, sufficient for bulk rehash of large
datasets.