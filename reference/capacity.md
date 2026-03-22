---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera capacity, structured capacity, domain separation
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
---

# structured capacity

the Hemera sponge state is 16 Goldilocks field elements. the first 8 (rate) absorb input and produce output. the last 8 (capacity) carry structural context — they are never overwritten by input data. this is how Hemera achieves domain separation without multiple modes.

## layout

```
state[0..8]   rate       input absorption / output squeezing
state[8..16]  capacity   structural context, never touched by input

state[8]      counter    chunk position in file (0-based, u64)
state[9]      flags      structural role (bitfield)
state[10]     msg_length total input byte count (sponge finalization)
state[11]     domain_tag API mode selector
state[12]     ns_min     namespace lower bound (NMT only, zero otherwise)
state[13]     ns_max     namespace upper bound (NMT only, zero otherwise)
state[14]     reserved   must be zero
state[15]     reserved   must be zero
```

## flags (state[9])

three single-bit flags, combined via bitwise OR:

```
FLAG_ROOT   = 0x01    this hash finalizes a tree root
FLAG_PARENT = 0x02    this hash combines two child hashes (internal node)
FLAG_CHUNK  = 0x04    this hash derives a leaf chaining value
```

valid combinations:

| context | flags | value |
|---|---|---|
| plain sponge hash | (none) | 0x00 |
| non-root leaf | CHUNK | 0x04 |
| root leaf (single-chunk file) | CHUNK \| ROOT | 0x05 |
| non-root internal node | PARENT | 0x02 |
| root internal node (tree root) | PARENT \| ROOT | 0x03 |

flags encode what the hash IS, not what it contains. a flag combination that does not appear in the table is invalid.

## domain tags (state[11])

```
DOMAIN_HASH             = 0x00    plain hash (default)
DOMAIN_KEYED            = 0x01    keyed hash (MAC)
DOMAIN_DERIVE_KEY_CTX   = 0x02    key derivation — context phase
DOMAIN_DERIVE_KEY_MAT   = 0x03    key derivation — material phase
```

domain tags are set before the first absorption and never modified. they are orthogonal to flags — a keyed hash of a Merkle leaf would have `state[9] = FLAG_CHUNK` and `state[11] = DOMAIN_KEYED`.

## counter (state[8])

the counter tracks chunk position within a file. chunk 0 gets counter 0, chunk 1 gets counter 1, and so on. the counter prevents chunk reordering: the same data at position 0 and position 5 produces different chaining values.

the counter is set during the structural binding pass of `hash_leaf` (see [[tree]]). plain sponge hashes leave it at zero.

## message length (state[10])

total input byte count, stored during sponge finalization. this prevents length extension attacks and distinguishes messages of different lengths that would otherwise produce the same padded block.

## namespace bounds (state[12..14])

used only by NMT (Namespace Merkle Tree) nodes. `ns_min` and `ns_max` commit the namespace range of a subtree into the hash. when both are zero, `hash_node_nmt` reduces to `hash_node`.

the verifier checks: `parent.ns_min ≤ left.ns_max < right.ns_min ≤ parent.ns_max` (for sorted NMT). namespace bounds enable completeness proofs — cryptographic proof that nothing was withheld for a given namespace.

## how capacity provides domain separation

different contexts produce different hashes because different capacity values enter the permutation — not because different functions are called:

```
plain hash:      state[9] = 0x00, state[11] = 0x00
keyed hash:      state[9] = 0x00, state[11] = 0x01
leaf chunk:      state[8] = counter, state[9] = 0x04
root chunk:      state[8] = counter, state[9] = 0x05
internal node:   state[9] = 0x02
root node:       state[9] = 0x03
NMT node:        state[9] = 0x02, state[12] = ns_min, state[13] = ns_max
```

capacity fields are mixed into every permutation output. two hashes with identical rate input but different capacity values produce completely different outputs. this is how Hemera maintains one function, one mode, and still prevents cross-context collisions.

## security invariant

the capacity region is never XORed with input data. input absorption touches only `state[0..8]`. capacity values are set by the API (flags, counter, domain tag, namespace bounds) or by finalization (message length). the permutation mixes rate and capacity together — but input never directly overwrites capacity.

this separation is what makes the flags, counters, and domain tags trustworthy. an attacker who controls the input cannot set capacity values. capacity is controlled by the protocol, not by the data.