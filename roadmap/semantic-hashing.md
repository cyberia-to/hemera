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

`breaks_hash: yes` — every existing `root_hash` value changes. this is intentional and one-time: the new construction is the eternal hash.

## the problem with fixed chunking

hemera currently splits bytes into fixed 4KB chunks from byte offset 0. for opaque bytes this is correct. for structured data it destroys deduplication:

two `.model` files sharing 1 GB of identical weights but differing in frontmatter length (say 200 vs 300 bytes) → weights section begins at different byte offsets → every 4KB chunk window in the weights section contains different bytes → zero deduplication across gigabytes of shared data.

the root cause: fixed boundaries are absolute byte positions. any insertion before the data shifts all subsequent boundaries permanently. the problem is not the chunk size — it is the chunk *type*.

content-defined chunking (CDC) solves this: boundaries are determined by the content, not the offset. after a local change, boundaries re-synchronize within ~W elements. large unchanged regions deduplicate completely regardless of what precedes them.

## the core insight

hemera is an identification layer. not compression. not archival.

```
compression:      file.bin → file.bin.gz   (bytes transform, size shrinks)
identification:   file.bin → particle_id   (bytes unchanged, address permanent)
```

deduplication is a *consequence* of correct identification. if two chunks produce the same hash, the storage layer stores them once. the hash function does not know or care about storage — it only computes identity.

the design goal: particle_id must be maximally stable. one changed byte should change exactly the hashes of the chunks containing that byte and their ancestors to the root. nothing else.

## two-level construction

```
level 1: section boundaries     declared in [[files]] frontmatter, deterministic
level 2: CDC chunk boundaries    content-defined, self-healing within each section
```

a change to one section changes only its subtree. CDC within a section means a local change re-synchronizes within ~W elements, leaving the rest of the section unchanged.

## level 1: section tree

a `.cyb` file has a canonical section sequence:

```
section[0]       frontmatter bytes  (everything before the first ~~~)
section[1]       first [[files]] entry  (in declaration order)
section[2]       second [[files]] entry
...
section[N]       last [[files]] entry
```

the frontmatter is always section[0]. it is part of the file's content and commits to the section list, declared element sizes, and any other metadata. there is no "metadata that does not affect identity" — every byte of the file participates.

each section produces a section root via CDC tree hashing (level 2). the section roots form a left-balanced outer tree:

```
particle_id = left_balanced_tree(section_root[0], ..., section_root[N])
```

result: a change to section[k] recomputes section_root[k] and at most ⌈log₂(N+1)⌉ nodes on the path to particle_id. all other section roots are unchanged.

## level 2: element-aligned CDC

### gear table

256 entries. each entry is the first 8 bytes of `hemera([i as u8])`, interpreted as a little-endian u64:

```
gear_table[i] = u64::from_le_bytes(hemera([i as u8])[0..8])     for i in 0..256
```

`hemera([i])` hashes the single byte `i` using the full hemera function (with round constants). the gear table is computed once, is deterministic, and is frozen forever alongside the permutation parameters.

### element fingerprint

for an element `e` of `element_size` bytes S:

```
fp(e) = XOR of  rotate_left(gear_table[e[k]], (k * 11) % 64)
         for k in 0..S
```

the rotation increment 11 is odd and coprime to 64, giving a full 64-cycle before any rotation repeats. for all canonical element sizes (S ≤ 34), every byte in the element has a distinct rotation weight. there is no cancellation between byte positions.

for element_size = 1: `fp(e) = gear_table[e[0]]` (single table lookup, no rotation).

### element_size per section

each section has an atomic element: the smallest unit that CDC must not split. declared in `[[files]]` as an optional `element` field:

```toml
[[files]]
name = "weights"
format = "tensors"
size = 1200000000
element = 18
```

canonical element sizes for `.model` encodings:

| encoding | element_size | atomic unit |
|----------|-------------|-------------|
| text, TOML, unknown | 1 byte | byte |
| u16 | 2 bytes | one value |
| u32 | 4 bytes | one value |
| ternary | 1 byte | one packed byte (4 values) |
| q4 | 18 bytes | one quant block (scale + 32 nibbles) |
| q8 | 34 bytes | one quant block (scale + 32 i8s) |

if `element` is absent from `[[files]]`: element_size = 1.

frontmatter (section[0]): element_size = 1 always (it has no declared [[files]] entry).

for sections with mixed encodings (e.g., weights section containing q4 and u32 tensors): declare no `element` field — element_size defaults to 1. CDC with element_size=1 and W=4096 still provides self-healing boundaries and near-perfect deduplication for large identical regions (~99% dedup for tensors larger than ~4KB).

section byte count must be a multiple of element_size. if `len(section_bytes) % element_size != 0`, the file is malformed for the declared element_size. authors must not declare `element` for sections with non-uniform encodings.

### window size W

derived deterministically from element_size to target ~4KB average chunk size:

```
W = next_power_of_two(max(64, floor(4096 / element_size)))
```

| element_size | W | avg chunk bytes |
|---|---|---|
| 1 | 4096 | ~4096 B |
| 2 | 2048 | ~4096 B |
| 4 | 1024 | ~4096 B |
| 18 | 256 | ~4608 B |
| 34 | 128 | ~4352 B |

min_chunk = W / 2 elements. max_chunk = 2 × W elements.

### boundary rule

let `n = len(section_bytes) / element_size`.

to find the next boundary after `chunk_start`:

```
search_start = chunk_start + min_chunk
search_end   = min(chunk_start + max_chunk, n)

if search_start >= n:
    boundary at n   (end of data)
else:
    scan fp(element[i]) for i in [search_start, search_end)
    boundary at the first i where fp(element[i]) is minimal
    (first occurrence wins on ties)
```

degenerate case: all fp values in `[search_start, search_end)` are equal (e.g., zero-padded or abliterated tensors). the minimum is fp(element[search_start]), the boundary falls at search_start = chunk_start + min_chunk. this produces regular chunks of exactly min_chunk elements — fixed-size fallback, fully deterministic, maximum dedup for uniform data.

repeat from the new chunk_start until `chunk_start = n`.

final boundary is always at `n`.

### section CDC tree

for a section with bytes `data` and `element_size`:

```
1. find all boundaries: [0, b1, b2, ..., n]
2. for each chunk k between boundary[k] and boundary[k+1]:
       leaf[k] = hash_leaf(data[boundary[k] * S .. boundary[k+1] * S],
                           counter = k,
                           is_root = false)
3. section_root = left_balanced_tree(leaf[0], ..., leaf[M-1])
                  with is_root = false on every node
```

a single-chunk section (data shorter than min_chunk elements): `section_root = hash_leaf(data, counter=0, is_root=false)`.

an empty section: `section_root = hash_leaf([], counter=0, is_root=false)`.

section roots are always computed with `is_root=false`. only the final outer tree root carries `is_root=true`.

## particle_id construction

```
section_roots[0..=N]:
    section_roots[0] = section_cdc_tree(frontmatter_bytes, element_size=1)
    section_roots[i] = section_cdc_tree(section_bytes[i], element_size = files[i].element ?? 1)

particle_id = left_balanced_tree(section_roots[0..=N], is_root=true on the final node)
```

flag assignment:

| node type | flags in state[9] |
|---|---|
| CDC leaf within section | FLAG_CHUNK (0x04) |
| CDC internal node within section | FLAG_PARENT (0x02) |
| CDC section root | FLAG_PARENT (0x02) — is_root=false |
| outer tree internal node | FLAG_PARENT (0x02) |
| particle_id root | FLAG_PARENT \| FLAG_ROOT (0x03) |

exactly one node in the entire computation carries FLAG_ROOT: the final particle_id. this preserves the existing invariant: FLAG_ROOT marks exactly the particle identity, distinguishing it from all internal chaining values.

special case — single section (N=0): the outer tree degenerates. `particle_id = section_cdc_tree(frontmatter_bytes, 1)` with the top CDC node using `is_root=true`. no outer tree node needed.

## security properties

| property | mechanism |
|---|---|
| section independence | each section hashed as an isolated subtree; sibling sections cannot interfere |
| chunk reordering | counter in state[8] binds each CDC chunk to its position within the section |
| leaf/node confusion | FLAG_CHUNK (0x04) vs FLAG_PARENT (0x02) in state[9] |
| root uniqueness | FLAG_ROOT (0x01) carried only by the particle_id root |
| boundary stability | CDC boundaries content-defined; an N-byte insertion shifts boundaries only within the next max_chunk elements (~8KB), then re-synchronizes |
| degenerate resistance | max_chunk limit prevents forced arbitrarily large chunks |
| gear table integrity | derived from hemera outputs; security reduces to security of the permutation |

the attacker controls rate (state[0..8]) through input data but cannot reach capacity (state[8..15]). all security proofs from the existing sponge construction carry forward without modification.

## cost

for a file with M sections and total data D bytes split into K CDC chunks:

| operation | cost |
|---|---|
| CDC boundary scan | 1 gear fp computation per element; O(D) total; negligible vs permutations |
| leaf hashing | K × 75 permutations (74 absorb + 1 bind, for 4KB chunks) |
| section tree internal nodes | K − M permutations |
| outer tree | M − 1 permutations (≤ 6 for a typical .model with 7 sections) |

total: within 1% of fixed-4KB tree hash for large files. the boundary scan overhead is dominated by permutation cost at any reasonable data size.

## dedup quality

| case | dedup quality | mechanism |
|------|--------------|-----------|
| identical sections across files | perfect | same bytes → same CDC → same section_root |
| one tensor changed, others identical | ~99% | CDC re-synchronizes within max_chunk (~8KB) of the change |
| prepended frontmatter only differs | perfect on sections | section[0] differs, all section[1..N] roots unchanged |
| zero-padded / abliterated weights | perfect | degenerate case → fixed-size chunks → identical chunk hashes |
| random bytes | none | expected; nothing to deduplicate |
| compressed data (JPEG, video) | none | near-random bytes; expected |

## backwards compatibility

`particle_id` under this construction differs from `root_hash` for all inputs. this is intentional: the new construction IS the eternal hash. there is no in-place migration — all particle addresses change once on adoption.

`root_hash(data)` is retained as a single-section alias:

```rust
pub fn root_hash(data: &[u8]) -> Hash {
    particle_id(&[Section { data, element_size: 1 }])
}
```

this preserves the existing API surface while making the CDC construction the canonical implementation.

## connections to adjacent fields

CDC-based chunking is not novel — it underlies git, rsync, bup, borg, and restic. what is novel here is combining it with the sponge capacity model:

**MinHash (Broder 1997)**: local minimum CDC is MinHash applied to chunking. the theoretical guarantee — two regions that are k-similar will chunk identically with probability proportional to their similarity — makes CDC the mathematically optimal chunking strategy for maximizing deduplication across similar content.

**rsync**: uses rolling hash to find matching blocks between source and destination for delta transfer. our CDC applies the same idea to find stable boundaries that survive content shifts, enabling deduplication across independently-created files.

**bup / borg / restic**: production CDC backup systems. years of empirical validation that CDC boundaries are stable under the workloads we care about (model fine-tuning, knowledge graph updates, incremental dataset changes).

**what we do NOT borrow**: approximate matching (SimHash, LSH). those find "nearly identical" content. hemera requires exact identity. approximate matching would change the semantics of particle_id from deterministic address to probabilistic similarity — incompatible with a blockchain system.

## spec targets

`specs/tree.md` — replace "Chunk Size: 4 KB" section with:
- **Gear Table**: construction, 256 entries, u64 per entry
- **Element Fingerprint**: formula, rotation increment 11
- **Window Size W**: formula from element_size
- **Boundary Rule**: scan [min_chunk, max_chunk), first minimum, degenerate case
- **Section CDC Tree**: leaf counter, is_root=false, empty/single-chunk cases
- **particle_id**: outer section tree, flag assignment table

`specs/api.md` — update:
- `particle_id(sections: &[Section]) -> Hash`
- `Section { data: &[u8], element_size: usize }`
- `root_hash(data: &[u8]) -> Hash` as single-section alias

`cyb/root/format.md` — add `## identity`:
- `[[files]]` entries may declare `element = N`
- identity computation: see `hemera/specs/tree.md §particle_id`
- mixed-encoding sections: omit `element` field, default to 1
