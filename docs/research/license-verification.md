# Locked dependency notice verification

Reviewed 28 October 2026 for Alpha 1. This pass examined all 13 pending
records, the bundled font metadata, and the notice generation/release checks.
It resolves four notice records and leaves nine pending. It does not claim
complete legal clearance for distribution.

The cached registry archives for all 13 reviewed packages and
`epaint_default_fonts 0.33.3` matched their SHA-256 checksums in Cargo.lock.
Source revisions came from the published `.cargo_vcs_info.json` files, or
the recorded release tags for the older `block` and `malloc_buf` archives.
Individual decisions, archive checksums and evidence links are retained in
`legal/review-decisions.json`; exact notice bytes and hashes are in
`legal/upstream/`. Dependency versions and application behaviour were not changed.

## Resolved records

| Dependency | Evidence and action |
| --- | --- |
| `fax 0.2.6` and `fax_derive 0.2.0` | Upstream [PR #12](https://github.com/pdf-rs/fax/pull/12) merged full MIT text and the derive crate's licence symlink. The [comparison from the published revision](https://github.com/pdf-rs/fax/compare/40b226eb97b391355e971fd95a8dd532a4c127a0...ad46e7e76dfe74c171ef04f2ccb70fd84b2a117f) contains only those two added files. The notice therefore accompanies unchanged release code. Its copyright text is retained verbatim, including upstream's spelling. |
| `hexf-parse 0.2.1` | The [exact manifest](https://github.com/lifthrasiir/hexf/blob/4225763d744183d720f575ae96d04161b4d08ea0/parse/Cargo.toml) declares CC0-1.0. [CC0's legal terms](https://creativecommons.org/publicdomain/zero/1.0/legalcode.en) do not impose MIT's copyright-notice condition. The previous generic missing-copyright flag was inappropriate. Preserve the declaration and official CC0 text. |
| `zune-inflate 0.2.54` | Preserve the [original project copyright declaration](https://github.com/etemesi254/zune-image/blob/69502ce83fdfecdd0beefd677e2abb3781b29d98/LICENSE.md), the published source header, and the full MIT text accepted in [PR #206](https://github.com/etemesi254/zune-image/pull/206). The maintainer confirmed project-wide options in [#139](https://github.com/etemesi254/zune-image/issues/139) and accepted the missing-text correction in [#203](https://github.com/etemesi254/zune-image/issues/203). Select MIT, which the locked manifest declares. This uses the later notice to complete the existing grant; it does not assert unchanged code or common Git ancestry with the later tree. |

## Nine records still pending

| Dependency | Remaining evidence needed |
| --- | --- |
| `block 0.1.6` | The archive and release tree omit a full notice. [PR #14](https://github.com/SSheldon/rust-block/pull/14) remains unmerged and proposes text copied from another project. Its attribution cannot be assumed to apply. [Issue #13](https://github.com/SSheldon/rust-block/issues/13) already contains a historical-version request. |
| `dispatch 0.2.0` | The exact tree omits a full notice. [Issue #18](https://github.com/SSheldon/rust-dispatch/issues/18) is closed, but the historical-version question has no authoritative answer. Closed issue status alone does not establish the missing text. |
| `malloc_buf 0.0.6` | Preserve the notice accepted in [PR #3](https://github.com/SSheldon/malloc_buf/pull/3) as additional evidence. Unlike fax, the [comparison](https://github.com/SSheldon/malloc_buf/compare/a7811e5f4c6f9685dd57ad9bfe34e9bf6b0ba4f9...d9a3e539642bd90e07df458d226b19cdfa606863) includes a substantial rewrite and version 1.0.0. We have not established that this later notice completely covers 0.0.6. |
| `objc2 0.6.4`, `objc2-encode 4.1.0`, and `objc2-{app-kit,foundation,core-foundation,core-graphics} 0.3.2` | [Issue #826](https://github.com/madsmtm/objc2/issues/826) confirms the old declared licences, but the maintainer does not confirm completeness of the later copyright notice for other contributors. [Source history](https://github.com/madsmtm/objc2/commit/cfb199226661ec6e4939fcd7cd7531b3c5453076) shows removal of Steven Sheldon's earlier MIT file during licence reorganisation. Preserve that historical notice and the later Mads Marquart notice; complete the historical-attribution assessment before clearing these six selected MIT records. |

These nine packages enter our native builds through macOS dependencies:
`wgpu-hal`/`metal` bring `block` and `malloc_buf`; the older windowing stack
brings `dispatch`; clipboard/framework wrappers bring the newer `objc2` crates.
The current source archive vendors dependencies for every platform, so these
records still affect that shared source asset. A Windows/Linux-only release
would need its own explicitly scoped source and notice review.

Existing upstream questions were read; no new issues, comments or messages
were sent. The remaining routes are authoritative historical-notice evidence,
a targeted licensing assessment of the collected notices, or a separately
tested dependency change to versions with complete notices. Do not substitute
package-author metadata for an invented copyright statement.

The [macOS SDK-derived binding review](macos-licensing.md) remains separate.
It is now recorded in `legal/release-review.json` and remains a release gate
even if every notice record is resolved.

## Font notices and verification checks

Read OpenType name-table records 0 (copyright), 7 (trademark), 13 (licence)
and 14 (licence URL) directly from the four locked font binaries. Preserve
their exact text and binary hashes in `legal/font-metadata.json`; add it to
About and the HTML notices alongside the existing full font licences.
This includes Canonical's 2011 copyright/trademark text and Google's 2013
copyright/Noto trademark text, which were absent from our displayed notices.
Hack's embedded attribution is retained as well. The emoji icon font has no
matching name-table entries; its supplied MIT file remains included.

Font data retains its own licence and is not relabelled GPL. The
[OFL FAQ](https://openfontlicense.org/ofl-faq/) addresses bundling with software;
the bundled Ubuntu Font Licence expressly allows embedding/bundling while
retaining its terms. Font files and names were not modified in this work.

The verifier now checks both generated notice outputs and additional upstream
files, as well as generator inputs. Focused tests demonstrate that stripped
attribution fails verification and a pending platform review still blocks
release after notice resolution. Regeneration is deterministic against the
same report and locked dependencies.

Reproduce after installing cargo-about 0.9.3:

```sh
cargo about generate --locked --fail --format json -o /tmp/clay-licenses.json
python3 scripts/generate-notices.py /tmp/clay-licenses.json
python3 scripts/check-notices.py
python3 scripts/test-notices.py
python3 scripts/check-notices.py --release
```

The final command is expected to fail while the nine notice records and
SDK-binding review remain unresolved. Passing ordinary verification means
the reviewed inventory is intact, not that publication has been cleared.
