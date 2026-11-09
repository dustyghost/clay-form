#!/usr/bin/env python3
"""Stage unchanged production code with only retopology test cases enabled.

Cargo's test-name filter still compiles the entire app test harness. On the
8 GB development laptop that exceeds the compiler's memory cap. This creates
a disposable source copy, adding cfg(any()) only to unrelated #[test] items.
Test helpers and all production modules remain present. CI uses the ordinary
unmodified harness. Re-run this staging command after editing source files.
"""
import argparse
import re
import shutil
from pathlib import Path

root = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("destination", type=Path)
args = parser.parse_args()
destination = args.destination.resolve()
if destination == root or root.is_relative_to(destination):
    raise SystemExit("Choose a disposable destination outside the source tree's ancestors")
destination.mkdir(parents=True, exist_ok=True)
marker = destination / ".focused-retopology-tests"
if any(destination.iterdir()) and not marker.exists():
    raise SystemExit("Destination must be empty or previously staged by this script")
marker.touch()
shutil.copytree(root / "src", destination / "src", dirs_exist_ok=True)
for name in ["Cargo.toml", "Cargo.lock", "build.rs"]:
    shutil.copy2(root / name, destination / name)
for name in ["crates", "legal", "tests", "docs", "scripts", "LICENSE", "README.md", "COPYRIGHT.md"]:
    path = destination / name
    if not path.exists():
        path.symlink_to(root / name, target_is_directory=(root / name).is_dir())
enabled = []
disabled = 0
related = {
    "groups_and_mask_intensity_are_independent_undoable_and_survive_levels",
    "group_all_replaces_groups_on_every_level_and_undo_restores_them",
    "face_replacement_at_group_limit_reclaims_an_old_slot_atomically",
}
pattern = re.compile(r"(?m)^(\s*)#\[test\](.*?\bfn\s+(\w+)\s*\()", re.S)
for path in (destination / "src").rglob("*.rs"):
    def select(match):
        global disabled
        name = match[3]
        if "retopology" in name or name.startswith("density_paint_") or name in related:
            enabled.append(f"{path.relative_to(destination)}::{name}")
            return match[0]
        disabled += 1
        return f"{match[1]}#[cfg(any())]\n{match[1]}#[test]{match[2]}"
    path.write_text(pattern.sub(select, path.read_text()))
print(f"Staged {len(enabled)} retopology cases; disabled {disabled} unrelated test cases.")
print("\n".join(enabled))
