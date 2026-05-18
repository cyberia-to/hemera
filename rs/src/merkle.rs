// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
//! Merkle path verification.
//!
//! Provides a standalone `merkle_verify_path` that authenticates a leaf hash
//! against a known root using hemera's Poseidon2 node-hashing convention
//! (the same convention used by `tree::hash_node`).

use crate::sponge::Hash;
use crate::tree::hash_node;

/// The side a sibling occupies relative to the current node.
///
/// `Left` means the sibling is the left child; the current node is on the
/// right.  `Right` means the sibling is the right child; the current node is
/// on the left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Side {
    /// Sibling is the left child; current node is the right child.
    Left,
    /// Sibling is the right child; current node is the left child.
    Right,
}

/// Verify a Merkle inclusion path using hemera's node-hashing convention.
///
/// `leaf_hash` is the pre-computed leaf hash to authenticate.
/// `path` is the authentication path from leaf to root: each entry is
/// `(sibling_hash, side)` where `side` describes the sibling's position.
/// The path is ordered **leaf-to-root** — the first entry is the sibling of
/// the leaf, the last entry is the sibling of the root's child.
///
/// At each step:
/// - `Side::Left`  → `new_hash = hash_node(sibling, current, is_root)`
/// - `Side::Right` → `new_hash = hash_node(current, sibling, is_root)`
///
/// where `is_root` is `true` only for the final step (matching hemera's
/// domain separation between interior nodes and the root).
///
/// Returns `true` if the recomputed root equals `root`.
pub fn merkle_verify_path(root: &Hash, leaf_hash: &Hash, path: &[(Hash, Side)]) -> bool {
    if path.is_empty() {
        return leaf_hash == root;
    }

    let last = path.len() - 1;
    let mut current = *leaf_hash;

    for (i, (sibling, side)) in path.iter().enumerate() {
        let is_root = i == last;
        current = match side {
            Side::Left  => hash_node(sibling, &current, is_root),
            Side::Right => hash_node(&current, sibling, is_root),
        };
    }

    current == *root
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::tree::{hash_leaf, hash_node};
    use crate::params::OUTPUT_BYTES;

    /// Build a minimal 2-leaf tree and return (root, leaf0_hash, leaf1_hash).
    ///
    /// Tree layout:
    ///
    ///       root
    ///      /    \
    ///   leaf0  leaf1
    ///
    /// hash_node(leaf0, leaf1, is_root=true) = root.
    fn two_leaf_tree() -> (Hash, Hash, Hash) {
        let leaf0 = hash_leaf(b"leaf zero data", 0, false);
        let leaf1 = hash_leaf(b"leaf one data",  1, false);
        let root  = hash_node(&leaf0, &leaf1, true);
        (root, leaf0, leaf1)
    }

    // ── Valid paths ───────────────────────────────────────────────────

    #[test]
    fn two_leaf_left_leaf_verifies() {
        // leaf0 is the left child; its sibling (leaf1) is on the right.
        let (root, leaf0, leaf1) = two_leaf_tree();
        let path = [(leaf1, Side::Right)];
        assert!(merkle_verify_path(&root, &leaf0, &path));
    }

    #[test]
    fn two_leaf_right_leaf_verifies() {
        // leaf1 is the right child; its sibling (leaf0) is on the left.
        let (root, leaf0, leaf1) = two_leaf_tree();
        let path = [(leaf0, Side::Left)];
        assert!(merkle_verify_path(&root, &leaf1, &path));
    }

    // ── Invalid paths ─────────────────────────────────────────────────

    #[test]
    fn wrong_leaf_hash_fails() {
        let (root, _leaf0, leaf1) = two_leaf_tree();
        let wrong_leaf = Hash::from_bytes([0xAA; OUTPUT_BYTES]);
        let path = [(leaf1, Side::Right)];
        assert!(!merkle_verify_path(&root, &wrong_leaf, &path));
    }

    #[test]
    fn wrong_sibling_hash_fails() {
        let (root, leaf0, _leaf1) = two_leaf_tree();
        let wrong_sibling = Hash::from_bytes([0xBB; OUTPUT_BYTES]);
        let path = [(wrong_sibling, Side::Right)];
        assert!(!merkle_verify_path(&root, &leaf0, &path));
    }

    #[test]
    fn swapped_sides_fails() {
        // Presenting leaf0 as if it were the right child (swapped side).
        let (root, leaf0, leaf1) = two_leaf_tree();
        let path = [(leaf1, Side::Left)]; // wrong side
        assert!(!merkle_verify_path(&root, &leaf0, &path));
    }

    #[test]
    fn wrong_root_fails() {
        let (_root, leaf0, leaf1) = two_leaf_tree();
        let wrong_root = Hash::from_bytes([0xFF; OUTPUT_BYTES]);
        let path = [(leaf1, Side::Right)];
        assert!(!merkle_verify_path(&wrong_root, &leaf0, &path));
    }

    // ── Edge cases ────────────────────────────────────────────────────

    #[test]
    fn empty_path_leaf_equals_root() {
        let leaf = hash_leaf(b"solo", 0, true);
        assert!(merkle_verify_path(&leaf, &leaf, &[]));
    }

    #[test]
    fn empty_path_wrong_hash_fails() {
        let leaf  = hash_leaf(b"solo", 0, true);
        let other = Hash::from_bytes([0x01; OUTPUT_BYTES]);
        assert!(!merkle_verify_path(&leaf, &other, &[]));
    }

    // ── Deeper tree (4 leaves) ────────────────────────────────────────

    #[test]
    fn four_leaf_tree_all_leaves_verify() {
        //        root
        //       /    \
        //     p01    p23
        //    /   \  /   \
        //   c0  c1 c2   c3

        let c0 = hash_leaf(b"c0", 0, false);
        let c1 = hash_leaf(b"c1", 1, false);
        let c2 = hash_leaf(b"c2", 2, false);
        let c3 = hash_leaf(b"c3", 3, false);

        let p01  = hash_node(&c0, &c1, false);
        let p23  = hash_node(&c2, &c3, false);
        let root = hash_node(&p01, &p23, true);

        // c0: sibling=c1 (right), then sibling=p23 (right)
        assert!(merkle_verify_path(&root, &c0, &[(c1, Side::Right), (p23, Side::Right)]));

        // c1: sibling=c0 (left), then sibling=p23 (right)
        assert!(merkle_verify_path(&root, &c1, &[(c0, Side::Left), (p23, Side::Right)]));

        // c2: sibling=c3 (right), then sibling=p01 (left)
        assert!(merkle_verify_path(&root, &c2, &[(c3, Side::Right), (p01, Side::Left)]));

        // c3: sibling=c2 (left), then sibling=p01 (left)
        assert!(merkle_verify_path(&root, &c3, &[(c2, Side::Left), (p01, Side::Left)]));
    }
}
