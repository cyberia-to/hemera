// ── Sponge ────────────────────────────────────────────────────────
//
// Full sponge hash: absorb rate blocks + pad + finalize.
// Matches rs/src/sponge.rs (Hasher::new + update + finalize).

fn sponge_hash_domain(chunk_start: u32, chunk_len: u32, domain: u32) -> array<vec2<u32>, 8> {
    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_DOMAIN] = vec2<u32>(domain, 0u);

    var absorbed = 0u;

    for (var _block = 0u; _block < 256u; _block++) {
        if absorbed + RATE_BYTES > chunk_len { break; }
        for (var i = 0u; i < RATE; i++) {
            let elem = encode_element(chunk_start + absorbed + i * BYTES_PER_ELEMENT, BYTES_PER_ELEMENT);
            state[i] = gl_add(state[i].x, state[i].y, elem.x, elem.y);
        }
        permute_state(&state);
        absorbed += RATE_BYTES;
    }

    let remaining = chunk_len - absorbed;
    for (var i = 0u; i < RATE; i++) {
        let elem = encode_padded_element(chunk_start + absorbed, remaining, i);
        state[i] = gl_add(state[i].x, state[i].y, elem.x, elem.y);
    }

    state[CAP_LENGTH] = vec2<u32>(chunk_len, 0u);
    permute_state(&state);

    var output: array<vec2<u32>, 8>;
    for (var i = 0u; i < 8u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}

fn sponge_hash(chunk_start: u32, chunk_len: u32) -> array<vec2<u32>, 8> {
    return sponge_hash_domain(chunk_start, chunk_len, DOMAIN_HASH);
}

// Keyed sponge: key at aux[0..64), chunk data at aux[data_start..).
// Virtual stream = key(64B) || chunk_data(data_len B).
fn sponge_hash_keyed(data_start: u32, data_len: u32) -> array<vec2<u32>, 8> {
    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_DOMAIN] = vec2<u32>(DOMAIN_KEYED, 0u);

    let total_len = OUTPUT_BYTES_CONST + data_len;
    var absorbed = 0u;

    for (var _block = 0u; _block < 256u; _block++) {
        if absorbed + RATE_BYTES > total_len { break; }
        for (var i = 0u; i < RATE; i++) {
            let voff = absorbed + i * BYTES_PER_ELEMENT;
            let elem = encode_element_keyed(voff, BYTES_PER_ELEMENT, data_start);
            state[i] = gl_add(state[i].x, state[i].y, elem.x, elem.y);
        }
        permute_state(&state);
        absorbed += RATE_BYTES;
    }

    let remaining = total_len - absorbed;
    for (var i = 0u; i < RATE; i++) {
        let elem = encode_padded_element_keyed(absorbed, remaining, i, data_start);
        state[i] = gl_add(state[i].x, state[i].y, elem.x, elem.y);
    }

    state[CAP_LENGTH] = vec2<u32>(total_len, 0u);
    permute_state(&state);

    var output: array<vec2<u32>, 8>;
    for (var i = 0u; i < 8u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}

// Derive key material sponge: seed state with context hash, then absorb material.
// cv_base: offset in aux_data (u32 index) where the 16 u32s of context hash start.
fn sponge_hash_derive_material(cv_base: u32, data_start: u32, data_len: u32) -> array<vec2<u32>, 8> {
    var state: array<vec2<u32>, 16>;
    for (var i = 0u; i < 16u; i++) {
        state[i] = vec2<u32>(0u, 0u);
    }
    state[CAP_DOMAIN] = vec2<u32>(DOMAIN_DERIVE_KEY_MATERIAL, 0u);

    for (var i = 0u; i < 8u; i++) {
        state[i] = load_aux_elem(cv_base, i);
    }
    permute_state(&state);

    var absorbed = 0u;
    for (var _block = 0u; _block < 256u; _block++) {
        if absorbed + RATE_BYTES > data_len { break; }
        for (var i = 0u; i < RATE; i++) {
            let elem = encode_element(data_start + absorbed + i * BYTES_PER_ELEMENT, BYTES_PER_ELEMENT);
            state[i] = gl_add(state[i].x, state[i].y, elem.x, elem.y);
        }
        permute_state(&state);
        absorbed += RATE_BYTES;
    }

    let remaining = data_len - absorbed;
    for (var i = 0u; i < RATE; i++) {
        let elem = encode_padded_element(data_start + absorbed, remaining, i);
        state[i] = gl_add(state[i].x, state[i].y, elem.x, elem.y);
    }
    state[CAP_LENGTH] = vec2<u32>(data_len, 0u);

    permute_state(&state);

    var output: array<vec2<u32>, 8>;
    for (var i = 0u; i < 8u; i++) {
        output[i] = reduce(state[i].x, state[i].y);
    }
    return output;
}
