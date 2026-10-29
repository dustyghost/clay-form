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
upstream = json.loads((root / "legal/upstream/index.json").read_text())
sources = list(upstream.values())
sources.append(json.loads((root / "legal/additional-notices.json").read_text()))
for entries in sources:
    for entry in entries:
        data = (root / "legal/upstream" / entry["file"]).read_bytes()
        if hashlib.sha256(data).hexdigest() != entry["sha256"]:
            raise SystemExit(f"Upstream notice changed: {entry['file']}")
if not records or any(not all(r.get(k) for k in ("name", "repository", "license", "text")) for r in records):
    raise SystemExit("Incomplete third-party notices")
print(f"Verified {len(records)} third-party notices and their input hashes")
pending = json.loads((root / "legal/pending-review.json").read_text())
decisions = json.loads((root / "legal/review-decisions.json").read_text())
supplements = json.loads((root / "legal/notice-supplements.json").read_text())
if not set(pending).issubset(decisions):
    raise SystemExit("Every pending notice needs a recorded review decision")
for key, decision in decisions.items():
    if decision["status"] not in ("pending", "resolved", "documented"):
        raise SystemExit(f"Unknown notice review status: {key}")
    if (key in pending) != (decision["status"] == "pending"):
        raise SystemExit(f"Notice review decision disagrees with pending inventory: {key}")
documented = {key for key, decision in decisions.items() if decision["status"] == "documented"}
if documented != set(supplements):
    raise SystemExit("Documented notice decisions require matching attribution supplements")
for key in documented:
    if not upstream.get(key) or not supplements[key].get("notice_context") or not decisions[key].get("release_basis"):
        raise SystemExit(f"Documented attribution is incomplete: {key}")
if documented:
    print(f"Documented attribution decisions: {len(documented)} (non-blocking; see legal/review-decisions.json)")
if pending:
    print(f"NOT CLEARED FOR DISTRIBUTION: {len(pending)} notice records need review")
reviews = json.loads((root / "legal/release-review.json").read_text())
for name, review in reviews.items():
    if review["status"] not in ("pending", "blocked", "resolved", "documented"):
        raise SystemExit(f"Unknown release review status: {name}")
    if review["status"] == "documented":
        if not review.get("reason") or not review.get("release_basis"):
            raise SystemExit(f"Documented release decision is incomplete: {name}")
        print(f"DOCUMENTED REVIEW: {name} (non-blocking; see legal/release-review.json)")
unresolved = [name for name, review in reviews.items() if review["status"] in ("pending", "blocked")]
if unresolved:
    print("RELEASE REVIEW PENDING: " + ", ".join(unresolved))
if "--release" in sys.argv and (pending or unresolved):
    raise SystemExit("Resolve the pending notices and release review before building a distributable release")
if "--release" in sys.argv:
    print("Release notice checks passed; documented uncertainties are retained")
