---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera content identifiers, raw CIDs, no headers, particle identity
diffusion: 0.00010722364868599256
springs: 0.0030383438859984337
heat: 0.0020874522638244853
focus: 0.0013826054429074057
gravity: 0
density: 1.31
---

# particle identifiers: raw bytes, no headers

## what a particle is

a [[particle]] in [[cyber]] is a standalone unit of knowledge. not a file, not a blob, not a document — a unit of knowledge. any sequence of bytes that has been hashed and addressed by the [[cybergraph]] becomes a particle. its Hemera hash is its permanent, unique identity. [[cyberlinks]] connect particles into a knowledge graph. [[neurons]] rank these connections. the entire system — ranking, consensus, proofs, storage — operates on particle addresses.

a particle's address is 64 raw bytes. that is all. there is no wrapper, no envelope, no metadata frame. the address IS the particle's identity in the graph.

## why no headers

IPFS pioneered self-describing content identifiers: a CID carries its own hash function code, codec, and version. this was the right design for a system that must interoperate across dozens of hash functions, serialization formats, and protocol versions.

[[cyber]] is a different system. it has one hash function (Hemera), one field (Goldilocks), one output size (64 bytes), one encoding (little-endian canonical), and one mode (sponge). there is nothing to negotiate, nothing to disambiguate, nothing to version. the system is the description.

| | IPFS CIDv1 | nox particle address |
|---|---|---|
| structure | version + codec + hash-code + length + digest | digest only |
| size | 36-38 bytes typical | 64 bytes fixed |
| hash agility | yes (identified by prefix) | no (one hash, permanent) |
| self-describing | yes | no — the system is the description |
| composable | no (must strip/reattach headers) | yes (endofunction) |

five reasons for no headers:

1. **overhead at scale.** 5 bytes × 10²⁴ cyberlinks = 5 ZB of metadata that describes nothing the system does not already know. this is not a rounding error — it is a petabyte-scale architectural tax paid forever, on every lookup, every proof, every edge, every packet.

2. **one hash function — nothing to disambiguate.** every address in [[nox]] is a Hemera output. a header saying "this is Hemera" adds exactly zero information. headers exist to answer questions. when there is only one answer, the question wastes space.

3. **headers create the illusion of upgradability.** a version byte implies the system can switch hash functions gracefully. it cannot. every existing particle address was produced by Hemera from specific bytes. changing the hash function means every address in the graph is invalid. the response is full rehash via [[storage proofs]], not graceful version negotiation. the header promises a capability the system will never exercise.

4. **endofunction closure.** `Hemera(Hemera(x) ∥ Hemera(y))` must type-check. the output of one hash must be valid input to the next without transformation. headers break this — prepending metadata to a hash output before feeding it back means the input includes non-content bytes. every Merkle tree node, every proof chain, every nested composition would require strip/reattach at boundaries. raw 64 bytes compose cleanly. tagged values do not.

5. **flat namespace.** every entity in [[nox]] — particle, edge, neuron, commitment, proof — has a 64-byte address in one flat namespace. domain separation lives in the hash input (different serialization, different capacity flags), not in type prefixes on the output. the output is pure, untagged, universal. a particle address and a cyberlink edge ID are the same type. the graph does not need type tags to function — it needs content to be addressable.

## the difference from IPFS

IPFS is a content-addressed filesystem for a heterogeneous internet. it must handle SHA-256, BLAKE2, BLAKE3, Poseidon, and whatever comes next. it must handle dag-pb, dag-cbor, raw, and dozens of codecs. CID headers are the price of this universality — and the correct price for that design.

[[cyber]] is a homogeneous knowledge graph where every operation — hashing, proving, ranking, consensus — happens in the same field. the hash output is a field element tuple. the proof system operates on field elements. the ranking engine operates on field elements. there is no codec negotiation because there is no codec boundary — it is field arithmetic from content to commitment to proof to consensus.

in IPFS, a CID crosses protocol boundaries (bitswap, graphsync, HTTP gateways) where each peer may support different hash functions. the header tells the peer how to verify. in cyber, every node runs the same hash function. there is nothing to tell.

## compatibility

interop with external systems happens at the network boundary, never inside the graph:

```
inbound:   [multicodec | multihash | digest] → strip → 64-byte address
outbound:  64-byte address → prepend [multicodec | multihash] → CIDv1
```

the translation is stateless and lossless. gateways add it. gateways strip it. the graph never sees it. translation is a gateway concern, not a protocol concern.

## principle

a content identifier identifies content. it does not identify itself. the 64 bytes ARE the identity — complete, self-sufficient, and universal. any byte spent saying "this is a Hemera hash" is a byte replicated 10²⁴ times, a byte not spent on security, and a byte that implies the system might one day be something other than what it is.