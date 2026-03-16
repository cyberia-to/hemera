# Hemera documentation structure

Status: draft
Date: 2026-03-16

---

## Current state

One 1012-line monolith at `reference/README.md` mixing:
- austere specification (parameters, sponge, encoding, tree, bootstrap, matrices)
- design rationale (why each parameter, why no headers, why 4KB, why sponge-only)
- ecosystem comparison (Poseidon2 landscape)
- performance analysis
- implementation plan (phases 1-5)
- migration/emergency protocols
- etymology

## Target structure

Following Diataxis (CLAUDE.md §documentation methodology).

```
hemera/
├── reference/                    canonical specification (source of truth)
│   ├── README.md                 index + abstract + parameter summary box
│   ├── sponge.md                 sponge operation, capacity layout, flags, domain tags
│   ├── encoding.md               byte encoding (input 7B, output 8B), padding
│   ├── tree.md                   tree hashing: 4KB chunks, leaves, nodes, NMT, proof format
│   ├── bootstrap.md              round constant generation procedure (deterministic)
│   ├── matrices.md               M_E, M_I construction
│   ├── api.md                    public API surface (Hasher, tree, key derivation)
│   └── bibliography.md           academic references
│
├── docs/
│   ├── README.md                 index — links to all four quadrants
│   │
│   ├── tutorials/
│   │   ├── first-hash.md         hash content, read the output, verify
│   │   └── merkle-proof.md       build a tree, generate proof, verify it
│   │
│   ├── guides/
│   │   ├── cli.md                CLI: hash files, check sums, encode/decode streams
│   │   ├── streaming.md          streaming hash for large files (verified streaming)
│   │   ├── key-derivation.md     derive_key usage patterns
│   │   └── gpu.md                GPU acceleration via wgsl crate
│   │
│   └── explanation/
│       ├── why-hemera.md         why a new name, permanence constraint, identity vs execution
│       ├── parameters.md         rationale for every parameter (field, sbox, width, rounds)
│       ├── security.md           security margins, quantum, algebraic degree, ecosystem comparison
│       ├── self-bootstrap.md     why self-bootstrapping, non-circularity, security analysis
│       ├── content-ids.md        why raw 64-byte CIDs, no headers, endofunction closure
│       ├── sponge-only.md        why no compression mode (practical, economic, mathematical)
│       ├── chunk-size.md         why 4KB — the full 10-point analysis
│       ├── performance.md        hash rate, proving cost, steady-state adequacy
│       ├── migration.md          emergency protocols, no algorithm agility, storage proofs
│       └── the-name.md           etymology: Hemera in the Protogenoi, genealogy of hash names
```

## Content mapping (monolith → split)

Every section gets its own standalone file. No merging, no nesting.

| Monolith section                    | Target file               | Type       |
|-------------------------------------|---------------------------|------------|
| Abstract (partial)                  | reference/README.md       | reference  |
| §1 Why a New Name                   | docs/explanation/why-hemera.md | explanation |
| §2 Permanence Constraint            | docs/explanation/why-hemera.md | explanation |
| §3.1-3.5 Parameter Decisions        | docs/explanation/parameters.md | explanation |
| §4.1 Parameters box                 | reference/README.md       | reference  |
| §4.2 Computational Elegance         | docs/explanation/parameters.md | explanation |
| §4.3 Sponge (capacity, flags, ops)  | reference/sponge.md       | reference  |
| §4.3.5 Why Not Compression          | docs/explanation/sponge-only.md | explanation |
| §4.4 Byte Encoding                  | reference/encoding.md     | reference  |
| §4.5 Output Format                  | reference/encoding.md     | reference  |
| §4.5.1 Raw CIDs rationale           | docs/explanation/content-ids.md | explanation |
| §4.6 Tree Hashing (spec)            | reference/tree.md         | reference  |
| §4.6.1 Why 4KB (rationale)          | docs/explanation/chunk-size.md | explanation |
| §4.7 Operational Semantics          | reference/sponge.md       | reference  |
| §4.8 Round Constant Generation      | reference/bootstrap.md    | reference  |
| §4.8.1 Security of Self-Bootstrap   | docs/explanation/self-bootstrap.md | explanation |
| §4.9 Matrix Construction            | reference/matrices.md     | reference  |
| §5 Ecosystem Context                | docs/explanation/security.md | explanation |
| §6 Performance                      | docs/explanation/performance.md | explanation |
| §7 Implementation Plan              | (drop — historical)       |            |
| §8 Migration/Emergency              | docs/explanation/migration.md | explanation |
| §9 The Name                         | docs/explanation/the-name.md | explanation |
| See also                            | reference/README.md       | reference  |
| References                          | reference/bibliography.md | reference  |
| Phase 3 API listing                 | reference/api.md          | reference  |

## Principles

1. **reference/ is austere** — what the system does. No rationale, no comparisons.
   Tables, code blocks, precise definitions. If you can express it as a formula, do.
   Exception: tightly-coupled rationale (why 4KB, why sponge-only, why self-bootstrap)
   stays with its spec section — do not split parent/child into separate files.

2. **docs/explanation/ is narrative** — standalone design essays. Background, context,
   trade-off analysis. References the spec but does not duplicate it.

3. **docs/tutorials/ are complete journeys** — reader follows steps, builds something,
   succeeds. No choices.

4. **docs/guides/ are task-oriented** — reader knows what they want, needs the steps.

5. **reference/README.md stays lean** — parameter box, abstract (3 paragraphs max),
   links to sub-pages. Under 100 lines.

6. **§7 Implementation Plan is dropped** — it was a roadmap, now historical.
   Completed phases are reflected in the code; future phases belong in project tracking.

## Resolved

- **Frontmatter**: this repo is a subgraph of `~/git/cyber/`. YAML frontmatter
  (tags, crystal-type, crystal-domain, stake) are knowledge graph nodes.
  Preserve in all files that have it. New reference/ files get frontmatter too.

- **Tutorials**: both CLI and Rust API. first-hash.md covers CLI,
  merkle-proof.md covers Rust API. Tutorials are structure-only for now —
  content written later when the API stabilizes.

## Scope

Phase 1 (now): split the monolith into reference/ + docs/explanation/.
Phase 2 (later): write tutorials and guides when API is stable.
