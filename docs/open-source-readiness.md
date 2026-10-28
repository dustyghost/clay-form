# Open-source readiness

Preparation started 28 October 2026; tracked in
[#76](https://github.com/dustyghost/clay-form/issues/76). This document records
completed preparation and remaining publication requirements. It is not a
claim that Clay Form is already cleared or tested on every platform.

## Licensing and ownership

The chosen first public testing version is **0.1.0-alpha.1**, displayed as
**Alpha 1**. This is a prerelease designation, not a claim of production readiness.
A startup notice explains development status before the workspace opens. Its
optional dismissal is stored per version; pending recovery prompts remain
available after continuing.

The owner confirmed all original code/artwork is theirs to release and that
no borrowed proprietary assets were included. Original repository content uses
GPL-3.0-only, with complete terms in LICENSE and package metadata. Official
downloads remain free of charge. GPLv3 permits charging for redistributed
copies; distributed modifications must satisfy its source/licensing terms.

COPYRIGHT.md, README and About explicitly state that models, textures, renders
and other creations belong to their creators. Merely using Clay Form does not
put output under GPL, require credit, or require publishing it. Imported
third-party material retains its own terms.

The full Cargo metadata inventory contained 344 external packages. Filtering
the build graph to six native targets and excluding development-only packages
left 251 external packages. Licence records can exceed the package count:
multiple notices and font-specific terms must also be retained. Notices are
embedded in About and provided in THIRD-PARTY-NOTICES.html. Exact upstream
notice files, origins and hashes are retained in `legal/upstream/`.

The [follow-up verification](research/license-verification.md) resolved four
of the original 13 pending records: fax, fax_derive, hexf-parse and zune-inflate.
Nine remain. It also added copyright/trademark text extracted from the exact
bundled fonts and checked the relevant registry archives against Cargo.lock.

**Outstanding:** `legal/pending-review.json` identifies dependencies whose
manifest declares a licence but whose complete release-specific notices remain
unresolved. Generic copyright placeholders are explicitly marked pending in
the draft inventory, never represented as completed attribution. The release
workflow blocks distributable builds until these entries are resolved. The
[macOS review](research/macos-licensing.md) separately records SDK-binding and
historical-attribution questions. These require a targeted licensing assessment;
automated SPDX acceptance is insufficient evidence to close them.
The SDK-binding question has its own gate in `legal/release-review.json` so
clearing notice records alone cannot silently clear that separate review.

## Platforms

| Target | Build/package configuration | Runtime validation still required |
| --- | --- | --- |
| Windows x86-64 | Native MSVC build, portable ZIP | Launch, save/overwrite/recovery, DirectX 12 GPU, mouse/pen, optional tools |
| Windows ARM64 | Native ARM runner and MSVC target, portable ZIP | Same checks on actual ARM Windows; dependency/toolchain availability |
| macOS Intel | Native Mac runner, .app in ZIP, deployment target 11 | OS/GPU compatibility, input, save/recovery, Gatekeeper behaviour |
| macOS Apple Silicon | Native ARM Mac runner, .app in ZIP, deployment target 11 | Same checks on an Apple Silicon Mac |
| Linux x86-64 | Ubuntu 22.04 baseline, portable tarball | Arch, Ubuntu, Debian, Fedora, openSUSE; Wayland/X11, hardware Vulkan |
| Linux ARM64 | Ubuntu 22.04 ARM baseline, portable tarball | ARM64 distro and GPU driver availability; pen input |

Linux archives require glibc 2.35 or newer and the applicable X11/Wayland,
xkbcommon and Vulkan runtime libraries/driver. They are not musl/Alpine builds,
32-bit ARM builds or a guarantee for old distro releases. The Arch source
recipe avoids the prebuilt glibc baseline when built locally. Native `.deb`,
`.rpm`, Flatpak/AppImage, AUR publication, winget and Homebrew distribution are
follow-up packaging choices; portable archives are the initial delivery format.

The inspected Windows x86-64 executable imports `VCRUNTIME140.dll`. A clean
Windows installation needs the [matching Microsoft Visual C++ v14 runtime](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)
if it is absent. The runtime is installed separately, not redistributed in our
ZIP. Verify ARM64's matching runtime during clean-machine testing.

The Linux-specific tablet reader is guarded by target configuration. Native
pressure events are accepted where available on other platforms, with full
pressure as fallback; physical pen support must not be claimed based on a
successful compile. Windows preferences use APPDATA with USERPROFILE fallback.
macOS retains the existing HOME/.config location to avoid an unplanned migration.

## Repository/IP inspection

- Gitleaks 8.30.1 scanned all 95 currently reachable commits with redacted
  reporting and found no matching secrets. Repeat on the final public revision.
- Reviewed tracked paths and historical names. No tracked vendored SDK/library
  tree, font binaries, credentials or proprietary brush-pack archives were found.
- Visually reviewed contact sheets of all 157 tracked PNGs. They show app test
  scenes, procedural alphas, simple geometry/reference studies and material
  comparisons. This thumbnail review cannot establish provenance by itself.
- docs/images/README.md documents procedural front/side reference renders and
  screenshot capture tests. The owner confirmed original artwork rights.
- Source searches found factual format/interoperability references and research
  citations, rather than a bundled proprietary sculpting SDK. Preserve source
  citations and required attribution. Product names alone are not proof of an
  infringement; do not imply endorsement or use third-party logos as branding.
- Local untracked sculpt sessions and diagnostic bundles were excluded from
  this worktree and must not be added to a public release accidentally.

This is not a forensic proof that every line is independently authored, a
patent search, or trademark clearance. Review the public project name/logo,
commit author details, screenshots and any future contributed assets before
publication. The source archive includes platform-specific Cargo dependencies,
so their notice review matters even when distributing a different platform's binary.

## Remaining launch work

1. Resolve pending notices and the macOS binding assessment with evidence.
2. Require final CI and native package jobs to pass; inspect the generated
   archives and rebuild their corresponding source.
3. Record real-platform smoke tests, especially file replacement/recovery,
   rendering, pen pressure, non-ASCII paths and external editor discovery.
4. Decide Windows signing and Apple Developer ID/notarization. Current Windows
   archives are unsigned and macOS bundles are only ad-hoc signed. Do not ask
   users to disable OS security globally.
5. Enable private vulnerability reporting and repository security controls;
   add dependency-advisory scanning and maintain a response process.
6. Set release version/support expectations and publish contributor guidance,
   issue templates and a changelog. Maintain an asset provenance ledger for new
   external material; contributions keep their authors' copyrights.
7. Review the final public history and assets, then change repository visibility
   and publish reviewed releases. Neither action is part of this preparation PR.

Recommended follow-ups: Flatpak for broad Linux desktop delivery; an AUR
package after the tagged source recipe is verified; signed downloads and
build-provenance attestations; reproducible build improvements; an accessible
bug-report template with warnings about private diagnostic artwork.
