---
tags: cyber, hemera, spec, prop
crystal-type: prop
crystal-domain: cyber
status: accepted
breaks_hash: yes
alias: semantic hashing, structured identity, section tree, element-aligned CDC, particle identity, semantic index
---

# semantic hashing — structured identity and independently addressable content

## status

Architectural direction accepted. The first additive implementation is in
`commitment` and `cyb_commitment`; its experimental encoding is specified in
[structural-commitments](../specs/structural-commitments.md). Existing particle
addresses and fixed-chunk proofs have not migrated to this construction.

## architecture

```
Goldilocks permutation / byte sponge
  → format-independent blobs, ordered sequences and nominal records
    → .cyb extraction adapter
```

Hemera provides an identity protocol above the cryptographic permutation:
section/chunk reuse, addressable structure and verifiable openings. This is
additional functionality, not evidence that the underlying permutation is more
secure than Poseidon2.

A tensor has its own address only when the container exposes it as its own
section. A monolithic embedded model file remains one section unless its adapter
extracts a finer structure. No inference of semantic meaning from arbitrary
bytes or file extension occurs.

## accepted identity rules

- ContentId identifies exact blob bytes or a nominal record, independent of
  placement. Equal chunks share an ID even at different indices.
- SequenceId commits ordered children and cardinality. Position belongs to
  the opening, not the content ID.
- Blob, internal node, empty sequence, sequence root and record have distinct
  framed inputs. Node counts bind the shape. Every sequence has a root wrapper.
- A section ID remains identical standalone and embedded; singleton cases do
  not change its identity. Section metadata binds element size and byte length.
- Container entries bind name, declaration and content with distinct roles.
  Preamble edits preserve independent content IDs; reordering changes the
  container commitment while preserving individual entries.
- All security statements are conditional on collision resistance of H and
  the correctness of the adapter's extraction.

The previous design used the same parent domain for nested section and outer
nodes, made chunk hashes position-dependent and threaded ROOT differently into
singleton sections. Those rules are replaced by the structural framing contract.

## CDC boundary profile in the reference implementation

For element size S in 1..64 and n=len(data)/S, reject nonintegral n. No format
inference; absent element metadata means S=1. Preamble/declarations use S=1.

```
gear[i] = LE64(hash([i])[0..8])
fp(element) = XOR_k rotate_left(gear[element[k]], (11*k) mod 64)
W = next_power_of_two(max(64, floor(4096/S)))
minimum = W/2 elements
maximum = 2*W elements
```

Starting at element offset start, search the inclusive range
`[start+minimum-1, min(start+maximum-1,n-1)]`. End the chunk immediately after
the first minimum fingerprint. If fewer than minimum elements remain, take
the remainder. Repeat until n. Empty content has no chunks in the new
structural layer, and has a distinct empty sequence and section commitment.

The bounds apply to elements; byte sizes are multiplied by S. Equal fingerprints
choose the earliest candidate, producing minimum-sized chunks. Average size
and deduplication quality are empirical properties, not promised 4-KB constants.
Distinct rotation weights do not make a 64-bit XOR fingerprint collision-free.
This fingerprint selects boundaries; cryptographic integrity comes from the
commitments over the actual bytes.

There is **no worst-case 8192-byte re-synchronization guarantee**. In a uniform
byte section, insertion of one identical byte shifts the correspondence to old
content offsets for the entire remaining stream. The initial proposal's MinHash
optimality and bounded re-synchronization claims were not justified. Boundary
stability must be evaluated on real corpora and adversarial periodic content.

## adapter boundary and remaining decisions

The reference adapter takes extracted fields. It checks unique valid names,
element sizes and divisibility, but does not parse TOML or establish that the
caller extracted those fields from the original file correctly.

Before calling this the final `.cyb` particle identity:

1. Settle exact bytes versus canonical structured equivalence, including whether
   omitted delimiter bytes are uniquely reconstructible.
2. Define TOML multiline handling, duplicate fields/names, binary sizes, section
   order, delimiter matching, LF rules and trailing-data rejection in the actual
   `.cyb` format spec. Byte-pattern splitting alone cannot replace this contract.
3. Implement/verify parser extraction and bind manifests to extracted metadata.
4. Specify section/chunk openings against the final root, including how remote
   verification establishes canonical partitioning when required.
5. Move streaming, batch proofs and progress APIs to one documented identity
   contract, or prove the mapping between distinct roots.
6. Freeze primitive, frames, CDC profile and interoperability vectors before
   changing permanent particle addresses. Hash values currently remain provisional.

The existing `root_hash` uses legacy CDC leaf binding, while fixed-tree proof
APIs use `fixed_chunk_root`; neither is a structural SequenceId. The new types
make this distinction explicit. Migration is a separate protocol action.
