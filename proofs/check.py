#!/usr/bin/env python3
"""Check reviewed source fingerprints, then run the strict Eidos checker."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def main():
    proofs = Path(__file__).resolve().parent
    root = proofs.parent
    sources = json.loads((proofs / "sources.json").read_text())
    for name, expected in sources.items():
        actual = hashlib.sha256((root / name).read_bytes()).hexdigest()
        if actual != expected:
            sys.exit(
                f"Source changed: {name}. Review the Eidos model against the "
                "new implementation before updating proofs/sources.json."
            )
        print(f"SOURCE {name}: sha256 {actual}", flush=True)
    subprocess.run(
        ["cargo", "run", "--manifest-path", str(proofs / "checker/Cargo.toml")],
        check=True,
    )


if __name__ == "__main__":
    main()
