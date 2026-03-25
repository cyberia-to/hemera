---
status: draft
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# capacity typing — type integration into the hash

hemera has two reserved capacity slots (`state[14]`, `state[15]`). the protocol is frozen — permutation parameters, round constants, S-box are permanent. but capacity slots are protocol-level metadata, not cryptographic parameters. using reserved slots for type information is a compatible extension.

## motivation

every particle in the cybergraph has a type. currently type is external metadata — stored alongside the hash, not inside it. this means two particles with identical content but different types produce identical hashes. type confusion is possible: the same bytes interpreted as "image" and "executable" share a CID.

integrating type into capacity makes type intrinsic to identity. same content, different type → different hash. type becomes part of what the content IS, not a label attached after.

## proposal

use `state[14]` for a type tag:

```
state[14]     type_tag    content type identifier (u64, from a fixed registry)
state[15]     reserved    remains zero (future use)
```

### type registry

a fixed set of type identifiers, frozen at genesis:

```
TYPE_RAW        = 0x00    untyped bytes (default, backward compatible)
TYPE_PARTICLE   = 0x01    cybergraph particle (content-addressed node)
TYPE_CYBERLINK  = 0x02    cyberlink record (7-tuple)
TYPE_SIGNAL     = 0x03    signal (batch of cyberlinks + proof)
TYPE_POLYNOMIAL = 0x04    polynomial commitment data
TYPE_PROOF      = 0x05    zheng proof
TYPE_KEY        = 0x06    public key / neuron identity
TYPE_NULLIFIER  = 0x07    spent record nullifier
TYPE_COMMITMENT = 0x08    private record commitment
```

### backward compatibility

`TYPE_RAW = 0x00` is the default. existing hashes (with `state[14] = 0`) are type-raw hashes. all current hemera outputs remain valid. the extension is purely additive.

### API change

```
hash(bytes) → Hash                      // existing (type_tag = 0x00)
hash_typed(bytes, type_tag: u64) → Hash  // new: type-aware hash
```

`hash(bytes)` remains unchanged. `hash_typed(bytes, TYPE_PARTICLE)` produces a different hash than `hash_typed(bytes, TYPE_CYBERLINK)` even for identical bytes. domain separation through capacity — the same mechanism hemera already uses for flags, counter, and domain tags.

## consequences

### type confusion prevention

a particle hash and a cyberlink hash can never collide even if the underlying bytes are identical. the type tag enters the permutation through capacity — the same security mechanism that separates keyed from unkeyed hashes.

### verifiable typing

given a hash and a claimed type, verification is: recompute `hash_typed(content, claimed_type)` and compare. if the hash matches, the type is authentic. type is cryptographically bound to content.

### structural restriction

the type registry is finite and frozen at genesis. new types require a protocol-level decision. this is intentional — type inflation defeats the purpose. the registry should be small enough that every type is meaningful.

## interaction with other capacity fields

capacity fields are orthogonal:

```
state[8]  = counter      (chunk position)
state[9]  = flags        (structural role: root, parent, chunk)
state[10] = msg_length   (finalization)
state[11] = domain_tag   (API mode: hash, keyed, derive_key)
state[12] = ns_min       (NMT namespace lower)
state[13] = ns_max       (NMT namespace upper)
state[14] = type_tag     (content type) ← NEW
state[15] = reserved     (zero)
```

a typed keyed hash of a Merkle leaf would have: `state[9] = FLAG_CHUNK`, `state[11] = DOMAIN_KEYED`, `state[14] = TYPE_PARTICLE`. all three are independent — type does not interfere with existing domain separation.

## open questions

- should the type registry be extensible post-genesis? (recommendation: no — frozen registry, same as hemera parameters)
- should type_tag be a single u64 or split into (type_class: u32, type_variant: u32)? (simpler = better)
- interaction with NMT: does type_tag affect namespace sorting? (recommendation: no — type is per-hash, namespace is per-tree)
- should hash() eventually become hash_typed() with mandatory type? (recommendation: keep hash() as TYPE_RAW for backward compatibility, add hash_typed() as the typed entry point)
- cost impact: zero — same number of permutations, capacity values just enter the existing permutation. no additional constraints
