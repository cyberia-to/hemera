---
tags: cyber, hemera, spec, prop
crystal-type: prop
crystal-domain: cyber
status: accepted
breaks_hash: yes
alias: semantic hashing, structured identity, section tree, element-aligned CDC, particle identity
---

# prop: semantic hashing

## status

accepted. replaces fixed-4KB chunking in `particle_id` for all content.

`breaks_hash: yes` — every existing `root_hash` value changes. intentional and one-time: the new construction is the eternal hash.

## the problem with fixed chunking

hemera currently splits bytes into fixed 4KB chunks from byte offset 0. for opaque bytes this is correct. for structured data it destroys deduplication:

two `.model` files sharing 1 GB of identical weights but differing in frontmatter length (200 vs 300 bytes) → weights section begins at different byte offsets → every 4KB chunk window in the weights section contains different bytes → zero deduplication across gigabytes of shared data.

the root cause: fixed boundaries are absolute byte positions. any insertion before the data shifts all subsequent boundaries permanently.

content-defined chunking (CDC) solves this: boundaries are determined by the content, not the offset. after a local change, boundaries re-synchronize within ~W elements. large unchanged regions deduplicate completely regardless of what precedes them.

## the core insight

hemera is an identification layer. not compression. not archival.

```
compression:      file.bin → file.bin.gz   (bytes transform, size shrinks)
identification:   file.bin → particle_id   (bytes unchanged, address permanent)
```

deduplication is a consequence of correct identification. the design goal: one changed byte should change exactly the hashes of the chunks containing that byte and their ancestors to the root. nothing else.

## two-level construction

```
level 1: section boundaries     declared in [[files]] frontmatter, deterministic
level 2: CDC chunk boundaries    content-defined, self-healing within each section
```

a change to one section changes only its subtree. CDC within a section means a local change re-synchronizes within ~W elements, leaving the rest of the section unchanged.

## level 1: section tree

### section sequence

a `.cyb` file has a canonical section sequence derived from its bytes:

```
section[0]       frontmatter bytes
section[1]       first [[files]] entry bytes, in declaration order
...
section[N]       last [[files]] entry bytes
```

### section byte extraction

per `.cyb` format spec (format.md §parsing). the `~~~name\n` delimiter is format structure — not included in section bytes.

```
frontmatter (section[0]):
    bytes [0, first_tilde_line_start)
    where first_tilde_line_start is the byte offset of the first line starting with ~~~

text section (no size field):
    bytes after ~~~name\n up to (not including) the next ~~~name\n or EOF

binary section (has size field):
    exactly size bytes starting after ~~~name\n
```

the frontmatter is section[0]. it commits the section list, element sizes, and all metadata. every byte of the file participates in identity.

### outer tree

each section produces a section root via CDC tree hashing (see level 2). the particle_id is the left-balanced tree over the section roots (tree shape and hash_node as defined in specs/tree.md):

```
M = N + 1   // total sections (frontmatter + [[files]] entries)

if M == 1:
    particle_id = section_cdc_tree(section[0], element_size=1, is_root=true)

else:
    // element_size[i]: i=0 → 1 (frontmatter); i≥1 → files[i-1].element ?? 1
    root[i] = section_cdc_tree(section[i], element_size[i], is_root=false)
              for i in 0..M
    particle_id = left_balanced_tree(root[0..M], is_root=true on the final node)
```

for M=1 (no [[files]] entries), the CDC tree of the single section computes particle_id directly with is_root=true throughout its top node. no outer tree node is added.

result: a change to section[k] recomputes root[k] and at most ⌈log₂(M)⌉ outer nodes. all other section roots are unchanged.

## level 2: element-aligned CDC

### gear table

256 entries. entry i is the first 8 bytes of `hemera::hash(&[i as u8])`, interpreted as little-endian u64:

```
gear_table[i] = u64::from_le_bytes(hemera::hash(&[i as u8])[0..8])
    for i in 0..256
```

`hemera::hash(data)` is the standard plain sponge call: `Hasher::new()` with domain_tag=0x00, absorb data, finalize — identical to the existing `hash()` API in specs/api.md. no tree flags, no counter.

gear_table is precomputed once. it is deterministic and frozen alongside the permutation parameters.

### element_size constraints

valid range: element_size ∈ [1, 64].

- element_size=0 is forbidden (division in W formula undefined)
- element_size > 64 is forbidden (rotation formula loses uniqueness at k=64)
- len(section_bytes) must be an exact multiple of element_size. if not, the file is malformed for the declared element_size — implementations must reject, not silently fall back. authors must not declare `element` for sections with non-uniform encodings; such sections use element_size=1.

recommended element sizes for authors writing explicit `element` declarations on uniform-encoding sections. not a dispatch table — implementations never use this to infer element_size:

| encoding | element_size | atomic unit |
|----------|-------------|-------------|
| text, TOML, frontmatter, unknown | 1 | byte |
| u16 | 2 | one value |
| u32 | 4 | one value |
| q4 | 18 | one quant block (scale + 32 nibbles) |
| q8 | 34 | one quant block (scale + 32 i8s) |
| mixed / unknown encoding | 1 | byte (use this when in doubt) |

declared in `[[files]]` as optional `element` field. absent → element_size=1. frontmatter always element_size=1.

**no inference.** when `element` is absent, implementations use element_size=1 unconditionally. implementations must not infer element_size from the `format` field, file extension, or any content inspection. incorrect inference silently misaligns boundaries and destroys deduplication without error.

### element fingerprint

for an element `e` of element_size S bytes (S ∈ [1, 64]):

```
fp(e) = XOR of  rotate_left(gear_table[e[k]], (k * 11) % 64)
         for k in 0..S
```

rotation increment 11 is odd and coprime to 64 → full 64-cycle before any rotation repeats. for S ≤ 64 every byte position has a distinct rotation weight. no cancellation between byte positions for any canonical format (max S = 34).

for S=1: `fp(e) = gear_table[e[0]]` (single table lookup, no rotation).

### window size W

```
W = next_power_of_two(max(64, floor(4096 / element_size)))
```

| element_size | W | avg chunk |
|---|---|---|
| 1 | 4096 | ~4096 B |
| 2 | 2048 | ~4096 B |
| 4 | 1024 | ~4096 B |
| 18 | 256 | ~4608 B |
| 34 | 128 | ~4352 B |

```
min_chunk = W / 2    elements
max_chunk = 2 × W   elements
```

### boundary rule

let `S = element_size`, `n = len(section_bytes) / S` (integer; section size must be exact multiple of S).

**boundaries are element indices.** chunk k contains elements [boundary[k], boundary[k+1]) i.e. bytes [boundary[k]×S, boundary[k+1]×S).

the minimum fp element is the *last element* of its chunk. the next chunk starts immediately after it:

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

    boundaries.append(min_pos + 1)           // boundary = element after minimum
    last_boundary ← min_pos + 1

// always: boundaries[0]=0, boundaries[-1]=n
// chunk k size: boundaries[k+1] - boundaries[k] ∈ [min_chunk, max_chunk]
// last chunk may be < min_chunk if section is short
```

degenerate case: all fp values in [lo, hi] are equal (e.g., zero-padded data). the first minimum is at lo = chunk_start + min_chunk - 1 → boundary at chunk_start + min_chunk → chunk size exactly min_chunk. produces regular fixed-size chunks deterministically. maximum dedup for uniform data.

### section CDC tree

for section bytes `data` with element_size S, n elements, and computed boundaries:

```
section_cdc_tree(data, S, is_root):
    n ← len(data) / S

    if n == 0:
        return hash_leaf([], counter=0, is_root=is_root)   // pre-check: empty section

    compute boundaries using the boundary rule above
    // boundaries[0]=0, boundaries[-1]=n, K = len(boundaries)-1 chunks

    for chunk k in 0..K:
        chunk_bytes = data[ boundaries[k]×S .. boundaries[k+1]×S ]
        leaf[k] = hash_leaf(chunk_bytes, counter=k, is_root=false)

    if K == 1:
        return hash_leaf(data, counter=0, is_root=is_root)
    else:
        return left_balanced_tree(leaf[0..K], is_root=is_root)
```

K is the number of CDC chunks (len(boundaries)-1). K ≥ 1 for all non-empty sections.

the `is_root` flag threads from the outer particle_id construction into the topmost node of each section tree. for M > 1 outer sections, every section tree uses is_root=false and the outer tree's root node uses is_root=true. for M=1, the single section tree uses is_root=true.

## flag assignment

| node | state[9] |
|---|---|
| CDC leaf within section | FLAG_CHUNK (0x04) |
| CDC internal node within section | FLAG_PARENT (0x02) |
| outer tree internal node | FLAG_PARENT (0x02) |
| particle_id root (any tree, M=1 or M>1) | FLAG_PARENT\|FLAG_ROOT (0x03) for multi-chunk; FLAG_CHUNK\|FLAG_ROOT (0x05) for single-chunk single-section |

exactly one node in the entire computation carries FLAG_ROOT: the top node of the particle_id tree. this preserves the invariant from specs/tree.md: FLAG_ROOT marks exactly the particle identity.

## security properties

| property | mechanism |
|---|---|
| section independence | each section hashed as isolated subtree; siblings cannot interfere |
| chunk reordering | counter in state[8] binds chunk to its position within section |
| leaf/node confusion | FLAG_CHUNK (0x04) vs FLAG_PARENT (0x02) |
| root uniqueness | FLAG_ROOT (0x01) on exactly the particle_id root |
| boundary stability | CDC boundaries content-defined; insertion shifts boundaries within next max_chunk elements (max_chunk × element_size bytes), then re-synchronizes |
| degenerate resistance | max_chunk bounds maximum chunk size; forced min_chunk boundary for uniform data |
| element alignment | CDC never splits a quant block; fingerprint computed over whole elements |
| gear table integrity | derived from hemera::hash outputs; security reduces to permutation security |

## cost

| operation | cost |
|---|---|
| gear table precomputation | 256 × 1 hemera call, once at startup |
| CDC boundary scan | 1 fp computation per element; O(D/S) total; negligible vs permutations |
| leaf hashing | K × 75 permutations (74 absorb + 1 bind, 4KB chunk) |
| section tree internal nodes | K − 1 permutations (per section) |
| outer tree | M − 1 permutations (≤ 6 for typical .model with 7 sections) |

total: within 1% of fixed-4KB tree hash for large files.

## dedup quality

| case | quality | mechanism |
|------|---------|-----------|
| identical sections across files | perfect | same bytes → same CDC → same section_root |
| one tensor changed, rest identical | ~99% | CDC re-synchronizes within max_chunk × element_size bytes |
| frontmatter differs, sections identical | perfect on sections | section[0] differs, section[1..N] roots unchanged |
| zero-padded / abliterated weights | perfect | degenerate fallback → fixed-size chunks → identical chunk hashes |
| random bytes | none | nothing to deduplicate; expected |
| compressed (JPEG, video) | none | near-random bytes; expected |

## backwards compatibility

`particle_id` under this construction differs from `root_hash` for all inputs. all particle addresses change once on adoption. there is no in-place migration.

`root_hash(data)` is retained as a single-section alias with element_size=1:

```rust
pub fn root_hash(data: &[u8]) -> Hash {
    section_cdc_tree(data, element_size=1, is_root=true)
}
```

this preserves API surface. the underlying computation is now CDC-based.

## connections to adjacent fields

**MinHash (Broder 1997)**: local minimum CDC is MinHash applied to chunking. the theoretical guarantee — two regions that are k-similar will chunk identically with probability proportional to their similarity — makes CDC the mathematically optimal chunking strategy for deduplication across similar content.

**rsync**: uses rolling hash to find matching blocks for delta transfer. our CDC applies the same idea to find stable boundaries that survive content shifts.

**bup / borg / restic**: production CDC backup systems. years of empirical validation that CDC boundaries are stable under the workloads we care about (model fine-tuning, dataset updates, knowledge graph edits).

**what we do NOT borrow**: approximate matching (SimHash, LSH). those find "nearly identical" content. hemera requires exact identity. approximate matching would change particle_id from deterministic address to probabilistic similarity — incompatible with a blockchain system.

## spec targets

`specs/tree.md`:
- replace "Chunk Size: 4 KB" with gear table, fingerprint, W formula, boundary rule, section CDC tree
- add section tree construction and flag assignment table

`specs/api.md`:
- `particle_id(sections: &[Section]) -> Hash`
- `Section { data: &[u8], element_size: usize }`
- `root_hash(data: &[u8]) -> Hash` — single-section alias, element_size=1

`cyb/root/format.md` — add `## identity`:
- `[[files]]` entries may declare `element = N` where N ∈ [1, 64]
- mixed-encoding sections: omit `element`, defaults to 1
- particle_id computation: see hemera/specs/tree.md §particle_id
