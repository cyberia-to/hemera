---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera bootstrap, round constant generation
---

# round constant generation

Hemera generates her own round constants. No external primitives.

The permutation structure (S-box x⁷ in every round, matrices M_E and M_I, round flow 6 + 48 + 6) is fully defined before constants exist. With all constants set to zero, the permutation is still a well-defined nonlinear function. We call this Hemera₀.

```
1. Define Hemera₀ = the Hemera 0.4 permutation with all 240 round constants = 0

2. For counter k = 0, 1, 2, …, 255:

   seed_k = [0x63, 0x79, 0x62, 0x65, 0x72]          if k = 0   ("cyber")
          = [0x63, 0x79, 0x62, 0x65, 0x72, k]       if k ≥ 1   (k as one byte)

   state = [0; 16]
   absorb seed_k into state as the plain-hash sponge does, with Hemera₀:
       rate ← encode(seed_k ‖ 0x01 ‖ 0x00*)  (one 56-byte block, 7 bytes per element)
       state[10] ← len(seed_k)
       state ← Hemera₀(state)
   squeeze 240 field elements, 8 per permutation:
       elements state[0..8], then state ← Hemera₀(state), state[0..8], …

   if the 240 elements pass the constant checks: stop.

3. First 192 elements → RC_FULL[192] (rows 0–11, round-major)
   Next 48 elements   → RC_PARTIAL[48]

4. Hemera = Hemera₀ + these constants. Freeze forever.
```

The shipped constants are accepted at k = 0: the seed is exactly the five genesis bytes.

## constant checks

| check | condition |
|---|---|
| C1 canonical | every constant < p (squeezed elements are canonical by construction) |
| C2 nonzero | no constant is 0 |
| C3 distinct | the 240 constants are pairwise distinct |

The Poseidon2 subspace-trail conditions are conditions on the linear layers, checked on the matrices themselves ([[matrices]]): an internal matrix whose powers M_I^1 … M_I^32 have irreducible characteristic polynomials has no invariant subspace, for any choice of constants. The constant checks exclude the degenerate cases left — a zero constant leaves an S-box input unshifted, a repeated constant repeats a round's affine offset.

Review question R2 of the cryptanalysis campaign (`audit/campaign.md` §5.2) asks whether this self-bootstrap followed by these checks is as good as the Grain LFSR generator of the Poseidon scripts.

## seed

Five bytes [0x63, 0x79, 0x62, 0x65, 0x72]. Not "UTF-8 encoding of cyber" — the bytes themselves are the specification. No character set, no encoding, no text convention.

The parameters do not appear in the seed because they are not data — they are the structure of Hemera₀ itself.

## zero-state fixed point

The all-zero state is a fixed point of Hemera₀. This does not affect constant generation because the sponge absorbs the seed first. After absorbing one non-zero byte, the state is non-zero.

Do not use Hemera₀ for any purpose other than constant generation from non-trivial seeds.

## reproducibility

The procedure is fully deterministic. Same S-box, same matrices, same round structure, same sponge, same seed — same 240 field elements. Two independent implementations agree: `rs/src/bootstrap.rs` and `audit/campaign/hemera_ref.py`.

Hemera 0.3 ran the same procedure (without the checks and the counter) through its own Hemera₀ — x⁷ full rounds 4 + 4, x⁻¹ partial rounds 16, the circ(2, 3, 1, 1) block — and produced 144 constants; the Python model reproduces them as the record.
