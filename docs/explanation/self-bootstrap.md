---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera self-bootstrap, self-bootstrapping security"
---

# security analysis of self-bootstrapping

## why self-bootstrapping

Hemera is a system built entirely on Goldilocks field arithmetic. Importing SHA-256 or ChaCha20 to generate round constants would introduce a foreign primitive — a second cryptographic assumption unrelated to the structure being built. Using Hemera_0 (the zero-constant permutation) as its own PRNG is the most honest construction: the security of the constants reduces to the security of the structure itself.

## verifiability

If someone claims the constants are backdoored, they must argue that the zero-constant permutation produces weak output from five non-zero bytes. This is strictly harder than attacking any external PRNG, because the zero-constant permutation is a subset of the full permutation — it has the same algebraic structure, the same S-box, the same MDS matrix, just with all round constants set to zero.

Anyone can reproduce the constants by running Hemera_0 on the specified seed. No trust in an external tool, no "nothing-up-my-sleeve" argument that requires faith in a different algorithm.

## the non-circularity argument

```
algebraic structure -> Hemera_0 -> constants -> Hemera (done)
```

Hemera_0 is fully-specified and independent. It requires no round constants — they are all zero. The algebraic structure (Goldilocks field, d=7 S-box, t=16 MDS matrix) exists before any constants are generated. Hemera_0 is applied to a fixed seed to produce the constant table. The constants are then loaded into the full permutation.

There is no fixed-point equation. There is no circularity. The construction is a straight-line computation from axioms to parameters.

## coupled security

With an external PRNG (say, ChaCha20), two independent cryptographic assumptions are needed:

1. The Poseidon2 algebraic structure is sound.
2. ChaCha20 does not produce weak constants.

If either assumption fails, the system fails. The assumptions are independent — a breakthrough in stream cipher cryptanalysis could compromise the constants even if Poseidon2 itself is secure.

With self-bootstrapping, only one assumption is needed: "the Poseidon2 algebraic structure is sound." If the structure is sound, the zero-constant permutation is a competent PRNG. If the structure is unsound, no choice of constants saves it. The security of the constants and the security of the permutation are the same claim.
