---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera explanation, design rationale
---

# why Hemera works the way it does

design decisions behind the [[Hemera]] hash primitive.

## philosophy

- [[why-hemera]] — six design principles: permanence, endofunction, self-reference, identity, unity, the double seven
- [[the-name]] — etymology: Hemera in the Protogenoi, genealogy of hash names

## parameters

- [[parameters]] — rationale for every parameter (field, S-box, width, rounds, computational elegance)
- [[chunk-size]] — why 4 KB chunks (10-point analysis)

## architecture

- [[sponge-only]] — why no compression mode (practical, economic, mathematical)
- [[content-ids]] — why raw 64-byte CIDs, no headers, endofunction closure
- [[self-bootstrap]] — why self-bootstrapping, non-circularity argument

## analysis

- [[security]] — security margins, quantum resistance, ecosystem comparison
- [[performance]] — hash rate, proving cost, steady-state adequacy

## operations

- [[migration]] — emergency protocols, no algorithm agility, storage proofs
