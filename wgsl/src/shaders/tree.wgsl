// ── Tree ──────────────────────────────────────────────────────────
//
// Leaf and node hashing with tree domain separation.
// Matches rs/src/tree.rs (hash_leaf, hash_node, hash_node_nmt).

fn tree_hash_leaf(chunk_start: u32, chunk_len: u32, counter: u32, flags: u32) -> array<vec2<u32>, 8> {
    let base_hash = sponge_hash(chunk_start, chunk_len);

    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 8u; i++) {
        state[i] = base_hash[i];
    }
    for (var i = 8u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_COUNTER] = vec2<u32>(counter, 0u);
    state[CAP_FLAGS] = vec2<u32>(flags, 0u);

    permute_state(&state);

    var output: array<vec2<u32>, 8>;
    for (var i = 0u; i < 8u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}

fn tree_hash_node(
    left: array<vec2<u32>, 8>,
    right: array<vec2<u32>, 8>,
    flags: u32,
) -> array<vec2<u32>, 8> {
    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_FLAGS] = vec2<u32>(flags, 0u);

    for (var i = 0u; i < 8u; i++) {
        state[i] = gl_add(state[i].x, state[i].y, left[i].x, left[i].y);
    }
    permute_state(&state);

    for (var i = 0u; i < 8u; i++) {
        state[i] = gl_add(state[i].x, state[i].y, right[i].x, right[i].y);
    }
    permute_state(&state);

    var output: array<vec2<u32>, 8>;
    for (var i = 0u; i < 8u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}

fn tree_hash_node_nmt(
    left: array<vec2<u32>, 8>,
    right: array<vec2<u32>, 8>,
    flags: u32,
    ns_min: vec2<u32>,
    ns_max: vec2<u32>,
) -> array<vec2<u32>, 8> {
    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_FLAGS] = vec2<u32>(flags, 0u);
    state[CAP_NS_MIN] = ns_min;
    state[CAP_NS_MAX] = ns_max;

    for (var i = 0u; i < 8u; i++) {
        state[i] = gl_add(state[i].x, state[i].y, left[i].x, left[i].y);
    }
    permute_state(&state);

    for (var i = 0u; i < 8u; i++) {
        state[i] = gl_add(state[i].x, state[i].y, right[i].x, right[i].y);
    }
    permute_state(&state);

    var output: array<vec2<u32>, 8>;
    for (var i = 0u; i < 8u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}
