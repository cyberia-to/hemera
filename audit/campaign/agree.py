"""Rust crate against the Python model on random and edge inputs.

Generates permutation states (uniform canonical limbs, plus zeros, ones,
p - 1, single-limb units, and non-canonical limbs in [p, 2^64)), messages
of random length across the 56-byte rate boundaries, and tree-node pairs
(including the all-0xff non-canonical digest), sends them to
`rs/examples/agree.rs` and compares every answer with `hemera_ref`.

usage: python3 agree.py [n_perm] [n_hash] [n_node]   (defaults 2000 1000 500)
"""
import os, random, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import hemera_ref as h

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
n_perm, n_hash, n_node = (int(x) for x in (sys.argv[1:] + ['2000', '1000', '500'])[:3])
rng = random.Random(20261016)
P = h.P

states = [[0] * 16, [1] * 16, [P - 1] * 16, [2**64 - 1] * 16, [P] * 16, list(range(16))]
states += [[int(i == j) for j in range(16)] for i in range(16)]
while len(states) < n_perm:
    k = rng.randrange(8)
    if k == 0:
        states.append([rng.choice([0, 1, P - 1, P, P + 1, 2**64 - 1, 2**32, 2**32 - 1]) for _ in range(16)])
    elif k == 1:
        states.append([rng.randrange(P, 2**64) for _ in range(16)])
    else:
        states.append([rng.randrange(P) for _ in range(16)])
msgs = [b'', bytes(55), bytes(56), bytes(57), bytes([0xff]) * 112]
while len(msgs) < n_hash:
    msgs.append(bytes(rng.randrange(256) for _ in range(rng.randrange(300))))
nodes = [(b'\0' * 32, b'\0' * 32, False), (b'\xff' * 32, b'\xff' * 32, True)]
while len(nodes) < n_node:
    nodes.append((bytes(rng.randrange(256) for _ in range(32)), bytes(rng.randrange(256) for _ in range(32)),
                  rng.random() < 0.2))

req = [('p ' + ' '.join('%016x' % x for x in s)) for s in states]
req += [('h ' + (m.hex() or '-')) for m in msgs]
req += [('n %s %s %d' % (l.hex(), r.hex(), root)) for l, r, root in nodes]
env = dict(os.environ, CARGO_TARGET_DIR=os.environ.get('CARGO_TARGET_DIR', os.path.join(ROOT, 'target')))
out = subprocess.run(['cargo', 'run', '-q', '--release', '-p', 'cyber-hemera', '--example', 'agree'],
                     input='\n'.join(req) + '\n', capture_output=True, text=True, cwd=ROOT, env=env, check=True)
ans = out.stdout.splitlines()
assert len(ans) == len(req), (len(ans), len(req))
bad = 0
for i, s in enumerate(states):
    bad += ans[i] != ' '.join('%016x' % x for x in h.V04.permute(s))
o = len(states)
for i, m in enumerate(msgs):
    bad += ans[o + i] != h.hash_bytes(h.V04, m).hex()
o += len(msgs)
for i, (l, r, root) in enumerate(nodes):
    bad += ans[o + i] != h.hash_node(h.V04, l, r, root).hex()
print(f'agreement Rust crate vs Python model: {len(states)} permutations, {len(msgs)} hashes, '
      f'{len(nodes)} nodes — {"all equal" if bad == 0 else f"{bad} MISMATCHES"}')
sys.exit(1 if bad else 0)
