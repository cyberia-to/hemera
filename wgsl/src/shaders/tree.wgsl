// ── Tree ──────────────────────────────────────────────────────────
//
// Leaf and node hashing with tree domain separation.
// Matches rs/src/tree.rs (hash_leaf, hash_node, hash_node_nmt).
// Output is 4 Goldilocks elements (32 bytes).

fn tree_hash_leaf(chunk_start: u32, chunk_len: u32, counter: u32, flags: u32) -> array<vec2<u32>, 4> {
    let base_hash = sponge_hash(chunk_start, chunk_len);

    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 4u; i++) {
        state[i] = base_hash[i];
    }
    for (var i = 4u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_COUNTER] = vec2<u32>(counter, 0u);
    state[CAP_FLAGS] = vec2<u32>(flags, 0u);

    permute_state(&state);

    var output: array<vec2<u32>, 4>;
    for (var i = 0u; i < 4u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}

fn tree_hash_node(
    left: array<vec2<u32>, 4>,
    right: array<vec2<u32>, 4>,
    flags: u32,
) -> array<vec2<u32>, 4> {
    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_FLAGS] = vec2<u32>(flags, 0u);

    // Absorb both children (4 + 4 = 8 elements = one full rate block).
    for (var i = 0u; i < 4u; i++) {
        state[i] = gl_add(state[i].x, state[i].y, left[i].x, left[i].y);
    }
    for (var i = 0u; i < 4u; i++) {
        state[4u + i] = gl_add(state[4u + i].x, state[4u + i].y, right[i].x, right[i].y);
    }
    permute_state(&state);

    var output: array<vec2<u32>, 4>;
    for (var i = 0u; i < 4u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}

fn tree_hash_node_nmt(
    left: array<vec2<u32>, 4>,
    right: array<vec2<u32>, 4>,
    flags: u32,
    ns_min: vec2<u32>,
    ns_max: vec2<u32>,
) -> array<vec2<u32>, 4> {
    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_FLAGS] = vec2<u32>(flags, 0u);
    state[CAP_NS_MIN] = ns_min;
    state[CAP_NS_MAX] = ns_max;

    // Absorb both children (4 + 4 = 8 elements = one full rate block).
    for (var i = 0u; i < 4u; i++) {
        state[i] = gl_add(state[i].x, state[i].y, left[i].x, left[i].y);
    }
    for (var i = 0u; i < 4u; i++) {
        state[4u + i] = gl_add(state[4u + i].x, state[4u + i].y, right[i].x, right[i].y);
    }
    permute_state(&state);

    var output: array<vec2<u32>, 4>;
    for (var i = 0u; i < 4u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}
