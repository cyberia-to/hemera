// ── Permutation ───────────────────────────────────────────────────
//
// MDS layers, internal diffusion, and full Poseidon2 permutation.
// Matches rs/src/permutation.rs and rs/src/field.rs (MDS/matmul).

fn load_rc(idx: u32) -> vec2<u32> {
    let off = idx * 2u;
    return vec2<u32>(round_constants[off], round_constants[off + 1u]);
}

fn load_diag(idx: u32) -> vec2<u32> {
    let off = idx * 2u;
    return vec2<u32>(matrix_diag[off], matrix_diag[off + 1u]);
}

// 4×4 circulant MDS matrix [2,3,1,1]
fn apply_mat4(x: ptr<function, array<vec2<u32>, 4>>) {
    let x0 = (*x)[0]; let x1 = (*x)[1]; let x2 = (*x)[2]; let x3 = (*x)[3];

    let t01 = gl_add(x0.x, x0.y, x1.x, x1.y);
    let t23 = gl_add(x2.x, x2.y, x3.x, x3.y);
    let t0123 = gl_add(t01.x, t01.y, t23.x, t23.y);
    let t01123 = gl_add(t0123.x, t0123.y, x1.x, x1.y);
    let t01233 = gl_add(t0123.x, t0123.y, x3.x, x3.y);

    let dbl_x0 = gl_double(x0.x, x0.y);
    let dbl_x2 = gl_double(x2.x, x2.y);

    (*x)[3] = gl_add(t01233.x, t01233.y, dbl_x0.x, dbl_x0.y);
    (*x)[1] = gl_add(t01123.x, t01123.y, dbl_x2.x, dbl_x2.y);
    (*x)[0] = gl_add(t01123.x, t01123.y, t01.x, t01.y);
    (*x)[2] = gl_add(t01233.x, t01233.y, t23.x, t23.y);
}

// External MDS: M4 per 4-element chunk + column sums
fn mds_external(s: ptr<function, array<vec2<u32>, 16>>) {
    for (var c = 0u; c < 4u; c++) {
        var chunk: array<vec2<u32>, 4>;
        chunk[0] = (*s)[c * 4u + 0u];
        chunk[1] = (*s)[c * 4u + 1u];
        chunk[2] = (*s)[c * 4u + 2u];
        chunk[3] = (*s)[c * 4u + 3u];
        apply_mat4(&chunk);
        (*s)[c * 4u + 0u] = chunk[0];
        (*s)[c * 4u + 1u] = chunk[1];
        (*s)[c * 4u + 2u] = chunk[2];
        (*s)[c * 4u + 3u] = chunk[3];
    }

    var sums: array<vec2<u32>, 4>;
    for (var k = 0u; k < 4u; k++) {
        var acc = (*s)[k];
        acc = gl_add(acc.x, acc.y, (*s)[4u + k].x, (*s)[4u + k].y);
        acc = gl_add(acc.x, acc.y, (*s)[8u + k].x, (*s)[8u + k].y);
        acc = gl_add(acc.x, acc.y, (*s)[12u + k].x, (*s)[12u + k].y);
        sums[k] = acc;
    }

    for (var i = 0u; i < WIDTH; i++) {
        let k = i % 4u;
        (*s)[i] = gl_add((*s)[i].x, (*s)[i].y, sums[k].x, sums[k].y);
    }
}

// Internal diffusion: M_I = 1 + diag(d)
fn matmul_internal(s: ptr<function, array<vec2<u32>, 16>>) {
    var sum = (*s)[0];
    for (var i = 1u; i < WIDTH; i++) {
        sum = gl_add(sum.x, sum.y, (*s)[i].x, (*s)[i].y);
    }
    for (var i = 0u; i < WIDTH; i++) {
        let d = load_diag(i);
        let prod = gl_mul(d.x, d.y, (*s)[i].x, (*s)[i].y);
        (*s)[i] = gl_add(prod.x, prod.y, sum.x, sum.y);
    }
}

// Full permutation: initial MDS → 4 full → 64 partial → 4 full
fn permute_state(s: ptr<function, array<vec2<u32>, 16>>) {
    mds_external(s);

    for (var r = 0u; r < ROUNDS_F / 2u; r++) {
        for (var i = 0u; i < WIDTH; i++) {
            let rc = load_rc(r * WIDTH + i);
            (*s)[i] = gl_add((*s)[i].x, (*s)[i].y, rc.x, rc.y);
        }
        for (var i = 0u; i < WIDTH; i++) {
            (*s)[i] = gl_pow7((*s)[i].x, (*s)[i].y);
        }
        mds_external(s);
    }

    let internal_rc_offset = ROUNDS_F * WIDTH;
    for (var r = 0u; r < ROUNDS_P; r++) {
        let rc = load_rc(internal_rc_offset + r);
        (*s)[0] = gl_add((*s)[0].x, (*s)[0].y, rc.x, rc.y);
        (*s)[0] = gl_pow7((*s)[0].x, (*s)[0].y);
        matmul_internal(s);
    }

    let terminal_rc_offset = (ROUNDS_F / 2u) * WIDTH;
    for (var r = 0u; r < ROUNDS_F / 2u; r++) {
        for (var i = 0u; i < WIDTH; i++) {
            let rc = load_rc(terminal_rc_offset + r * WIDTH + i);
            (*s)[i] = gl_add((*s)[i].x, (*s)[i].y, rc.x, rc.y);
        }
        for (var i = 0u; i < WIDTH; i++) {
            (*s)[i] = gl_pow7((*s)[i].x, (*s)[i].y);
        }
        mds_external(s);
    }
}
