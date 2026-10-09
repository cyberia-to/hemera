---
tags: hemera, audit, performance, permutation
crystal-type: audit
crystal-domain: crypto
---
# permutation performance — 2026-10-09

Work package P of the proof-system repair: make the hemera permutation and
its 2-to-1 node compression fast on Apple Silicon and in portable Rust,
without changing an output bit. Branch `perf/permutation-neon` on
`23f3bbc` (hemera 0.3.1).

Machine: Apple M4 Max (12 P + 4 E cores), 48 GiB, macOS 26.4.1, rustc
1.95.0 (Homebrew), default `release` profile (no LTO, no
`target-cpu`). The machine was shared with other sessions throughout:
load average 8–19 during every measurement below. macOS offers no core
pinning; absolute numbers carry ±10 % run-to-run noise, which is why the
final comparison alternates binaries (§3).

## 1. bit-exactness

Every optimised path is compared on canonical values against
`reference.rs`, a frozen oracle that keeps the 0.3.1 arithmetic and round
order but computes x⁻¹ by plain square-and-multiply over the bits of p−2
(the definition, independent of any addition chain).
`rs/tests/differential.rs`, release build, `HEMERA_DIFF_N` default 10⁶:

| family | inputs |
|---|---|
| permutation, single and `permute_batch` (flush sizes random 1..=40, so every lane group 16/8/4/2/1 runs) | 1 000 000 states |
| `hash` (lengths 0..=200 B, across the 56-byte rate) | 1 000 000 messages |
| `hash_node`, single and `hash_node_batch`, both root flags | 1 000 000 pairs |
| `hash_leaf` | 125 000 leaves |
| `hash_leaf_batch` (groups 1..=40, equal and unequal lengths) | 250 000 leaves |
| x⁷ RF 6+6 / RP 48 profile shape, and the same shape with x⁻¹, single and batched | 100 000 states each |

Inputs include non-canonical representatives (`[p, 2⁶⁴)`), all-zero and
all-p states, and edge limbs (0, 1, p−1, p, p+1, 2⁶⁴−1, 2³², 2³²−1).
Plus 100 000 states checking that `permute_traced` and `StepSponge` end
bit-identical (raw limbs) to `permute`. Total ≈ 3.7 × 10⁶ randomized
inputs, 0 mismatches, 6.8 s on all cores.
The same test passes with the portable arithmetic forced on aarch64
(`RUSTFLAGS="--cfg hemera_portable"`), and in a debug build (10⁴ per
family, overflow checks on). The pinned vectors (`rs/tests/vectors.rs`)
and all 247 unit tests pass unchanged.

Output representation: `permute`, `permute_traced` and the last
`StepSponge` round now return canonical limbs. The 0.3.1 code could
return a limb in `[p, 2⁶⁴)` (field-equal, probability ≈ 2⁻³² per limb);
the consumers found (hash output encoding, the wgsl GPU test, zheng
`execution/hash.rs` and `ccs/root.rs`) read canonical values. `Goldilocks * Goldilocks` returns the same
representative as before, bit for bit.

## 2. steps, measured during development

`examples/bench.rs` (median of N timed batches) and `examples/split.rs`
(median of 41). Single runs under load, so read the trend, not the last
digit. "batch" = 1024 independent permutations through the best API at
that step.

| step | change | permute, single (ns) | batch (ns / perm) |
|---|---|---|---|
| 0 | baseline 0.3.1 | 3 979 – 4 042 | 3 806 – 4 049 (loop) |
| 1 | Goldilocks mul: hand-scheduled aarch64 reduction (latency form) | 3 654 | 3 258 (loop) |
| 2 | kernel generic over profile and lanes: u128 external layer, `x' = (d₀+1)·y + (rest + c)` carried directly, depth-69 inverse chain, Montgomery batch inversion, 8 lanes | 2 961 – 3 155 | 1 022 – 1 201 |
| 2b | lane count sweep | — | 4: 1 416 · 8: 1 132 · 16: 968 · 32: 912 |
| 3 | 9-instruction throughput mul form for S-boxes and the internal layer; 16 lanes | 2 964 | 897 |
| 4 | partial rounds scheduled per lane count (internal-layer products issued before the S-box for 1 lane; one fused multiply-add per element for many) | 2 832 – 2 865 | 793 – 837 |
| 5 | the 15 internal-layer rows of round r interleaved into round r+1's inversion chain (program order, so the OoO core overlaps them) | 2 747 – 2 873 | 722 – 734 |
| 6 | batch tails in groups of 8/4/2 instead of one by one | — | 20 states: 781 (before: 16 × 722 + 4 × 2 750 ⇒ 1 128 computed) |

Breakdown at step 5 (`split`, median): single = full rounds 413 + partial
rounds 2 347; batched = full rounds 388 + partial rounds 348 ns per
permutation. Replacing the S-box by the identity (throwaway build) put
the batched linear layers at 130 ns (external) and 196 ns (internal), so
batched inversion costs ≈ 150–210 ns per permutation.

### Goldilocks multiplication (scratch harness, 10⁷ random pairs checked per variant)

| form | dependent-chain latency (ns) | throughput, 8–16 chains (ns) |
|---|---|---|
| 0.3.1 `reduce128` as compiled | 2.35 – 2.54 | 0.43 – 0.45 |
| asm, constant-time `csel` for the borrow | 2.25 – 2.35 | 0.49 – 0.52 |
| asm latency form (shipped `arith::mul`): borrow as predicted branch, `t1` and `t1+ε` in parallel, carry → `csel` | 1.92 – 2.03 | 0.42 – 0.51 |
| asm throughput form (shipped `arith::mul_t`): shifted/extended-register operands, 9 instructions | 2.16 – 2.42 | 0.38 – 0.42 |

The latency form is ~7.7 cycles at the measured ~3.9 GHz: `umulh` (3) +
shift + subtract + add + select. The borrow branch fires when
`lo < hi_hi < 2³²` (≈ 2⁻³² per product); Plonky3's Goldilocks reduction
makes the same choice. It is a data-dependent branch in a hash that also
runs keyed; the constant-time form costs ≈ 0.3 ns per serial multiply
(≈ +16 × 70 × 0.3 ns ≈ +0.34 µs per single permutation). Recorded here so
the trade is visible.

### NEON (`examples/neon_mul.rs`)

aarch64 NEON has no 64×64→128 multiply; a 2-lane Goldilocks product
takes four `umull` plus carry and reduction logic (~29 vector
instructions). Checked equal to the scalar product on 10⁶ random pairs.

| | ns per lane-multiplication (median) |
|---|---|
| scalar, 16 independent chains | 0.453 |
| NEON, 8 vectors (16 lanes) | 0.966 |
| NEON, 16 vectors (32 lanes) | 0.853 |
| hybrid, 12 scalar + 2 vectors | 0.577 |
| hybrid, 16 scalar + 8 vectors | 0.479 |
| scalar latency, 1 chain | 2.041 |
| NEON latency, 1 vector (per lane) | 4.464 |

NEON emulation is ~2× slower per lane than scalar, and adding NEON lanes
beside scalar chains did not raise total throughput in either mix
measured. Lane parallelism is therefore done
as scalar interleave (ILP) in a structure-of-arrays layout, which the
kernel does for 2/4/8/16 lanes. NEON for the addition-only external layer
(≈ 130 ns of 720 per batched permutation) was not attempted; the
ceiling of that change is below 18 %.

### honeycrisp

`honeycrisp/acpu/src/field/goldilocks.rs` offers `gl_mul` (inline
`mul`+`umulh`, then the same reduction as 0.3.1) and `gl_mul_batch` (4
interleaved scalar chains). No vector or matrix-unit field multiply
exists there to use; `gl_mul` reduces exactly like the 0.3.1 row of the
table above, which the shipped forms beat (acpu itself was not
benchmarked). Not used.

### nebu

`strata/nebu/rs/field.rs::reduce128` is the 0.3.1 reduction (table
above). Side finding, outside this package: a standalone copy of
`nebu/rs/algebra_impl.rs` `dot` + its local `reduce128` (lines 73–117)
returns the wrong value for 10 000 of 10 000 random length-4 inputs
(the u128 accumulator wraps after two products, and `hi.wrapping_mul(ε)`
is only correct for `hi < 2³²`). Measured on the copy, not on nebu itself.

## 3. final numbers (alternating binaries)

Baseline = `origin/main` 23f3bbc with the same bench (batch APIs as plain
loops). aarch64 = this branch. portable = this branch with
`--cfg hemera_portable` (no asm; the code path non-aarch64 targets get).
Five alternating rounds × 31 runs each, median of the five medians,
ns per operation. Load average 9–17.

| operation | baseline | aarch64 | portable | speedup aarch64 | speedup portable |
|---|---|---|---|---|---|
| Goldilocks mul, dependent chain | 2.7 | 2.0 | 2.6 | 1.35× | 1.04× |
| permute, single (chained) | 4 146 | 2 932 | 3 803 | 1.41× | 1.09× |
| hash 64 B (2 permutations) | 8 482 | 6 082 | 7 558 | 1.39× | 1.12× |
| hash_node, single (chained) | 4 166 | 3 026 | 3 648 | 1.38× | 1.14× |
| 1024 × permute, loop | 4 132 | 2 992 | 3 953 | 1.38× | 1.05× |
| 1024 × `permute_batch` | 4 084 | 832 | 914 | 4.91× | 4.47× |
| 1024 × `hash_node_batch` | 4 079 | 784 | 998 | 5.20× | 4.09× |
| 1024 × hash_leaf 144 B, loop | 16 799 | 12 160 | 15 851 | 1.38× | 1.06× |
| 1024 × `hash_leaf_batch` 144 B | 16 775 | 3 268 | 3 929 | 5.13× | 4.27× |

The x⁷ RF 6+6 / RP 48 shape (profile-v2 proposal, hemera PR #15) through
the same kernel (`split`, placeholder constants — cost only): single
1 375 ns, batched 1 242 ns per permutation. Against the live profile:
half the single-permutation latency (no 70-deep inverse chain), 1.7× the
batched cost (48 partial rounds × 16 multiplications against 16 × ~25).

## 4. lens WHIR verify

Permutations counted with a throwaway counter (hemera, not committed) in
one `Whir::verify`, rate 1/16, lens `feat/rs-whir-pcs` 8f188a9:

| num_vars | permutations | node compressions | leaf permutations (leaves) | other (transcript, PoW) |
|---|---|---|---|---|
| 10 | 530 | 245 | 224 (56) | 61 |
| 20 | 2 467 | 1 572 | 680 (114) | 215 |

Verify time, three lens binaries alternated five times (15 runs each,
median of medians, load 13–19):

| num_vars | baseline hemera | this branch, lens unchanged | this branch + lens batching (throwaway) |
|---|---|---|---|
| 10 | 2.606 ms | 1.913 ms | 0.794 ms |
| 20 | 11.928 ms | 8.769 ms | 3.592 ms |

"lens batching" is a throwaway lens diff (not committed): `merkle::verify`
hashes each level's nodes with one `hash_node_batch` call, and WHIR's
`check_opening` hashes a round's opened leaves with one `hash_leaf_batch`
call. `cargo test --release -p cyber-lens-rspcs` (28 tests: completeness
to 2²⁰, wrong-claim rejection, bit-flip scans, cheating prover) passes
with it.

Projection by count × cost (cost per permutation from §3: single
2.93 µs, batched 0.80 µs; non-hash residual = baseline verify − 530 or
2 467 × 4.15 µs = 0.41 ms at 2¹⁰, 1.69 ms at 2²⁰):

| num_vars | lens unchanged: residual + all × 2.93 µs | lens batching: residual + (nodes + leaf) × 0.80 µs + other × 2.93 µs |
|---|---|---|
| 10 | 0.41 + 1.55 = 1.96 ms (measured 1.91) | 0.41 + 0.38 + 0.18 = 0.97 ms (measured 0.79) |
| 20 | 1.69 + 7.23 = 8.92 ms (measured 8.77) | 1.69 + 1.80 + 0.63 = 4.12 ms (measured 3.59) |

The ≤ 1 ms target holds at 2¹⁰ once lens adopts the batch calls; at 2²⁰
it does not.

## 5. what limits further speedup

- single permutation, live profile: the partial rounds are one serial
  chain, 16 × (69 squarings/multiplications of the inverse + 1
  multiply-add) ≈ 1 100 dependent multiplications at ≈ 2.0 ns = 2.2 µs of
  the 2.75–2.93 µs. The chain depth is ≥ 64 for any exponentiation
  (log₂ p); 69 is within 5 of that bound. A further cut needs a
  non-exponentiation inverse (binary GCD / safe-gcd — not evaluated) or
  the x⁷ profile (§3: 1.38 µs).
- batched: instruction throughput. A Goldilocks product is 9–13 scalar
  instructions (no 64-bit modular multiply, no useful NEON form); a
  batched permutation retires ≈ 14 k instructions. Full rounds are
  512 S-box multiplications; partial rounds 15 internal-diagonal
  multiply-adds per round — the diagonal entries are full 64-bit values,
  so no shift-and-add form exists (the Poseidon2 reference picks small
  diagonals; changing them changes the hash).
- lens: transcript and PoW permutations (61 / 215) are serial; the
  non-hash verifier work (sumcheck, folding, Fp3 arithmetic) is ≈ 0.4 ms
  at 2¹⁰ and ≈ 1.7 ms at 2²⁰ by the residual estimate — at 2²⁰ that alone
  exceeds 1 ms. The verifier is single-threaded; spreading the 2 252
  batchable permutations at 2²⁰ across cores is the remaining hashing
  lever (≈ 1.8 ms single-core; not measured).

## reproduce

```text
cargo test  --release -p cyber-hemera --test differential -- --nocapture
cargo run   --release -p cyber-hemera --example bench [-- runs]
cargo run   --release -p cyber-hemera --example split
cargo run   --release -p cyber-hemera --example neon_mul
RUSTFLAGS="--cfg hemera_portable" cargo test --release -p cyber-hemera   # portable path
```
