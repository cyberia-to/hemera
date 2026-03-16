---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera chunk size, why 4KB"
---

# Why 4 KB Chunks

Ten independent lines of evidence converge on 4096 bytes as the
optimal chunk size for verified streaming.

## 1. Field alignment

4096 = 2^12. Each chunk requires 74 absorptions + 1 binding =
75 permutations per leaf. Clean power-of-two alignment with the
field arithmetic.

## 2. OS page alignment

4 KB is the native page size on x86, ARM, and RISC-V. It matches
the default block size of ext4, XFS, NTFS, and APFS. It is the
minimum NVMe transfer unit. Zero-copy mmap operates at page
granularity — 4 KB chunks require no partial-page bookkeeping.

## 3. L1 cache fit

4 KB fits entirely in L1 data cache (32-64 KB typical). 8 KB
increases cache pressure. 16 KB exceeds L1 on most
microarchitectures. Processing a chunk without evicting working
set from L1 is a hard performance boundary.

## 4. STARK proof granularity

75 permutations x ~1,200 constraints = ~90,000 constraints per
leaf. Large enough to amortize proof overhead, small enough to
prove individual chunks without excessive trace length.

## 5. Tree depth and proof size

| Data size | Chunks     | Tree depth | Proof size |
|-----------|------------|------------|------------|
| 1 MB      | 256        | 8          | 512 B      |
| 1 GB      | 262,144    | 18         | 1,152 B    |
| 1 TB      | 268M       | 28         | 1,792 B    |
| 1 PB      | 274B       | 38         | 2,432 B    |
| 1 EB      | 281T       | 48         | 3,072 B    |
| 1 YB      | 288 x 10^18| 68         | 4,352 B    |

MMR depth at 10^24 cyberlinks remains tractable.

## 6. Overhead ratio

64 bytes of metadata per 4096-byte chunk = ~1.6% overhead.
At 256 B chunks: 25%. At 64 KB chunks: 0.1%. 1.6% is the
practical minimum where overhead is negligible but granularity
is still useful.

## 7. Deduplication quality

4 KB matches the page size of databases, VM disk images, and
document container formats. Content-defined chunking at 4 KB
aligns with existing storage deduplication infrastructure.

## 8. Streaming verification

Buffer one chunk + one Merkle proof = approximately 6 KB. A
receiver can verify and process data incrementally without
buffering the entire file.

## 9. Network transport

4 KB = 3 TCP segments at MTU 1500, or 1 jumbo frame at MTU 9000.
Legacy MTU 1500 networks carry a chunk in 3 packets. Jumbo frame
networks carry it in 1.

## 10. Bounded locality

Changing one byte requires rehashing: 75 permutations (the
affected chunk) + 2 x log2(N) permutations (Merkle path to root).
The blast radius of a single-byte edit is bounded and predictable.

## Comparison table

```
                    256B   1KB     4KB     8KB    16KB    64KB
Absorbs/chunk         5     19      74    147     293    1171
Perms/leaf            6     20      75    148     294    1172
1GB tree depth       22     20      18     17      16      14
1GB proof (bytes)  1408   1280    1152   1088    1024     896
Overhead ratio       25%     6%    1.6%   0.8%    0.4%    0.1%
OS page aligned       x      x      yes     x       x       x
L1 cache fit        yes    yes      yes     ~       x       x
STARK constraints   7.2K  24.0K   90.0K   178K   353K    1.4M
Streaming buffer   256B     1K      4K     8K     16K     64K
Dedup quality      poor   fair    good   good    fair    poor
Network packets       1      1       3      6      12      46
```

4 KB is the only row with yes on both page alignment and L1 cache fit.
