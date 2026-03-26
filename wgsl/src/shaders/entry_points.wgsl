// ── I/O helpers ───────────────────────────────────────────────────

fn load_io(base: u32, idx: u32) -> vec2<u32> {
    let off = base + idx * 2u;
    return vec2<u32>(io_data[off], io_data[off + 1u]);
}

fn store_io(base: u32, idx: u32, val: vec2<u32>) {
    let off = base + idx * 2u;
    io_data[off] = val.x;
    io_data[off + 1u] = val.y;
}

fn load_aux_elem(base: u32, idx: u32) -> vec2<u32> {
    let off = base + idx * 2u;
    return vec2<u32>(aux_data[off], aux_data[off + 1u]);
}

fn store_hash_output(base_idx: u32, result: array<vec2<u32>, 4>) {
    let out_base = base_idx * HASH_U32S;
    for (var i = 0u; i < 4u; i++) {
        store_io(out_base, i, result[i]);
    }
}

// ── Entry Points ──────────────────────────────────────────────────

@compute @workgroup_size(64)
fn hemera_permute(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if idx >= dp.count { return; }

    let base = idx * STATE_U32S;
    var s: array<vec2<u32>, 16>;
    for (var i = 0u; i < WIDTH; i++) {
        s[i] = load_io(base, i);
    }

    permute_state(&s);

    for (var i = 0u; i < WIDTH; i++) {
        store_io(base, i, s[i]);
    }
}

@compute @workgroup_size(64)
fn hemera_hash_leaf(@builtin(global_invocation_id) gid: vec3<u32>) {
    let chunk_idx = gid.x;
    if chunk_idx >= dp.count { return; }

    let chunk_start = chunk_idx * dp.chunk_size;
    let chunk_len = min(dp.chunk_size, dp.total_bytes - chunk_start);
    // Global counter = local index + batch offset (dp.ns_min_lo repurposed).
    let counter = chunk_idx + dp.ns_min_lo;

    let leaf_flags = FLAG_CHUNK | dp.flags;
    let result = tree_hash_leaf(chunk_start, chunk_len, counter, leaf_flags);
    store_hash_output(chunk_idx, result);
}

@compute @workgroup_size(64)
fn hemera_hash_node(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair_idx = gid.x;
    if pair_idx >= dp.count { return; }

    let in_base = pair_idx * (HASH_U32S * 2u);
    var left: array<vec2<u32>, 4>;
    var right: array<vec2<u32>, 4>;
    for (var i = 0u; i < 4u; i++) {
        left[i] = load_aux_elem(in_base, i);
        right[i] = load_aux_elem(in_base + HASH_U32S, i);
    }

    let node_flags = FLAG_PARENT | dp.flags;
    let result = tree_hash_node(left, right, node_flags);
    store_hash_output(pair_idx, result);
}

// Plain sponge hash with domain separation.
// dp: count, domain (in flags), chunk_size, total_bytes
@compute @workgroup_size(64)
fn hemera_hash_chunk(@builtin(global_invocation_id) gid: vec3<u32>) {
    let chunk_idx = gid.x;
    if chunk_idx >= dp.count { return; }

    let chunk_start = chunk_idx * dp.chunk_size;
    let chunk_len = min(dp.chunk_size, dp.total_bytes - chunk_start);

    let result = sponge_hash_domain(chunk_start, chunk_len, dp.flags);
    store_hash_output(chunk_idx, result);
}

// Keyed hash: key at aux[0..32), data chunks at aux[32..).
// dp: count, 0, chunk_size, total_data_bytes
@compute @workgroup_size(64)
fn hemera_keyed_hash(@builtin(global_invocation_id) gid: vec3<u32>) {
    let chunk_idx = gid.x;
    if chunk_idx >= dp.count { return; }

    let data_start = OUTPUT_BYTES_CONST + chunk_idx * dp.chunk_size;
    let chunk_len = min(dp.chunk_size, dp.total_bytes - chunk_idx * dp.chunk_size);

    let result = sponge_hash_keyed(data_start, chunk_len);
    store_hash_output(chunk_idx, result);
}

// Derive key material phase.
// dp: count, 0, chunk_size, material_total_bytes
// aux_data: [context_hash(8 u32s)] [material bytes...]
@compute @workgroup_size(64)
fn hemera_derive_key_material(@builtin(global_invocation_id) gid: vec3<u32>) {
    let chunk_idx = gid.x;
    if chunk_idx >= dp.count { return; }

    let cv_base = 0u;
    let data_byte_offset = OUTPUT_BYTES_CONST;
    let chunk_start = data_byte_offset + chunk_idx * dp.chunk_size;
    let chunk_len = min(dp.chunk_size, dp.total_bytes - chunk_idx * dp.chunk_size);

    let result = sponge_hash_derive_material(cv_base, chunk_start, chunk_len);
    store_hash_output(chunk_idx, result);
}

// NMT node hashing with namespace bounds from dispatch params.
// dp: count, flags, 0, 0, ns_min_lo, ns_min_hi, ns_max_lo, ns_max_hi
@compute @workgroup_size(64)
fn hemera_hash_node_nmt(@builtin(global_invocation_id) gid: vec3<u32>) {
    let pair_idx = gid.x;
    if pair_idx >= dp.count { return; }

    let ns_min = vec2<u32>(dp.ns_min_lo, dp.ns_min_hi);
    let ns_max = vec2<u32>(dp.ns_max_lo, dp.ns_max_hi);

    let in_base = pair_idx * (HASH_U32S * 2u);
    var left: array<vec2<u32>, 4>;
    var right: array<vec2<u32>, 4>;
    for (var i = 0u; i < 4u; i++) {
        left[i] = load_aux_elem(in_base, i);
        right[i] = load_aux_elem(in_base + HASH_U32S, i);
    }

    let node_flags = FLAG_PARENT | dp.flags;
    let result = tree_hash_node_nmt(left, right, node_flags, ns_min, ns_max);
    store_hash_output(pair_idx, result);
}
