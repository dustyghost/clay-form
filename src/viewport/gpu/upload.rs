//! Independent mesh-buffer updates and bounded, aligned vertex writes.
use std::{ops::Range, sync::Arc};

const BLOCK: usize = 4096;
const MAX_WRITES: usize = 64;

/// Shared immutable storage proves the uploaded bytes are unchanged, including
/// non-reflexive float values. Rebuilt payloads retain the previous value check.
pub(super) fn face_data_unchanged<T: PartialEq>(before: &Arc<Vec<T>>, after: &Arc<Vec<T>>) -> bool {
    Arc::ptr_eq(before, after) || before == after
}

#[cfg(test)]
pub(super) fn changed_ranges(before: &[u8], after: &[u8]) -> Vec<Range<usize>> {
    assert_eq!(before.len(), after.len());
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut changed = 0;
    for (block, (a, b)) in before.chunks(BLOCK).zip(after.chunks(BLOCK)).enumerate() {
        // Compare bytes, including signed zero and every material channel.
        if a == b {
            continue;
        }
        let start = block * BLOCK;
        let end = start + b.len();
        changed += b.len();
        if let Some(last) = ranges.last_mut()
            && last.end == start
        {
            last.end = end;
        } else {
            ranges.push(start..end);
        }
        // Global normal changes and scattered edits should use one write, not
        // hundreds of tiny staging allocations. All packed data is f32-aligned.
        if ranges.len() > MAX_WRITES || changed > after.len() / 2 {
            return vec![0..after.len()];
        }
    }
    ranges
}

/// Track monotonically ordered record edits without rescanning a second array.
pub(super) struct DirtyRanges {
    ranges: Vec<Range<usize>>,
    bytes: usize,
    len: usize,
    full: bool,
}
impl DirtyRanges {
    pub(super) fn new(len: usize) -> Self {
        Self {
            ranges: Vec::new(),
            bytes: 0,
            len,
            full: false,
        }
    }
    pub(super) fn mark(&mut self, start: usize, end: usize) {
        debug_assert!(start < end && end <= self.len);
        if self.full {
            return;
        }
        let start = start / BLOCK * BLOCK;
        let end = end.div_ceil(BLOCK).saturating_mul(BLOCK).min(self.len);
        if let Some(last) = self.ranges.last_mut()
            && last.end >= start
        {
            debug_assert!(start >= last.start);
            self.bytes += end.saturating_sub(last.end);
            last.end = last.end.max(end);
        } else {
            self.bytes += end - start;
            self.ranges.push(start..end);
        }
        if self.ranges.len() > MAX_WRITES || self.bytes > self.len / 2 {
            self.full = true;
            self.ranges.clear();
        }
    }
    pub(super) fn finish(self) -> Vec<Range<usize>> {
        if self.full {
            vec![0..self.len]
        } else {
            self.ranges
        }
    }
}

/// Preserve an allocation when its payload size is unchanged. Exact-size
/// replacements keep growing/shrinking meshes within the binding limit.
pub(super) fn write_buffer(
    device: &super::wgpu::Device,
    queue: &super::wgpu::Queue,
    target: &mut super::wgpu::Buffer,
    label: &str,
    bytes: &[u8],
) {
    let size = if bytes.is_empty() { 16 } else { bytes.len() };
    if target.size() == size as u64 {
        if !bytes.is_empty() {
            queue.write_buffer(target, 0, bytes);
        }
    } else {
        *target = super::buffer(
            device,
            label,
            bytes,
            super::wgpu::BufferUsages::STORAGE | super::wgpu::BufferUsages::COPY_DST,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_payload_reuse_preserves_snapshots_and_float_fallback() {
        let original = Arc::new(vec![[0., 1., 0., 1.]]);
        let mut changed = original.clone();
        assert!(face_data_unchanged(&original, &changed));
        Arc::make_mut(&mut changed)[0][0] = 0.25;
        assert!(!face_data_unchanged(&original, &changed));
        assert_eq!(original[0][0], 0.);

        let rebuilt = Arc::new((*original).clone());
        assert!(!Arc::ptr_eq(&original, &rebuilt));
        assert!(face_data_unchanged(&original, &rebuilt));
        let signed_zero = Arc::new(vec![[-0., 1., 0., 1.]]);
        assert!(face_data_unchanged(&original, &signed_zero));
        assert!(!face_data_unchanged(&original, &Arc::new(Vec::new())));

        let nan = Arc::new(vec![[f32::from_bits(0x7fc00001), 1., 0., 1.]]);
        // Reusing the exact bytes is valid even though NaN != NaN. A separate
        // allocation still takes the old value comparison and requests a write.
        assert!(face_data_unchanged(&nan, &nan.clone()));
        assert!(!face_data_unchanged(&nan, &Arc::new((*nan).clone())));
    }

    fn ranges(before: &[u8], after: &[u8]) -> Vec<Range<usize>> {
        let mut dirty = DirtyRanges::new(after.len());
        for (i, (a, b)) in before.chunks(BLOCK).zip(after.chunks(BLOCK)).enumerate() {
            if a != b {
                dirty.mark(i * BLOCK, i * BLOCK + b.len());
            }
        }
        let ranges = dirty.finish();
        assert_eq!(ranges, changed_ranges(before, after));
        ranges
    }

    #[test]
    fn vertex_upload_ranges_cover_sparse_edits_and_aligned_tail() {
        let before = vec![0; BLOCK * 10 + 12];
        let mut after = before.clone();
        for i in [4, BLOCK + 8, BLOCK * 7 + 4, BLOCK * 10 + 8] {
            after[i] = 1;
        }
        let writes = ranges(&before, &after);
        assert_eq!(
            writes,
            vec![0..BLOCK * 2, BLOCK * 7..BLOCK * 8, BLOCK * 10..after.len()]
        );
        let mut uploaded = before;
        for range in writes {
            assert_eq!(range.start % 4, 0);
            assert_eq!(range.len() % 4, 0);
            uploaded[range.clone()].copy_from_slice(&after[range]);
        }
        assert_eq!(uploaded, after);
        assert!(ranges(&after, &after).is_empty());
        assert!(ranges(&[], &[]).is_empty());
    }

    #[test]
    fn vertex_upload_ranges_bound_fragmented_and_global_writes() {
        let before = vec![0; BLOCK * 300];
        let mut after = before.clone();
        for i in 0..65 {
            after[i * BLOCK * 3] = 1;
        }
        assert_eq!(ranges(&before, &after), vec![0..after.len()]);
        after.fill(1);
        assert_eq!(ranges(&before, &after), vec![0..after.len()]);
        let before = [0_f32; 4096];
        let mut after = before;
        after[0] = -0.;
        after[1024] = f32::from_bits(0x7fc00001);
        let ranges = ranges(bytemuck::cast_slice(&before), bytemuck::cast_slice(&after));
        assert_eq!(ranges, vec![0..BLOCK * 2]);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod gpu_tests {
    use super::super::{ui_capture::Headless, *};
    use crate::{
        camera::Camera,
        sculpt::{Dab, Geometry, Kind, Material, Mesh},
    };

    fn draw(gpu: &mut Gpu, display: &DisplayMesh, wire: bool) -> Vec<u8> {
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(128.));
        let (right, up, view) = Camera::default().basis();
        let params = Params::new(
            rect,
            right,
            up,
            view,
            44.8,
            rect.center(),
            display.depth_extent,
        );
        gpu.paint(
            &params,
            [128, 128],
            wire,
            WireMode::Visible,
            false,
            &Preview {
                show_polygroups: true,
                uv_checker: true,
                ..Default::default()
            },
        );
        gpu.read()
    }

    fn assert_fresh_pixels(headless: &Headless, gpu: &mut Gpu, display: &Arc<DisplayMesh>) {
        let mut fresh = Gpu::new(&headless.state.device, &headless.state.queue, COLOR);
        fresh.upload(display);
        for wire in [false, true] {
            assert_eq!(
                draw(gpu, display, wire),
                draw(&mut fresh, display, wire),
                "incremental upload must match fresh buffers, wire={wire}"
            );
        }
    }

    #[test]
    #[ignore = "requires Vulkan; verifies sparse vertex writes, pixels and history"]
    fn gpu_vertex_upload_sparse_strokes_match_fresh_buffers() {
        let headless = Headless::new();
        let mut geometry = Geometry::new(Mesh::starting_sphere());
        for _ in 0..2 {
            geometry.subdivide().unwrap();
        }
        geometry.mesh.materials =
            vec![Material::from_srgb([0.2, 0.4, 0.6], 0.65, 0.); geometry.mesh.vertices.len()];
        for hidden in [false, true] {
            let mut geometry = geometry.clone();
            if hidden {
                geometry.mesh.hidden_faces = geometry
                    .mesh
                    .triangles
                    .iter()
                    .map(|t| t.iter().all(|&i| geometry.mesh.vertices[i].0 > 0.1))
                    .collect();
            }
            for kind in [Kind::Standard, Kind::Clay, Kind::Paint, Kind::Mask] {
                let mut doc = Document::default();
                doc.replace_geometry(geometry.clone()).unwrap();
                let mut renderer = Renderer::new(&headless.state).unwrap();
                let first = renderer.geometry(&doc);
                let mut gpu = Gpu::new(&headless.state.device, &headless.state.queue, COLOR);
                gpu.upload(&first);
                let vertices = gpu.meshes[0].vertices.clone();
                let indices = gpu.meshes[0].indices.clone();
                let colors = gpu.meshes[0].colors.clone();
                let uv = gpu.meshes[0].uv.clone();
                let mut brush = Brush::new(kind);
                brush.strength = 0.15;
                brush.set_paint_colour(false, [0.9, 0.1, 0.2]);
                for step in 0..2 {
                    doc.apply_dab(
                        &brush,
                        Dab {
                            center: Vec3(-0.2 + step as f32 * 0.03, 0., 0.98),
                            normal: Vec3(0., 0., 1.),
                            right: Vec3(1., 0., 0.),
                            up: Vec3(0., 1., 0.),
                            radius: 0.3,
                            pressure: 0.8,
                            delta: Vec3::default(),
                            invert: false,
                        },
                    );
                    let display = renderer.geometry(&doc);
                    assert!(Arc::ptr_eq(&first.poly_colors, &display.poly_colors));
                    assert!(Arc::ptr_eq(&first.uv_corners, &display.uv_corners));
                    gpu.upload(&display);
                    assert_eq!(gpu.meshes[0].vertices, vertices);
                    assert_eq!(gpu.meshes[0].indices, indices);
                    assert_eq!(gpu.meshes[0].colors, colors);
                    assert_eq!(gpu.meshes[0].uv, uv);
                    if step == 1 {
                        assert!(gpu.vertex_pack_count > 0);
                        assert!(
                            gpu.vertex_pack_count < display.shading.vertices.len() / 2,
                            "{kind:?} must not repack untouched vertices"
                        );
                        assert!(gpu.vertex_write_bytes > 0, "{kind:?} must write the edit");
                        assert!(
                            gpu.vertex_write_bytes < gpu.packed_vertices.len() * 4 / 2,
                            "{kind:?} must upload less than half the vertex data"
                        );
                    }
                    assert_fresh_pixels(&headless, &mut gpu, &display);
                }
                doc.end_stroke();
                let display = renderer.geometry(&doc);
                gpu.upload(&display);
                assert_fresh_pixels(&headless, &mut gpu, &display);
                doc.undo();
                let display = renderer.geometry(&doc);
                gpu.upload(&display);
                assert_eq!(draw(&mut gpu, &display, false), {
                    let mut original =
                        Gpu::new(&headless.state.device, &headless.state.queue, COLOR);
                    original.upload(&first);
                    draw(&mut original, &first, false)
                });
                doc.redo();
                let display = renderer.geometry(&doc);
                gpu.upload(&display);
                assert_fresh_pixels(&headless, &mut gpu, &display);
                renderer.destroy();
            }
        }
    }

    #[test]
    #[ignore = "requires Vulkan; verifies independent buffers through canonical release fan changes"]
    fn gpu_vertex_upload_release_splits_and_merges_fans() {
        let headless = Headless::new();
        let mut geometry = Geometry::new(Mesh::starting_sphere());
        for _ in 0..2 {
            geometry.subdivide().unwrap();
        }
        geometry.mesh.hidden_faces = geometry
            .mesh
            .triangles
            .iter()
            .map(|t| t.iter().all(|&i| geometry.mesh.vertices[i].0 > 0.1))
            .collect();
        let original = geometry.clone();
        let index = geometry
            .mesh
            .vertices
            .iter()
            .position(|p| p.2 > 0.99 && p.0 <= 0.)
            .unwrap();
        let mut doc = Document::default();
        doc.replace_geometry(geometry.clone()).unwrap();
        let mut renderer = Renderer::new(&headless.state).unwrap();
        let initial = renderer.geometry(&doc);
        let mut gpu = Gpu::new(&headless.state.device, &headless.state.queue, COLOR);
        gpu.upload(&initial);
        for (height, splitting) in [(0.8, true), (0., false)] {
            geometry.mesh.vertices[index].2 = original.mesh.vertices[index].2 + height;
            doc.replace_geometry(geometry.clone()).unwrap();
            doc.begin_stroke();
            let live = renderer.geometry(&doc);
            assert!(renderer.live_display);
            gpu.upload(&live);
            let vertices = gpu.meshes[0].vertices.clone();
            let indices = gpu.meshes[0].indices.clone();
            let colors = gpu.meshes[0].colors.clone();
            let uv = gpu.meshes[0].uv.clone();
            doc.end_stroke();
            let released = renderer.geometry(&doc);
            assert!(!renderer.live_display);
            assert!(Arc::ptr_eq(&live.poly_colors, &released.poly_colors));
            assert!(Arc::ptr_eq(&live.uv_corners, &released.uv_corners));
            assert_ne!(live.shading.triangles, released.shading.triangles);
            assert_eq!(
                released.shading.vertices.len() > live.shading.vertices.len(),
                splitting
            );
            let canonical = DisplayMesh::new(&doc);
            assert_eq!(released.shading.vertices, canonical.shading.vertices);
            assert_eq!(released.shading.triangles, canonical.shading.triangles);
            gpu.upload(&released);
            assert_ne!(gpu.meshes[0].vertices, vertices);
            assert_eq!(
                gpu.meshes[0].indices, indices,
                "face count is stable even when fans change"
            );
            assert_eq!(gpu.meshes[0].colors, colors);
            assert_eq!(gpu.meshes[0].uv, uv);
            assert_eq!(
                gpu.face_write_bytes,
                [released.shading.triangles.len() * 12, 0, 0]
            );
            assert_eq!(
                gpu.vertex_write_bytes,
                released.shading.vertices.len() * gpu.vertex_stride * 4
            );
            assert_fresh_pixels(&headless, &mut gpu, &released);
        }
        renderer.destroy();
    }

    #[test]
    #[ignore = "requires Vulkan; verifies independent buffer updates and batching"]
    fn gpu_vertex_upload_layout_changes_match_fresh_buffers() {
        let headless = Headless::new();
        let doc = Document::default();
        let initial = Arc::new(DisplayMesh::new(&doc));
        let mut gpu = Gpu::new(&headless.state.device, &headless.state.queue, COLOR);
        for case in 0..6 {
            gpu.upload(&initial);
            let previous = gpu.meshes[0].vertices.clone();
            let indices = gpu.meshes[0].indices.clone();
            let colors = gpu.meshes[0].colors.clone();
            let uv = gpu.meshes[0].uv.clone();
            let mut next = (*initial).clone();
            let data = Arc::make_mut(&mut next.geometry);
            match case {
                0 => Arc::make_mut(&mut data.shading.triangles).swap(0, 1), // Same count, different indices.
                1 => Arc::make_mut(&mut data.poly_colors)[0] = [255, 0, 0, 255],
                2 => Arc::make_mut(&mut data.uv_corners)[0][0] = [0.1, 0.2, 1., 0.],
                3 => {
                    let mut material = Material::default();
                    material.emission_strength = 0.5;
                    data.mesh.materials = Arc::new(vec![material; data.mesh.vertices.len()]);
                }
                4 => {
                    let vertex = data.shading.vertices[0];
                    Arc::make_mut(&mut data.shading.vertices).push(vertex);
                }
                5 => {
                    Arc::make_mut(&mut data.shading.triangles).clear();
                    Arc::make_mut(&mut data.mesh.triangles).clear();
                    Arc::make_mut(&mut data.mesh.quads).clear();
                    Arc::make_mut(&mut data.poly_colors).clear();
                    Arc::make_mut(&mut data.uv_corners).clear();
                }
                _ => unreachable!(),
            }
            let next = Arc::new(next);
            assert_eq!(
                Arc::ptr_eq(&initial.poly_colors, &next.poly_colors),
                !matches!(case, 1 | 5)
            );
            assert_eq!(
                Arc::ptr_eq(&initial.uv_corners, &next.uv_corners),
                !matches!(case, 2 | 5)
            );
            gpu.upload(&next);
            if matches!(case, 3 | 4) {
                assert_ne!(
                    gpu.meshes[0].vertices, previous,
                    "only vertex storage must resize"
                );
                assert!(gpu.vertex_write_bytes > 0);
            } else {
                assert_eq!(
                    gpu.meshes[0].vertices, previous,
                    "case {case}: preserve vertex storage"
                );
                assert_eq!(gpu.vertex_write_bytes, 0);
            }
            if case != 5 {
                assert_eq!(gpu.meshes[0].indices, indices);
                assert_eq!(gpu.meshes[0].colors, colors);
                assert_eq!(gpu.meshes[0].uv, uv);
            } else {
                assert_eq!(gpu.meshes[0].count, 0);
                assert_eq!(gpu.meshes[0].indices.size(), 16);
                assert_eq!(gpu.meshes[0].colors.size(), 16);
                assert_eq!(gpu.meshes[0].uv.size(), 16);
            }
            for (channel, bytes) in gpu.face_write_bytes.iter().enumerate() {
                assert_eq!(
                    *bytes > 0,
                    case < 3 && channel == case,
                    "case {case}: write only the changed face channel {channel}"
                );
            }
            assert_fresh_pixels(&headless, &mut gpu, &next);
            if case == 3 {
                // Extended material records cross 4 KiB boundaries. Exercise
                // sparse writes to channels beyond the compact twelve floats.
                let before = draw(&mut gpu, &next, false);
                let buffer = gpu.meshes[0].vertices.clone();
                let mut painted = (*next).clone();
                let data = Arc::make_mut(&mut painted.geometry);
                for (material, position) in Arc::make_mut(&mut data.mesh.materials)
                    .iter_mut()
                    .zip(data.mesh.vertices.iter())
                {
                    if position.2 > 0.9 {
                        material.emission = [2., 0.1, 0.1];
                    }
                }
                let painted = Arc::new(painted);
                gpu.upload(&painted);
                assert_eq!(gpu.vertex_stride, 77);
                assert_eq!(gpu.meshes[0].vertices, buffer);
                assert!(gpu.vertex_write_bytes > 0);
                assert!(gpu.vertex_write_bytes < gpu.packed_vertices.len() * 4 / 2);
                assert_ne!(draw(&mut gpu, &painted, false), before);
                assert_fresh_pixels(&headless, &mut gpu, &painted);
            }
        }
        gpu.upload(&initial);
        let same_values = Arc::new(DisplayMesh::new(&doc));
        assert!(!Arc::ptr_eq(&initial.poly_colors, &same_values.poly_colors));
        assert!(!Arc::ptr_eq(&initial.uv_corners, &same_values.uv_corners));
        gpu.upload(&same_values);
        assert_eq!(gpu.face_write_bytes, [0; 3]);
        assert_eq!(gpu.vertex_pack_count, 0);
        assert_eq!(
            gpu.vertex_write_bytes, 0,
            "identical packed values need no writes"
        );
        gpu.upload(&initial);
        let normal = draw(&mut gpu, &initial, false);
        gpu.upload_meshes(&initial, 16_384);
        assert!(gpu.meshes.len() > 1);
        assert!(gpu.packed_vertices.is_empty());
        assert_eq!(draw(&mut gpu, &initial, false), normal);
        gpu.upload(&initial);
        assert_eq!(gpu.meshes.len(), 1);
        assert_eq!(draw(&mut gpu, &initial, false), normal);
        gpu.clear();
        assert!(gpu.packed_vertices.is_empty());
        assert!(gpu.mesh_geometry.is_none());
        gpu.upload(&initial);
        assert_eq!(draw(&mut gpu, &initial, false), normal);
    }
}

/// Opt-in stage timings for the manual release-upload diagnosis only.
#[cfg(test)]
#[derive(Default)]
pub(super) struct Trace {
    last: Option<std::time::Instant>,
    samples: Vec<(&'static str, f64)>,
}
#[cfg(test)]
impl Trace {
    pub(super) fn start(&mut self) {
        self.samples.clear();
        self.samples.reserve(12);
        self.last = Some(std::time::Instant::now());
    }
    pub(super) fn mark(&mut self, name: &'static str) {
        if let Some(last) = &mut self.last {
            let now = std::time::Instant::now();
            self.samples
                .push((name, now.duration_since(*last).as_secs_f64() * 1000.));
            *last = now;
        }
    }
    pub(super) fn stop(&mut self) -> Vec<(&'static str, f64)> {
        self.last = None;
        std::mem::take(&mut self.samples)
    }
}
