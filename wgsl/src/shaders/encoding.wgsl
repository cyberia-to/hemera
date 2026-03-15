// ── Encoding ──────────────────────────────────────────────────────
//
// Byte → Goldilocks element conversion.
// Matches rs/src/encoding.rs (7 bytes per element, little-endian).

fn read_aux_byte(byte_off: u32) -> u32 {
    let word = aux_data[byte_off >> 2u];
    return (word >> ((byte_off & 3u) * 8u)) & 0xFFu;
}

fn encode_element(byte_start: u32, avail: u32) -> vec2<u32> {
    var lo = 0u;
    var hi = 0u;
    for (var i = 0u; i < 7u; i++) {
        if i < avail {
            let b = read_aux_byte(byte_start + i);
            if i < 4u {
                lo |= b << (i * 8u);
            } else {
                hi |= b << ((i - 4u) * 8u);
            }
        }
    }
    return vec2<u32>(lo, hi);
}

// Encode one element of the padded final sponge block.
// Handles: data bytes | 0x01 pad marker | 0x00 fill
fn encode_padded_element(data_start: u32, remaining: u32, elem_idx: u32) -> vec2<u32> {
    var lo = 0u;
    var hi = 0u;
    let elem_start = elem_idx * 7u;
    for (var j = 0u; j < 7u; j++) {
        let pos = elem_start + j;
        var b = 0u;
        if pos < remaining {
            b = read_aux_byte(data_start + pos);
        } else if pos == remaining {
            b = 1u;
        }
        if j < 4u {
            lo |= b << (j * 8u);
        } else {
            hi |= b << ((j - 4u) * 8u);
        }
    }
    return vec2<u32>(lo, hi);
}

// ── Keyed (split-region) encoding ───────────────────────────────
//
// For keyed hash: key at aux[0..64), chunk data at aux[data_start..).
// Virtual byte stream = key || chunk_data.

fn read_keyed_byte(voff: u32, data_start: u32) -> u32 {
    if voff < OUTPUT_BYTES_CONST {
        return read_aux_byte(voff);
    }
    return read_aux_byte(data_start + voff - OUTPUT_BYTES_CONST);
}

fn encode_element_keyed(voff: u32, avail: u32, data_start: u32) -> vec2<u32> {
    var lo = 0u;
    var hi = 0u;
    for (var i = 0u; i < 7u; i++) {
        if i < avail {
            let b = read_keyed_byte(voff + i, data_start);
            if i < 4u {
                lo |= b << (i * 8u);
            } else {
                hi |= b << ((i - 4u) * 8u);
            }
        }
    }
    return vec2<u32>(lo, hi);
}

fn encode_padded_element_keyed(voff: u32, remaining: u32, elem_idx: u32, data_start: u32) -> vec2<u32> {
    var lo = 0u;
    var hi = 0u;
    let elem_start = elem_idx * 7u;
    for (var j = 0u; j < 7u; j++) {
        let pos = elem_start + j;
        var b = 0u;
        if pos < remaining {
            b = read_keyed_byte(voff + pos, data_start);
        } else if pos == remaining {
            b = 1u;
        }
        if j < 4u {
            lo |= b << (j * 8u);
        } else {
            hi |= b << ((j - 4u) * 8u);
        }
    }
    return vec2<u32>(lo, hi);
}
