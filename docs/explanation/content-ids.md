---
tags: cyber, cip
crystal-type: entity
crystal-domain: cyber
alias: "Hemera content identifiers, raw CIDs, no headers"
---

# content identifiers: raw bytes, no headers

nox CIDs are raw 64-byte Hemera outputs. No multicodec, no multihash, no version byte.

Comparison:

| | IPFS CIDv1 | nox CID |
|---|---|---|
| size | 36-38 bytes typical | 64 bytes fixed |
| structure | version + codec + hash-code + length + digest | digest only |
| hash agility | yes (identified by prefix) | no (one hash, permanent) |
| self-describing | yes | no — the system is the description |

Five reasons for no headers:

1. **Overhead at scale.** 5 bytes x 10^15 particles = 5 PB of metadata that describes nothing the system does not already know.

2. **One hash function — nothing to disambiguate.** Every CID in nox is a Hemera output. A header saying "this is Hemera" adds no information.

3. **Headers create the illusion of upgradability.** A version byte implies the system can switch hash functions. It cannot. The cost of rehashing the graph is O(10^15) operations. The header promises a capability the system will never exercise.

4. **Endofunction closure.** Hemera(Hemera(x) || Hemera(y)) must produce a valid CID. Headers break this — the output of Hemera is 64 raw bytes, and prepending metadata to that output before feeding it back means the input to the next hash includes non-content bytes.

5. **Flat namespace.** Every entity — particle, neuron, link, block — has the same 64-byte address. No type tags, no length prefixes, no version markers. A CID is a CID.

## compatibility

A thin translation layer at network boundary converts between nox CIDs and external formats:

```
wire format (inbound):  [multicodec | multihash | digest] → strip → 64-byte CID
wire format (outbound): 64-byte CID → prepend [multicodec | multihash] → CIDv1
```

The translation is stateless and lossless. Internal storage and computation never see headers.

A content identifier identifies content. It does not identify itself.
