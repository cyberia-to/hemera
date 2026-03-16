---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Why Hemera, permanence constraint, design philosophy
---

# why hemera

Hemera is a cryptographic hash function built on six design principles. each principle is a deliberate departure from how the ZK ecosystem builds hash primitives today.

## permanence

every Poseidon2 deployment in production today — SP1, RISC Zero, Starknet, Plonky3, Miden — treats hashing as an execution-layer concern. trace commitments live for seconds. Merkle proofs are verified and discarded. parameters are updatable in the next software release.

[[cyber]] uses Hemera as an identity-layer primitive. a [[particle]]'s Hemera hash is its permanent address in the [[cybergraph]]. every [[cyberlink]] references particles by hash. every [[neuron]]'s state commitment depends on hashes. the global state root depends on every shard.

| property | zkVM (SP1, RISC Zero) | cyber/core |
|---|---|---|
| hash lifetime | seconds to hours | decades to permanent |
| parameter update | software release | impossible without rehash |
| rehash cost | zero (ephemeral) | O(10¹⁵) operations |
| adversary budget | current computational | future computational + quantum |
| cost of parameter error | reissue proofs | lose the graph |

parameters chosen at genesis are permanent commitments. the threat model is not "what attacks exist today" but "what attacks will exist over the lifetime of the system." this asymmetry drives every decision in Hemera: wider state (t=16 vs t=12), more rounds (R_P=64 vs R_P=22), doubled capacity (c=8 vs c=4). the cost is ~38% slower native hashing and ~3.2× proving cost. Moore's law eliminates any constant-factor penalty in two years. a broken hash function is permanent.

there is no version byte. there is no escape hatch. if Hemera is ever broken, the response is full graph rehash — every particle, every cyberlink, every commitment. [[storage proofs]] make this possible. versioning headers do not save you — they waste bytes multiplied by 10¹⁵ particles.

## endofunction

a Hemera hash takes bytes in and produces 64 bytes out. those 64 bytes are valid input to the same function.

```
Hemera(Hemera(x) ∥ Hemera(y))    — type-checks
Hemera(content)                   — type-checks
Hemera(proof ∥ commitment)        — type-checks
```

the output space IS the input space. the algebra closes. composition, chaining, nesting — all work without conversion, without stripping headers, without encode/decode at boundaries.

this is why Hemera has no compression mode. a compression function takes 128 bytes in and produces 64 bytes out — a different type signature. introducing it would mean two functions sharing one output space, and every downstream system must track which function produced each address. the sponge is an endofunction. the compression function is not. we reject leaving the category.

this is why there are no CID headers. a Hemera output with a multicodec prefix is not raw bytes — it is a tagged value that must be stripped before hashing and reattached after. every Merkle tree node, every proof chain, every composition would require encode/decode at boundaries. headers break the endofunction property. raw 64 bytes compose cleanly.

## self-reference

Hemera generates her own round constants. the permutation structure (S-box x⁷, matrices M_E and M_I, round flow 4+64+4) is fully defined before constants exist. with all 192 constants set to zero, the permutation is already a well-defined nonlinear function — Hemera₀. feed the genesis seed `[0x63, 0x79, 0x62, 0x65, 0x72]` through Hemera₀ as a sponge, squeeze 192 field elements. those are the round constants. done.

```
algebraic structure → Hemera₀ → constants → Hemera
```

no SHA-256 in the dependency chain. no ChaCha20. no foreign primitive. the security of the constants reduces to the security of the structure itself. if Hemera₀ cannot produce pseudorandom output from a non-trivial input, then the S-box and MDS layers are already broken — and the final Hemera would be broken regardless of how constants were generated.

importing an external PRNG couples two independent security assumptions: "the PRNG is sound" AND "the permutation is sound." self-bootstrapping collapses them to one: "the permutation is sound." this is a strictly stronger argument.

## identity

the 64-byte Hemera output IS the particle address. not a representation of it. not a pointer to it. not an encoding that must be decoded.

no version prefix. no multicodec header. no length indicator. no framing of any kind. a content identifier identifies content — it does not identify itself. every byte spent saying "this is a Hemera hash" is a byte replicated 10¹⁵ times, a byte not spent on security, and a byte that implies the system might one day be something other than what it is.

every entity in [[nox]] — particle, edge, neuron, commitment, proof — has a 64-byte address in one flat namespace. no type tags. no interpretation hints. the same function produces all identifiers. domain separation lives in the hash input (different serialization, different capacity flags), not in type prefixes on the output. the output is pure, untagged, universal.

## unity

Hemera has exactly one primitive: the sponge. there is no compression mode. every hash — particle content, Merkle leaves, Merkle internal nodes, cyberlink edges, key derivation, polynomial commitments — passes through the same permutation, the same absorption, the same squeezing.

four tree types in [[cyber]] — content trees, MMR, NMT, FRI commitment trees — all use the same `hash_node`. the difference is what their leaves contain, not how they combine. one security analysis covers all trees. one implementation covers all trees. one optimization covers all trees.

domain separation happens through the capacity region of the sponge — 8 field elements that never receive input data. flags, counters, namespace bounds, domain tags all ride in capacity. different contexts produce different hashes because different capacity values enter the permutation, not because different functions are called. the sponge stays universal. the contexts stay structured.

the cost is 2× per Merkle internal node (two permutations via sponge vs one via compression). this is a permanent architectural decision traded against a temporary performance penalty. Moore's law eliminates 2× in two years. design ambiguity is permanent.

## the double seven

the [[Goldilocks field]] (p = 2⁶⁴ − 2³² + 1) forces the number 7 to appear twice, for two independent reasons.

the S-box must be a bijection over F_p, requiring gcd(d, p−1) = 1. for Goldilocks: p−1 = 2³² × (2³² − 1), which has factors 2, 3, 5. d=3 fails (gcd=3). d=5 fails (gcd=5). d=7 is the minimum invertible exponent.

the input encoding must map bytes to field elements without conditional reduction. the maximum 7-byte value is 2⁵⁶ − 1 < p. the maximum 8-byte value can exceed p. 7 bytes is the maximum whole-byte count that fits [0, p) unconditionally.

the same mathematical structure — the Goldilocks prime — constrains both the nonlinear layer and the encoding layer to the same number. this is not a design choice. it is a consequence of the field.

## the name

Hemera absorbs all parameters into a single, unambiguous identifier. if you say "Hemera," every parameter is determined: the field, the S-box, the width, the round counts, the rate, the capacity, the padding, the encoding, the output format. if you change any parameter, it is no longer Hemera.

the name exists because [[cyber]]'s hash function cannot afford ambiguity. "Poseidon2 with Goldilocks t=16 r=8 c=8 d=7 R_F=8 R_P=64" is a description. "Hemera" is an identity.
