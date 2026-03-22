---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera security, ecosystem context"
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
---

# ecosystem context

## Poseidon2 deployment landscape

| System | Field | t | R_F | R_P | Capacity | Status |
|---|---|---|---|---|---|---|
| Plonky3 | Goldilocks | 12 | 8 | 22 | 4 (128-bit) | Production |
| SP1 | BabyBear | 16 | 8 | 13 | 8 (124-bit) | Production |
| RISC Zero | BabyBear | 16 | 8 | 13 | 8 (124-bit) | Production |
| Stwo/Starknet | M31 | 16 | 8 | 14 | 8 (124-bit) | Production (mainnet) |
| Miden | Goldilocks | 12 | 8 | 22 | 4 (128-bit) | Production |
| Aztec/Noir | BN254 | 4 | 8 | 56 | 1 (127-bit) | Production |
| Hemera | Goldilocks | 16 | 8 | 64 | 8 (256-bit) | Genesis |

## what is novel, what is not

Not novel:

- Poseidon2 with t=16. SP1, RISC Zero, and Stwo all deploy t=16.
- Poseidon2 on Goldilocks. Plonky3 and Miden use Goldilocks with t=12.
- The security proof methodology. Hemera follows the same wide trail and algebraic degree analysis as all Poseidon2 instantiations.
- MDS construction. The matrix design follows known techniques for Poseidon2.

Novel:

- Goldilocks + t=16 combination. No production system uses Goldilocks at width 16. Plonky3 and Miden use t=12. The systems that use t=16 (SP1, RISC Zero, Stwo) use 31-bit fields.
- R_P=64. The highest partial round count in any deployed Poseidon2. The next highest is Aztec/Noir at R_P=56 (on BN254, a very different field). On small fields, the maximum deployed is R_P=22.

Actual risk: a subtle error in the specific M_E or M_I matrix constructed for Goldilocks at t=16. The permutation structure, S-box, and round counts are conservative. The MDS matrices are the only component that must be validated specifically for this field-width combination.