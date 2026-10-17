# Releasing Clay Form

Use **Actions → Release → Run workflow** after the version change reaches `main` and CI passes.

## Create a release

1. Update `version` in `Cargo.toml`. Commit that change and the resulting `Cargo.lock` change.
2. Merge into `main` and wait for CI to pass.
3. Open **Actions → Release → Run workflow**.
4. Select `main` and enter the exact Cargo version, for example `0.1.0`.

[The release workflow](workflows/release.yml) checks the requested version against `Cargo.toml`, runs tests, builds the binary, creates an annotated tag such as `v0.1.0` and publishes a GitHub Release.

**Releasing from a tag:** the same workflow accepts directly pushed `v*` tags. The tag version must match `Cargo.toml`.

**Rerunning a release:** if the tag already exists, it must point to the commit being released.

## Release assets

| Asset | Contents |
| --- | --- |
| `clay-form-vX.Y.Z-linux-x86_64.tar.gz` | Linux x86-64 executable and README |
| `SHA256SUMS` | SHA-256 checksum for the archive |

GitHub generates release notes from commits and pull requests since the previous GitHub Release.
