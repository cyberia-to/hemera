---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera security, ecosystem context"
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
| Hemera 0.4 | Goldilocks | 16 | 12 | 48 | 8 (512 bits; 32-byte digest: 128-bit collision) | Genesis |

## what is novel, what is not

Not novel:

- Poseidon2 with t=16. SP1, RISC Zero, and Stwo all deploy t=16.
- Poseidon2 on Goldilocks. Plonky3 and Miden use Goldilocks with t=12.
- The security proof methodology. Hemera follows the same wide trail and algebraic degree analysis as all Poseidon2 instantiations.
- the linear layers. Hemera 0.4 uses the Poseidon2 reference matrices for Goldilocks t = 16: the Horizen Labs 4×4 block and internal diagonal, reproduced from their published derivation (`specs/matrices.md`).
- the S-box. x⁷ in every round, the reference S-box.

Novel:

- Goldilocks + t=16 in a hash that addresses permanent content. Plonky3 and Miden hash with t=12; the systems that use t=16 (SP1, RISC Zero, Stwo) use 31-bit fields. The t = 16 Goldilocks reference instance exists (Horizen Labs, Plonky3) and Hemera uses its matrices.
- R_F = 12, R_P = 48: above the reference (8, 22) by a stated rule (`docs/explanation/parameters.md`). Among deployed Poseidon2 instances only Aztec/Noir runs more partial rounds (R_P = 56, on BN254).
- self-bootstrapped round constants: Hemera₀ over the seed "cyber" with the constant checks, in place of the Grain LFSR (`specs/bootstrap.md`; review question R2 of the campaign).

Security of the 32-byte digest:

| metric | value | bound |
|---|---|---|
| classical collision | 2^128 | birthday on p⁴ ≈ 2^256 digests |
| BHT quantum collision | 2^85.3 | p^(4/3) |
| classical preimage | 2^256 | p⁴ (capacity 2^256 as well) |
| Grover quantum preimage | 2^128 | |

Longer outputs (48 or 64 bytes of the XOF) raise the collision rows to 2^192 / 2^256 classical; see `specs/README.md` § digest length.

Actual risk: the permutation is measured, not proven. The public cryptanalysis campaign (`audit/campaign.md`) publishes the frozen constants, vectors and a bounty instance table for this exact profile; its results log is the record of what is known.
