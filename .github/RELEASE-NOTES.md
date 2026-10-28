**Alpha 1 — 0.1.0-alpha.1.** An early testing release, with features and file
formats still evolving. Keep backups of important projects.

Clay Form is free and open source under GPLv3. Your models, textures, renders
and other creations remain yours to use, sell, license or keep private.

Download the archive for your operating system and CPU. `aarch64` means ARM64;
Apple Silicon Macs use `macos-aarch64`. Linux archives target glibc 2.35+
desktops with a working Vulkan driver. They are not Alpine/musl builds.

Windows requires the Microsoft Visual C++ v14 runtime for the selected
architecture. Use [Microsoft's supported redistributable installer](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)
if it is not already installed. The runtime is not bundled with Clay Form.

**Corresponding source:** the `source.tar.gz` asset contains the exact app
source, build scripts, lockfile and vendored Cargo dependencies for these
binaries. See its README and pinned Rust toolchain to rebuild. The compiler
and operating-system SDKs are installed separately. Licence notices are
included in each binary archive and in the app's About window. `SHA256SUMS`
contains archive hashes.

Release reviewer: replace this paragraph with the tested OS/GPU/input matrix,
known limitations and changes before publishing. Windows packages are unsigned;
macOS packages have an ad-hoc signature only and are not notarized. Do not label
a target verified based solely on a successful build.
