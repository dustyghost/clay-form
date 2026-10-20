#!/usr/bin/env bash
# Render synthetic example scenes through the real application/wgpu code.
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
capture_dir=$(mktemp -d "${TMPDIR:-/tmp}/clay-doc-screenshots.XXXXXX")
export CLAY_GPU_SOFTWARE=1
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
export CLAY_GUIDE_PREVIEW="$capture_dir/guides"

cargo test --locked --lib offscreen_preview -- --ignored --test-threads=1
cargo test --locked --lib edge_workflow_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_transform_gizmo_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_component_construction_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_face_group_gap_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_milestones_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_workflow_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_symmetry_dissolve_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_selection_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_boundary_join_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib delete_submodel_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib detail_projection_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib alpha_library_stencil_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib modeling_region_fuse_offscreen_review -- --ignored --test-threads=1
cargo test --locked --lib gpu_lasso_preview_and_commit -- --ignored
cargo test --locked --lib gpu_rotation_controls_and_primitive_regeneration -- --ignored

# Copy only the documentation selections after every capture succeeds.
mkdir -p docs/images
cp "$capture_dir/perspective-orthographic.png" docs/images/view-orthographic.png
cp "$capture_dir/perspective-ui.png" docs/images/view-perspective.png
cp "$capture_dir/detail-solid.png" docs/images/detail-groove.png
cp "$capture_dir/lasso-mask-3.png" docs/images/mask-lasso-preview.png
cp "$capture_dir/lasso-mask-5.png" docs/images/mask-lasso-result.png
cp "$capture_dir/lasso-hide-3.png" docs/images/hide-lasso-preview.png
cp "$capture_dir/lasso-hide-5.png" docs/images/hide-lasso-result.png
cp "$capture_dir/rotation-4.png" docs/images/rotation-controls.png
for name in alpha-library alpha-editor alpha-surface-stencil submodel-delete detail-projection-preview modeling-separate-fuse-result modeling-region-fuse-result modeling-new-mesh-warning modeling-move-gizmo modeling-scale-gizmo modeling-rotate-gizmo modeling-point-split modeling-point-tube modeling-edge-construction-preview modeling-face-split modeling-face-group-gap modeling-tap-repeat modeling-milestone-inset modeling-milestone-smooth modeling-milestone-preset modeling-symmetry-selection modeling-symmetry-extrude modeling-dissolve-before modeling-dissolve-after modeling-paint-selection modeling-face-loop modeling-edge-ring modeling-edge-loop modeling-loop-drag modeling-boundary-join modeling-loop-preview modeling-selection modeling-extrusion-preview modeling-tunnel edge-loops-before edge-loops-after crease-smooth-before crease-smooth-after crease-selection thickness-modifier mirror-before mirror-after brush-reset-before brush-reset-after first-sculpt-before first-sculpt-after symmetry-stroke adaptive-settings primitive-cube slice-trim-preview slice-trim-result reference-images obj-import menu-placement recovery-dialog; do
    cp "$capture_dir/guides/$name.png" "docs/images/$name.png"
done
cp "$capture_dir/guides/front-study.png" docs/examples/front-study.png
cp "$capture_dir/guides/side-study.png" docs/examples/side-study.png
printf 'Documentation screenshots updated. Extra captures remain in %s\n' "$capture_dir"
