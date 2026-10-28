#!/usr/bin/env python3
"""Package a prebuilt native binary, documentation and offline licence notices."""
import argparse
import hashlib
import json
import plistlib
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    "x86_64-unknown-linux-gnu": "linux-x86_64",
    "aarch64-unknown-linux-gnu": "linux-aarch64",
    "x86_64-pc-windows-msvc": "windows-x86_64",
    "aarch64-pc-windows-msvc": "windows-aarch64",
    "x86_64-apple-darwin": "macos-x86_64",
    "aarch64-apple-darwin": "macos-aarch64",
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=TARGETS)
    parser.add_argument("--profile", choices=["debug", "release"], default="release")
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    name = f"clay-form-v{version}-{TARGETS[args.target]}"
    stage = ROOT / "dist" / name
    stage.mkdir(parents=True, exist_ok=False)
    windows = "windows" in args.target
    mac = "apple" in args.target
    binary = "clay-form.exe" if windows else "clay-form"
    destination = stage
    if mac:
        contents = stage / "Clay Form.app" / "Contents"
        destination = contents / "MacOS"
        destination.mkdir(parents=True)
        (contents / "Info.plist").write_bytes(plistlib.dumps({
            "CFBundleExecutable": binary, "CFBundleIdentifier": "io.github.dustyghost.clay-form",
            "CFBundleName": "Clay Form", "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version, "CFBundleVersion": version,
            "NSHighResolutionCapable": True,
        }))
    shutil.copy2(ROOT / "target" / args.target / args.profile / binary, destination / binary)
    for filename in ["LICENSE", "COPYRIGHT.md", "README.md", "THIRD-PARTY-NOTICES.html"]:
        shutil.copy2(ROOT / filename, stage / filename)
    shutil.copytree(ROOT / "docs", stage / "docs")
    shutil.copytree(ROOT / "legal", stage / "legal")
    if "linux" in args.target:
        shutil.copytree(ROOT / "packaging/linux", stage / "packaging/linux")
    # Rust's linked runtime has notices beyond the Cargo dependency graph.
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    runtime = stage / "legal/rust"
    runtime.mkdir()
    for filename in ["COPYRIGHT-library.html"]:
        source = sysroot / "share/doc/rust" / filename
        if not source.is_file():
            raise SystemExit(f"Missing Rust runtime notice: {source}")
        shutil.copy2(source, runtime / filename)
    shutil.copytree(sysroot / "share/doc/rust/licenses", runtime / "licenses")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    (stage / "BUILD-INFO.json").write_text(json.dumps({
        "version": version, "commit": revision, "target": args.target, "profile": args.profile,
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "source": f"https://github.com/dustyghost/clay-form/releases/tag/v{version}",
        "source_asset": f"clay-form-v{version}-source.tar.gz",
    }, indent=2) + "\n")
    if mac:
        # Ad-hoc identity supports local execution; this is not notarization.
        subprocess.run(["codesign", "--force", "--sign", "-", str(stage / "Clay Form.app")], check=True)
    if windows or mac:
        archive = stage.parent / (stage.name + ".zip")
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
            for file in sorted(stage.rglob("*")):
                if file.is_file():
                    output.write(file, file.relative_to(stage.parent))
    else:
        archive = stage.parent / (stage.name + ".tar.gz")
        with tarfile.open(archive, "w:gz") as output:
            output.add(stage, arcname=stage.name)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    (archive.parent / (archive.name + ".sha256")).write_text(f"{checksum}  {archive.name}\n")
    print(archive)


if __name__ == "__main__":
    main()
