---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: capacity explanation, structured capacity rationale, domain separation
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
---

# structured capacity: one function, unlimited contexts

## the problem

Hemera must produce different hashes for different structural contexts. a Merkle leaf and a Merkle internal node might receive identical input bytes, but they must produce different outputs — otherwise an attacker could substitute a leaf for a node and forge proofs. a keyed hash must differ from a plain hash. a chunk at position 0 must differ from the same chunk at position 5.

the conventional solution: separate functions. `hash_content()`, `hash_leaf()`, `hash_node()`, `hash_keyed()`, `hash_derive_key()`. five functions, five security analyses, five implementations, five optimization targets. every downstream system must track which function produced each address.

Hemera's solution: one function, structured capacity.

## what capacity is

the sponge state has 16 Goldilocks field elements. the first 8 are the **rate** — they absorb input and produce output. the last 8 are the **capacity** — they carry structural context and are never overwritten by input data.

```
┌─────────────────────────────────────────────────────┐
│  state[0]  state[1]  ...  state[7]                  │  rate: input goes here
├─────────────────────────────────────────────────────┤
│  state[8]  state[9]  ...  state[15]                 │  capacity: context goes here
└─────────────────────────────────────────────────────┘
```

the capacity slots carry six pieces of context:

| slot | name | set by | purpose |
|---|---|---|---|
| state[8] | counter | `hash_leaf` | chunk position in file (prevents reordering) |
| state[9] | flags | tree API | structural role: leaf, node, root (prevents confusion) |
| state[10] | msg_length | sponge finalization | total input bytes (prevents length extension) |
| state[11] | domain_tag | API mode | plain / keyed / key derivation (prevents cross-mode collision) |
| state[12] | ns_min | NMT API | namespace lower bound (enables completeness proofs) |
| state[13] | ns_max | NMT API | namespace upper bound (enables completeness proofs) |
| state[14–15] | reserved | — | must be zero (future extensibility) |

## how it works

before the first absorption, the API writes context into the capacity slots. then the permutation runs normally. the permutation does not distinguish rate from capacity — the S-box, the MDS layers, the round constants operate on all 16 elements equally. after one permutation, the capacity values have influenced every element of the state.

```
step 1:  API writes flags, counter, domain_tag into capacity
step 2:  input bytes are absorbed into rate (state[0..8])
step 3:  permutation mixes all 16 elements together
step 4:  output is squeezed from rate (state[0..8])
```

the output depends on both the input (rate) and the context (capacity). identical input with different context → completely different output. this is domain separation — enforced by the permutation's mixing, not by convention.

a concrete example: hashing the bytes `[1, 2, 3]` as a plain hash vs as a Merkle leaf chunk at position 5:

```
plain hash:   rate = encode([1,2,3]),  capacity = [0, 0x00, 0, 0x00, 0, 0, 0, 0]
leaf chunk:   rate = encode([1,2,3]),  capacity = [5, 0x04, 0, 0x00, 0, 0, 0, 0]
                                                   ^  ^^^^
                                            counter  FLAG_CHUNK
```

the same input enters the rate. different capacity values enter the permutation. the outputs are completely unrelated — as unrelated as two random 32-byte strings.

## why it is secure

the sponge construction's security proof rests on one invariant: **the attacker cannot control the capacity.** input absorption touches only state[0..8]. an attacker who chooses the input can set any rate values they want, but they cannot reach state[8..15]. capacity is set by the protocol.

the permutation mixes rate and capacity together — that is its job. but the mixing is one-way from the attacker's perspective: they can influence the rate, and the permutation propagates that influence into the capacity, but they cannot choose what the capacity starts as. the starting capacity is protocol-controlled context: flags, counter, domain tag.

this is stronger than prefix-based domain separation (prepending a tag byte to the input). a prefix lives in the rate — it occupies the same space as input data. an attacker who controls the input controls the prefix too. capacity lives outside the input space. the attacker cannot reach it regardless of what input they craft.

collision resistance is bounded by capacity size: 2^(c×32) for c capacity elements of 64 bits each. with c = 8 elements = 512 bits of capacity, collision resistance is 2^256 (birthday bound). this is the theoretical maximum for a 512-bit output — Hemera achieves it because the full capacity is unreachable by the attacker.

## what it costs

**zero extra permutations.** the state is always 16 elements. the permutation always processes all 16 elements. setting a capacity value is a single write to a state slot before the first permutation. no additional rounds, no additional computation, no additional constraints in a STARK circuit.

**half the throughput.** this is the real cost. with t = 16 elements total and c = 8 capacity, only r = 8 elements absorb input (56 bytes per block). if the entire state were rate (c = 0), throughput would double to 112 bytes per block. but c = 0 means zero collision resistance — the sponge security proof requires capacity > 0.

| capacity | rate | throughput | collision resistance |
|---|---|---|---|
| c = 0 | r = 16 | 112 B/block | 0 bits (broken) |
| c = 4 | r = 12 | 84 B/block | 128 bits |
| c = 8 | r = 8 | 56 B/block | 256 bits |

Hemera uses c = 8 because particle addresses are permanent. 128-bit collision resistance (c = 4) is standard for ephemeral proofs — adequate when commitments live for seconds. permanent addresses require the full 256-bit level. the throughput cost (56 vs 84 bytes per block, ~33% reduction) is the price of permanence.

in practice, the throughput cost is smaller than it appears. a 4 KB chunk requires ⌈4096/56⌉ = 74 absorptions with c = 8 vs ⌈4096/84⌉ = 49 absorptions with c = 4. that is 74 vs 49 permutations — a 51% increase. but leaf hashing adds one binding permutation, so the total is 75 vs 50 — still 50% more. for Merkle internal nodes, the cost is 2 permutations regardless of capacity (two 32-byte child hashes fit in the rate at either capacity level). tree overhead is capacity-independent.

## why not separate functions

five functions for five contexts would work. each could be independently secure. but:

**one audit.** the capacity-based approach has one permutation, one security proof, one implementation. five functions means five attack surfaces, five chances for a subtle difference to create a vulnerability.

**one optimization.** a hardware accelerator for Hemera accelerates every context — leaf hashing, node hashing, keyed hashing, key derivation, NMT. five functions means five accelerators, or one generic accelerator that is optimal for none.

**one circuit.** in a STARK, the permutation is a fixed-width gadget. the capacity values are witness inputs — the prover supplies them, the verifier checks them. the circuit does not branch on context. five functions would mean five gadgets or a multiplexer, increasing circuit complexity.

**extensibility.** two capacity slots are reserved. future contexts (new tree types, new domain tags, new structural roles) require no changes to the permutation, the circuit, or the hardware. they only require writing a new value into a capacity slot.

## why capacity produces zero storage overhead

capacity encodes real information — flags, counters, domain tags, namespace bounds. this is structural data. yet it adds zero bytes to what is stored. the hash output is still 32 bytes. where did the information go?

it went into the hash output through the permutation's mixing. the capacity values influence the 32-byte result, but they are not stored alongside it. the output does not carry the capacity values — it is pure, untagged, 32 bytes.

the information is recoverable from context. when verifying a Merkle proof, the protocol already knows each intermediate node was hashed with `FLAG_PARENT` — because the Merkle verification algorithm says so. when verifying a leaf chunk, the protocol knows the counter value — it is the chunk's position in the file. when calling `hash()` on raw bytes, `domain_tag = 0x00` — because that is what the API specifies.

```
leaf chunk at position 5:
  state[8]  = 5          ← counter (known: it is the 5th chunk)
  state[9]  = 0x04       ← FLAG_CHUNK (known: you are hashing a leaf)
  state[10] = 4096       ← msg_length (known: full chunk = 4096 bytes)
  state[11] = 0x00       ← domain_tag (known: it is a tree leaf)

  output: 32 bytes       ← this is ALL that gets stored
```

every capacity value is **reconstructible** from the context in which the hash appears. the verifier does not need a tag saying "this hash was computed with FLAG_PARENT=0x02" — the protocol already knows, because the protocol is what told it to verify a parent node in the first place.

this is the opposite of CID headers. a multihash stores type information *in the output*: `0x1220...` means "SHA-256, 32 bytes." that costs bytes on every hash — bytes replicated across every particle address, every Merkle node, every cyberlink in the graph. capacity stores type information *in the computation*. the output is pure. the protocol reconstructs the capacity at verification time from the same context that triggered the verification.

one way to think about it: capacity is like a salt in password hashing. a salted password hash is different from an unsalted one, but the salt is not stored inside the hash output — it is stored separately or derived from context. capacity values are derived from protocol context. they shape the output without inflating it.

this is the "identity" principle in action: the 32-byte output IS the address. no version prefix, no type tag, no framing. domain separation lives in the hash input (capacity), not in type prefixes on the output. the output is universal.

## the design insight

capacity is not a feature added to the sponge. it is the sponge. the rate/capacity split is what makes a sponge a sponge — what distinguishes it from a raw permutation applied directly to input. Hemera's contribution is making the capacity **structured**: instead of treating it as opaque hidden state, Hemera assigns meaning to each slot (counter, flags, domain tag, namespace bounds) and uses those meanings to achieve domain separation, tree binding, and protocol-level context — all without leaving the sponge.

the permutation does not know what it is hashing. it sees 16 field elements and mixes them. the capacity knows what is being hashed — and ensures that different contexts can never collide.