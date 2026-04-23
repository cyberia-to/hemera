---
tags: cyber, hemera, spec, prop
crystal-type: prop
crystal-domain: cyber
status: draft
breaks_hash: yes
alias: semantic hashing, structured identity, section tree, element-aligned CDC
---

# prop: semantic hashing

## status

draft

## the core insight

hemera is an identification layer. not compression. not archival.

compression transforms bytes to reduce size. identification assigns every piece of data a permanent, verifiable address. deduplication is a consequence: if two chunks have the same address, they are the same chunk — store once, reference many times.

```
compression:      file.bin → file.bin.gz   (bytes change, size shrinks)
identification:   file.bin → particle_id   (bytes unchanged, address permanent)
```

dedup, sync efficiency, and graph ranking all follow from correct identification. they are not the goal — they are what happens when identity is done right.

## the problem

hemera's current tree hash splits bytes into fixed 4 KB chunks. this is correct for opaque bytes. it fails for structured data:

**boundary alignment problem**: two .model files share identical weight tensors, but the tensor starts at a different byte offset because frontmatter sizes differ. 4 KB cuts fall at different positions → no deduplication.

**semantic unit problem**: for weights, the meaningful unit is a quantization block (18 bytes for q4), not a 4 KB window. cutting through the middle of a block is semantically wrong and produces no dedup.

## two levels of structure

structured data has identity at two levels:

```
level 1: section boundaries    (from format declaration)
level 2: chunk boundaries      (from content, within each section)
```

level 1 is format-specific. level 2 is content-defined.

## level 1: section tree

for .cyb containers, sections are the top-level semantic units:

```
1. parse frontmatter → ordered section list
2. section_root[i] = hemera::root_hash(section_bytes[i])
3. particle_id = left_balanced_tree([section_root[0], ..., section_root[n]])
```

sections taken in [[files]] declaration order. `root_hash` applies level 2 chunking internally.

two containers sharing an identical section share that `section_root`. storage deduplicates at section granularity automatically.

frontmatter is not a section — it declares order but is not hashed as a unit. identity is insensitive to frontmatter serialization.

## level 2: element-aligned local minimum CDC

within each section, chunk boundaries are content-defined. different content types need different element granularity.

### gear table

```
gear_table[256] = [hemera(0), hemera(1), ..., hemera(255)]
```

generated once from hemera. deterministic. frozen forever.

### fingerprint per element

for an element of size S bytes:

```
fp(element) = gear_table[e[0]]
            ^ rotate_left(gear_table[e[1]], 8)
            ^ rotate_left(gear_table[e[2]], 16)
            ^ ...  (up to S bytes)
```

element boundaries are never crossed. raw bytes: S=1. q4 quant block: S=18.

### chunk boundary = local minimum

slide window of W elements. chunk boundary where fp is the local minimum:

```
boundary at position i  iff  fp[i] = min(fp[i-W/2 .. i+W/2])
```

guaranteed ~W elements between boundaries. no min/max parameters. no MASK threshold.

### by content type

| content | element_size | W | why |
|---------|-------------|---|-----|
| q4 weights | 18 bytes (quant block) | 256 | boundaries between quant blocks |
| q8 weights | 34 bytes (quant block) | 128 | same |
| u16 (embeddings) | 2 bytes | 2048 | per weight |
| u32 (norms) | 4 bytes | 1024 | per weight |
| ternary | 8 bytes (32-value block) | 512 | per ternary block |
| text / TOML | 1 byte | 4096 | standard CDC |
| raw bitmap | bytes per pixel (3-4) | 1024 | per pixel |
| WAV audio | 2-4 bytes (sample) | 1024 | per sample |
| compressed (JPEG, video) | 1 byte | 4096 | no semantic structure, no dedup expected |

`element_size` comes from the section's encoding declaration (tensors.toml). unknown binary: `element_size = 1` → degrades to standard byte-level CDC.

### dedup quality by case

| case | quality | why |
|------|---------|-----|
| identical tensors across models | excellent | same bytes → same fp → same minima |
| zero-runs (abliteration) | excellent | constant fp → regular minima |
| identical vocab across fine-tunes | good | text CDC, chunks align |
| compressed video/audio | none | near-random bytes, expected |
| random bytes | none | correct, nothing to dedup |

## connections to adjacent fields

**git**: content-addressed objects. every blob, tree, commit is identified by hash. dedup across branches and history is automatic. we are applying this to binary data at sub-file granularity.

**rsync**: rolling hash finds matching blocks between source and destination. transfers only the diff. our CDC uses the same idea to find stable chunk boundaries — same content at different offsets still chunks identically.

**bup / borg / restic**: CDC-based backup systems. identical to our approach. bup uses Rabin fingerprinting; restic uses CDC with variable chunk sizes. years of production validation.

**MinHash (Broder 1997)**: local minimum IS MinHash applied to chunking. MinHash finds the minimum hash value in a set — an unbiased estimator of Jaccard similarity. our local minimum CDC finds minimum fingerprint positions, which are maximally stable across similar content. the theoretical guarantee: two regions that are mostly identical will chunk identically with high probability.

**information theory — Kolmogorov complexity**: the Merkle DAG over CDC chunks IS a near-minimum description of the data. identical chunks referenced once, unique chunks stored once. approaches the information-theoretic lower bound for structured redundant data.

**LZ compression**: finds repeated substrings within one file. we find repeated chunks across files. different scope, same mathematical intuition: pattern = compressible = dedupable.

**what compression theory contributes**: entropy estimation. high-entropy content (compressed video, random bytes) → dedup ratio ≈ 0 regardless of chunk size. if entropy estimate > threshold → use 4 KB flat chunks, skip CDC overhead. this could be added as a fast pre-check.

**what we do NOT borrow**: approximate/perceptual matching (Shazam, SimHash, LSH for similarity search). these find "nearly identical" content. we require exact identity. approximate matching changes the meaning of particle_id.

## spec targets

`specs/tree.md` — two new sections:
- **Semantic Units**: section tree construction for .cyb containers
- **Element-Aligned Chunking**: local minimum CDC, gear table, element_size by encoding

`root/format.md` in cyb repo — `## identity` section (pending this prop acceptance).
