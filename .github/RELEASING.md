# Releasing Clay Form

Releases are built and published by `.github/workflows/release.yml`. The workflow creates an annotated version tag, a GitHub Release, a Linux x86-64 archive and a SHA-256 checksum file.

## Create a release

1. Update the `version` in `Cargo.toml` and commit the resulting `Cargo.lock` change.
2. Merge that version change into `main` and wait for CI to pass.
3. Open **Actions → Release → Run workflow**.
4. Select `main` and enter the exact Cargo version, such as `0.1.0`.

The workflow validates the requested version against `Cargo.toml`, runs the tests, builds the release binary, creates an annotated tag such as `v0.1.0`, and publishes the assets.

The same workflow also accepts a directly pushed `v*` tag. The tag version must match `Cargo.toml`. If a tag already exists, a rerun is accepted only when it points to the commit being released.

## Release assets

- `clay-form-vX.Y.Z-linux-x86_64.tar.gz` contains the executable and README.
- `SHA256SUMS` contains the archive checksum.

Generated release notes are based on the commits and pull requests since the previous GitHub Release.
