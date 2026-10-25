---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera matrices, MDS matrices
---

# matrix construction

hemera 0.4 uses the Poseidon2 reference linear layers for Goldilocks at t = 16 — the matrices of the Horizen Labs reference implementation, also shipped by Plonky3 (`Poseidon2GoldilocksHL`). both are reproduced from their published derivation by `audit/campaign/poseidon2_params.py` and checked against the conditions below on the layers the crate computes by `rs/tests/poseidon2_conditions.rs`.

## external matrix M_E (16×16)

block matrix of the 4×4 MDS block M4, following Poseidon2 §5.1 (t = 4k):

### 4×4 block M4

```
[ 5 7 1 3 ]
[ 4 6 1 1 ]
[ 1 3 5 7 ]
[ 1 1 4 6 ]
```

the matrix of Poseidon2 Appendix B and the Horizen Labs reference (Plonky3 `HLMDSMat4`). it is MDS: all 69 square submatrices have nonzero determinant over F_p. evaluation in 8 additions and 4 doublings:

```
t0 = x0 + x1          t1 = x2 + x3
t2 = 2·x1 + t1        t3 = 2·x3 + t0
t4 = 4·t1 + t3        t5 = 4·t0 + t2
y0 = t3 + t5   y1 = t5   y2 = t2 + t4   y3 = t4
```

hemera 0.3 used the circulant block circ(2, 3, 1, 1) (Plonky3 `MDSMat4`); 0.4 adopts the reference block so that the external layer is exactly the one the Poseidon2 analysis and the Goldilocks reference instances use.

### full 16×16 construction

```
M_E = [ 2·M4   M4    M4    M4  ]
      [  M4   2·M4   M4    M4  ]
      [  M4    M4   2·M4   M4  ]
      [  M4    M4    M4   2·M4 ]
```

algorithm:
1. apply M4 to each 4-element chunk of the state
2. compute column sums: `s[k] = Σ state[j+k]` for `j ∈ {0,4,8,12}`, `k ∈ {0,1,2,3}`
3. add the appropriate column sum to each element: `state[i] += s[i mod 4]`

M_E is invertible. at t = 16 it is MDS block by block (each 4×4 block is M4 or 2·M4); the 16×16 matrix as a whole has singular 2×2 minors (rows 0 and 4 restricted to columns 8 and 9 are both (5, 7)), as every Poseidon2 external layer for t ≥ 8 has by construction. the construction and its diffusion are those analysed in Poseidon2 §5.1, and `audit/campaign.md` §3 is written for exactly this layer. hemera 0.3's documents called both 16×16 matrices MDS; that claim was wrong (hemera#2) and is withdrawn.

## internal matrix M_I (16×16)

```
M_I = 1 + diag(d₀, d₁, ..., d₁₅)
```

where `1` is the all-ones matrix. the multiplication is:

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

all values are canonical Goldilocks elements. this is `MAT_DIAG16_M_1` of the Horizen Labs Goldilocks t = 16 instance (Plonky3 `MATRIX_DIAG_16_GOLDILOCKS`), unchanged from hemera 0.3.

### derivation

the Poseidon2 parameter script (`poseidon2_rust_params.sage`, HorizenLabs/poseidon2) draws the diagonal from the Grain LFSR used by every Poseidon script:

1. initialise Grain with the instance parameters: field type 1 (prime field), S-box 0 (x^α), field size 64, t = 16, R_F = 8, R_P = 22 (the reference round numbers for Goldilocks t = 16);
2. draw the R_F·t + R_P = 150 round constants (64-bit draws, rejection-sampled below p);
3. draw 16 raw 64-bit values r₀ … r₁₅ and form M = (1 − I) + diag(r) = 1 + diag(r − 1);
4. accept M if, for every i = 1 … 2t = 32, the characteristic polynomial of M^i is irreducible over F_p (so its minimal polynomial has full degree 16); otherwise draw 16 new values and repeat;
5. d = r − 1.

`audit/campaign/poseidon2_params.py` reimplements Grain and this loop in pure Python: it reproduces the Horizen Labs width-8 Goldilocks round constants and diagonal (accepted at candidate 3), and for t = 16 rejects candidates 0 … 45 and accepts candidate 46, which equals the table above. the reference instance's round constants are not used by hemera — hemera's constants come from its own seed ([[bootstrap]]).

## conditions

checked by `rs/tests/poseidon2_conditions.rs` on the matrices read off the implementation (`mds_light_permutation`, `matmul_internal` applied to unit vectors) and by `audit/campaign/poseidon2_params.py` on the definitions:

| condition | why | status |
|---|---|---|
| M4 is MDS (69 square minors nonzero) | required of the 4×4 block by the Poseidon2 external-layer construction | holds |
| M_E invertible | the permutation is a bijection | holds |
| M_I invertible | the permutation is a bijection | holds |
| charpoly(M_I^i) irreducible over F_p, i = 1 … 32 | no M_I^i has a nontrivial invariant subspace, the sufficient condition against infinitely long subspace trails through the partial rounds (ePrint 2020/500; Poseidon2 parameter script) | holds |

an irreducible characteristic polynomial of degree 16 is also the minimal polynomial, and a matrix whose characteristic polynomial is irreducible has no invariant subspace other than 0 and F_p^16. the condition is independent of the round constants: no choice of constants opens an invariant subspace through M_I.
