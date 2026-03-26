// ── Field ─────────────────────────────────────────────────────────
//
// Goldilocks arithmetic (p = 2^64 - 2^32 + 1) emulated as u32 pairs.
// Matches rs/src/field.rs exactly.

fn add64(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let lo = a_lo + b_lo;
    let carry = select(0u, 1u, lo < a_lo);
    let hi = a_hi + b_hi + carry;
    return vec2<u32>(lo, hi);
}

fn sub64(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let borrow = select(0u, 1u, a_lo < b_lo);
    let lo = a_lo - b_lo;
    let hi = a_hi - b_hi - borrow;
    return vec2<u32>(lo, hi);
}

fn gte64(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> bool {
    if a_hi > b_hi { return true; }
    if a_hi < b_hi { return false; }
    return a_lo >= b_lo;
}

fn reduce(lo: u32, hi: u32) -> vec2<u32> {
    if gte64(lo, hi, P_LO, P_HI) {
        return sub64(lo, hi, P_LO, P_HI);
    }
    return vec2<u32>(lo, hi);
}

fn gl_add(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let sum = add64(a_lo, a_hi, b_lo, b_hi);
    let overflow = (sum.y < a_hi) || (sum.y == a_hi && sum.x < a_lo);
    if overflow {
        let adj = add64(sum.x, sum.y, NEG_ORDER, 0u);
        let overflow2 = (adj.y < sum.y) || (adj.y == sum.y && adj.x < sum.x);
        if overflow2 {
            let adj2 = add64(adj.x, adj.y, NEG_ORDER, 0u);
            return reduce(adj2.x, adj2.y);
        }
        return reduce(adj.x, adj.y);
    }
    return reduce(sum.x, sum.y);
}

fn gl_sub(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let diff = sub64(a_lo, a_hi, b_lo, b_hi);
    let underflow = (a_hi < b_hi) || (a_hi == b_hi && a_lo < b_lo);
    if underflow {
        let adj = sub64(diff.x, diff.y, NEG_ORDER, 0u);
        let underflow2 = (diff.y == 0u && diff.x < NEG_ORDER);
        if underflow2 {
            let adj2 = sub64(adj.x, adj.y, NEG_ORDER, 0u);
            return adj2;
        }
        return adj;
    }
    return diff;
}

fn mul32(a: u32, b: u32) -> vec2<u32> {
    let a_lo = a & 0xFFFFu;
    let a_hi = a >> 16u;
    let b_lo = b & 0xFFFFu;
    let b_hi = b >> 16u;

    let ll = a_lo * b_lo;
    let lh = a_lo * b_hi;
    let hl = a_hi * b_lo;
    let hh = a_hi * b_hi;

    let mid = lh + (ll >> 16u);
    let mid2 = (mid & 0xFFFFu) + hl;

    let lo = (mid2 << 16u) | (ll & 0xFFFFu);
    let hi = hh + (mid >> 16u) + (mid2 >> 16u);
    return vec2<u32>(lo, hi);
}

// Exact reduce128 matching CPU field.rs
fn gl_mul(a_lo: u32, a_hi: u32, b_lo: u32, b_hi: u32) -> vec2<u32> {
    let ll = mul32(a_lo, b_lo);
    let lh = mul32(a_lo, b_hi);
    let hl = mul32(a_hi, b_lo);
    let hh = mul32(a_hi, b_hi);

    let r0 = ll.x;
    let t1 = add64(ll.y, 0u, lh.x, 0u);
    let t2 = add64(t1.x, t1.y, hl.x, 0u);
    let r1 = t2.x;
    let carry1 = t2.y;
    let t3 = add64(lh.y, 0u, hl.y, 0u);
    let t4 = add64(t3.x, t3.y, hh.x, 0u);
    let t5 = add64(t4.x, t4.y, carry1, 0u);
    let r2 = t5.x;
    let carry2 = t5.y;
    let r3 = hh.y + carry2;

    let sub_borrow = select(0u, 1u, r0 < r3);
    var t0_lo = r0 - r3;
    var t0_hi = r1 - sub_borrow;
    let real_borrow = (r1 == 0u && r0 < r3) || (r1 < sub_borrow);

    if real_borrow {
        let sub2_borrow = select(0u, 1u, t0_lo < NEG_ORDER);
        t0_lo = t0_lo - NEG_ORDER;
        t0_hi = t0_hi - sub2_borrow;
    }

    let t1_val = mul32(r2, NEG_ORDER);
    let res = add64(t0_lo, t0_hi, t1_val.x, t1_val.y);
    let add_carry = (res.y < t0_hi) || (res.y == t0_hi && res.x < t0_lo);

    if add_carry {
        let final_val = add64(res.x, res.y, NEG_ORDER, 0u);
        return final_val;
    }
    return vec2<u32>(res.x, res.y);
}

fn gl_double(x_lo: u32, x_hi: u32) -> vec2<u32> {
    return gl_add(x_lo, x_hi, x_lo, x_hi);
}

fn gl_pow7(x_lo: u32, x_hi: u32) -> vec2<u32> {
    let x2 = gl_mul(x_lo, x_hi, x_lo, x_hi);
    let x3 = gl_mul(x2.x, x2.y, x_lo, x_hi);
    let x4 = gl_mul(x2.x, x2.y, x2.x, x2.y);
    let x7 = gl_mul(x3.x, x3.y, x4.x, x4.y);
    return x7;
}

// Field inversion: x^(p-2) via square-and-multiply.
// p-2 = 0xFFFFFFFEFFFFFFFF. Convention: 0^(-1) = 0.
fn gl_inv(x_lo: u32, x_hi: u32) -> vec2<u32> {
    // Check for zero: 0^(-1) = 0
    if x_lo == 0u && x_hi == 0u {
        return vec2<u32>(0u, 0u);
    }

    // exp = p - 2 = 0xFFFFFFFEFFFFFFFF
    // exp_lo = 0xFFFFFFFF, exp_hi = 0xFFFFFFFE
    var result = vec2<u32>(1u, 0u); // 1 in Goldilocks
    var base = vec2<u32>(x_lo, x_hi);

    // Process low 32 bits of exponent (0xFFFFFFFF = all ones)
    for (var i = 0u; i < 32u; i++) {
        // All bits are 1 in the low word
        result = gl_mul(result.x, result.y, base.x, base.y);
        base = gl_mul(base.x, base.y, base.x, base.y);
    }

    // Process high 32 bits of exponent (0xFFFFFFFE = all ones except bit 0)
    for (var i = 0u; i < 32u; i++) {
        let bit = (0xFFFFFFFEu >> i) & 1u;
        if bit == 1u {
            result = gl_mul(result.x, result.y, base.x, base.y);
        }
        if i < 31u {
            base = gl_mul(base.x, base.y, base.x, base.y);
        }
    }

    return result;
}
