# Open-source preparation checks

- Focused egui interaction test opens Settings → About Clay Form,
  checks the ownership statement and verifies that pointer input behind the
  window cannot sculpt or create undo entries: passed.
- Manual Vulkan/software-rendered About capture: passed and visually reviewed
  at 1000 × 760. Text, close control, source links and scrollable notices are
  reachable. See [capture](about.png).
- Archive unit checks cover six native target layouts, stable and alpha versions,
  source/build identity, executable permissions, mandatory licences/checksums,
  macOS bundle metadata and the signing invocation: passed using fixtures.
  These tests do not establish native compilation or app launch behaviour.
- Notice input hashes and collected upstream-file hashes: passed. The release
  gate deliberately rejects the unresolved notice inventory; these are draft
  audit records, not completed legal clearance.
- Gitleaks 8.30.1: scanned 95 reachable commits / about 6.23 MB, no matching
  leaks. A clean pattern scan is not proof of complete absence of private data.
- Native compilation and packaging passed in PR CI for all six targets, and
  the vendored source archive passed its offline relocation check. Complete
  tests/lint and existing GPU regressions also run in PR CI. Physical hardware
  and distro smoke tests remain outstanding.
