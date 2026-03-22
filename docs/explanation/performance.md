---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera performance, hash rate, proving cost"
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
---

# Performance Characteristics

## Native Hash Rate

| Metric                       | Hemera | Plonky3 Goldilocks t=12 | Ratio |
|------------------------------|--------|-------------------------|-------|
| State width                  | 16 elements | 12 elements        | 1.33x |
| Total rounds                 | 72          | 30                 | 2.40x |
| Permutation field muls       | ~3,648      | ~2,050             | 1.78x |
| Input bytes per permutation  | 56          | 56                 | 1.00x |
| Estimated hash rate          | ~53 MB/s    | ~86 MB/s           | 0.62x |
| Perms for 1 KB               | 19          | 19                 | 1.00x |

38% native hash rate reduction comes from the wider permutation and
additional partial rounds. Throughput per permutation is identical
between the two designs. Partial rounds are lightweight (~19 field
multiplications each vs ~304 for full rounds), so the round count
increase (72 vs 30) overstates the actual computational cost
difference.

## Proving Cost

STARK trace dimensions change from Plonky3 Goldilocks to Hemera:

- Trace width: 12 -> 16 columns (~1.33x)
- Trace length: 30 -> 72 rows (~2.40x)
- Combined: ~3.2x proving cost per hash

System-level impact depends on what fraction of total proving time
is spent on hashing. If hashing is 20% of total proving time, the
system-level overhead is ~0.44x (20% x 3.2 + 80% x 1.0 = 1.44x).
If hashing is 40% of total proving time, overhead rises to ~0.88x
(40% x 3.2 + 60% x 1.0 = 1.88x). Wider state provides security
margin that justifies this cost at both operating points.

## Steady-State Adequacy

At scale: 10²⁴ cyberlinks with 1% annual update rate.

- 10^24 x 0.01 / (365.25 x 86,400) = ~317B cyberlinks/sec required
- Each particle = 64 bytes = ~1 permutation
- Single core at ~53 MB/s = ~946,000 permutations/sec
- Single core handles steady-state with ~3x headroom

Burst scenarios (bulk import, migration, recovery) benefit from
parallelism. The permutation is independently computable per chunk,
scaling linearly with core count. A 64-core machine sustains
~60 million permutations/sec, sufficient for bulk rehash of large
datasets.