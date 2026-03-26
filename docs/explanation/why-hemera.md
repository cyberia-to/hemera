---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Why Hemera, permanence constraint, design philosophy
diffusion: 0.00010722364868599256
springs: 0.002190885472169872
heat: 0.0015239469650863775
focus: 0.0010156668590112204
gravity: 0
density: 0.66
---

# why hemera

Hemera is a particle-addressing primitive: a permutation and a tree, fused into one construction. the permutation provides cryptographic strength. the tree provides scale, streaming, and structure. together they form a closed system — from raw bytes to permanent identity to verifiable proof — with nothing else in the dependency chain.

Hemera uses Poseidon2 because it operates directly over the [[Goldilocks field]] — the same field that runs the STARK prover, the FHE encryption, the neural inference, and the quantum circuits in [[cyber]]. no field conversion at any boundary. ~736 constraints per hash in a STARK circuit vs ~50,000-100,000 for BLAKE3. the hash is native to the field. the field is native to the proof system. the proof system is native to the execution layer. one algebraic substrate from content to commitment to proof to consensus.

eight design principles shape every decision. each is a deliberate departure from how the ZK ecosystem builds hash primitives today.

## permanence

every Poseidon2 deployment in production today — SP1, RISC Zero, Starknet, Plonky3, Miden — treats hashing as an execution-layer concern. trace commitments live for seconds. Merkle proofs are verified and discarded. parameters are updatable in the next software release.

[[cyber]] uses Hemera as an identity-layer primitive. a [[particle]]'s Hemera hash is its permanent address in the [[cybergraph]]. every [[cyberlink]] references particles by hash. every [[neuron]]'s state commitment depends on hashes. the global state root depends on every shard.

| property | zkVM (SP1, RISC Zero) | cyber/core |
|---|---|---|
| hash lifetime | seconds to hours | decades to permanent |
| parameter update | software release | impossible without rehash |
| rehash cost | zero (ephemeral) | O(10²⁴) operations |
| adversary budget | current computational | future computational + quantum |
| cost of parameter error | reissue proofs | lose the graph |

parameters chosen at genesis are permanent commitments — and this applies to the tree equally. the 4 KB chunk size, the binary left-balanced shape, the two-pass leaf construction, the capacity flag layout — all are as permanent as the round counts. the threat model is not "what attacks exist today" but "what attacks will exist over the lifetime of the system." this asymmetry drives every decision: wider state (t=16 vs t=12), more rounds (R_P=16 vs R_P=22), doubled capacity (c=8 vs c=4). the cost is ~38% slower native hashing and ~3.2× proving cost. Moore's law eliminates any constant-factor penalty in two years. a broken hash function is permanent.

there is no version byte. there is no escape hatch. if Hemera is ever broken, the response is full graph rehash — every particle, every cyberlink, every commitment. [[storage proofs]] make this possible. versioning headers do not save you — they waste bytes multiplied by 10²⁴ cyberlinks.

## the tree

the permutation hashes bytes. the tree addresses content. without the tree, Hemera is another Poseidon2 instantiation. with the tree, it is a complete particle-addressing system: any byte sequence — 1 byte or 1 exabyte — receives a single 32-byte permanent identity.

```
bytes → 4 KB chunks → hash_leaf (sponge + binding) → binary Merkle tree → 32-byte address
```

the tree is not bolted onto the permutation. the permutation was designed for the tree. the 8-element capacity region exists so the tree can carry structural context — flags, counters, namespace bounds — through the same permutation that hashes content. the sponge did not come first and the tree second. they were co-designed.

three properties emerge from the tree that the permutation alone cannot provide:

**verified streaming.** chunks arrive over the network with their Merkle proof. each chunk is verified independently — the receiver never needs the full file. a 1 TB particle is verifiable one 4 KB chunk at a time. this is what makes planetary-scale content delivery possible.

**incremental computation.** modifying one chunk requires rehashing one leaf (75 permutations) plus the path from leaf to root (log₂(N) nodes × 2 permutations). for a 1 GB file: 111 permutations to update any single chunk. the tree makes content mutation O(log N) instead of O(N).

**structural domain separation.** the capacity region carries flags that encode what a hash IS: a leaf chunk (CHUNK), an internal node (PARENT), a tree root (ROOT). the counter binds chunk position. namespace bounds enable completeness proofs. all of this enters the permutation without changing it — the same S-box, the same matrices, the same round constants. the tree's structure rides in the permutation's capacity.

the two-pass leaf construction separates content hashing from structural binding. pass one: sponge the chunk data into a base hash. pass two: one permutation with the base hash in the rate and position/flags in the capacity. the base hash is cacheable — the same chunk at different positions reuses the expensive 74-absorption pass. the binding permutation is cheap. the sponge stays pure: no tree metadata in its input stream.

## endofunction

a Hemera hash takes bytes in and produces 32 bytes out. those 32 bytes are valid input to the same function. the tree is the endofunction in action:

```
hash_node(Hemera(x), Hemera(y))  — type-checks, builds trees
Hemera(Hemera(x) ∥ Hemera(y))    — type-checks, chains proofs
Hemera(content)                   — type-checks, addresses content
```

the output space IS the input space. the algebra closes. composition, chaining, nesting — all work without conversion, without stripping headers, without encode/decode at boundaries. every Merkle node takes two 32-byte hashes and produces one 32-byte hash. the tree grows by applying the same function to its own outputs. this is closure.

this is why Hemera has no compression mode. a compression function takes 64 bytes in and produces 32 bytes out — a different type signature. introducing it would mean two functions sharing one output space, and every downstream system must track which function produced each address. the sponge is an endofunction. the compression function is not. we reject leaving the category.

this is why there are no CID headers. a Hemera output with a multicodec prefix is not raw bytes — it is a tagged value that must be stripped before hashing and reattached after. every Merkle tree node, every proof chain, every composition would require encode/decode at boundaries. headers break the endofunction property. raw 32 bytes compose cleanly.

## self-reference

Hemera generates her own round constants. the permutation structure (S-box x⁷, matrices M_E and M_I, round flow 4+16+4) is fully defined before constants exist. with all 144 constants set to zero, the permutation is already a well-defined nonlinear function — Hemera₀. feed the genesis seed `[0x63, 0x79, 0x62, 0x65, 0x72]` through Hemera₀ as a sponge, squeeze 144 field elements. those are the round constants. done.

```
algebraic structure → Hemera₀ → constants → Hemera
```

no SHA-256 in the dependency chain. no ChaCha20. no foreign primitive. the security of the constants reduces to the security of the structure itself. if Hemera₀ cannot produce pseudorandom output from a non-trivial input, then the S-box and MDS layers are already broken — and the final Hemera would be broken regardless of how constants were generated.

importing an external PRNG couples two independent security assumptions: "the PRNG is sound" AND "the permutation is sound." self-bootstrapping collapses them to one: "the permutation is sound." this is a strictly stronger argument.

## identity

the 32-byte Hemera output IS the particle address. not a representation of it. not a pointer to it. not an encoding that must be decoded. the tree is what makes this work at scale — a 1 TB file and a 5 byte message both resolve to the same type: 32 raw bytes.

no version prefix. no multicodec header. no length indicator. no framing of any kind. a content identifier identifies content — it does not identify itself. every byte spent saying "this is a Hemera hash" is a byte replicated 10²⁴ times, a byte not spent on security, and a byte that implies the system might one day be something other than what it is.

every entity in [[nox]] — particle, edge, neuron, commitment, proof — has a 32-byte address in one flat namespace. no type tags. no interpretation hints. the same function produces all identifiers. domain separation lives in the hash input (different serialization, different capacity flags), not in type prefixes on the output. the output is pure, untagged, universal.

## unity

one permutation. one sponge mode. one tree primitive. every hash — particle content, Merkle leaves, Merkle internal nodes, cyberlink edges, key derivation, polynomial commitments — passes through the same permutation, the same absorption, the same squeezing. every tree — content, MMR, NMT, WHIR — passes through the same `hash_node`.

```
                    hash_node(left, right, is_root)
                                │
          ┌─────────────┬───────┴───────┬──────────────┐
          │             │               │              │
     Content tree      MMR             NMT       WHIR commit
     (file hash)    (append log)    (DA proofs)  (poly commit)
```

four tree types. one function. the difference is what their leaves contain, not how they combine. one security analysis covers all trees. one implementation covers all trees. one optimization covers all trees. one hardware accelerator covers all trees.

domain separation happens through the capacity region of the sponge — 8 field elements that never receive input data. flags, counters, namespace bounds, domain tags all ride in capacity. different contexts produce different hashes because different capacity values enter the permutation, not because different functions are called. the sponge stays universal. the contexts stay structured. the tree carries the context. the permutation does not know what it is hashing — and does not need to.

the cost is 2× per Merkle internal node (two permutations via sponge vs one via compression). this is a permanent architectural decision traded against a temporary performance penalty. Moore's law eliminates 2× in two years. design ambiguity is permanent.

but unity extends beyond hashing. Hemera's Goldilocks field is the same field that runs every computational domain in [[cyber]]:

```
         ┌──────────┬──────────┬──────────┬──────────┬──────────┐
         │  Hashing │  Proving │    FHE   │  Neural  │ Quantum  │
         │ (Hemera) │  (STARK) │   (LWE)  │(inference)│(circuits)│
         └────┬─────┴────┬─────┴────┬─────┴────┬─────┴────┬─────┘
              │          │          │          │          │
              └──────────┴──────────┴──────────┴──────────┘
                        Goldilocks field (p = 2⁶⁴ − 2³² + 1)
```

BLAKE3 hashes bytes. to enter a STARK circuit, its output must be decomposed into field elements — ~50,000-100,000 constraints per hash. Hemera's output IS field elements. the hash feeds directly into the prover, the polynomial commitment, the WHIR query, the consensus check. no conversion. no impedance mismatch. the hash is arithmetic. the proof is arithmetic. they share the same arithmetic.

[[trident]] demonstrates this with [[Trinity]]: five computational domains — neural inference, homomorphic encryption, cryptographic hashing, programmable bootstrapping, quantum circuits — executing inside one STARK trace, all over the Goldilocks field. LWE ciphertexts are Goldilocks vectors. neural weights are Goldilocks elements. Hemera round constants are Goldilocks elements. WHIR commitments are Hemera hashes. the field is the universal substrate. Hemera is its hash function.

this is why Poseidon2 and not BLAKE3, not Keccak, not SHA-256. those are faster on CPUs. Hemera is faster in proofs — by a factor of 68-136×. when every particle address, every cyberlink, every state commitment must be proven, the proof cost dominates. the field-native hash is the only hash that makes planetary-scale proving feasible.

## beauty

Hemera's parameters are not arbitrary. they emerge from the Goldilocks prime as mathematical consequences — and they are beautiful.

**the double seven.** the number 7 appears twice, for two independent reasons. the S-box must be a bijection over F_p, requiring gcd(d, p−1) = 1. for Goldilocks: p−1 = 2³² × (2³² − 1), which has factors 2, 3, 5. d=3 fails (gcd=3). d=5 fails (gcd=5). d=7 is the minimum invertible exponent. separately, the input encoding must map bytes to field elements without conditional reduction. the maximum 7-byte value is 2⁵⁶ − 1 < p. the maximum 8-byte value can exceed p. 7 bytes is the maximum whole-byte count that fits [0, p) unconditionally. the same mathematical structure — the Goldilocks prime — constrains both the nonlinear layer and the encoding layer to the same number. this is not a design choice. it is a consequence of the field.

**the powers of two.** every structural parameter in Hemera is a power of 2:

```
t  = 16 = 2⁴     state width
R_F = 8 = 2³     full rounds
R_P = 16 = 2⁴    partial rounds
r  = 8  = 2³     rate elements
c  = 8  = 2³     capacity elements
```

this is not numerology. powers of 2 mean bit shifts instead of division, aligned memory instead of padding, native SIMD lanes instead of masking. R_P = 16 partial rounds fit in 128 bytes of round constants (16 × 8 bytes). the state width t = 16 maps to one AVX-512 register or four NEON registers. r = 8 rate elements absorb in a single aligned load. every parameter falls on a hardware boundary.

the 4 KB chunk size (2¹²) aligns with page size, disk sector size, and network MTU conventions. the 56-byte input rate (7 × 8) is the one parameter that is not a power of 2 — forced by the double seven. every parameter that could be a power of 2, is.

the Goldilocks prime itself embodies this: p = 2⁶⁴ − 2³² + 1. reduction is a subtract and an add — no division, no trial loop. the prime's structure makes the field fast. the field's structure makes the parameters clean. the parameters' structure makes the implementation efficient. beauty and efficiency are the same thing.

## the name

Hemera absorbs all parameters into a single, unambiguous identifier. if you say "Hemera," every parameter is determined: the field, the S-box, the width, the round counts, the rate, the capacity, the padding, the encoding, the output format, the chunk size, the tree shape, the flag layout. if you change any parameter, it is no longer Hemera.

the name exists because [[cyber]]'s hash function cannot afford ambiguity. "Poseidon2 with Goldilocks t=16 r=8 c=8 d=7 R_F=8 R_P=16 and a binary Merkle tree with 4 KB chunks" is a description. "Hemera" is an identity.