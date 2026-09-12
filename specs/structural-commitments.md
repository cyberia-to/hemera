---
tags: hemera, identity, verification
crystal-type: spec
crystal-domain: crypto
status: experimental
---
# Structural commitments

Hemera has three layers: field permutation and byte sponge; format-independent
structural commitments; format adapters such as `.cyb`. The advantage of the
upper layers is stable addressable structure with composable openings. They add
functionality to a permutation; their security still depends on the primitive.

This is an additive experimental construction, not a migration of `root_hash`,
fixed-chunk proofs or existing particle addresses. Its encoding is reviewable but
not frozen for mainnet. Do not treat these roots as existing particle IDs.

## Encoding

All frames start with ASCII `hemera-struct-v1` followed by NUL and one tag byte.
All integers are u64 little-endian. Hashes are the existing canonical 32-byte
Hemera output. `H` is the plain byte sponge; structural domains are in the
message, without allocating new capacity lanes.

| tag | payload | result |
|---|---|---|
| 0 | byte length, exact bytes | blob ContentId |
| 1 | left item count, right item count, left digest, right digest | internal digest |
| 2 | no payload | empty internal digest |
| 3 | total item count, internal digest | SequenceId |
| 4 | ContentId of kind bytes, SequenceId of fields | record ContentId |

A sequence is left-balanced: for n>1 split at the largest power of two strictly
less than n. A singleton's internal digest is its ContentId; an empty sequence
uses tag 2. Every sequence, including a singleton, gets the tag-3 wrapper.
`record(kind, fields)` uses a byte string kind and ordered ContentId fields.
These are nominal structural domains, not a frozen content-type registry.

`record_from_sequence` reconstructs a record from its kind and field sequence
commitment. `verify_record_field` verifies an opening against a record ID;
the caller additionally checks that index/count match the requested schema field.
Both ID types support canonical decoding from 32-byte wire values.

Blob identity is independent of position. Positions belong to sequence proofs.
Repeated blobs share ContentId; swapping distinct children changes the framed
sequence input. Security statements are conditional on collision resistance,
not assertions that different inputs can never collide.

## Inclusion proofs

A proof contains index, total item count and sibling digests in leaf-to-root
order. The verifier derives directions and subtree sizes from index/count,
requires exactly the derived path length, rejects noncanonical field encodings,
reconstructs every branch and compares the final tag-3 commitment with the
caller-supplied trusted SequenceId. There are at most 64 siblings.

Verification commits to sequence cardinality, position and content ID. Verifying
bytes additionally requires recomputing their blob ID. A valid opening proves
membership in the committed sequence; it does not prove that an untrusted
producer parsed a container correctly or chose canonical CDC boundaries.

## `.cyb` adapter

The initial adapter consumes a validated extraction of preamble, declarations
and content sections. It does not parse TOML or scan binary delimiters. Each
entry carries its name, exact declaration bytes, content bytes and element size.
Names must be unique and nonempty, with no CR/LF; element size is 1..64 and
content length must be divisible by it. These checks are runtime errors.

Every section uses CDC to obtain position-independent blob IDs; its ordered
chunk sequence binds positions. The section is the record
`cyb.section.v1(element-size blob, byte-length blob, chunk-sequence-ID blob)`.
Preamble/declaration use element size 1. An entry is the record
`cyb.entry.v1(name blob, declaration section, content section)`.
A container is `cyb.container.v1(preamble section, entry-sequence-ID blob)`.

Section IDs are identical standalone and embedded, including singleton cases.
Names, roles, order, lengths and element sizes are bound explicitly. The parser
must establish that these fields exactly reflect the source before the output
can be called a `.cyb` particle identity. Byte-for-byte container reconstruction
and the handling of omitted delimiters remain parser/spec obligations.

## Proof obligations and migration gates

1. Framing is injective before hashing; node/empty/root/record frames are disjoint.
2. Sequence proofs are complete and collision-conditionally binding for index/count.
3. CDC partitions bytes without gaps, overlap or element splits; termination and
   arithmetic/resource bounds hold. Worst-case bounded re-synchronization is
   not claimed; equal-fingerprint runs give a counterexample.
4. Parser extraction is deterministic and unambiguous, rejects malformed input,
   and preserves the chosen identity equivalence. In particular settle TOML
   multiline values, duplicate names, trailing data and delimiter reconstruction.
5. Streaming/batch openings use this same commitment or a verified mapping to it.
6. Freeze primitive, encoding and cross-language vectors together before migration.

The structural modules do not allocate internally. SequenceBuilder keeps at
most 64 subtree digests. InclusionProof has a fixed 64-digest array and a used
length; unused slots are not part of the logical proof. CDC iterates borrowed
bytes with bounded lookahead. The container caller provides a scratch slice of
names for duplicate detection (at least one slot per entry); it is overwritten
and sorted. No fixed maximum number of container sections is introduced.
Input parsing, network streaming and batch openings remain subsequent work.

Run `cargo run -p cyber-hemera --example structural_identity` for a complete
example of changing container metadata, preserving a section/chunk ID and
verifying an opening at a different position.
