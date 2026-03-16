---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera sponge, sponge specification
---

# Sponge Specification

## Capacity Layout

```
State:     state[0..8]  = rate region (input absorption / output squeezing)
           state[8..16] = capacity region (structural context, never XORed with input)

Capacity:  state[8]  = counter       chunk position in file (0-based, u64)
           state[9]  = flags         structural role (bitfield, see below)
           state[10] = msg_length    total input byte count (sponge only)
           state[11] = domain_tag    API mode selector (see below)
           state[12] = ns_min        namespace lower bound (NMT only, zero otherwise)
           state[13] = ns_max        namespace upper bound (NMT only, zero otherwise)
           state[14..16] = 0         reserved, must be zero
```

## Flags (state[9])

Three single-bit flags, combined via bitwise OR:

```
FLAG_ROOT   = 0x01    This hash finalizes a tree root
FLAG_PARENT = 0x02    This hash combines two child hashes (internal node)
FLAG_CHUNK  = 0x04    This hash derives a leaf chaining value
```

Valid combinations:

| Context | Flags | Value |
|---|---|---|
| Plain sponge hash | (none) | 0x00 |
| Non-root leaf | CHUNK | 0x04 |
| Root leaf (single-chunk file) | CHUNK \| ROOT | 0x05 |
| Non-root internal node | PARENT | 0x02 |
| Root internal node (tree root) | PARENT \| ROOT | 0x03 |

Flags encode what the hash IS, not what it contains.

## Domain Tags (state[11])

```
DOMAIN_HASH             = 0x00    Plain hash (default)
DOMAIN_KEYED            = 0x01    Keyed hash (MAC)
DOMAIN_DERIVE_KEY_CTX   = 0x02    Key derivation — context phase
DOMAIN_DERIVE_KEY_MAT   = 0x03    Key derivation — material phase
```

Domain tags are set before the first absorption and never modified. Orthogonal to flags.

## Sponge Operation

```
Initialize:  state ← [0; 16]
             state[11] ← domain_tag

Absorb:      for each 8-element block of padded input:
               state[0..8] += block        (Goldilocks field addition, element-wise)
               state ← permute(state)

Finalize:    state[10] ← total_input_bytes
             state ← permute(state)

Squeeze:     output ← state[0..8]           (8 elements = 64 bytes)
```

Absorption uses Goldilocks field addition (mod p), not XOR and not wrapping addition. This preserves the algebraic structure that Poseidon2's security proof relies on.

## Operational Semantics

Every use of the sponge in the Hemera stack:

- **Particle addressing.** Small content (up to 4096 bytes) is hashed directly through the sponge. Large content is split into 4096-byte chunks, each chunk produces a chaining value via the sponge, and the chaining values are combined in a binary Merkle tree whose root is the particle address.

- **Cyberlink identity.** A cyberlink is identified by the sponge hash of its canonical encoding. The hash commits to the link's source, destination, and tag particles.

- **Merkle proofs.** Internal nodes combine two child hashes using the sponge with FLAG_PARENT set. Verification recomputes the path from leaf to root using the same sponge configuration.

- **Incremental hashing.** The counter field (state[8]) tracks chunk position, enabling incremental computation. A file can be hashed chunk-by-chunk without buffering the entire input.

- **Streaming verification.** Chunks arrive over the network with their Merkle proof. Each chunk is verified independently using the sponge — the verifier never needs the full file.

- **MMR peaks (AOCL in BBG Layer 4).** Append-only commitment lists use a Merkle Mountain Range. Each peak is a sponge hash with FLAG_PARENT | FLAG_ROOT over its subtree.

- **NMT commitments.** Namespace Merkle Trees store namespace bounds in the capacity region (state[12..14]). Internal nodes propagate min/max namespace from children. The sponge absorbs child hashes while the capacity carries namespace metadata.

- **FRI/WHIR polynomial commitments.** Polynomial commitment schemes use the sponge as a Fiat-Shamir transcript. The field-native property eliminates the conversion overhead between hash output and field elements.

- **Field-native computation.** The Poseidon2 sponge operates directly over the Goldilocks field, requiring approximately 1,200 constraints per hash in a STARK circuit. SHA-256 requires approximately 25,000 constraints for the same role.

- **Tri-kernel lookup.** The three kernel modes (consensus, settlement, data availability) use the sponge for all hash-addressed lookups, providing a single primitive across the entire stack.
