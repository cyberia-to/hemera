# Persistent production finalization

Authorization: continue across as many sessions as necessary. The active goal
is not complete after an intermediate experiment or implementation milestone.

## Required outcome

Resolve the exact inverse-16 profile against concrete security games, or find
an attack invalidating a requirement and carry a justified replacement through
production selection. Functional correctness and cryptanalytic security have
separate obligations; neither substitutes for the other.

Working requirements inferred from current interfaces and consumers:

- Content identifiers: approximately 128-bit classical collision target for
  the four-field-element digest. The codomain is F_p^4, not uniform arbitrary
  256-bit strings. Preimage and second-preimage targets must account for input
  domain size; full-digest nominal ceiling is log2(p^4), approximately 256.
- Fiat-Shamir: zheng actually consumes Hemera challenges. Collision resistance
  alone is insufficient to justify that use; transcript and random-oracle
  assumptions need explicit treatment. A statistical property of field encoding
  is not by itself a break of the correctly specified codomain.
- Implementation: exact field arithmetic, total inverse, all round constants,
  sponge/length/domain encoding, streaming and structural commitments.
- No unconditional security or post-quantum level is asserted merely from
  output length, tests, solver timeouts, or Eidos type checking.

## Work sequence and exit evidence

1. Run actual searches using the full-round constraint model. Validate solver
   encodings using SAT/UNSAT controls; report resource limits and unknowns.
   Recheck any candidate against the production implementation.
2. Extend beyond the seven-byte slice, investigate reduced-round scaling and
   algebraic/subspace attacks, and resolve whether any full-round result breaks
   the stated games. Never relabel a distinguisher as a collision/preimage.
3. Carry the universal arithmetic obligations from separately trusted SMT into
   proof terms checked by Eidos; establish algebraic premises for the actual
   implementation and harden the proof infrastructure as needed.
4. Consolidate security arguments and independent-review obligations for the
   exact profile. If an alternative is needed, preserve the same requirements
   and compare the complete profile, not only its round count.
5. Finalize versioned parameters, vectors, implementation checks and deployment
   contract only when the required evidence exists. Keep goal active otherwise.

Current baseline: full-round model and universal manually translated reduce128
SMT obligations exist; neither a full-round break nor sufficient cryptanalytic
security argument exists. Eidos has binary arithmetic and concrete reduction
certificates. See `research/full-round-status.md` and subsequent campaign reports.
