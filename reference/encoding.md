---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: Hemera encoding, byte encoding specification
---

# encoding specification

## Input Encoding (Bytes → Field Elements)

Pack input bytes into 7-byte little-endian chunks. Each 7-byte chunk is zero-extended to 8 bytes and interpreted as a u64 in little-endian order, producing one Goldilocks field element.

```
bytes[0..7]   → element 0    (zero-extend to u64 LE)
bytes[7..14]  → element 1
bytes[14..21] → element 2
...
bytes[49..56] → element 7    (= one full rate block)
```

Why 7 bytes, not 8: The maximum 7-byte value is 2⁵⁶ − 1 = 0x00FF_FFFF_FFFF_FFFF. The Goldilocks prime is p = 0xFFFF_FFFF_0000_0001. Since 2⁵⁶ − 1 < p, every 7-byte value is a valid field element without reduction. No conditional splitting, no branching, no overflow handling. The encoding is a single `u64::from_le_bytes` with a zero high byte — branchless, constant-time, and injective.

At 8 bytes per element, approximately 1 in 2³² inputs would require splitting (when the value ≥ p), making encoding data-dependent and variable-length. The 7-byte encoding trades 12.5% rate reduction (56 vs 64 bytes per block) for unconditional simplicity. At planetary scale, branch-free encoding is worth one extra permutation per 8 rate blocks.

Rate block: 8 elements × 7 bytes = 56 input bytes per absorption. One permutation processes 56 bytes of content.

## Output Encoding (Field Elements → Bytes)

Output uses the full canonical u64 representation: 8 bytes per element, little-endian. Output elements are guaranteed to be in [0, p) by the permutation — no reduction needed.

```
element 0 → bytes[0..8]     (u64 to LE bytes)
element 1 → bytes[8..16]
...
element 7 → bytes[56..64]   (= 64-byte hash output)
```

The asymmetry — 7 bytes in, 8 bytes out — is deliberate. Input encoding must be injective for collision resistance. Output encoding must preserve full field element fidelity for algebraic composability. These are different constraints with different optima.

## Padding (Hemera: 0x01 ∥ 0x00*)

After all input bytes are buffered:

1. Append a single 0x01 byte (padding marker)
2. Pad with 0x00 bytes to fill the rate block (56 bytes total)
3. Encode the padded block as 8 field elements and absorb
4. Store total input byte count in state[10] (capacity length field)

The padding is rate-aligned: every message, regardless of length, ends with exactly one padded absorption. The 0x01 marker distinguishes `message ∥ 0x00` from `message` — standard multi-rate padding adapted to the 7-byte element encoding.

## Output Format

A Hemera hash is 64 bytes. Nothing more. No version prefix, no mode byte, no escape hatch. The raw output of 8 Goldilocks field elements in little-endian canonical form IS the particle address.

```
Hemera output = 8 × 8 bytes = 64 bytes (little-endian, canonical range [0, p))
```

If Hemera is ever broken, the entire graph rehashes. Storage proofs make this possible. Versioning headers do not save you — they waste bytes multiplied by 10¹⁵ particles.
