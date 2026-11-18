---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera matrices, Hemera linear layers
---

# matrix construction

## external matrix M_E (16×16)

circulant of 4×4 MDS sub-blocks, following Poseidon2 paper §4.2.

### 4×4 sub-block M4

```
[ 2 3 1 1 ]
[ 1 2 3 1 ]
[ 1 1 2 3 ]
[ 3 1 1 2 ]
```

circulant — each row is the previous row rotated right by one position.

### full 16×16 construction

the state is partitioned into four 4-element chunks. M4 is applied to each chunk independently, then column sums are added back:

```
M_E = circ(2·M4, M4, M4, M4)
```

explicitly:

```
         chunk_0   chunk_1   chunk_2   chunk_3
       ┌─────────┬─────────┬─────────┬─────────┐
       │  2·M4   │   M4    │   M4    │   M4    │
       │   M4    │  2·M4   │   M4    │   M4    │
       │   M4    │   M4    │  2·M4   │   M4    │
       │   M4    │   M4    │   M4    │  2·M4   │
       └─────────┴─────────┴─────────┴─────────┘
```

algorithm:
1. apply M4 to each 4-element chunk of the state
2. compute column sums: `s[k] = Σ state[j+k]` for `j ∈ {0,4,8,12}`, `k ∈ {0,1,2,3}`
3. add the appropriate column sum to each element: `state[i] += s[i mod 4]`

## internal matrix M_I (16×16)

```
M_I = 1 + diag(d₀, d₁, ..., d₁₅)
```

where `1` is the all-ones matrix (every entry = 1). the multiplication is:

```
state'[i] = d[i] · state[i] + Σ state[j]   for j = 0..15
```

### diagonal values

```
d[ 0] = 0xde9b91a467d6afc0
d[ 1] = 0xc5f16b9c76a9be17
d[ 2] = 0x0ab0fef2d540ac55
d[ 3] = 0x3001d27009d05773
d[ 4] = 0xed23b1f906d3d9eb
d[ 5] = 0x5ce73743cba97054
d[ 6] = 0x1c3bab944af4ba24
d[ 7] = 0x2faa105854dbafae
d[ 8] = 0x53ffb3ae6d421a10
d[ 9] = 0xbcda9df8884ba396
d[10] = 0xfc1273e4a31807bb
d[11] = 0xc77952573d5142c0
d[12] = 0x56683339a819b85e
d[13] = 0x328fcbd8f0ddc8eb
d[14] = 0xb5101e303fce9cb7
d[15] = 0x774487b8c40089bb
```

all values are canonical Goldilocks elements (< p = 0xFFFFFFFF00000001). follows the Plonky3 convention for t=16.

## verification

what is checked over the Goldilocks field, and what holds:

- M4, the 4×4 block, is MDS: all 69 square sub-matrices have non-zero determinant.
- M_E and M_I are invertible.
- neither 16×16 matrix is MDS. in M_E rows 0 and 4 restricted to columns 8 and 9 are both `(2, 3)`; in M_I any 2×2 minor taken from off-diagonal entries is all-ones. both minors are singular.
- both layers diffuse fully in one application: no entry of M_E or M_I is zero, so every output element depends on every input element.

the non-MDS result is the Poseidon2 construction working as designed — no Poseidon2 external or internal layer is MDS at t ≥ 8 — so branch-number arguments of the MDS kind do not apply to the 16×16 layers. the conditions that matter for M_I are the subspace-trail and invariant-subspace conditions of ePrint 2020/500; no such check is recorded for this diagonal.

earlier revisions of this page stated that both 16×16 matrices are MDS with maximum branch number. that claim was wrong and is withdrawn (hemera#2; `audit/campaign.md` §1.3, merged to `main` after this branch was cut).
