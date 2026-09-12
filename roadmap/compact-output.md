---
tags: cyber, cip
crystal-type: process
crystal-domain: cyber
status: implemented
date: 2026-03-17
---
# compact output — 64-byte → 32-byte hash

reduce hemera output from 8 field elements (64 bytes) to 4 field elements (32 bytes). sponge state unchanged (16 elements, approximately 1024 bits; capacity approximately 512 bits). only the squeeze extraction changes.

## squeeze change

```
before (64-byte output): output = state[0..8]     → 8 elements, 64 bytes
after  (32-byte output): output = state[0..4]     → 4 elements, 32 bytes
```

for multi-block output (XOF): squeeze 4 elements, permute, squeeze 4 more.

## security

| property | before (64-byte output) | after (32-byte output) |
|---|---|---|
| classical collision | 2^256 | 2^128 |
| classical preimage | 2^256 | 2^256 (output ceiling) |
| quantum collision (BHT) | 2^170 | 2^85 |
| quantum preimage (Grover) | 2^128 | 2^128 (capacity-limited) |

The generic output collision ceiling is approximately 128 bits. Concrete security depends on the permutation; output length and capacity both constrain possible guarantees.

birthday probability among 2^80 particles (planetary scale): 2^{80} × (2^{80} - 1) / (2 × 2^{256}) ≈ 2^{-97}. negligible.

## tree hashing: 2× faster

```
before (64-byte output): binary node = 128 bytes (2 × 64) = 16 elements = 2 rate blocks = 2 permutations
after  (32-byte output): binary node = 64 bytes (2 × 32) = 8 elements = 1 rate block = 1 permutation
```

single-permutation binary nodes. tree hashing throughput: ~26 MB/s → ~53 MB/s.

## storage savings

every hash in the system halves:

| structure | before (64-byte output) | after (32-byte output) | savings |
|---|---|---|---|
| particle address | 64 bytes | 32 bytes | 2× |
| neuron identity | 64 bytes | 32 bytes | 2× |
| Merkle node | 128 bytes | 64 bytes | 2× |
| NMT node | 192 bytes | 128 bytes | 1.5× |
| Brakedown commitment | 64 bytes | 32 bytes | 2× |
| nox noun identity | 64 bytes | 32 bytes | 2× |

At 10^24 stored digests, reducing each by 32 bytes saves 3.2×10^25 bytes (32 YB, decimal), before replication and metadata.

## endofunction property

Hashing 32-byte inputs returns 32-byte outputs. Equal input/output lengths do not imply bijectivity or absence of fixed points; neither is claimed.

## impact on nox encoding

```
before (64-byte output): encoding sizes: 8 (atom), 64 (hash), 128 (cell)  — 2³, 2⁶, 2⁷
after  (32-byte output): encoding sizes: 8 (atom), 32 (hash), 64 (cell)   — 2³, 2⁵, 2⁶
```

nox identity = 32 bytes. computation key = 64 bytes. Rs Address (32 bytes) = hemera output (32-byte). types converge.

## impact on zheng

Brakedown commitments, transcript digests, Merkle path nodes all shrink to 32 bytes. proof size reduction: ~30% (Merkle paths dominate proof bulk).

## impact on bbg

BBG sub-roots already 32 bytes. binary Merkle nodes drop to 64 bytes. storage halved at every layer.

see [[inversion-sbox]] for the S-box upgrade, [[hemera]] for base specification