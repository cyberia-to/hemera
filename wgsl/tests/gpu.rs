// ---
// tags: hemera, rust
// crystal-type: source
// crystal-domain: comp
// ---
use cyber_hemera::field::Goldilocks;
use cyber_hemera::permutation::permute;
use cyber_hemera::sparse::SparseTree;
use cyber_hemera::{Hash, CHUNK_SIZE, OUTPUT_BYTES, WIDTH};
use cyber_hemera_wgsl::GpuContext;

fn gpu() -> Option<GpuContext> {
    pollster::block_on(GpuContext::new())
}

macro_rules! require_gpu {
    ($gpu:ident) => {
        let Some($gpu) = gpu() else {
            eprintln!("No GPU available, skipping test");
            return;
        };
    };
}

// ── Permutation tests ───────────────────────────────────────

#[test]
fn gpu_matches_cpu_zero_state() {
    require_gpu!(gpu);
    let mut cpu_state = [Goldilocks::new(0); WIDTH];
    permute(&mut cpu_state);

    let gpu_states = pollster::block_on(gpu.batch_permute(&[[Goldilocks::new(0); WIDTH]]));
    for i in 0..WIDTH {
        assert_eq!(
            cpu_state[i].as_canonical_u64(),
            gpu_states[0][i].as_canonical_u64(),
            "element {i} mismatch",
        );
    }
}

#[test]
fn gpu_matches_cpu_sequential_state() {
    require_gpu!(gpu);
    let input: [Goldilocks; WIDTH] = core::array::from_fn(|i| Goldilocks::new(i as u64 + 1));
    let mut cpu_state = input;
    permute(&mut cpu_state);

    let gpu_states = pollster::block_on(gpu.batch_permute(&[input]));
    for i in 0..WIDTH {
        assert_eq!(
            cpu_state[i].as_canonical_u64(),
            gpu_states[0][i].as_canonical_u64(),
            "element {i} mismatch",
        );
    }
}

#[test]
fn gpu_matches_cpu_large_values() {
    require_gpu!(gpu);
    let p = cyber_hemera::field::P;
    let input: [Goldilocks; WIDTH] = core::array::from_fn(|i| Goldilocks::new(p - 1 - i as u64));
    let mut cpu_state = input;
    permute(&mut cpu_state);

    let gpu_states = pollster::block_on(gpu.batch_permute(&[input]));
    for i in 0..WIDTH {
        assert_eq!(
            cpu_state[i].as_canonical_u64(),
            gpu_states[0][i].as_canonical_u64(),
            "element {i} mismatch",
        );
    }
}

#[test]
fn gpu_batch_multiple() {
    require_gpu!(gpu);
    let inputs: Vec<[Goldilocks; WIDTH]> = (0..10)
        .map(|batch| core::array::from_fn(|i| Goldilocks::new((batch * 16 + i) as u64)))
        .collect();

    let gpu_results = pollster::block_on(gpu.batch_permute(&inputs));
    for (batch, (input, gpu_result)) in inputs.iter().zip(gpu_results.iter()).enumerate() {
        let mut cpu_state = *input;
        permute(&mut cpu_state);
        for i in 0..WIDTH {
            assert_eq!(
                cpu_state[i].as_canonical_u64(),
                gpu_result[i].as_canonical_u64(),
                "batch {batch} element {i} mismatch",
            );
        }
    }
}

// ── Plain hash tests ────────────────────────────────────────

#[test]
fn gpu_hash_matches_cpu() {
    require_gpu!(gpu);
    let data = b"hello hemera";
    let gpu_hashes = pollster::block_on(gpu.batch_hash(data, 4096));
    assert_eq!(gpu_hashes.len(), 1);
    assert_eq!(gpu_hashes[0], cyber_hemera::hash(data));
}

#[test]
fn gpu_hash_multi_chunk() {
    require_gpu!(gpu);
    let data = vec![0x42u8; 200];
    let chunk_size = 100;
    let gpu_hashes = pollster::block_on(gpu.batch_hash(&data, chunk_size));
    assert_eq!(gpu_hashes.len(), 2);
    for (i, chunk) in data.chunks(chunk_size).enumerate() {
        assert_eq!(gpu_hashes[i], cyber_hemera::hash(chunk), "chunk {i} mismatch");
    }
}

// ── Keyed hash tests ────────────────────────────────────────

#[test]
fn gpu_keyed_hash_matches_cpu() {
    require_gpu!(gpu);
    let key = [0x42u8; OUTPUT_BYTES];
    let data = b"keyed test data";

    let gpu_hashes = pollster::block_on(gpu.batch_keyed_hash(&key, data, 4096));
    assert_eq!(gpu_hashes.len(), 1);

    let cpu_hash = cyber_hemera::keyed_hash(&key, data);
    assert_eq!(gpu_hashes[0], cpu_hash);
}

#[test]
fn gpu_keyed_hash_differs_from_plain() {
    require_gpu!(gpu);
    let key = [0u8; OUTPUT_BYTES];
    let data = b"test";

    let plain = pollster::block_on(gpu.batch_hash(data, 4096));
    let keyed = pollster::block_on(gpu.batch_keyed_hash(&key, data, 4096));
    assert_ne!(plain[0], keyed[0]);
}

// ── Derive key tests ────────────────────────────────────────

#[test]
fn gpu_derive_key_matches_cpu() {
    require_gpu!(gpu);
    let context = "test context v1";
    let material = b"key material";

    let gpu_hashes = pollster::block_on(gpu.batch_derive_key(context, material, 4096));
    assert_eq!(gpu_hashes.len(), 1);

    let cpu_key = cyber_hemera::derive_key(context, material);
    assert_eq!(gpu_hashes[0].as_bytes(), &cpu_key);
}

#[test]
fn gpu_derive_key_differs_from_plain() {
    require_gpu!(gpu);
    let data = b"material";
    let plain = pollster::block_on(gpu.batch_hash(data, 4096));
    let derived = pollster::block_on(gpu.batch_derive_key("ctx", data, 4096));
    assert_ne!(plain[0].as_bytes(), derived[0].as_bytes());
}

// ── Hash leaf tests ─────────────────────────────────────────

#[test]
fn gpu_hash_leaves_match_cpu() {
    require_gpu!(gpu);
    let data = vec![0x42u8; 8192];
    let chunk_size = 4096;

    let gpu_hashes = pollster::block_on(gpu.batch_hash_leaves(&data, chunk_size, false));
    assert_eq!(gpu_hashes.len(), 2);

    for (i, chunk) in data.chunks(chunk_size).enumerate() {
        let cpu_hash = cyber_hemera::tree::hash_leaf(chunk, i as u64, false);
        assert_eq!(gpu_hashes[i], cpu_hash, "leaf {i} mismatch");
    }
}

#[test]
fn gpu_hash_leaf_partial_chunk() {
    require_gpu!(gpu);
    let data = vec![0xABu8; 5000];
    let chunk_size = 4096;

    let gpu_hashes = pollster::block_on(gpu.batch_hash_leaves(&data, chunk_size, false));
    assert_eq!(gpu_hashes.len(), 2);

    for (i, chunk) in data.chunks(chunk_size).enumerate() {
        let cpu_hash = cyber_hemera::tree::hash_leaf(chunk, i as u64, false);
        assert_eq!(gpu_hashes[i], cpu_hash, "leaf {i} mismatch");
    }
}

#[test]
fn gpu_hash_leaf_small_data() {
    require_gpu!(gpu);
    let data = b"hello world";
    let chunk_size = 4096;

    let gpu_hashes = pollster::block_on(gpu.batch_hash_leaves(data, chunk_size, false));
    assert_eq!(gpu_hashes.len(), 1);

    let cpu_hash = cyber_hemera::tree::hash_leaf(data, 0, false);
    assert_eq!(gpu_hashes[0], cpu_hash);
}

#[test]
fn gpu_hash_leaf_root_single_chunk() {
    require_gpu!(gpu);
    let data = b"single chunk root";
    let chunk_size = 4096;

    let gpu_hashes = pollster::block_on(gpu.batch_hash_leaves(data, chunk_size, true));
    assert_eq!(gpu_hashes.len(), 1);

    let cpu_hash = cyber_hemera::tree::hash_leaf(data, 0, true);
    assert_eq!(gpu_hashes[0], cpu_hash);
}

// ── Hash node tests ─────────────────────────────────────────

#[test]
fn gpu_hash_nodes_match_cpu() {
    require_gpu!(gpu);
    let left = cyber_hemera::hash(b"left child");
    let right = cyber_hemera::hash(b"right child");

    let cpu_node = cyber_hemera::tree::hash_node(&left, &right, false);
    let gpu_nodes = pollster::block_on(gpu.batch_hash_nodes(&[(left, right)], false));
    assert_eq!(gpu_nodes[0], cpu_node);
}

#[test]
fn gpu_hash_nodes_root_match_cpu() {
    require_gpu!(gpu);
    let left = cyber_hemera::hash(b"left");
    let right = cyber_hemera::hash(b"right");

    let cpu_root = cyber_hemera::tree::hash_node(&left, &right, true);
    let gpu_roots = pollster::block_on(gpu.batch_hash_nodes(&[(left, right)], true));
    assert_eq!(gpu_roots[0], cpu_root);
}

#[test]
fn gpu_hash_nodes_batch() {
    require_gpu!(gpu);
    let pairs: Vec<(Hash, Hash)> = (0..5)
        .map(|i| {
            let l = cyber_hemera::hash(&[i as u8; 10]);
            let r = cyber_hemera::hash(&[i as u8 + 100; 10]);
            (l, r)
        })
        .collect();

    let gpu_nodes = pollster::block_on(gpu.batch_hash_nodes(&pairs, false));
    for (i, (left, right)) in pairs.iter().enumerate() {
        let cpu_node = cyber_hemera::tree::hash_node(left, right, false);
        assert_eq!(gpu_nodes[i], cpu_node, "node {i} mismatch");
    }
}

// ── NMT node tests ──────────────────────────────────────────

#[test]
fn gpu_hash_node_nmt_matches_cpu() {
    require_gpu!(gpu);
    let left = cyber_hemera::hash(b"nmt left");
    let right = cyber_hemera::hash(b"nmt right");
    let ns_min = 42u64;
    let ns_max = 100u64;

    let cpu_hash = cyber_hemera::tree::hash_node_nmt(&left, &right, ns_min, ns_max, false);
    let gpu_hashes =
        pollster::block_on(gpu.batch_hash_nodes_nmt(&[(left, right)], ns_min, ns_max, false));
    assert_eq!(gpu_hashes[0], cpu_hash);
}

#[test]
fn gpu_hash_node_nmt_root() {
    require_gpu!(gpu);
    let left = cyber_hemera::hash(b"nmt root left");
    let right = cyber_hemera::hash(b"nmt root right");
    let ns_min = 1u64;
    let ns_max = 255u64;

    let cpu_hash = cyber_hemera::tree::hash_node_nmt(&left, &right, ns_min, ns_max, true);
    let gpu_hashes =
        pollster::block_on(gpu.batch_hash_nodes_nmt(&[(left, right)], ns_min, ns_max, true));
    assert_eq!(gpu_hashes[0], cpu_hash);
}

#[test]
fn gpu_hash_node_nmt_zero_ns_matches_hash_node() {
    require_gpu!(gpu);
    let left = cyber_hemera::hash(b"zero ns left");
    let right = cyber_hemera::hash(b"zero ns right");

    let node_hash = cyber_hemera::tree::hash_node(&left, &right, false);
    let nmt_hash = cyber_hemera::tree::hash_node_nmt(&left, &right, 0, 0, false);
    assert_eq!(node_hash, nmt_hash);

    let gpu_nmt =
        pollster::block_on(gpu.batch_hash_nodes_nmt(&[(left, right)], 0, 0, false));
    assert_eq!(gpu_nmt[0], node_hash);
}

#[test]
fn gpu_hash_node_nmt_large_ns() {
    require_gpu!(gpu);
    let left = cyber_hemera::hash(b"large ns");
    let right = cyber_hemera::hash(b"large ns r");
    let ns_min = 0x0000_0001_0000_0000u64;
    let ns_max = 0x0000_0002_0000_0000u64;

    let cpu_hash = cyber_hemera::tree::hash_node_nmt(&left, &right, ns_min, ns_max, false);
    let gpu_hashes =
        pollster::block_on(gpu.batch_hash_nodes_nmt(&[(left, right)], ns_min, ns_max, false));
    assert_eq!(gpu_hashes[0], cpu_hash);
}

// ── Edge cases ──────────────────────────────────────────────

#[test]
fn gpu_empty_inputs() {
    require_gpu!(gpu);
    assert!(pollster::block_on(gpu.batch_permute(&[])).is_empty());
    assert!(pollster::block_on(gpu.batch_hash(&[], 4096)).is_empty());
    assert!(pollster::block_on(gpu.batch_keyed_hash(&[0; OUTPUT_BYTES], &[], 4096)).is_empty());
    assert!(pollster::block_on(gpu.batch_derive_key("ctx", &[], 4096)).is_empty());
    assert!(pollster::block_on(gpu.batch_hash_leaves(&[], 4096, false)).is_empty());
    assert!(pollster::block_on(gpu.batch_hash_nodes(&[], false)).is_empty());
    assert!(pollster::block_on(gpu.batch_hash_nodes_nmt(&[], 0, 0, false)).is_empty());
}

// ── Root hash tests ─────────────────────────────────────────

#[test]
fn gpu_root_hash_empty() {
    require_gpu!(gpu);
    let cpu = cyber_hemera::tree::root_hash(b"");
    let gpu_hash = pollster::block_on(gpu.root_hash(b""));
    assert_eq!(gpu_hash, cpu);
}

#[test]
fn gpu_root_hash_single_chunk() {
    require_gpu!(gpu);
    let data = b"hello hemera root hash";
    let cpu = cyber_hemera::tree::root_hash(data);
    let gpu_hash = pollster::block_on(gpu.root_hash(data));
    assert_eq!(gpu_hash, cpu);
}

#[test]
fn gpu_root_hash_two_chunks() {
    require_gpu!(gpu);
    let data = vec![0xAB; CHUNK_SIZE + 1];
    let cpu = cyber_hemera::tree::root_hash(&data);
    let gpu_hash = pollster::block_on(gpu.root_hash(&data));
    assert_eq!(gpu_hash, cpu);
}

#[test]
fn gpu_root_hash_three_chunks() {
    require_gpu!(gpu);
    let data = vec![0xCD; CHUNK_SIZE * 3];
    let cpu = cyber_hemera::tree::root_hash(&data);
    let gpu_hash = pollster::block_on(gpu.root_hash(&data));
    assert_eq!(gpu_hash, cpu);
}

#[test]
fn gpu_root_hash_five_chunks() {
    require_gpu!(gpu);
    let data = vec![0xEF; CHUNK_SIZE * 4 + 500];
    let cpu = cyber_hemera::tree::root_hash(&data);
    let gpu_hash = pollster::block_on(gpu.root_hash(&data));
    assert_eq!(gpu_hash, cpu);
}

#[test]
fn gpu_root_hash_power_of_two() {
    require_gpu!(gpu);
    let data = vec![0x42; CHUNK_SIZE * 8];
    let cpu = cyber_hemera::tree::root_hash(&data);
    let gpu_hash = pollster::block_on(gpu.root_hash(&data));
    assert_eq!(gpu_hash, cpu);
}

#[test]
fn gpu_root_hash_many_chunks() {
    require_gpu!(gpu);
    let data = vec![0x77; CHUNK_SIZE * 17 + 999];
    let cpu = cyber_hemera::tree::root_hash(&data);
    let gpu_hash = pollster::block_on(gpu.root_hash(&data));
    assert_eq!(gpu_hash, cpu);
}

// ── Outboard tests ──────────────────────────────────────────

#[test]
fn gpu_outboard_single_chunk() {
    require_gpu!(gpu);
    let data = b"small outboard data";
    let (cpu_root, cpu_ob) = cyber_hemera::stream::outboard(data);
    let (gpu_root, gpu_ob) = pollster::block_on(gpu.outboard(data));
    assert_eq!(gpu_root, cpu_root);
    assert_eq!(gpu_ob, cpu_ob);
}

#[test]
fn gpu_outboard_two_chunks() {
    require_gpu!(gpu);
    let data = vec![0xAB; CHUNK_SIZE + 1];
    let (cpu_root, cpu_ob) = cyber_hemera::stream::outboard(&data);
    let (gpu_root, gpu_ob) = pollster::block_on(gpu.outboard(&data));
    assert_eq!(gpu_root, cpu_root);
    assert_eq!(gpu_ob, cpu_ob);
}

#[test]
fn gpu_outboard_four_chunks() {
    require_gpu!(gpu);
    let data = vec![0xCD; CHUNK_SIZE * 4];
    let (cpu_root, cpu_ob) = cyber_hemera::stream::outboard(&data);
    let (gpu_root, gpu_ob) = pollster::block_on(gpu.outboard(&data));
    assert_eq!(gpu_root, cpu_root);
    assert_eq!(gpu_ob, cpu_ob);
}

#[test]
fn gpu_outboard_seven_chunks() {
    require_gpu!(gpu);
    let data = vec![0xEF; CHUNK_SIZE * 7 + 500];
    let (cpu_root, cpu_ob) = cyber_hemera::stream::outboard(&data);
    let (gpu_root, gpu_ob) = pollster::block_on(gpu.outboard(&data));
    assert_eq!(gpu_root, cpu_root);
    assert_eq!(gpu_ob, cpu_ob);
}

#[test]
fn gpu_outboard_verifiable() {
    require_gpu!(gpu);
    let data = vec![0x42; CHUNK_SIZE * 3 + 100];
    let (root, ob) = pollster::block_on(gpu.outboard(&data));
    cyber_hemera::stream::verify_outboard(&data, &ob, &root).unwrap();
}

// ── Proof verification tests ────────────────────────────────

#[test]
fn gpu_verify_single_proof() {
    require_gpu!(gpu);
    let data = vec![0x42; CHUNK_SIZE * 4];
    let root = cyber_hemera::tree::root_hash(&data);
    let (_, proof) = cyber_hemera::tree::prove(&data, 1);
    let chunk = &data[CHUNK_SIZE..CHUNK_SIZE * 2];

    let results = pollster::block_on(gpu.batch_verify_proofs(&[(chunk, &proof, &root)]));
    assert_eq!(results, vec![true]);
}

#[test]
fn gpu_verify_multiple_proofs() {
    require_gpu!(gpu);
    let data = vec![0xAB; CHUNK_SIZE * 8];
    let root = cyber_hemera::tree::root_hash(&data);

    let mut entries = Vec::new();
    let mut proofs = Vec::new();
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    for i in 0..4 {
        let (_, proof) = cyber_hemera::tree::prove(&data, i);
        proofs.push(proof);
        let start = i as usize * CHUNK_SIZE;
        let end = start + CHUNK_SIZE;
        chunks.push(data[start..end].to_vec());
    }

    for i in 0..4 {
        entries.push((chunks[i].as_slice(), &proofs[i], &root));
    }

    let results = pollster::block_on(gpu.batch_verify_proofs(&entries));
    assert_eq!(results, vec![true; 4]);
}

#[test]
fn gpu_verify_wrong_data_fails() {
    require_gpu!(gpu);
    let data = vec![0x42; CHUNK_SIZE * 4];
    let root = cyber_hemera::tree::root_hash(&data);
    let (_, proof) = cyber_hemera::tree::prove(&data, 0);
    let wrong_chunk = vec![0xFF; CHUNK_SIZE];

    let results = pollster::block_on(gpu.batch_verify_proofs(&[(&wrong_chunk, &proof, &root)]));
    assert_eq!(results, vec![false]);
}

// ── XOF squeeze tests ───────────────────────────────────────

#[test]
fn gpu_squeeze_matches_cpu() {
    require_gpu!(gpu);
    let mut hasher = cyber_hemera::Hasher::new();
    hasher.update(b"xof test");
    let state = hasher.finalize_state();

    let gpu_blocks = pollster::block_on(gpu.batch_squeeze(&[state], 3));
    assert_eq!(gpu_blocks.len(), 1);
    assert_eq!(gpu_blocks[0].len(), 3);

    // Compare with CPU XOF.
    let mut xof = cyber_hemera::Hasher::new().update(b"xof test").finalize_xof();
    for block in &gpu_blocks[0] {
        let mut cpu_block = [0u8; OUTPUT_BYTES];
        xof.fill(&mut cpu_block);
        assert_eq!(block, &cpu_block);
    }
}

#[test]
fn gpu_squeeze_batch() {
    require_gpu!(gpu);
    let states: Vec<[Goldilocks; WIDTH]> = (0..3).map(|i| {
        let mut h = cyber_hemera::Hasher::new();
        h.update(&[i as u8; 10]);
        h.finalize_state()
    }).collect();

    let gpu_blocks = pollster::block_on(gpu.batch_squeeze(&states, 2));
    assert_eq!(gpu_blocks.len(), 3);

    for (i, blocks) in gpu_blocks.iter().enumerate() {
        let mut xof = cyber_hemera::Hasher::new().update(&[i as u8; 10]).finalize_xof();
        for block in blocks {
            let mut cpu_block = [0u8; OUTPUT_BYTES];
            xof.fill(&mut cpu_block);
            assert_eq!(block, &cpu_block);
        }
    }
}

// ── Sparse proof verification tests ────────────────────────

#[test]
fn gpu_verify_sparse_inclusion() {
    require_gpu!(gpu);
    let mut tree = SparseTree::new(8);
    let key = [0u8; 32];
    tree.insert(&key, b"value");
    let proof = tree.prove(&key);
    let root = tree.root();

    let results = pollster::block_on(
        gpu.batch_verify_sparse_proofs(&[(&proof, Some(b"value".as_slice()), &root)], 8),
    );
    assert_eq!(results, vec![true]);
}

#[test]
fn gpu_verify_sparse_non_inclusion() {
    require_gpu!(gpu);
    let mut tree = SparseTree::new(8);
    let k1 = [0u8; 32];
    tree.insert(&k1, b"exists");
    let absent = [0xFF; 32];
    let proof = tree.prove(&absent);
    let root = tree.root();

    let results = pollster::block_on(
        gpu.batch_verify_sparse_proofs(&[(&proof, None, &root)], 8),
    );
    assert_eq!(results, vec![true]);
}

#[test]
fn gpu_verify_sparse_wrong_value_fails() {
    require_gpu!(gpu);
    let mut tree = SparseTree::new(8);
    let key = [0u8; 32];
    tree.insert(&key, b"correct");
    let proof = tree.prove(&key);
    let root = tree.root();

    let results = pollster::block_on(
        gpu.batch_verify_sparse_proofs(&[(&proof, Some(b"wrong".as_slice()), &root)], 8),
    );
    assert_eq!(results, vec![false]);
}

#[test]
fn gpu_verify_sparse_batch_multiple() {
    require_gpu!(gpu);
    let mut tree = SparseTree::new(8);
    let k1 = [0u8; 32];
    let k2 = [0xFF; 32];
    let k3 = [0x42; 32];
    tree.insert(&k1, b"one");
    tree.insert(&k2, b"two");
    let root = tree.root();

    let p1 = tree.prove(&k1);
    let p2 = tree.prove(&k2);
    let p3 = tree.prove(&k3);

    let results = pollster::block_on(gpu.batch_verify_sparse_proofs(
        &[
            (&p1, Some(b"one".as_slice()), &root),
            (&p2, Some(b"two".as_slice()), &root),
            (&p3, None, &root),
        ],
        8,
    ));
    assert_eq!(results, vec![true, true, true]);
}

#[test]
fn gpu_verify_sparse_matches_cpu() {
    require_gpu!(gpu);
    let mut tree = SparseTree::new(16);
    let keys: Vec<[u8; 32]> = (0..5).map(|i| {
        let mut k = [0u8; 32];
        k[0] = i;
        k
    }).collect();

    for (i, key) in keys.iter().enumerate() {
        tree.insert(key, &[i as u8; 10]);
    }
    let root = tree.root();

    let proofs: Vec<_> = keys.iter().map(|k| tree.prove(k)).collect();
    let values: Vec<Vec<u8>> = (0..5).map(|i| vec![i as u8; 10]).collect();

    for (i, proof) in proofs.iter().enumerate() {
        assert!(SparseTree::verify(proof, Some(&values[i]), &root, 16));
    }

    let entries: Vec<_> = proofs.iter().enumerate().map(|(i, p)| {
        (p, Some(values[i].as_slice()), &root)
    }).collect();
    let results = pollster::block_on(gpu.batch_verify_sparse_proofs(&entries, 16));
    assert_eq!(results, vec![true; 5]);
}

#[test]
fn gpu_verify_sparse_empty_input() {
    require_gpu!(gpu);
    let results = pollster::block_on(gpu.batch_verify_sparse_proofs(&[], 8));
    assert!(results.is_empty());
}
