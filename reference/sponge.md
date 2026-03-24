---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera sponge, sponge specification
diffusion: 0.00010722364868599256
springs: 0.002289426463559174
heat: 0.0015732806631330253
focus: 0.0010550958960373398
gravity: 0
density: 0.32
---

# sponge specification

Hemera has exactly one primitive: the sponge. there is no compression mode.

see [[capacity]] for the structured capacity layout (flags, domain tags, counters, namespace bounds).

## operation

```
Initialize:  state ← [0; 16]
             state[11] ← domain_tag

Absorb:      for each 8-element block of padded input:
               state[0..8] += block        (Goldilocks field addition, element-wise)
               state ← permute(state)

Finalize:    state[10] ← total_input_bytes
             state ← permute(state)

Squeeze:     output ← state[0..4]           (4 elements = 32 bytes)
```

absorption uses Goldilocks field addition (mod p), not XOR and not wrapping addition. this preserves the algebraic structure that Poseidon2's security proof relies on.

## operational semantics

every use of the sponge in the Hemera stack:

- **particle addressing.** small content (up to 4096 bytes) is hashed directly through the sponge. large content is split into 4096-byte chunks, each chunk produces a chaining value via the sponge, and the chaining values are combined in a binary Merkle tree whose root is the particle address.

- **cyberlink identity.** a cyberlink is identified by the sponge hash of its canonical encoding. the hash commits to the link's source, destination, and tag particles.

- **Merkle proofs.** internal nodes combine two child hashes using the sponge with FLAG_PARENT set. verification recomputes the path from leaf to root using the same sponge configuration.

- **incremental hashing.** the counter field (state[8]) tracks chunk position, enabling incremental computation. a file can be hashed chunk-by-chunk without buffering the entire input.

- **streaming verification.** chunks arrive over the network with their Merkle proof. each chunk is verified independently using the sponge — the verifier never needs the full file.

- **MMR peaks (AOCL in BBG Layer 4).** append-only commitment lists use a Merkle Mountain Range. each peak is a sponge hash with FLAG_PARENT | FLAG_ROOT over its subtree.

- **NMT commitments.** Namespace Merkle Trees store namespace bounds in the capacity region (state[12..14]). internal nodes propagate min/max namespace from children. the sponge absorbs child hashes while the capacity carries namespace metadata.

- **WHIR polynomial commitments.** [[cyber]] uses [[WHIR]] for polynomial commitments. the sponge serves as the Fiat-Shamir transcript and builds the commitment Merkle trees. the field-native property eliminates the conversion overhead between hash output and field elements.

- **field-native computation.** the Poseidon2 sponge operates directly over the Goldilocks field, requiring ~736 constraints per hash in a STARK circuit (24 rounds, x⁻¹ partial S-box). BLAKE3 requires ~15,000 constraints for the same role.

- **tri-kernel lookup.** the three kernel modes (consensus, settlement, data availability) use the sponge for all hash-addressed lookups, providing a single primitive across the entire stack.