---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera explanation, design rationale
---

# why Hemera works the way it does

design decisions behind the [[Hemera]] hash primitive.

## philosophy

- [[why-hemera]] — eight design principles: permanence, the tree, endofunction, self-reference, identity, unity, beauty, the name
- [[the-name]] — etymology: Hemera in the Protogenoi, genealogy of hash names

## parameters

- [[parameters]] — rationale for every parameter (field, S-box, width, rounds, computational elegance)
- [[chunk-size]] — why 4 KB chunks (10-point analysis)

## architecture

- [[sponge-only]] — why no compression mode (practical, economic, mathematical)
- [[capacity]] — structured capacity: one function, unlimited contexts, cost analysis
- [[content-ids]] — why raw 64-byte CIDs, no headers, endofunction closure
- [[self-bootstrap]] — why self-bootstrapping, non-circularity argument

## analysis

- [[security]] — security margins, quantum resistance, ecosystem comparison
- [[performance]] — hash rate, proving cost, steady-state adequacy

## operations

- [[migration]] — emergency protocols, no algorithm agility, storage proofs
