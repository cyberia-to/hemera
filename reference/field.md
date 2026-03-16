---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Goldilocks field, Hemera field
---

# field specification

the Goldilocks prime field specification lives in [[nebu]]: the standalone field arithmetic library.

**canonical source:** `~/git/nebu/reference/field.md`

## summary

all Hemera arithmetic operates over the Goldilocks prime field:

```
p = 2⁶⁴ − 2³² + 1 = 0xFFFFFFFF00000001
```

the field provides native u64 arithmetic, two-adicity of 32 for NTT, and the minimum invertible S-box exponent d = 7. see [[nebu/reference/field]] for the complete specification: arithmetic operations, properties, NTT compatibility, encoding rules, and hardware support.

## Hemera-specific usage

- **S-box exponent:** d = 7 (minimum invertible for this field). see [[permutation]].
- **encoding:** 7 bytes per field element, little-endian. see [[encoding]].
- **MDS matrices:** M_E and M_I are defined over this field. see [[matrices]].
- **round constants:** 192 elements of this field. see [[constants]].
