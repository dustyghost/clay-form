# Third-party notices

`third-party.json` is embedded in the About window; the same notices are
available offline in `THIRD-PARTY-NOTICES.html`. The inventory covers the union
of six native build targets and includes build dependencies. It deliberately
includes platform-specific crates that may not be linked into every binary.

Regenerate after changing Cargo dependencies, features or `about.toml`:

```sh
cargo install cargo-about --version 0.9.3 --locked --features cli
cargo about generate --locked --fail --format json -o /tmp/clay-licenses.json
python3 scripts/generate-notices.py /tmp/clay-licenses.json
python3 scripts/check-notices.py
```

Review warnings and the resulting diff. Generic fallback licence templates can
omit copyright notices: resolve missing upstream texts before distribution.
Unresolved records are explicit in `pending-review.json`; draft notices mark
them as incomplete. `check-notices.py --release` rejects releases while any
remain. `additional-notices.json` preserves upstream licensing explanations
and later notices without asserting that they settle historical attribution.
Keep crate-specific clarifications tied to upstream commits/checksums. The
generator also preserves the actual four font notices from the locked
`epaint_default_fonts` package; Hack includes Bitstream Vera/DejaVu terms that
must not be reduced to the package's umbrella licence expression.

Font licences apply to the bundled font data, not to Clay Form's original code
or the user's artwork. Do not rename or modify fonts without checking their
specific terms. Blender and GIMP are separately installed optional applications,
not bundled binaries. Rust standard-library notices are copied from the pinned
toolchain into each binary archive under `legal/rust/`.

`inputs.json` records the reviewed input hashes so CI rejects stale notices.
An automated inventory is evidence for review, not proof that every embedded
asset, copied snippet, patent or trademark issue has been cleared.
