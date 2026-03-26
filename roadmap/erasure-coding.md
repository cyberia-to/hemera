---
status: draft
tags: hemera, roadmap
crystal-type: entity
crystal-domain: crypto
---
# erasure coding over Goldilocks

Reed-Solomon erasure coding as a hemera module. same field, same NTT, same pipeline — encode data for availability, commit via hemera, verify via Lens.

## motivation

hemera's mission: identity and authentication for content. erasure coding is the availability extension — content addressed by hemera hash, recoverable via hemera erasure encoding. one crate handles both "what is it?" (hash) and "can I get it back?" (erasure).

the math is already here. Reed-Solomon encoding is polynomial evaluation at distinct points over a finite field. hemera already lives in Goldilocks. nebu already provides NTT. RS encoding = NTT(data) + evaluate at extension points. no new field, no new arithmetic.

## specification

### interface

```
rs_encode(data: &[F_p], k: usize) → Vec<F_p>
  input:  k data symbols (Goldilocks field elements)
  output: n = 2k symbols (k data + k parity)
  method: interpret data as polynomial coefficients,
          evaluate at n distinct points via NTT

rs_decode(shares: &[(usize, F_p)], k: usize) → Result<Vec<F_p>, DecodeError>
  input:  any k-of-n symbols with their positions
  output: original k data symbols
  method: Lagrange interpolation (or equivalent via inverse NTT)
  error:  DecodeError if fewer than k shares provided

rs_verify_encoding(commitment: H, row_index: usize, cells: &[(usize, F_p)], k: usize) → bool
  verify that cells are consistent with claimed polynomial degree < k
  if degree(interpolated) ≥ k → bad encoding → fraud proof
```

### 2D grid layout (for DAS)

```
rs_encode_2d(data: &[F_p], sqrt_k: usize) → Grid2D
  arrange data into √k × √k grid
  RS-extend rows: √k → 2√k
  RS-extend columns: √k → 2√k
  result: 2√k × 2√k grid (4× original size)

rs_sample_verify(grid_commitment: H, row: usize, col: usize, value: F_p, proof: LensOpening) → bool
  verify single cell against grid commitment
  uses Lens opening (from zheng) — not hemera internal
```

### fraud proof generation

```
rs_fraud_proof(row_commitment: H, cells: &[(usize, F_p)], k: usize) → Option<FraudProof>
  if cells.len() ≥ k+1:
    interpolate polynomial from k+1 points
    if degree(poly) ≥ k: encoding is invalid
    return FraudProof { cells, interpolated_poly, row_commitment }
  else: None (insufficient cells for fraud detection)
```

## why hemera, not a separate crate

1. **same field.** RS over Goldilocks uses identical F_p arithmetic. no conversion.

2. **same NTT.** RS encoding is a domain evaluation via NTT — the same NTT nebu provides, the same NTT hemera's Poseidon2 permutation structure is designed around.

3. **pipeline continuity.** hemera already does: data → hash → tree → commitment. erasure extends to: data → RS encode → hash chunks → tree → commitment. the encode step inserts before the existing hash step.

4. **scope coherence.** hemera = "make data identifiable and available." hash = identifiable. erasure = available. same mission, same crate.

## what stays outside hemera

- **DAS sampling protocol** (which cells to sample, how many, confidence levels) → bbg
- **Lens opening proofs** for grid cells → zheng
- **network-level chunk distribution** → radio
- **fraud proof broadcasting** → radio

hemera provides the codec. consumers provide the protocol.

## cost model

| operation | field ops | notes |
|-----------|-----------|-------|
| RS encode (k symbols) | O(k log k) | one NTT |
| RS decode (k shares) | O(k log k) | one inverse NTT + interpolation |
| fraud proof verify | O(k) | polynomial evaluation at k+1 points |
| 2D grid encode (k² data) | O(k² log k) | 2k row NTTs + 2k column NTTs |

## dependencies

- nebu: NTT over Goldilocks (evaluation and interpolation domains)
- hemera (internal): hash chunks after RS encoding

## open questions

- systematic vs non-systematic encoding (systematic = first k symbols are original data, simpler but leaks structure)
- optimal extension factor (2× is standard, 3× or 4× increases availability at cost of bandwidth)
- interleaved coding (multiple polynomials share evaluation points — amortizes overhead)
- integration with hemera tree: should RS-encoded chunks be leaves in the existing Merkle tree, or does erasure coding replace the tree for availability purposes?
