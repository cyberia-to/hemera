---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera bootstrap, round constant generation
diffusion: 0.00010722364868599256
springs: 0.00007019991600688145
heat: 0.00003419142694206788
focus: 0.00008151008453347325
gravity: 0
density: 0
---

# round constant generation

Hemera generates her own round constants. No external primitives.

The permutation structure (S-box x^7, matrices M_E and M_I, round flow 4+64+4) is fully defined before constants exist. With all constants set to zero, the permutation is still a well-defined nonlinear function. We call this Hemera_0.

```
1. Define Hemera_0 = Hemera permutation with all 192 round constants = 0
2. Feed the genesis word through Hemera_0 as a sponge:

   input = [0x63, 0x79, 0x62, 0x65, 0x72]    — "cyber" as raw bytes

   state = [0; 16]
   absorb input into state using Hemera_0
   squeeze 192 field elements from state using Hemera_0

3. First 128 elements → RC_FULL[128]
   Next 64 elements   → RC_PARTIAL[64]

4. Hemera = Hemera_0 + these constants. Freeze forever.
```

## seed

Five bytes [0x63, 0x79, 0x62, 0x65, 0x72]. Not "UTF-8 encoding of cyber" — the bytes themselves are the specification. No character set, no encoding, no text convention.

The parameters do not appear in the seed because they are not data — they are the structure of Hemera_0 itself.

## zero-state fixed point

The all-zero state is a fixed point of Hemera_0. This does not affect constant generation because the sponge absorbs the seed first. After absorbing one non-zero byte, the state is non-zero.

Do not use Hemera_0 for any purpose other than constant generation from non-trivial seeds.

## reproducibility

The procedure is fully deterministic. Same S-box, same matrices, same round structure, same sponge, same seed — same 192 field elements.