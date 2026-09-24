//! Experimental format-independent structural commitments.
//! Encoding and trust boundary: specs/structural-commitments.md.
use crate::{
    field::P,
    sponge::{Hash, Hasher},
};

const PREFIX: &[u8] = b"hemera-struct-v1\0";

/// A blob or nominal record identity, independent of position in a sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentId(Hash);

/// An ordered sequence commitment, including its cardinality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceId(Hash);

impl ContentId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
    pub fn from_bytes(bytes: [u8; 32]) -> Option<Self> {
        let hash = Hash::from_bytes(bytes);
        canonical(&hash).then_some(Self(hash))
    }
}
impl SequenceId {
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
    pub fn from_bytes(bytes: [u8; 32]) -> Option<Self> {
        let hash = Hash::from_bytes(bytes);
        canonical(&hash).then_some(Self(hash))
    }
}

/// Siblings are leaf-to-root. Direction and subtree counts are derived.
#[derive(Debug, Clone)]
pub struct InclusionProof {
    pub index: u64,
    pub count: u64,
    pub siblings: [Hash; 64],
    pub sibling_count: usize,
}

fn frame(tag: u8) -> Hasher {
    let mut h = Hasher::new();
    h.update(PREFIX);
    h.update(&[tag]);
    h
}

/// Commit to exact bytes, including their length.
pub fn blob(bytes: &[u8]) -> ContentId {
    let mut h = frame(0);
    h.update(&(bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    ContentId(h.finalize())
}

fn branch(left: Hash, ln: u64, right: Hash, rn: u64) -> Hash {
    let mut h = frame(1);
    h.update(&ln.to_le_bytes());
    h.update(&rn.to_le_bytes());
    h.update(left.as_bytes());
    h.update(right.as_bytes());
    h.finalize()
}

fn split(n: u64) -> u64 {
    1u64 << (63 - (n - 1).leading_zeros())
}

fn subtree(items: &[ContentId]) -> Hash {
    match items.len() {
        0 => frame(2).finalize(),
        1 => items[0].0,
        n => {
            let k = split(n as u64) as usize;
            branch(
                subtree(&items[..k]),
                k as u64,
                subtree(&items[k..]),
                (n - k) as u64,
            )
        }
    }
}

fn root(inner: Hash, n: u64) -> SequenceId {
    let mut h = frame(3);
    h.update(&n.to_le_bytes());
    h.update(inner.as_bytes());
    SequenceId(h.finalize())
}

pub fn sequence(items: &[ContentId]) -> SequenceId {
    root(subtree(items), items.len() as u64)
}

/// A sequence cannot contain more than u64::MAX items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SequenceOverflow;

/// Ordered sequence accumulator: at most 64 subtree digests, no heap allocation.
#[derive(Debug, Clone)]
pub struct SequenceBuilder {
    stack: [Hash; 64],
    count: u64,
}

impl Default for SequenceBuilder {
    fn default() -> Self {
        Self {
            stack: [Hash::from_bytes([0; 32]); 64],
            count: 0,
        }
    }
}

impl SequenceBuilder {
    pub fn push(&mut self, item: ContentId) -> Result<(), SequenceOverflow> {
        let next = self.count.checked_add(1).ok_or(SequenceOverflow)?;
        let mut digest = item.0;
        let mut level = 0;
        while (self.count >> level) & 1 == 1 {
            let n = 1u64 << level;
            digest = branch(self.stack[level], n, digest, n);
            level += 1;
        }
        self.stack[level] = digest;
        self.count = next;
        Ok(())
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn finish(&self) -> SequenceId {
        if self.count == 0 {
            return root(frame(2).finalize(), 0);
        }
        let mut digest = frame(2).finalize();
        let mut count = 0;
        for level in 0..64 {
            let n = 1u64 << level;
            if self.count & n != 0 {
                digest = if count == 0 {
                    self.stack[level]
                } else {
                    branch(self.stack[level], n, digest, count)
                };
                count += n;
            }
        }
        root(digest, self.count)
    }
}

/// Nominal type/role is expressed as committed structure.
pub fn record(kind: &[u8], fields: &[ContentId]) -> ContentId {
    record_from_sequence(kind, sequence(fields))
}

/// Bind a record kind to a field sequence without requiring all field values.
pub fn record_from_sequence(kind: &[u8], fields: SequenceId) -> ContentId {
    let mut h = frame(4);
    h.update(blob(kind).as_bytes());
    h.update(fields.as_bytes());
    ContentId(h.finalize())
}

/// Generate a reference opening in O(n) hashing work and O(log n) proof space.
pub fn prove(items: &[ContentId], index: usize) -> Option<(SequenceId, InclusionProof)> {
    if index >= items.len() {
        return None;
    }
    fn descend(items: &[ContentId], index: usize, path: &mut InclusionProof) -> Hash {
        if items.len() == 1 {
            return items[0].0;
        }
        let k = split(items.len() as u64) as usize;
        let (left, right) = if index < k {
            let left = descend(&items[..k], index, path);
            let right = subtree(&items[k..]);
            path.siblings[path.sibling_count] = right;
            path.sibling_count += 1;
            (left, right)
        } else {
            let right = descend(&items[k..], index - k, path);
            let left = subtree(&items[..k]);
            path.siblings[path.sibling_count] = left;
            path.sibling_count += 1;
            (left, right)
        };
        branch(left, k as u64, right, (items.len() - k) as u64)
    }
    let mut proof = InclusionProof {
        index: index as u64,
        count: items.len() as u64,
        siblings: [Hash::from_bytes([0; 32]); 64],
        sibling_count: 0,
    };
    let inner = descend(items, index, &mut proof);
    Some((root(inner, items.len() as u64), proof))
}

/// Verify against a caller-supplied root. No allocation during verification.
pub fn verify(item: ContentId, proof: &InclusionProof, expected: SequenceId) -> bool {
    reconstruct(item, proof) == Some(expected)
}

/// Open a field of a nominal record. The caller checks the schema's expected
/// field index and count as well as supplying the trusted record ID and kind.
pub fn verify_record_field(
    kind: &[u8],
    item: ContentId,
    proof: &InclusionProof,
    expected: ContentId,
) -> bool {
    reconstruct(item, proof).is_some_and(|fields| record_from_sequence(kind, fields) == expected)
}

fn reconstruct(item: ContentId, proof: &InclusionProof) -> Option<SequenceId> {
    if proof.count == 0 || proof.index >= proof.count || proof.sibling_count > 64 {
        return None;
    }
    let mut path = [(false, 0u64, 0u64); 64];
    let (mut n, mut index, mut depth) = (proof.count, proof.index, 0usize);
    while n > 1 {
        let k = split(n);
        let left = index < k;
        path[depth] = (left, k, n - k);
        depth += 1;
        if left {
            n = k;
        } else {
            n -= k;
            index -= k;
        }
    }
    if depth != proof.sibling_count {
        return None;
    }
    let mut digest = item.0;
    for ((left, ln, rn), sibling) in path[..depth]
        .iter()
        .rev()
        .zip(&proof.siblings[..proof.sibling_count])
    {
        if !canonical(sibling) {
            return None;
        }
        digest = if *left {
            branch(digest, *ln, *sibling, *rn)
        } else {
            branch(*sibling, *ln, digest, *rn)
        };
    }
    Some(root(digest, proof.count))
}

fn canonical(hash: &Hash) -> bool {
    hash.as_bytes()
        .as_chunks::<8>()
        .0
        .iter()
        .all(|limb| u64::from_le_bytes(*limb) < P)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_counter_overflow_rejects_without_mutation() {
        let mut builder = SequenceBuilder {
            count: u64::MAX,
            ..SequenceBuilder::default()
        };
        let before = builder.stack;
        assert_eq!(builder.push(blob(b"x")), Err(SequenceOverflow));
        assert_eq!(builder.count(), u64::MAX);
        assert_eq!(builder.stack, before);
    }

    #[test]
    fn sequence_carry_reaches_last_stack_slot_without_overflow() {
        let mut builder = SequenceBuilder {
            count: (1u64 << 63) - 1,
            ..SequenceBuilder::default()
        };
        builder.push(blob(b"x")).unwrap();
        assert_eq!(builder.count(), 1u64 << 63);
        assert_eq!(builder.finish(), root(builder.stack[63], 1u64 << 63));
    }
}
