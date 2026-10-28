#!/usr/bin/env python3
"""Fail if licence inputs change without regenerating reviewed notices."""
import hashlib
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parents[1]
for name, digest in json.loads((root / "legal/inputs.json").read_text()).items():
    if hashlib.sha256((root / name).read_bytes()).hexdigest() != digest:
        raise SystemExit(f"Regenerate and review third-party notices: {name} changed")
records = json.loads((root / "legal/third-party.json").read_text())
for entries in json.loads((root / "legal/upstream/index.json").read_text()).values():
    for entry in entries:
        data = (root / "legal/upstream" / entry["file"]).read_bytes()
        if hashlib.sha256(data).hexdigest() != entry["sha256"]:
            raise SystemExit(f"Upstream notice changed: {entry['file']}")
if not records or any(not all(r.get(k) for k in ("name", "repository", "license", "text")) for r in records):
    raise SystemExit("Incomplete third-party notices")
print(f"Verified {len(records)} third-party notices and their input hashes")
pending = json.loads((root / "legal/pending-review.json").read_text())
if pending:
    print(f"NOT CLEARED FOR DISTRIBUTION: {len(pending)} notice records need review")
    if "--release" in sys.argv:
        raise SystemExit("Resolve legal/pending-review.json before building a distributable release")
