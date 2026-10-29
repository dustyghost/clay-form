# Locked dependency notice verification

Reviewed 28 October 2026 for Alpha 1. This pass examined all 13 pending
records, the bundled font metadata, and the notice generation/release checks.
Four records are resolved by additional evidence. Nine historical attribution
questions are documented release decisions under the maintainer-approved
policy below. Release notice checks pass; this does not claim complete legal
clearance or erase the recorded uncertainties.

## Release policy

Use the published licence declarations, retain available upstream notices,
include full terms and source links, and record the basis for assembled
attribution. A fresh statement from every historical maintainer is not a
prerequisite imposed by the [MIT text](https://opensource.org/license/mit).
[Slint's published notices](https://docs.slint.dev/latest/docs/node/thirdparty/)
provide comparable practice for these same versions: package/author attribution
and licence terms. This precedent informs our approach; it is not an upstream
permission grant or proof that another project's notices are complete.

`documented` means the uncertainty is retained with an explicit distribution
decision and supporting attribution. It does not mean `resolved`. The original
assessment remains in each decision's `historical_uncertainty` field. Known
conflicting terms, missing required material, unknown licence fallbacks and
unreviewed changes still require action and can block release. The nine cases
below are not treated as established violations merely because their historical
notice coverage lacks an explicit confirmation.

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

## Nine documented attribution decisions

| Dependency | Retained attribution and uncertainty |
| --- | --- |
| `block 0.1.6` | Preserve the published MIT declaration, separately labelled package-author credit and full standard MIT terms. No copyright year is invented. The archive omits a full notice; [PR #14](https://github.com/SSheldon/rust-block/pull/14) is unmerged and its borrowed attribution is not used. [Issue #13](https://github.com/SSheldon/rust-block/issues/13) remains a source of possible future clarification. |
| `dispatch 0.2.0` | Preserve the declared MIT licence, separately labelled package-author credit and full MIT terms. The exact tree omits a full notice; the closed [issue #18](https://github.com/SSheldon/rust-dispatch/issues/18) does not provide historical confirmation. The uncertainty remains recorded. |
| `malloc_buf 0.0.6` | Include the maintainer-accepted MIT notice from [PR #3](https://github.com/SSheldon/malloc_buf/pull/3) directly with this package's declared licence and author credit. Preserve its 2020 date without backdating. The [comparison](https://github.com/SSheldon/malloc_buf/compare/a7811e5f4c6f9685dd57ad9bfe34e9bf6b0ba4f9...d9a3e539642bd90e07df458d226b19cdfa606863) includes a rewrite; no version-specific confirmation is claimed. |
| `objc2 0.6.4`, `objc2-encode 4.1.0`, and `objc2-{app-kit,foundation,core-foundation,core-graphics} 0.3.2` | Select the published MIT option and retain both Steven Sheldon's historical notice and Mads Marquart's later project notice directly with each entry. [Issue #826](https://github.com/madsmtm/objc2/issues/826) confirms old licence declarations but does not confirm exhaustive copyright coverage. [Source history](https://github.com/madsmtm/objc2/commit/cfb199226661ec6e4939fcd7cd7531b3c5453076) explains the older file's removal. Preserve these facts rather than claiming the new notice replaced every earlier notice. |

These nine packages enter our native builds through macOS dependencies:
`wgpu-hal`/`metal` bring `block` and `malloc_buf`; the older windowing stack
brings `dispatch`; clipboard/framework wrappers bring the newer `objc2` crates.
The current source archive vendors dependencies for every platform, so these
records and their retained attribution also accompany that shared source asset.

Existing upstream questions were read; no new issues, comments or messages
were sent. New authoritative notices or dependency updates should improve the
inventory as they become available. Package-author credit is labelled as such,
not presented as an invented copyright statement. Full permission, condition
and disclaimer paragraphs are retained even where a generic template's
placeholder copyright line is omitted.

The [macOS SDK-derived binding review](macos-licensing.md) remains separate.
It is recorded as a non-blocking distribution decision in
`legal/release-review.json`. The uncertainty is not legally resolved by that
decision. Native builds use normal Apple tools and packages must not contain
Apple SDKs or system frameworks. Specific conflicting terms or unauthorized
material identified later would require a blocking review.

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
attribution fails verification, documented entries retain all collected notice
text, and new pending/blocked reviews still stop release. Unknown statuses and
documented decisions lacking attribution fail. Regeneration is deterministic
against the same report and locked dependencies. The nine supplement entries
are tied to exact manifest hashes so dependency changes require a fresh review.

Reproduce after installing cargo-about 0.9.3:

```sh
cargo about generate --locked --fail --format json -o /tmp/clay-licenses.json
python3 scripts/generate-notices.py /tmp/clay-licenses.json
python3 scripts/check-notices.py
python3 scripts/test-notices.py
python3 scripts/check-notices.py --release
```

The final command passes for this reviewed inventory and prints the documented
decisions. `legal/pending-review.json` is reserved for new unresolved blockers;
it is currently empty. Passing these checks establishes the configured notice
policy, not a general legal opinion or completion of hardware testing.
