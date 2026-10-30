# Releasing Clay Form

The **Native packages** workflow builds six native targets and creates a
**draft** release after every build and the source bundle succeed. It does not
publish automatically. PRs build debug archives as packaging checks; these
are test artifacts, not public releases.

## Before the first public release

Complete [the readiness review](../docs/open-source-readiness.md), including
the recorded attribution decisions and macOS dependency assessment. The release
workflow refuses to proceed while `legal/pending-review.json` is nonempty.
A pending or blocked item in `legal/release-review.json` also blocks release.
Items with status `documented` retain an explicit non-blocking distribution
decision; they are not represented as legally resolved. See the
[verification findings](../docs/research/license-verification.md) for evidence
and the historical uncertainties. The current reviewed notice inventory passes
`python3 scripts/check-notices.py --release`.
A successful notice-hash check alone is not legal clearance.

Run the regular PR CI and native packaging matrix on the final commit. Smoke
test real desktop/GPU/input combinations and record results. Review exact
artifact contents, source correspondence and licence notices. Configure private
security reporting, signing and notarization as appropriate. Opening the
repository to the public is a separate maintainer action.

## Create a draft release

The current testing version is **0.1.0-alpha.2 (Alpha 2)**. Continue with
`0.1.0-alpha.3`, then `0.1.0-beta.1` when ready for beta testing, and `0.1.0`
when that development cycle is ready for a stable release. Alpha 1 remains
available under its original tag. Hyphenated versions
are automatically marked as GitHub prereleases, in addition to being drafts.
Arch uses `pkgver=0.1.0alpha2` and a separate `_upstream_version` for its tag;
update both together. macOS uses the numeric `0.1.0` marketing version and CI
run number as its bundle build counter; About retains the full alpha version.

1. Update Cargo.toml, Cargo.lock and `packaging/arch/PKGBUILD` to a new version.
   Do not reuse a published tag. Regenerate/review notices after metadata changes.
2. Merge to `main` once CI passes. The preparation PR is stacked on ongoing
   feature work, which must also be accepted before a release from main.
3. Run **Actions → Native packages → Run workflow** on `main`, with the exact
   Cargo version. Alternatively push the matching `vX.Y.Z` tag.
4. Download and verify every draft asset. Replace the release-note checklist
   with changes, tested platforms and limitations. Publish only after review.

An existing release (including a draft) is never overwritten automatically.
An existing tag must point to the release commit.

Publishing a prepared draft or separately pushing its tag can trigger the tag
workflow. If that tag already has a draft or published release, the workflow
skips native builds and source packaging. Manually requesting an existing
release fails before builds start.

During Alpha 2 preparation, GitHub's release-by-tag endpoint returned 404 for
an existing draft. A 404 alone therefore does not allow a build: the preflight
then searches the authenticated release list, checking pages of 100 until it
finds the exact tag or exhausts the list. The `prepare` job uses `contents: write`
because [GitHub only lists drafts for callers with push access](https://docs.github.com/en/rest/releases/releases#list-releases).
Pull requests skip this API lookup. API permission, service, or malformed-response
errors stop the preflight, including a 404 from the release-list endpoint.

## Artifacts

| Asset suffix | Contents |
| --- | --- |
| `windows-x86_64.zip`, `windows-aarch64.zip` | Native executable, docs and licences; currently unsigned |
| `macos-x86_64.zip`, `macos-aarch64.zip` | App bundle, docs and licences; ad-hoc signature only, not notarized |
| `linux-x86_64.tar.gz`, `linux-aarch64.tar.gz` | Portable glibc executable, docs, licences and optional device rules |
| `source.tar.gz` | Exact tracked source plus locked vendored Cargo dependencies and portable Cargo configuration |
| `SHA256SUMS` | SHA-256 of every binary and source archive |

Each binary archive includes BUILD-INFO.json with source commit, Rust version,
architecture and build profile. GPL corresponding source is an explicit
release asset alongside the binaries; GitHub's generated source snapshot alone
does not include the vendored dependencies. Retain corresponding source for
as long as the binaries are distributed. Rebuild the source archive with the
pinned Rust toolchain, the OS SDK/build libraries, and `cargo build --release
--frozen` (the Rust toolchain itself must already be installed).

Arch's source recipe is in `packaging/arch/PKGBUILD`; publish it to AUR only
after the public tag exists and `makepkg` has been validated. ARM64 Arch means
an appropriate Arch Linux ARM environment, not an official x86-64 Arch image.
