//! GPU backend for Hemera via wgpu compute shaders.
//!
//! Provides GPU-accelerated batch operations:
//! - `batch_permute` — raw Poseidon2 permutations
//! - `batch_hash` — plain sponge hash (no tree domain)
//! - `batch_keyed_hash` — keyed sponge hash
//! - `batch_derive_key` — two-phase key derivation
//! - `batch_hash_leaves` — full sponge + tree leaf hashing
//! - `batch_hash_nodes` — parent node hashing
//! - `batch_hash_nodes_nmt` — namespace-aware parent node hashing
//! - `root_hash` — full Merkle tree on GPU (leaves + level-by-level node merge)
//! - `outboard` — GPU leaf hashing + CPU tree serialization
//! - `batch_verify_proofs` — batch inclusion proof verification
//! - `batch_squeeze` — batch XOF squeeze via GPU permutations
//!
//! The GPU path is optional — the CPU backend (`cyber-hemera`) is always
//! available as fallback.

use std::num::NonZeroU64;

use cyber_hemera::field::Goldilocks;
use cyber_hemera::tree::{InclusionProof, Sibling};
use cyber_hemera::{Hash, CHUNK_SIZE, OUTPUT_BYTES, WIDTH};
use wgpu::util::DeviceExt;

/// Pre-compiled GPU compute pipelines and device handles.
#[derive(Debug)]
pub struct GpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    permute_pipeline: wgpu::ComputePipeline,
    hash_leaf_pipeline: wgpu::ComputePipeline,
    hash_node_pipeline: wgpu::ComputePipeline,
    hash_chunk_pipeline: wgpu::ComputePipeline,
    keyed_hash_pipeline: wgpu::ComputePipeline,
    derive_key_material_pipeline: wgpu::ComputePipeline,
    hash_node_nmt_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    rc_buffer: wgpu::Buffer,
    diag_buffer: wgpu::Buffer,
    dummy_buffer: wgpu::Buffer,
}

const FLAG_ROOT: u32 = 1;
const DOMAIN_HASH: u32 = 0;
const PARAMS_SIZE: u64 = 32; // DispatchParams struct: 8 × u32

impl GpuContext {
    /// Initialize GPU backend. Returns `None` if no suitable GPU is available.
    pub async fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok()?;

        if !adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        {
            return None;
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("hemera GPU"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                ..Default::default()
            })
            .await
            .ok()?;

        let shader_source = concat!(
            include_str!("shaders/params.wgsl"),
            include_str!("shaders/field.wgsl"),
            include_str!("shaders/encoding.wgsl"),
            include_str!("shaders/permutation.wgsl"),
            include_str!("shaders/sponge.wgsl"),
            include_str!("shaders/tree.wgsl"),
            include_str!("shaders/entry_points.wgsl"),
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hemera"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hemera bgl"),
            entries: &[
                buffer_entry(0, wgpu::BufferBindingType::Storage { read_only: false }, 4),
                buffer_entry(1, wgpu::BufferBindingType::Storage { read_only: true }, 4),
                buffer_entry(2, wgpu::BufferBindingType::Uniform, PARAMS_SIZE),
                buffer_entry(3, wgpu::BufferBindingType::Storage { read_only: true }, 4),
                buffer_entry(4, wgpu::BufferBindingType::Storage { read_only: true }, 4),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hemera layout"),
            bind_group_layouts: &[&bgl],
            immediate_size: 0,
        });

        let pipe = |ep: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(ep),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some(ep),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };

        let rc_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("round constants"),
            contents: bytemuck::cast_slice(&generate_round_constants_u32()),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let diag_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("matrix diag"),
            contents: bytemuck::cast_slice(&generate_matrix_diag_u32()),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let dummy_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("dummy"),
            contents: &[0u8; 4],
            usage: wgpu::BufferUsages::STORAGE,
        });

        Some(Self {
            permute_pipeline: pipe("hemera_permute"),
            hash_leaf_pipeline: pipe("hemera_hash_leaf"),
            hash_node_pipeline: pipe("hemera_hash_node"),
            hash_chunk_pipeline: pipe("hemera_hash_chunk"),
            keyed_hash_pipeline: pipe("hemera_keyed_hash"),
            derive_key_material_pipeline: pipe("hemera_derive_key_material"),
            hash_node_nmt_pipeline: pipe("hemera_hash_node_nmt"),
            bind_group_layout: bgl,
            rc_buffer,
            diag_buffer,
            dummy_buffer,
            device,
            queue,
        })
    }

    fn bind(&self, io: &wgpu::Buffer, params: &wgpu::Buffer, aux: &wgpu::Buffer) -> wgpu::BindGroup {
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: io.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.rc_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: self.diag_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: aux.as_entire_binding() },
            ],
        })
    }

    fn dispatch_readback(
        &self,
        pipeline: &wgpu::ComputePipeline,
        bg: &wgpu::BindGroup,
        io: &wgpu::Buffer,
        count: u32,
    ) -> Vec<u32> {
        let dl = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: io.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(io, 0, &dl, 0, io.size());
        self.queue.submit([enc.finish()]);
        let slice = dl.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        let mapped = slice.get_mapped_range();
        let out: Vec<u32> = bytemuck::cast_slice(&mapped).to_vec();
        drop(mapped);
        dl.unmap();
        out
    }

    fn params_buf(&self, p: [u32; 8]) -> wgpu::Buffer {
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&p),
            usage: wgpu::BufferUsages::UNIFORM,
        })
    }

    fn dispatch_hash(&self, pipeline: &wgpu::ComputePipeline, aux: &[u8], p: [u32; 8], n: u32) -> Vec<Hash> {
        let io = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (n as u64) * 16 * 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let aux_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: aux,
            usage: wgpu::BufferUsages::STORAGE,
        });
        let pb = self.params_buf(p);
        let bg = self.bind(&io, &pb, &aux_buf);
        u32s_to_hashes(&self.dispatch_readback(pipeline, &bg, &io, n), n as usize)
    }

    // ── Batch primitives ────────────────────────────────────────

    /// Run batch Poseidon2 permutations on GPU.
    pub async fn batch_permute(&self, states: &[[Goldilocks; WIDTH]]) -> Vec<[Goldilocks; WIDTH]> {
        if states.is_empty() { return vec![]; }
        let n = states.len() as u32;
        let data = flatten_states(states);
        let io = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
        let pb = self.params_buf([n, 0, 0, 0, 0, 0, 0, 0]);
        let bg = self.bind(&io, &pb, &self.dummy_buffer);
        unflatten_states(&self.dispatch_readback(&self.permute_pipeline, &bg, &io, n), states.len())
    }

    /// Batch sponge hash of data chunks (plain hash, no tree domain).
    pub async fn batch_hash(&self, data: &[u8], chunk_size: usize) -> Vec<Hash> {
        if data.is_empty() { return vec![]; }
        let n = data.len().div_ceil(chunk_size) as u32;
        self.dispatch_hash(
            &self.hash_chunk_pipeline,
            &pad4(data),
            [n, DOMAIN_HASH, chunk_size as u32, data.len() as u32, 0, 0, 0, 0],
            n,
        )
    }

    /// Batch keyed hash — key read from aux[0..64) by GPU, no per-chunk duplication.
    pub async fn batch_keyed_hash(&self, key: &[u8; OUTPUT_BYTES], data: &[u8], chunk_size: usize) -> Vec<Hash> {
        if data.is_empty() { return vec![]; }
        let n = data.len().div_ceil(chunk_size) as u32;
        let mut buf = Vec::with_capacity(OUTPUT_BYTES + data.len());
        buf.extend_from_slice(key);
        buf.extend_from_slice(data);
        self.dispatch_hash(
            &self.keyed_hash_pipeline,
            &pad4(&buf),
            [n, 0, chunk_size as u32, data.len() as u32, 0, 0, 0, 0],
            n,
        )
    }

    /// Derive key: context hash on CPU, material hash on GPU.
    pub async fn batch_derive_key(&self, context: &str, data: &[u8], chunk_size: usize) -> Vec<Hash> {
        if data.is_empty() { return vec![]; }
        let ctx_hash = cyber_hemera::Hasher::new_derive_key_context(context).finalize();
        self.batch_derive_key_material(&ctx_hash, data, chunk_size).await
    }

    /// Derive key material phase, seeded by a pre-computed context hash.
    pub async fn batch_derive_key_material(&self, ctx: &Hash, data: &[u8], chunk_size: usize) -> Vec<Hash> {
        if data.is_empty() { return vec![]; }
        let n = data.len().div_ceil(chunk_size) as u32;
        let mut aux_u32s: Vec<u32> = Vec::new();
        push_hash_u32s(&mut aux_u32s, ctx);
        aux_u32s.extend(bytemuck::cast_slice::<u8, u32>(&pad4(data)).iter());
        self.dispatch_hash(
            &self.derive_key_material_pipeline,
            bytemuck::cast_slice(&aux_u32s),
            [n, 0, chunk_size as u32, data.len() as u32, 0, 0, 0, 0],
            n,
        )
    }

    /// Hash leaf chunks on GPU (sponge + tree domain).
    pub async fn batch_hash_leaves(&self, data: &[u8], chunk_size: usize, is_root: bool) -> Vec<Hash> {
        if data.is_empty() { return vec![]; }
        let n = data.len().div_ceil(chunk_size) as u32;
        let flags = if is_root { FLAG_ROOT } else { 0 };
        self.dispatch_hash(
            &self.hash_leaf_pipeline,
            &pad4(data),
            [n, flags, chunk_size as u32, data.len() as u32, 0, 0, 0, 0],
            n,
        )
    }

    /// Combine pairs of child hashes into parent hashes.
    pub async fn batch_hash_nodes(&self, pairs: &[(Hash, Hash)], is_root: bool) -> Vec<Hash> {
        if pairs.is_empty() { return vec![]; }
        let n = pairs.len() as u32;
        let flags = if is_root { FLAG_ROOT } else { 0 };
        self.dispatch_hash(
            &self.hash_node_pipeline,
            &flatten_pairs(pairs),
            [n, flags, 0, 0, 0, 0, 0, 0],
            n,
        )
    }

    /// Combine pairs with namespace bounds (NMT). Full u64 ns support.
    pub async fn batch_hash_nodes_nmt(
        &self, pairs: &[(Hash, Hash)], ns_min: u64, ns_max: u64, is_root: bool,
    ) -> Vec<Hash> {
        if pairs.is_empty() { return vec![]; }
        let n = pairs.len() as u32;
        let flags = if is_root { FLAG_ROOT } else { 0 };
        self.dispatch_hash(
            &self.hash_node_nmt_pipeline,
            &flatten_pairs(pairs),
            [n, flags, 0, 0,
             ns_min as u32, (ns_min >> 32) as u32,
             ns_max as u32, (ns_max >> 32) as u32],
            n,
        )
    }

    // ── High-level tree operations ──────────────────────────────

    /// Compute the Merkle root hash on GPU.
    ///
    /// Hashes all leaves in one GPU dispatch, then merges level-by-level
    /// until a single root remains. Matches `cyber_hemera::tree::root_hash`.
    pub async fn root_hash(&self, data: &[u8]) -> Hash {
        if data.is_empty() {
            return cyber_hemera::tree::hash_leaf(data, 0, true);
        }
        let n = data.len().div_ceil(CHUNK_SIZE);
        if n == 1 {
            return self.batch_hash_leaves(data, CHUNK_SIZE, true).await.remove(0);
        }
        let leaves = self.batch_hash_leaves(data, CHUNK_SIZE, false).await;
        self.merge_tree(leaves).await
    }

    /// Build the left-balanced tree from leaf hashes and return the root.
    ///
    /// Decomposes n leaves into complete binary subtrees, reduces each
    /// level-by-level on GPU, then folds subtree roots along the spine.
    async fn merge_tree(&self, leaves: Vec<Hash>) -> Hash {
        let n = leaves.len();
        let segments = left_balanced_decompose(n);
        let num_segments = segments.len();

        // Each segment reduces to one root hash. Track per-segment.
        let mut seg_roots: Vec<Option<Hash>> = vec![None; num_segments];
        let mut current_level: Vec<Hash> = leaves;
        let max_height = segments[0].1.trailing_zeros();

        // Initialize size-1 segments as already complete.
        for (si, &(_, size)) in segments.iter().enumerate() {
            if size == 1 {
                seg_roots[si] = Some(current_level[segments[si].0]);
            }
        }

        for round in 0..max_height {
            let mut next_level = Vec::new();
            let mut pairs: Vec<(Hash, Hash)> = Vec::new();
            let mut pair_segment: Vec<usize> = Vec::new();

            let mut offset = 0;
            for (si, &(_, size)) in segments.iter().enumerate() {
                let height = size.trailing_zeros();
                if height <= round {
                    // Already complete or size-1.
                    if size > 1 { offset += size >> round; }
                    continue;
                }
                let cur_count = size >> round;
                for i in (0..cur_count).step_by(2) {
                    pairs.push((current_level[offset + i], current_level[offset + i + 1]));
                    pair_segment.push(si);
                }
                offset += cur_count;
            }

            if pairs.is_empty() { break; }

            // Determine is_root: only if single segment and final round.
            let is_root = num_segments == 1 && round + 1 == max_height;
            let results = self.batch_hash_nodes(&pairs, is_root).await;

            let mut ri = 0;
            for (si, &(_, size)) in segments.iter().enumerate() {
                let height = size.trailing_zeros();
                if height <= round || size == 1 {
                    continue;
                }
                let result_count = size >> (round + 1);
                if height == round + 1 {
                    // Segment completing this round.
                    seg_roots[si] = Some(results[ri]);
                    ri += 1;
                } else {
                    next_level.extend_from_slice(&results[ri..ri + result_count]);
                    ri += result_count;
                }
            }

            current_level = next_level;
        }

        // All segments now have roots. Fold right-to-left along the spine.
        // The tree structure: seg[0] is left child of root spine,
        // seg[1] is left child of next spine node, etc.
        // Fold: start from rightmost two, merge, then merge with next-left.
        let mut roots: Vec<Hash> = seg_roots.into_iter().map(|r| r.unwrap()).collect();

        while roots.len() > 1 {
            let right = roots.pop().unwrap();
            let left = roots.pop().unwrap();
            let is_root = roots.is_empty();
            let result = self.batch_hash_nodes(&[(left, right)], is_root).await;
            roots.push(result[0]);
        }

        roots.pop().unwrap()
    }

    /// Compute the outboard (hash tree without data) on GPU.
    ///
    /// GPU hashes all leaves in parallel. CPU builds the tree structure
    /// and serializes parent pairs in pre-order.
    /// Returns `(root_hash, outboard_bytes)` matching `cyber_hemera::stream::outboard`.
    pub async fn outboard(&self, data: &[u8]) -> (Hash, Vec<u8>) {
        let n = if data.is_empty() { 1 } else { data.len().div_ceil(CHUNK_SIZE) };

        if n <= 1 {
            let root = self.batch_hash_leaves(data, CHUNK_SIZE, true).await.remove(0);
            let mut out = Vec::with_capacity(8);
            out.extend_from_slice(&(data.len() as u64).to_le_bytes());
            return (root, out);
        }

        // GPU: batch hash all leaves.
        let leaves = self.batch_hash_leaves(data, CHUNK_SIZE, false).await;

        // CPU: recursive tree build + pre-order serialization.
        let num_parents = n - 1;
        let mut out = Vec::with_capacity(8 + num_parents * OUTPUT_BYTES * 2);
        out.extend_from_slice(&(data.len() as u64).to_le_bytes());
        let root = outboard_subtree_from_leaves(&leaves, 0, n, true, &mut out);
        (root, out)
    }

    /// Verify multiple inclusion proofs in batch on GPU.
    ///
    /// Each entry is `(chunk_data, proof, expected_root)`.
    /// Returns a Vec of bools indicating which proofs verified successfully.
    pub async fn batch_verify_proofs(
        &self,
        proofs: &[(&[u8], &InclusionProof, &Hash)],
    ) -> Vec<bool> {
        if proofs.is_empty() { return vec![]; }

        let max_depth = proofs.iter().map(|(_, p, _)| p.depth()).max().unwrap_or(0);

        // Compute initial hashes (leaf or subtree root).
        let mut current: Vec<Hash> = proofs.iter().map(|(chunk, proof, _)| {
            let start = proof.start_chunk;
            let end = proof.end_chunk;
            if end - start == 1 {
                cyber_hemera::tree::hash_leaf(chunk, start, proof.num_chunks == 1)
            } else {
                cyber_hemera::tree::root_hash(chunk)
            }
        }).collect();

        if max_depth == 0 {
            return proofs.iter().enumerate()
                .map(|(i, (_, _, root))| current[i] == **root)
                .collect();
        }

        // Walk proof levels leaf-to-root, batching hash_node calls on GPU.
        // Siblings are stored root-to-leaf, so index from the end.
        for level in 0..max_depth {
            let mut root_pairs: Vec<(Hash, Hash)> = Vec::new();
            let mut root_indices: Vec<usize> = Vec::new();
            let mut inner_pairs: Vec<(Hash, Hash)> = Vec::new();
            let mut inner_indices: Vec<usize> = Vec::new();

            for (i, (_, proof, _)) in proofs.iter().enumerate() {
                let siblings = proof.siblings();
                // level 0 = leaf-most sibling (last in array).
                let sib_idx = siblings.len().checked_sub(1 + level);
                let Some(sib_idx) = sib_idx else { continue; };
                let is_root = sib_idx == 0;
                let pair = match siblings[sib_idx] {
                    Sibling::Left(sib) => (sib, current[i]),
                    Sibling::Right(sib) => (current[i], sib),
                };
                if is_root {
                    root_pairs.push(pair);
                    root_indices.push(i);
                } else {
                    inner_pairs.push(pair);
                    inner_indices.push(i);
                }
            }

            if !inner_pairs.is_empty() {
                let results = self.batch_hash_nodes(&inner_pairs, false).await;
                for (j, &idx) in inner_indices.iter().enumerate() {
                    current[idx] = results[j];
                }
            }
            if !root_pairs.is_empty() {
                let results = self.batch_hash_nodes(&root_pairs, true).await;
                for (j, &idx) in root_indices.iter().enumerate() {
                    current[idx] = results[j];
                }
            }
        }

        proofs.iter().enumerate().map(|(i, (_, _, root))| current[i] == **root).collect()
    }

    /// Batch XOF squeeze: given finalized sponge states, produce `count`
    /// output blocks (64 bytes each) per state using GPU permutations.
    pub async fn batch_squeeze(
        &self,
        states: &[[Goldilocks; WIDTH]],
        count: usize,
    ) -> Vec<Vec<[u8; OUTPUT_BYTES]>> {
        if states.is_empty() || count == 0 { return vec![vec![]; states.len()]; }

        let n = states.len();
        let mut result = vec![Vec::with_capacity(count); n];

        // Block 0: extract from initial state (before any permutation).
        for (i, state) in states.iter().enumerate() {
            result[i].push(extract_output(state));
        }

        // Blocks 1..count: permute, then extract.
        let mut current_states: Vec<[Goldilocks; WIDTH]> = states.to_vec();
        for _ in 1..count {
            current_states = self.batch_permute(&current_states).await;
            for (i, state) in current_states.iter().enumerate() {
                result[i].push(extract_output(state));
            }
        }

        result
    }
}

// ── Helpers ─────────────────────────────────────────────────────

fn buffer_entry(binding: u32, ty: wgpu::BufferBindingType, min: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: Some(NonZeroU64::new(min).unwrap()),
        },
        count: None,
    }
}

fn pad4(data: &[u8]) -> Vec<u8> {
    let mut v = data.to_vec();
    v.resize(v.len().next_multiple_of(4), 0);
    v
}

fn flatten_states(states: &[[Goldilocks; WIDTH]]) -> Vec<u32> {
    let mut out = Vec::with_capacity(states.len() * WIDTH * 2);
    for state in states {
        for e in state {
            let v = e.as_canonical_u64();
            out.push(v as u32);
            out.push((v >> 32) as u32);
        }
    }
    out
}

fn unflatten_states(u32s: &[u32], count: usize) -> Vec<[Goldilocks; WIDTH]> {
    (0..count)
        .map(|idx| {
            core::array::from_fn(|i| {
                let off = idx * WIDTH * 2 + i * 2;
                Goldilocks::new(u32s[off] as u64 | ((u32s[off + 1] as u64) << 32))
            })
        })
        .collect()
}

fn push_hash_u32s(out: &mut Vec<u32>, hash: &Hash) {
    for i in 0..8 {
        let v = u64::from_le_bytes(hash.as_bytes()[i * 8..(i + 1) * 8].try_into().unwrap());
        out.push(v as u32);
        out.push((v >> 32) as u32);
    }
}

fn flatten_pairs(pairs: &[(Hash, Hash)]) -> Vec<u8> {
    let mut u32s = Vec::with_capacity(pairs.len() * 32);
    for (l, r) in pairs {
        push_hash_u32s(&mut u32s, l);
        push_hash_u32s(&mut u32s, r);
    }
    bytemuck::cast_slice(&u32s).to_vec()
}

fn u32s_to_hashes(u32s: &[u32], count: usize) -> Vec<Hash> {
    (0..count)
        .map(|idx| {
            let mut bytes = [0u8; OUTPUT_BYTES];
            for i in 0..8 {
                let off = idx * 16 + i * 2;
                let v = u32s[off] as u64 | ((u32s[off + 1] as u64) << 32);
                bytes[i * 8..(i + 1) * 8].copy_from_slice(&v.to_le_bytes());
            }
            Hash::from_bytes(bytes)
        })
        .collect()
}

fn generate_round_constants_u32() -> Vec<u32> {
    cyber_hemera::constants::ROUND_CONSTANTS_U64
        .iter()
        .flat_map(|&v| [v as u32, (v >> 32) as u32])
        .collect()
}

fn generate_matrix_diag_u32() -> Vec<u32> {
    cyber_hemera::field::MATRIX_DIAG_16
        .iter()
        .flat_map(|e| {
            let v = e.as_canonical_u64();
            [v as u32, (v >> 32) as u32]
        })
        .collect()
}

fn extract_output(state: &[Goldilocks; WIDTH]) -> [u8; OUTPUT_BYTES] {
    let mut out = [0u8; OUTPUT_BYTES];
    for i in 0..8 {
        out[i * 8..(i + 1) * 8].copy_from_slice(&state[i].as_canonical_u64().to_le_bytes());
    }
    out
}

// ── Tree helpers ─────────────────────────────────────────────

/// Decompose n into complete binary subtrees (left-balanced).
/// Returns [(start_leaf, size)] where each size is a power of 2.
fn left_balanced_decompose(n: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut offset = 0;
    let mut remaining = n;
    while remaining > 0 {
        if remaining.is_power_of_two() {
            result.push((offset, remaining));
            break;
        }
        let split = left_subtree_chunks(remaining);
        result.push((offset, split));
        offset += split;
        remaining -= split;
    }
    result
}

fn left_subtree_chunks(count: usize) -> usize {
    debug_assert!(count > 1);
    1 << (usize::BITS - (count - 1).leading_zeros() - 1)
}

/// Recursive outboard subtree using pre-computed leaf hashes (CPU hash_node).
fn outboard_subtree_from_leaves(
    leaves: &[Hash],
    offset: usize,
    count: usize,
    is_root: bool,
    out: &mut Vec<u8>,
) -> Hash {
    if count == 1 {
        return leaves[offset];
    }

    let split = left_subtree_chunks(count);

    // Reserve slot for this parent's hash pair (pre-order).
    let pair_start = out.len();
    out.extend_from_slice(&[0u8; OUTPUT_BYTES * 2]);

    let left = outboard_subtree_from_leaves(leaves, offset, split, false, out);
    let right = outboard_subtree_from_leaves(leaves, offset + split, count - split, false, out);

    // Fill in the reserved slot.
    out[pair_start..pair_start + OUTPUT_BYTES].copy_from_slice(left.as_ref());
    out[pair_start + OUTPUT_BYTES..pair_start + OUTPUT_BYTES * 2].copy_from_slice(right.as_ref());

    cyber_hemera::tree::hash_node(&left, &right, is_root)
}
