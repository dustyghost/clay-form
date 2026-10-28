#!/usr/bin/env python3
"""Generate portable offline notices from cargo-about's reviewed JSON output.

Run cargo about generate --locked --fail --format json -o /tmp/clay-licenses.json
then python3 scripts/generate-notices.py /tmp/clay-licenses.json.
No absolute Cargo cache paths are copied into the output.
"""
import argparse
import hashlib
import html
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    args = parser.parse_args()
    report = json.loads(args.report.read_text())
    upstream = ROOT / "legal/upstream"
    overrides = json.loads((upstream / "index.json").read_text())
    pending = json.loads((ROOT / "legal/pending-review.json").read_text())
    records = []
    for license in report["licenses"]:
        for user in license["used_by"]:
            package = user["crate"]
            if package["name"] == "clay-form":
                continue
            repository = package.get("repository")
            if repository and repository.startswith("http://github.com/"):
                repository = repository.replace("http://", "https://", 1)
            if package["name"] == "zune-inflate" and not repository:
                repository = "https://github.com/etemesi254/zune-image"
            if not repository or not repository.startswith("https://"):
                raise SystemExit(f"Review missing HTTPS source repository: {package['name']}")
            text = license["text"]
            key = f"{package['name']}@{package['version']}|{license['id']}"
            if key in overrides:
                texts = []
                for entry in overrides[key]:
                    data = (upstream / entry["file"]).read_bytes()
                    if hashlib.sha256(data).hexdigest() != entry["sha256"]:
                        raise SystemExit(f"Upstream notice hash mismatch: {key}")
                    texts.append(data.decode())
                text = "\n\n".join(texts)
            elif not license["source_path"]:
                if key not in pending:
                    raise SystemExit(f"Resolve or explicitly record canonical licence fallback: {key}")
                declaration = Path(package["manifest_path"]).read_bytes()
                if hashlib.sha256(declaration).hexdigest() != pending[key]["declaration_sha256"]:
                    raise SystemExit(f"Pending upstream declaration changed: {key}")
                # A draft audit must not invent copyright holders or quietly
                # turn a generic template into a completed attribution review.
                text = ("RELEASE REVIEW PENDING: " + pending[key]["reason"] + "\n\n"
                        + "Authors declared in package metadata: " + ", ".join(pending[key]["authors"])
                        + "\n\nStandard licence terms (not a complete copyright notice):\n\n" + text)
            records.append(dict(name=package["name"], version=package["version"],
                                repository=repository, license=license["id"], text=text))

    # Font notices contain additional copyright and naming conditions beyond
    # the umbrella crate licence. Preserve these verbatim, including Hack's
    # Bitstream Vera and DejaVu notices.
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=ROOT))
    fonts = next(p for p in metadata["packages"] if p["name"] == "epaint_default_fonts")
    directory = Path(fonts["manifest_path"]).parent / "fonts"
    font_metadata = json.loads((ROOT / "legal/font-metadata.json").read_text())
    for name, repository, license, filename, font_file in [
        ("Hack font", "https://github.com/source-foundry/Hack", "MIT AND Bitstream-Vera", "Hack-Regular.txt", "Hack-Regular.ttf"),
        ("Noto Emoji font", "https://github.com/googlefonts/noto-emoji", "OFL-1.1", "OFL.txt", "NotoEmoji-Regular.ttf"),
        ("Ubuntu font", "https://github.com/canonical/Ubuntu-fonts", "Ubuntu-font-1.0", "UFL.txt", "Ubuntu-Light.ttf"),
        ("emoji-icon-font", "https://github.com/jslegers/emoji-icon-font", "MIT", "emoji-icon-font-mit-license.txt", "emoji-icon-font.ttf"),
    ]:
        font = font_metadata[font_file]
        if hashlib.sha256((directory / font_file).read_bytes()).hexdigest() != font["sha256"]:
            raise SystemExit(f"Review changed font metadata: {font_file}")
        notices = [entry["text"] for entry in font["notices"]]
        notices.append((directory / filename).read_text())
        records.append(dict(name=name, version=f"bundled by epaint_default_fonts {fonts['version']}",
                            repository=repository, license=license,
                            text="\n\n".join(notices)))
    for entry in json.loads((ROOT / "legal/additional-notices.json").read_text()):
        data = (upstream / entry["file"]).read_bytes()
        if hashlib.sha256(data).hexdigest() != entry["sha256"]:
            raise SystemExit("Additional upstream notice hash mismatch")
        records.append(dict(name="Additional upstream notice", version="",
                            repository=entry["source"], license="See upstream declaration",
                            text=data.decode()))
    records.sort(key=lambda r: (r["name"].lower(), r["version"], r["license"], r["text"]))
    legal = ROOT / "legal"
    legal.mkdir(exist_ok=True)
    (legal / "third-party.json").write_text(json.dumps(records, ensure_ascii=False, indent=2) + "\n")
    parts = ['<!doctype html><html lang="en"><meta charset="utf-8">',
             '<meta name="viewport" content="width=device-width">',
             '<title>Clay Form third-party notices</title>',
             '<style>body{max-width:75ch;margin:2rem auto;padding:1rem;font:16px system-ui}pre{white-space:pre-wrap;overflow-wrap:anywhere}summary{cursor:pointer;margin:1rem 0}</style>',
             '<h1>Clay Form third-party notices</h1><p>Third-party components retain their own licences. Includes platform-specific and build dependencies and bundled fonts.</p>']
    for record in records:
        title = html.escape(f"{record['name']} {record['version']} — {record['license']}")
        parts.append(f'<details><summary>{title}</summary><a href="{html.escape(record["repository"], quote=True)}">Source repository</a><pre>{html.escape(record["text"])}</pre></details>')
    parts.append('</html>')
    (ROOT / "THIRD-PARTY-NOTICES.html").write_text("\n".join(parts) + "\n")
    hashes = {name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
              for name in ("Cargo.lock", "Cargo.toml", "about.toml", "scripts/generate-notices.py",
                           "legal/upstream/index.json", "legal/pending-review.json", "legal/additional-notices.json",
                           "legal/font-metadata.json", "legal/review-decisions.json", "legal/release-review.json",
                           "legal/third-party.json", "THIRD-PARTY-NOTICES.html")}
    (legal / "inputs.json").write_text(json.dumps(hashes, indent=2) + "\n")
    print(f"Wrote {len(records)} notices")


if __name__ == "__main__":
    main()
