// ── Params ────────────────────────────────────────────────────────
//
// Protocol constants matching rs/src/params.rs.

const P_LO: u32 = 0x00000001u;
const P_HI: u32 = 0xFFFFFFFFu;
const NEG_ORDER: u32 = 0xFFFFFFFFu;

const WIDTH: u32 = 16u;
const RATE: u32 = 8u;
const RATE_BYTES: u32 = 56u;
const BYTES_PER_ELEMENT: u32 = 7u;
const ROUNDS_F: u32 = 8u;
const ROUNDS_P: u32 = 16u;
const STATE_U32S: u32 = 32u;
const HASH_U32S: u32 = 8u;

const FLAG_ROOT: u32 = 1u;
const FLAG_PARENT: u32 = 2u;
const FLAG_CHUNK: u32 = 4u;

const CAP_COUNTER: u32 = 8u;
const CAP_FLAGS: u32 = 9u;
const CAP_LENGTH: u32 = 10u;
const CAP_DOMAIN: u32 = 11u;
const CAP_NS_MIN: u32 = 12u;
const CAP_NS_MAX: u32 = 13u;

const DOMAIN_HASH: u32 = 0u;
const DOMAIN_KEYED: u32 = 1u;
const DOMAIN_DERIVE_KEY_CONTEXT: u32 = 2u;
const DOMAIN_DERIVE_KEY_MATERIAL: u32 = 3u;

const OUTPUT_BYTES_CONST: u32 = 32u;

// ── Dispatch parameters ─────────────────────────────────────────

struct DispatchParams {
    count: u32,
    flags: u32,
    chunk_size: u32,
    total_bytes: u32,
    ns_min_lo: u32,
    ns_min_hi: u32,
    ns_max_lo: u32,
    ns_max_hi: u32,
}

// ── Bindings ────────────────────────────────────────────────────

@group(0) @binding(0)
var<storage, read_write> io_data: array<u32>;

@group(0) @binding(1)
var<storage, read> round_constants: array<u32>;

@group(0) @binding(2)
var<uniform> dp: DispatchParams;

@group(0) @binding(3)
var<storage, read> matrix_diag: array<u32>;

@group(0) @binding(4)
var<storage, read> aux_data: array<u32>;
