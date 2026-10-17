"""Independent Python model of the hemera 0.3 permutation and sponge.

Reads the constants from rs/src; `python3 hemera_ref.py` checks the model
against vectors/hemera.json and re-derives the 144 constants by the
self-bootstrap (specs/bootstrap.md).
"""
import sys, os, re, json

_RS = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', 'rs', 'src')


def _hexes(path, start):
    src = open(os.path.join(_RS, path)).read()
    body = src[src.index(start):]
    body = body[:body.index('];')]
    return [int(x, 16) for x in re.findall(r'0x([0-9A-Fa-f]+)', body)]


RC = _hexes('constants.rs', 'pub const ROUND_CONSTANTS_U64')   # 128 external + 16 internal
DIAG = _hexes('field.rs', 'pub const MATRIX_DIAG_16')
assert len(RC) == 144 and len(DIAG) == 16

P = 0xFFFFFFFF00000001
RF_HALF, RP = 4, 16


def inv(x):
    return pow(x, P - 2, P) if x else 0


def m4(a, b, c, d):
    return ((2*a + 3*b + c + d) % P, (a + 2*b + 3*c + d) % P,
            (a + b + 2*c + 3*d) % P, (3*a + b + c + 2*d) % P)


def m_e(s):
    out = []
    for j in range(4):
        out.extend(m4(*s[4*j:4*j+4]))
    sums = [sum(out[j*4 + k] for j in range(4)) % P for k in range(4)]
    return [(out[i] + sums[i % 4]) % P for i in range(16)]


def m_i(s):
    t = sum(s) % P
    return [(DIAG[i] * s[i] + t) % P for i in range(16)]


def permute(s, rc=RC, rf_half=RF_HALF, rp=RP, partial='inv'):
    ext, intl = rc[:128], rc[128:]
    s = m_e(list(s))
    r = 0
    for _ in range(rf_half):
        s = [pow((s[i] + ext[r*16 + i]) % P, 7, P) for i in range(16)]
        s = m_e(s); r += 1
    for j in range(rp):
        x = (s[0] + intl[j]) % P
        s[0] = inv(x) if partial == 'inv' else pow(x, 7, P)
        s = m_i(s)
    r = 4  # terminal rounds always use RC rows 4..7
    for k in range(rf_half):
        s = [pow((s[i] + ext[(r + k)*16 + i]) % P, 7, P) for i in range(16)]
        s = m_e(s)
    return s


def encode(block56):
    return [int.from_bytes(block56[7*i:7*i+7] + b'\0', 'little') for i in range(8)]


def hash_bytes(msg, domain=0, rc=RC):
    s = [0]*16
    s[11] = domain
    n = len(msg)
    full = n // 56
    # eager absorption of full blocks
    for b in range(full):
        blk = encode(msg[56*b:56*b+56])
        s = [(s[i] + blk[i]) % P if i < 8 else s[i] for i in range(16)]
        s = permute(s, rc)
    tail = msg[56*full:] + b'\x01'
    tail = tail + b'\0'*(56 - len(tail))
    blk = encode(tail)
    s = [(s[i] + blk[i]) % P if i < 8 else s[i] for i in range(16)]
    s[10] = n
    s = permute(s, rc)
    return b''.join(x.to_bytes(8, 'little') for x in s[:4])


def bootstrap():
    zero = [0]*144
    seed = b'cyber'
    pad = seed + b'\x01' + b'\0'*(56 - len(seed) - 1)
    s = [0]*16
    blk = encode(pad)
    for i in range(8):
        s[i] = (s[i] + blk[i]) % P
    s[10] = len(seed)
    s = permute(s, zero)
    out, pos = [], 0
    for i in range(144):
        if pos >= 8:
            s = permute(s, zero); pos = 0
        out.append(s[pos]); pos += 1
    return out


if __name__ == '__main__':
    vec = json.load(open(os.path.join(_RS, '..', '..', 'vectors', 'hemera.json')))['hash']
    for name, msg in [('empty', b''), ('hello', b'hello'), ('hemera', b'hemera')]:
        got = hash_bytes(msg).hex()
        print(name, got == vec[name], got)
    print('bootstrap constants match rs/src/constants.rs:', bootstrap() == RC)
