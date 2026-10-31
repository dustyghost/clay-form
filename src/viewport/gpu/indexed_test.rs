//! Compare shared-vertex drawing against the per-corner reference pipeline.
use super::{ui_capture::Headless, *};
use crate::{
    camera::Camera,
    sculpt::{Alpha, Geometry, Kind, Material, Mesh},
};

#[test]
#[ignore = "requires Vulkan; indexed surfaces must preserve reference pixels"]
fn gpu_indexed_surfaces_match_general_pixels_and_split_buffers() {
    let headless = Headless::new();
    let mut gpu = Gpu::new(&headless.state.device, &headless.state.queue, COLOR);
    let mut mesh = Mesh::starting_sphere();
    mesh.hidden_faces = mesh
        .triangles
        .iter()
        .map(|t| t.iter().all(|&i| mesh.vertices[i].0 > 0.2))
        .collect();
    mesh.materials = mesh
        .vertices
        .iter()
        .map(|p| Material::from_srgb([0.6, 0.3 + p.1 * 0.1, 0.2], 0.5 + p.0 * 0.1, 0.2))
        .collect();
    mesh.mask = mesh
        .vertices
        .iter()
        .map(|p| (p.1 + 0.5).clamp(0., 1.))
        .collect();
    let mut document = Document::default();
    document.replace_geometry(Geometry::new(mesh)).unwrap();
    document.duplicate_model(false).unwrap();
    let mut active = document.geometry_snapshot();
    for p in &mut active.mesh.vertices {
        *p = *p * 0.65 + Vec3(-0.5, 0.2, 0.5);
    }
    document.replace_geometry(active).unwrap();
    let display = Arc::new(DisplayMesh::new(&document));
    assert!(display.active_vertices < display.mesh.vertices.len());
    gpu.upload(&display);
    let mut brush = Brush::new(Kind::Standard);
    brush.alpha = Alpha::Image {
        width: 2,
        height: 2,
        pixels: vec![0., 1., 0.7, 0.2].into(),
    };
    let previews = [
        Preview::default(),
        Preview {
            backface_opacity: 0.3,
            brush: Some(BrushPreview {
                center: Vec3(0., 0., 0.9),
                normal: Vec3(0., 0., 1.),
                radius: 0.6,
                brush,
                color: [1., 0.8, 0.2],
            }),
            symmetry: Some((
                Symmetry {
                    enabled: true,
                    ..Default::default()
                },
                1.,
            )),
            ..Default::default()
        },
        Preview {
            inspection: InspectionMode::Color,
            ..Default::default()
        },
        Preview {
            inspection: InspectionMode::Roughness,
            ..Default::default()
        },
        Preview {
            inspection: InspectionMode::Metallic,
            ..Default::default()
        },
    ];
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(253., 189.));
    for split in [false, true] {
        if split {
            gpu.upload_meshes(&display, 16_384);
            assert!(gpu.meshes.len() > 1);
        }
        for perspective in [false, true] {
            let camera = Camera {
                perspective,
                yaw: 0.2,
                pitch: 0.15,
                ..Default::default()
            };
            let scene = Scene::from_geometry(rect, &camera, display.clone(), false);
            let mut params = Params::new(
                rect,
                scene.right,
                scene.up,
                scene.view,
                scene.scale,
                scene.center,
                display.depth_extent,
            );
            params.set_perspective(scene.eye_distance, display.depth_extent);
            for (wire, mode) in [
                (false, WireMode::Visible),
                (true, WireMode::Visible),
                (true, WireMode::Transparent),
            ] {
                for (case, preview) in previews.iter().enumerate() {
                    gpu.force_general_surface = false;
                    assert!(gpu.basic_surface_eligible(preview));
                    gpu.paint(&params, [253, 189], wire, mode, false, preview);
                    let actual = gpu.read();
                    gpu.force_general_surface = true;
                    gpu.paint(&params, [253, 189], wire, mode, false, preview);
                    let expected = gpu.read();
                    assert!(
                        expected
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .any(|p| p[..3] != [23, 26, 31])
                    );
                    assert_eq!(
                        actual,
                        expected,
                        "split={split} perspective={perspective} wire={wire} transparent={} preview={case}",
                        mode == WireMode::Transparent
                    );
                }
            }
        }
    }
    gpu.force_general_surface = false;
    for preview in [
        Preview {
            show_polygroups: true,
            ..Default::default()
        },
        Preview {
            uv_checker: true,
            ..Default::default()
        },
        Preview {
            inspection: InspectionMode::UvChecker,
            ..Default::default()
        },
    ] {
        assert!(!gpu.basic_surface_eligible(&preview));
    }
    // Resizing an index payload must preserve its indexed-draw usage too.
    let indices = &mut gpu.meshes[0].indices;
    for bytes in [&[0u8; 12][..], &[][..]] {
        upload::write_buffer(&gpu.device, &gpu.queue, indices, "index resize", bytes);
        assert!(indices.usage().contains(wgpu::BufferUsages::INDEX));
    }
}
