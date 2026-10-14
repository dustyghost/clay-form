#!/usr/bin/env bash
# Render synthetic example scenes through the real application/OpenGL code.
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
capture_dir=$(mktemp -d "${TMPDIR:-/tmp}/clay-doc-screenshots.XXXXXX")
export EGL_PLATFORM=surfaceless
export LIBGL_ALWAYS_SOFTWARE=1
export LP_NUM_THREADS=${LP_NUM_THREADS:-2}
export CLAY_PERSPECTIVE_PREVIEW="$capture_dir/perspective"
export CLAY_DETAIL_PREVIEW="$capture_dir/detail"
export CLAY_NAVIGATION_PREVIEW="$capture_dir/navigation"
export CLAY_LASSO_PREVIEW="$capture_dir/lasso"
export CLAY_REFERENCE_PREVIEW="$capture_dir/references.png"
export CLAY_OBJ_PREVIEW="$capture_dir/obj-import.png"
export CLAY_MENU_PREVIEW="$capture_dir/menu-placement.png"
export CLAY_RECOVERY_PREVIEW="$capture_dir/recovery.png"
export CLAY_ROTATION_PREVIEW="$capture_dir/rotation"

cargo test --locked --lib offscreen_preview -- --ignored --test-threads=1
cargo test --locked --lib gpu_lasso_preview_and_commit -- --ignored
cargo test --locked --lib gpu_rotation_controls_and_primitive_regeneration -- --ignored

# Copy only the documentation selections after every capture succeeds.
mkdir -p docs/images
cp "$capture_dir/perspective-orthographic.png" docs/images/view-orthographic.png
cp "$capture_dir/perspective-ui.png" docs/images/view-perspective.png
cp "$capture_dir/detail-solid.png" docs/images/detail-groove.png
cp "$capture_dir/navigation-brush-settings.png" docs/images/brush-settings.png
cp "$capture_dir/lasso-mask-3.png" docs/images/mask-lasso-preview.png
cp "$capture_dir/lasso-mask-5.png" docs/images/mask-lasso-result.png
cp "$capture_dir/references.png" docs/images/reference-images.png
cp "$capture_dir/obj-import.png" docs/images/obj-import.png
cp "$capture_dir/menu-placement.png" docs/images/menu-placement.png
cp "$capture_dir/recovery.png" docs/images/recovery-dialog.png
cp "$capture_dir/rotation-4.png" docs/images/rotation-controls.png
printf 'Documentation screenshots updated. Extra captures remain in %s\n' "$capture_dir"
