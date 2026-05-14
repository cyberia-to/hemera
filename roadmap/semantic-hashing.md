---
tags: cyber, hemera, spec, prop
crystal-type: prop
crystal-domain: cyber
status: accepted
breaks_hash: yes
alias: semantic hashing, structured identity, section tree, element-aligned CDC, particle identity, semantic index
---

# semantic hashing — permanent identity for structured content

## status

accepted. replaces fixed-4KB chunking in `particle_id` for all content.

`breaks_hash: yes` — every existing `root_hash` value changes. intentional and one-time.

---

## why this exists

### the address is the thing

in cyber, every particle has a permanent address derived from its content. that address is not an alias or a pointer — it IS the identity. content with the same address is the same thing, everywhere, forever. this is the foundation of the entire system: rank, stake, citations, reputation — all of it flows to addresses.

the address is computed by hashing the content. but hashing is not neutral. the choice of HOW to hash determines WHAT is addressable. and what is addressable determines what the system can reason about.

if the only addressable unit is "the whole file," then rank can only accumulate on whole files. you cannot reward the embedding weights independently of the attention weights. you cannot cite a specific tensor. you cannot say "this 4KB slice of data appears in 1000 models" — because that slice has no permanent address, it is just bytes inside a file.

the question is not whether to hash. the question is: at what granularity does identity live?

### the merkle tree is a semantic index

a hash tree is not just a verification structure. every internal node is a hash — a potential permanent address. in a content-addressed system, every node IS an identity. the structure of the tree determines what identities exist.

consider two constructions:

```
construction A: fixed 4KB chunks, flat tree
    particle_id
    ├── chunk_0  (bytes 0..4096)      ← address of an arbitrary window
    ├── chunk_1  (bytes 4096..8192)
    └── ...
```

```
construction B: semantically-structured tree
    particle_id
    ├── preamble                      ← address of "the model's global metadata"
    ├── tensor_0
    │   ├── declaration               ← address of "how tensor 0 is described"
    │   └── weights                   ← address of "tensor 0's actual values"
    │       ├── chunk_0               ← address of "first 4KB of tensor 0"
    │       └── ...
    └── tensor_1
        ├── declaration
        └── weights
```

construction A produces addresses that mean nothing. `chunk_1` is "bytes 4096 through 8192 of this file" — and if the frontmatter grows by one byte, chunk_1 contains completely different data in the next version.

construction B produces addresses that mean something. the address of `tensor_0.weights` is stable across model versions that don't change tensor_0. it accumulates rank from every model that uses those weights. it is citable, rankable, stakeable.

this is the central insight: **the hash tree is the semantic index of the cyber knowledge graph.** the structure of the tree determines the granularity at which meaning can accumulate.

### the streaming property

a file is read sequentially. with a semantically-structured hash tree, every semantic boundary you cross during reading produces a permanent address. by the time you finish reading the declaration of tensor_0, you have its address. you can query the cyber graph: how many files reference this tensor? what is its rank? is this worth downloading?

this enables decisions before the file is fully downloaded. high-rank tensors can be prioritized. low-rank tensors can be skipped. the hash construction makes the entire cyber knowledge graph queryable in streaming order.

with fixed 4KB chunks or byte-level CDC, this is impossible. addresses are meaningless — querying them tells you nothing about what the chunk contains.

### the deduplication consequence

correct semantic identity is sufficient for deduplication. if tensor A's weights appear in 1000 model files, and all 1000 files declare that tensor with the same bytes in the same encoding, they all produce the same content hash for that section. one copy stored, 1000 references. no special dedup logic — it follows directly from content-addressed identity.

but this only works if the hash boundaries align with the semantic boundaries. if two files differ only in frontmatter (model name, description) but share identical weights, fixed-offset chunking destroys deduplication — the weight chunks begin at different byte offsets and are therefore entirely different. semantic hashing preserves it: the weights section is an independent tree node, hashed from its own bytes, unaffected by anything before it.

---

## the problem with fixed chunking

hemera currently splits bytes into fixed 4KB chunks from byte offset 0. for opaque bytes this is correct. for structured data it destroys both deduplication and semantic identity.

**the offset problem:**

two `.model` files sharing 1 GB of identical weights but differing in frontmatter length (200 vs 300 bytes):
- weights section begins at different byte offsets
- every 4KB chunk window contains different bytes
- zero deduplication across gigabytes of shared data
- no stable address for the weights

the root cause: fixed boundaries are absolute byte positions. any insertion anywhere before the data shifts all subsequent boundaries permanently.

**the semantic problem:**

even with perfect deduplication restored (via CDC), if the chunk boundaries are arbitrary byte windows, addresses mean nothing. the cyber rank system needs addresses that correspond to semantic units — tensors, records, declarations. arbitrary boundaries produce arbitrary addresses.

content-defined chunking (CDC) solves the offset problem. semantic structure alignment solves the meaning problem. this proposal does both.

---

## the .cyb format as semantic structure

`.cyb` files have inherent semantic structure:

```
[cyb]                     ← global metadata: model name, type, author
types = ["model"]
name = "qwen3-0.6b"

[[files]]                 ← declaration of file 0: how to interpret its bytes
name = "config"
format = "toml"

[[files]]                 ← declaration of file 1
name = "weights"
format = "safetensors"
size = 1200000000
element = 18              ← atomic unit: one quant block

~~~config                 ← content of file 0: actual bytes
architecture = "Qwen3..."

~~~weights                ← content of file 1: actual bytes
<binary>
```

this structure has three distinct semantic levels:
1. **global metadata** — the preamble `[cyb]` block: what kind of thing this is
2. **file declarations** — each `[[files]]` entry: how each file is described
3. **file content** — each `~~~name` section: the actual bytes

the current hashing spec collapses levels 1 and 2 into a single frontmatter blob. this destroys the independent addressability of each file declaration. adding one tensor changes the address of ALL declarations. the preamble and every tensor's declaration share a single identity when they should each have their own.

**why we should be TOML-aware here:**

the hashing spec is already format-aware. it knows that `~~~name\n` is a structural delimiter. it knows that `[[files]]` array order defines section sequence. it knows that frontmatter ends at the first `~~~`. it is not "format agnostic" — it is deeply `.cyb`-aware.

given this, treating `[[files]]\n` as a structural delimiter is not a new departure. it is the same principle applied consistently. `~~~name\n` is a byte-pattern delimiter for content. `[[files]]\n` is a byte-pattern delimiter for declarations. both are recognized by exact byte matching, not by parsing semantics. the only authoring restriction required: frontmatter values must be single-line (no triple-quoted strings containing `[[files]]` on its own line). this is natural for metadata-only frontmatter.

there are exactly two kinds of data: **bytes** and **text**. binary sections are bytes — atomic unit is a fixed-size element. text sections (frontmatter, config) are text — atomic unit is a line, entry, or record. treating text as opaque bytes when its structure is known and frozen is a missed opportunity for the rank system.

---

## construction

### overview

```
three-level identity:

level 0: file identity      particle_id — the whole file
level 1: section identity   each [[files]] declaration, each ~~~content, the preamble
level 2: chunk identity     CDC chunks within each section
```

every level is a permanent address. changes propagate upward only along the path from the changed node to the root. unchanged sections are unchanged addresses.

### section sequence

given a `.cyb` file with N `[[files]]` entries, the canonical section sequence is:

```
section[0]          preamble bytes         (frontmatter before first [[files]] line)
section[2i-1]       [[files]] entry i      (i = 1..N, declaration bytes)
section[2i]         ~~~files[i-1] bytes    (i = 1..N, content bytes)
```

total sections M = 1 + 2N.

for N=0 (no `[[files]]` entries — pure frontmatter file): M=1, single section is the entire frontmatter.

**section indexing example** for a model with two tensors:

```
section[0]  preamble         ([cyb] block bytes)
section[1]  declaration 0    ([[files]] entry for "config")
section[2]  content 0        (~~~config bytes)
section[3]  declaration 1    ([[files]] entry for "weights")
section[4]  content 1        (~~~weights bytes)
```

M = 5.

### section byte extraction

all byte extraction uses exact byte-pattern matching. no format parsing.

**preamble (section[0]):**
```
bytes [0, first_files_line_start)
where first_files_line_start is the byte offset of the first line
that is exactly [[files]] (i.e., \n[[files]]\n or [[files]]\n at position 0)

if no [[files]] line exists: preamble = entire frontmatter bytes
```

**[[files]] declaration (section[2i-1]):**
```
bytes after [[files]]\n (the delimiter line is not included)
up to (not including) the next [[files]]\n or the first ~~~\n
```

**~~~name content (section[2i]):**
```
per .cyb format spec (format.md §parsing):

text section (no size field):
    bytes after ~~~name\n up to (not including) the next ~~~name\n or EOF

binary section (has size field):
    exactly size bytes starting after ~~~name\n
```

**delimiter invariants:**
- `[[files]]\n` means: byte sequence 0x5B 0x5B 0x66 0x69 0x6C 0x65 0x73 0x5D 0x5D 0x0A, preceded by 0x0A (LF) or at byte 0
- `\n` is always LF (0x0A). files use LF line endings (per format.md §encoding)
- delimiter lines are format structure — not included in section bytes

### element_size assignment

```
section[0]   (preamble)              element_size = 1
section[2i-1] ([[files]] declaration)  element_size = 1
section[2i]  (content)               element_size = files[i-1].element ?? 1
```

the `element` field in the `[[files]]` entry for file i specifies the atomic unit for that file's content section. absent → 1. declarations and preamble are always byte-granular.

**element_size contract:**
- valid range: element_size ∈ [1, 64]
- element_size=0 is forbidden (division in W formula undefined)
- element_size > 64 is permanent architectural limit — the fingerprint rotation modulus is 64 because gear table entries are u64. formats with atomic units larger than 64 bytes must use element_size=1
- len(section_bytes) must be an exact multiple of element_size. if not, the file is malformed — implementations must reject, not silently fall back
- element_size is declared by the author for sections they know are uniformly encoded. it is the byte size of the smallest indivisible unit: a quant block, a sample, a pixel, a field element
- **no inference.** when `element` is absent, implementations use element_size=1 unconditionally. inferring element_size from `format`, file extension, or content inspection is forbidden — incorrect inference silently misaligns boundaries without error

### outer tree

```
M = 1 + 2N   // total sections

if M == 1:
    particle_id = section_cdc_tree(section[0], element_size=1, is_root=true)

else:
    root[j] = section_cdc_tree(section[j], element_size[j], is_root=false)
              for j in 0..M
    particle_id = left_balanced_tree(root[0..M], is_root=true on the final node)
```

for M=1, the CDC tree of the single section computes particle_id directly. no outer tree node is added.

result: a change to section[j] recomputes root[j] and at most ⌈log₂(M)⌉ outer nodes. all other section roots are unchanged. a change to tensor k's weights changes only section[2k] and the path to the root — the preamble, all declarations, and all other tensors' content are untouched.

---

## level 2: element-aligned CDC

### the gear table

the fingerprint for CDC boundary detection is derived from the same cryptographic primitive as the hash. this bootstraps security: an adversary who can manipulate gear table values would need to break the permutation.

256 entries. entry i is the first 8 bytes of `hemera::hash(&[i as u8])`, interpreted as little-endian u64:

```
gear_table[i] = u64::from_le_bytes(hemera::hash(&[i as u8])[0..8])
    for i in 0..256
```

`hemera::hash(data)` is the standard plain sponge call: `Hasher::new()` with domain_tag=0x00, absorb data, finalize — identical to the `hash()` API in specs/api.md. no tree flags, no counter.

**permanence.** the gear table is frozen alongside the complete hemera sponge construction — permutation constants, padding scheme (0x01 || 0x00\*), domain tag (0x00), capacity encoding, rate/capacity split, and output extraction. any change to any of these invalidates the gear table and all particle_id values. this is the only intended breaking point.

### element fingerprint

for an element `e` of element_size S bytes (S ∈ [1, 64]):

```
fp(e) = XOR of  rotate_left(gear_table[e[k]], (k * 11) % 64)
         for k in 0..S
```

rotation increment 11 is odd and coprime to 64 → the sequence (k × 11) mod 64 for k=0..63 visits all 64 values exactly once. for S ≤ 64 every byte position has a distinct rotation weight. byte positions cannot cancel each other through XOR, making the fingerprint sensitive to every byte's position within the element.

for S=1: `fp(e) = gear_table[e[0]]` — single table lookup, no rotation.

**why XOR with rotations and not a hash per element?** computing a full hemera hash per element would cost one permutation per element — for q4 weights at 18 bytes/element this would be 55× slower than CDC boundary detection should be. XOR with gear table lookups is O(S) arithmetic operations per element, negligible relative to the subsequent hash computation. the statistical properties (stable boundary positions, degenerate resistance) are sufficient for CDC; full collision resistance is not needed.

### window size W

the window W controls average chunk size. the formula targets roughly 4096 bytes of data per chunk regardless of element_size:

```
W = next_power_of_two(max(64, floor(4096 / element_size)))
```

| element_size | W | avg chunk (bytes) |
|---|---|---|
| 1 | 4096 | ~4096 |
| 2 | 2048 | ~4096 |
| 4 | 1024 | ~4096 |
| 16 | 256 | ~4096 |
| 32 | 128 | ~4096 |
| 64 | 64 | ~4096 |

the power-of-two rounding means W is always ≥ floor(4096/element_size), keeping average chunks at or slightly above 4KB.

```
min_chunk = W / 2    elements    (prevents excessive fragmentation)
max_chunk = 2 × W   elements    (bounds worst-case chunk size)
```

### boundary rule

let `S = element_size`, `n = len(section_bytes) / S` (must be integer).

**boundaries are element indices.** chunk k contains elements [boundary[k], boundary[k+1]) — bytes [boundary[k]×S, boundary[k+1]×S).

the minimum-fingerprint element is the *last element* of its chunk. the next chunk starts immediately after it.

```
boundaries ← [0]

while last_boundary < n:
    chunk_start ← last_boundary
    lo ← chunk_start + min_chunk - 1         // first candidate last-element
    hi ← min(chunk_start + max_chunk - 1,    // last candidate last-element
              n - 1)

    if lo > n - 1:                           // < min_chunk elements remain
        boundaries.append(n)
        break

    // scan [lo, hi] inclusive; find first occurrence of minimum fp
    min_fp  ← fp(element[lo])
    min_pos ← lo
    for i in lo+1 ..= hi:
        if fp(element[i]) < min_fp:
            min_fp  ← fp(element[i])
            min_pos ← i

    boundaries.append(min_pos + 1)           // boundary after the minimum element
    last_boundary ← min_pos + 1

// invariants: boundaries[0]=0, boundaries[-1]=n
// chunk k size: boundaries[k+1] - boundaries[k] ∈ [min_chunk, max_chunk]
// last chunk may be < min_chunk if section has fewer than min_chunk elements
```

**why local minimum?** this is MinHash applied to chunking (Broder, 1997). the theoretical guarantee: two regions that share k-similar content will produce identical boundaries with probability proportional to their similarity. local minimum CDC is the mathematically optimal strategy for maximising boundary stability across similar content.

**why "last element" convention?** the minimum fingerprint element IS the natural boundary marker — it is the element whose position is most stable under local edits. making it the LAST element of its chunk rather than the first removes the off-by-one ambiguity that plagues many CDC implementations. the boundary is always "element after the minimum," never "the minimum itself."

**degenerate case:** all fp values in [lo, hi] are equal (e.g., all-zero section, all-0xFF section, any uniformly-encoded data). the first minimum is at lo = chunk_start + min_chunk - 1 → boundary at chunk_start + min_chunk → chunk size exactly min_chunk. produces regular fixed-size chunks deterministically. dedup quality for uniform data: perfect (identical chunks → identical addresses).

**re-synchronization:** after any local edit (insert or modify B bytes), CDC boundaries shift within the next max_chunk × element_size bytes, then re-synchronize. all chunks beyond that region are unchanged — same bytes, same addresses. for element_size=1: re-sync within 8192 bytes. for element_size=32: re-sync within 8192 bytes (256 elements × 32 bytes).

### section CDC tree

```
section_cdc_tree(data, S, is_root):
    n ← len(data) / S

    if n == 0:
        return hash_leaf([], counter=0, is_root=is_root)

    compute boundaries using the boundary rule above
    // K = len(boundaries) - 1 chunks, K ≥ 1

    if K == 1:
        return hash_leaf(data, counter=0, is_root=is_root)

    for chunk k in 0..K:
        chunk_bytes = data[ boundaries[k]×S .. boundaries[k+1]×S ]
        leaf[k] = hash_leaf(chunk_bytes, counter=k, is_root=false)

    return left_balanced_tree(leaf[0..K], is_root=is_root)
```

**K=1 case:** a section small enough to fit in one CDC chunk is hashed as a single leaf with `is_root` threaded through. no tree overhead for small sections — declarations and preambles are typically single leaves.

**counter:** `counter=k` in each leaf's state[8] binds the chunk to its position within the section. swapping two chunks changes their counters, changing the leaves, changing the section root. chunk reordering attacks are prevented.

**is_root threading:** the `is_root` flag propagates from the outer particle_id construction into the topmost node of each section tree. for M>1, every section tree uses is_root=false; the outer tree's final node uses is_root=true. for M=1, the single section's topmost node uses is_root=true.

---

## flag assignment

| node | state[9] |
|---|---|
| CDC leaf within section | FLAG_CHUNK (0x04) |
| CDC internal node within section | FLAG_PARENT (0x02) |
| outer tree internal node | FLAG_PARENT (0x02) |
| particle_id root — multi-chunk | FLAG_PARENT\|FLAG_ROOT (0x03) |
| particle_id root — single-chunk, single-section | FLAG_CHUNK\|FLAG_ROOT (0x05) |

exactly one node in the entire computation carries FLAG_ROOT. this is the particle_id. the invariant from specs/tree.md — FLAG_ROOT marks exactly the particle identity — is preserved.

---

## security properties

| property | mechanism |
|---|---|
| section independence | each section hashed as isolated subtree — siblings cannot interfere |
| chunk reordering | counter in state[8] binds each chunk to its position within its section |
| leaf/node confusion | FLAG_CHUNK (0x04) vs FLAG_PARENT (0x02) domain separation |
| root uniqueness | FLAG_ROOT (0x01) on exactly the particle_id root node |
| structural commitment | preamble (section[0]) commits preamble bytes, including all [[files]] ordering. declarations commit element_size and format. content commits actual bytes. every byte participates in identity |
| boundary stability | CDC boundaries content-defined; insertion shifts boundaries within max_chunk × element_size bytes, then re-synchronizes |
| degenerate resistance | max_chunk bounds maximum chunk size; forced min_chunk boundary for uniform data |
| element alignment | CDC never splits an atomic unit; fingerprint computed over whole elements |
| gear table integrity | derived from hemera::hash; security reduces to permutation security |

**structural commitment note:** the preamble contains the `[[files]]` declarations including element_size values. any change to the declared structure — adding a tensor, renaming a field, changing element_size — changes the preamble hash. the preamble hash is section[0] in the outer tree. particle_id cannot remain the same if the declared structure changes. this is the correct behavior: a different structure is a different file.

---

## cost

| operation | cost |
|---|---|
| gear table precomputation | 256 × 1 hemera call, once at startup |
| CDC boundary scan | 1 fp computation per element; O(total_bytes / element_size); negligible vs permutations |
| leaf hashing | K × (⌈chunk_bytes / 56⌉ + 1) permutations; ~75 for 4KB chunks at element_size=1 |
| section tree internal nodes | K − 1 permutations (per section) |
| outer tree | M − 1 permutations; M = 1 + 2N for N files |

**overhead of TOML-aware split:** for a model with N tensors, we add N declaration sections. each declaration is typically a few lines (< 200 bytes) — one CDC chunk, one hash_leaf call, ~4 permutations. for N=50 tensors: 50 × 4 = 200 extra permutations. total file is ~10GB: ~2.5M permutations for weights. the declaration overhead is 0.008% — unmeasurable.

total: within 1% of fixed-4KB tree hash for large files.

---

## dedup quality

| case | quality | mechanism |
|---|---|---|
| identical sections across files | perfect | same bytes → same CDC boundaries → same section root |
| tensor changed, others identical | perfect on unchanged | changed section's subtree recomputed; all other section roots stable |
| frontmatter metadata changed, weights identical | perfect on weights | preamble and declaration hashes change; content roots unchanged |
| new tensor added to model | perfect on existing | new declaration + content sections added; all existing section roots stable |
| local edit within weights section | ~99% | CDC re-synchronizes within max_chunk × element_size bytes |
| zero-padded / uniform weights | perfect | degenerate case → fixed-size chunks → identical chunk addresses |
| random bytes | none | nothing to deduplicate; expected |
| compressed content (JPEG, video) | none | near-random bytes; expected |

**the new cases enabled by TOML-aware structure:**

previously, ANY change to the frontmatter (adding a field to `[cyb]`, renaming a tensor, adding element_size to one tensor) invalidated the entire frontmatter address. now:
- changing the model name in `[cyb]` changes only the preamble section root
- changing tensor 5's element_size changes only declaration section for tensor 5
- all other declarations and all content roots are unchanged

---

## design decisions and tradeoffs

### why interleaved rather than grouped?

the section sequence alternates declarations and content: [preamble, decl0, content0, decl1, content1, ...] rather than grouping [preamble, decl0, decl1, ..., content0, content1, ...].

**interleaved** keeps declaration and content for each file adjacent in the tree's leaf sequence. in the left-balanced tree, adjacent leaves share subtree ancestors. declaration and content for the same file are likely siblings or near-siblings in the outer tree — their combined identity is close to the root without an extra level.

**grouped** would separate all declarations from all content. the "identity of tensor 5" would require the path to tensor 5's declaration AND the path to tensor 5's content — two separate branches. interleaved makes each file's combined identity naturally expressible.

### why separate declaration from content rather than paired?

an alternative construction pairs each file's declaration and content via hash_node:

```
file_node[i] = hash_node(decl_root[i], content_root[i])
particle_id = left_balanced_tree([preamble, file_node[0], ..., file_node[N-1]])
```

this gives each file a single combined identity. but it makes declaration and content inseparable — you cannot address the declaration without the content or vice versa.

with interleaved flat leaves, both the declaration and the content are independently addressable. the rank system can assign value to:
- the declaration of a tensor (how it is described) independently from
- the content of a tensor (the actual weights)

this enables attribution at finer granularity. the author who wrote the model spec (declarations) and the team who trained the weights (content) are credited separately.

### why byte-pattern matching for [[files]] rather than TOML parsing?

TOML parsing would be correct for all valid TOML. byte-pattern matching (`[[files]]\n` at the start of a line) covers all valid `.cyb` frontmatters with the restriction that values are single-line.

TOML is an evolving spec. binding the hash construction to a specific version of TOML parsing would create an external dependency that could change the meaning of section boundaries. byte-pattern matching is frozen — it depends only on the LF canonicalization rule (format.md §encoding) and the fact that `[[files]]` is always a structural line in valid `.cyb` frontmatters.

the restriction: frontmatter values must be single-line (no triple-quoted strings containing `[[files]]` on its own line). this is a natural restriction for metadata-only frontmatter. `.cyb` frontmatter is a machine-readable manifest, not prose.

### why not line-level hashing for text sections?

lines are variable length. the current element_size model requires fixed-size elements. extending to variable-length elements would require a different fingerprint formula, a different W calculation, and a different section_cdc_tree construction — essentially a parallel spec for text sections.

for text sections (element_size=1), byte-level CDC with W=4096 already provides near-line-level resilience. the average chunk covers ~4096 bytes ≈ 80-200 typical lines. a change to one line affects at most two CDC chunks (the one containing the changed line and possibly the next, until re-synchronization). for frontmatter mutations (adding one `[[files]]` entry), re-synchronization happens within 8192 bytes of the insertion point.

with the TOML-aware [[files]] split, the primary motivation for line-level hashing (dedup within the frontmatter when one declaration changes) is already addressed at the declaration-section level. the residual case — a multi-line change within the preamble or within one declaration — is handled acceptably by byte-level CDC.

### why element_size in [[files]] rather than format registry?

a format registry would map format strings ("safetensors", "wav", "bitmap") to element sizes. this appears simpler but fails:

1. **format families are not formats.** "safetensors" can contain fp16, fp32, q4, q8, or mixed tensors in one section. "wav" can be 16-bit, 24-bit, or 32-bit samples. a single element_size cannot represent a mixed-encoding section.

2. **inference fails silently.** a registry lookup on "wav" returning 2 (16-bit) would be wrong for 32-bit WAV files. the section length might still be divisible by 2, so no error is raised. dedup quietly fails.

3. **the format string is user-supplied metadata.** it is not a reliable key into a technical lookup table.

the `element` field in `[[files]]` is declared by the author who knows the exact encoding of their data. it is explicit, verified (divisibility check), and correct by construction when declared. when absent, byte-level CDC (element_size=1) is always correct, just not maximally aligned.

---

## the eternal hash

this is the final construction. the specific hash values produced by this construction are permanent. files identified by this construction will be identifiable by the same particle_id forever.

**what makes it final:**
- the .cyb format is frozen (three rules, no versions)
- `[[files]]` is a TOML array-of-tables — its structural role will not change
- the `~~~name\n` delimiter is frozen
- the gear table is derived from the hemera permutation — frozen alongside the permutation parameters, padding scheme, domain tag, and output encoding
- the element_size mechanism is self-describing — any format's atomic unit can be declared without changing the spec

**what could force a change:**
- a cryptographic break in Poseidon2 or the Goldilocks field — would require a new permutation, new gear table, new hashes. this is the only accepted breaking event
- a future format requiring element_size > 64 — cannot be represented; such sections use element_size=1 (always correct, not optimally aligned). the 64-byte ceiling is a permanent architectural constraint tied to the 64-bit rotation modulus

**what cannot force a change:**
- new content types — any atomic unit from 1 to 64 bytes is representable via the `element` field
- new file formats within .cyb — they follow the same three rules; new format strings are just strings
- changes to TOML — byte-pattern matching for `[[files]]\n` depends only on byte values, not TOML semantics
- platform differences — LF canonicalization (format.md §encoding) ensures identical section bytes everywhere

---

## connections to adjacent fields

**MinHash (Broder 1997):** the local-minimum CDC is MinHash applied to chunking. the theoretical guarantee — two k-similar regions produce identical chunk boundaries with probability proportional to their similarity — makes this the mathematically optimal chunking strategy for structured deduplication.

**rsync:** uses rolling hash to find matching blocks for delta transfer. CDC applies the same stability insight to permanent content identity rather than ephemeral delta computation.

**bup / borg / restic:** production CDC backup systems. years of empirical validation that local-minimum boundaries are stable under exactly the workloads we care about: fine-tuning (small weight changes), dataset updates, knowledge graph edits.

**git objects:** git stores blobs, trees, and commits as content-addressed objects. every internal object has a permanent SHA address. git's power comes precisely from this — every version of every file at every level of the tree is independently addressable. semantic hashing brings this property to binary structured files (model weights, graphs, vocabularies) where git's line-level text model does not apply.

**what we do NOT borrow:** approximate matching (SimHash, LSH). those find "nearly identical" content. hemera requires exact identity. approximate matching would change particle_id from a deterministic address to a probabilistic similarity score — incompatible with a blockchain system where the address IS the content.

---

## backwards compatibility

`particle_id` under this construction differs from `root_hash` for all inputs. all particle addresses change once on adoption. there is no in-place migration path.

`root_hash(data)` is retained as a convenience alias — a single-section hash with element_size=1:

```rust
pub fn root_hash(data: &[u8]) -> Hash {
    section_cdc_tree(data, element_size=1, is_root=true)
}
```

this preserves API surface for callers who hash arbitrary byte slices. the underlying computation is now CDC-based.

---

## spec targets

`specs/tree.md`:
- replace "Chunk Size: 4 KB" with: gear table, fingerprint formula, W formula, boundary rule, section_cdc_tree
- add section sequence definition (preamble + [[files]] declarations + ~~~content sections)
- add flag assignment table
- add outer tree construction for M = 1 + 2N sections

`specs/api.md`:
- `particle_id(file: &CybFile) -> Hash` — takes parsed .cyb structure
- `Section { data: &[u8], element_size: usize }`
- `root_hash(data: &[u8]) -> Hash` — single-section alias, element_size=1

`cyb/root/format.md` — add `## identity`:
- `[[files]]` entries may declare `element = N` where N ∈ [1, 64]
- `[[files]]` entry bytes are an independent section in the particle_id tree
- preamble bytes (before first `[[files]]`) are an independent section
- particle_id computation: see hemera/specs/tree.md §particle_id
