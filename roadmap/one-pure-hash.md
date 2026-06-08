---
status: draft
breaks_hash: yes
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# one pure hash — capacity for invariants only, no mode variance

hemera is the cyber stack's identity function: `hash(data) → particle`.
this proposal removes the two pieces of inherited variance — content-type
in capacity, and BLAKE3-style hash modes — leaving one pure hash.
supersedes [[capacity-typing]] (kept for rationale, not implemented).

context: this is the hemera half of a stack-wide type/terms cleanup. the
type theory lives in `soft3/specs/types.md`, the vocabulary in
`soft3/specs/terms.md`, the nox half in `nox/roadmap/data-model.md`, and
the cross-repo execution sequence in `soft3/roadmap/migration.md`.

## the rule

a hash mechanism earns a place in hemera only if it is a **substrate
invariant** — a truth of the system that can never legitimately change.
type and convenience modes are neither.

## 1. type does not go in capacity — reject capacity-typing

`capacity-typing.md` proposes `hash_typed(bytes, TYPE_PARTICLE)` —
content type in `state[14]`, a frozen genesis registry, type intrinsic to
identity. its goal (distinct identity for distinct referents) is right;
its mechanism is the one `types.md` rejects:

- a **refinement** (`word`, `bool`) is a proposition about a value,
  identity-neutral — it never touches the hash.
- a **nominal** type (`timestamp`, `particle` vs `cyberlink`) is a
  distinct referent, expressed as **structure**: `pair(domain, value)`.
  different content → different particle, for free, through ordinary
  content-addressing. the domain marker is itself a particle, so the type
  is a graph node you can link to — open, not a frozen registry.

so the type-confusion `capacity-typing.md` worries about is solved one
layer up, by structure, not by a capacity lane. `state[14]` stays unused.
mark `capacity-typing.md` superseded.

## 2. the real capacity layout (spec vs code)

the nox spec claimed `capacity[14] = atom type tag`. the code never
implements it. the actual 16-element state is `rate[0..8] | capacity[8..16]`:

```
state[8]   counter     chunk position in a file tree        hemera tree
state[9]   flags        FLAG_CHUNK / FLAG_PARENT / FLAG_ROOT   hemera tree   ← KEEP
state[10]  msg_length   absorbed length, bound at finalize    hemera sponge
state[11]  mode         hash / keyed / derive_key             hemera sponge ← REMOVE (§3)
state[12]  ns_min       NMT namespace lower                   hemera NMT
state[13]  ns_max       NMT namespace upper                   hemera NMT
state[14]  —            unused (capacity-typing proposed type) ← stays unused
state[15]  —            unused
```

`state[9]` (leaf/node/root) is the one genuine invariant — second-preimage
safety, true forever by the mathematics of authenticated trees. it earns
capacity. nox must stop writing the type tag into `data[8]` and the
counter argument (the nox half handles this).

## 3. remove the sponge modes — one pure hash

`state[11]` carries BLAKE3's three modes — `hash`, `keyed_hash`,
`derive_key` (`sponge.rs:13-16`, ported with commit `0278115` "for
BLAKE3"). they are a general-purpose-library convenience, not a stack
need: the cyber stack already separates concerns — hemera does identity,
**mudra** does keyed crypto (KEM, AEAD, threshold, KDF).

evidence (survey): across the whole stack the modes have exactly **one**
real caller — `radio/iroh-relay/src/protos/handshake.rs:215,594-597`
(`hemera::derive_key`, `keyed_hash`), doing challenge-keying — squarely
mudra's job. (bita's `derive_key` is its own PBKDF2; everything else is
hemera's CLI/tests/wgsl mirror.)

plan:

- move radio's handshake KDF / keyed-MAC to **mudra** (gate: confirm
  mudra exposes a KDF and a keyed-MAC — almost certainly yes; AEAD
  contains a MAC, a KEM stack a KDF).
- delete `keyed_hash` / `derive_key` (`lib.rs:94,105`) and the
  `DOMAIN_KEYED` / `DOMAIN_DERIVE_*` constants; drop the CLI subcommands.
- `state[11]` frees. per the discipline, it stays empty — capacity is
  governance-scarce; an empty lane is correct until a real invariant
  needs it.

result: hemera is `hash(data) → particle` and nothing else. one mode, no
`state[11]` variance, no `state[14]` typing.

## open question

does anything beyond radio need a fast symmetric MAC / KDF that mudra
would not cover? if so, it lives in mudra, not hemera. confirm before
deleting the modes.
</content>
