//! Opt-in rendering of the user's diagnostic export; no fixture data is modified.
use super::*;
use crate::{
    diagnostics::EditSettings, document::PathStrokeOptions, sculpt::Project, viewport::UiCapture,
};

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Vulkan and CLAY_RING_DIAGNOSTICS pointing to the ring export"]
fn exported_ring_automatic_stroke_offscreen_preview() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_RING_DIAGNOSTICS").expect("Set CLAY_RING_DIAGNOSTICS"),
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    let transition = manifest["transitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edit| edit["from"] == 18 && edit["to"] == 19)
        .expect("stage 18 -> 19 path stroke");
    let settings: EditSettings =
        serde_json::from_value(transition["settings_at_edit_start"].clone()).unwrap();
    let first = settings.first_dab.as_ref().expect("recorded path options");
    let id = first["path_id"].as_u64().unwrap();
    let mut options = PathStrokeOptions {
        radius: first["radius_world"].as_f64().unwrap() as f32,
        pressure: first["pressure"].as_f64().unwrap() as f32,
        reverse: first["reverse"].as_bool().unwrap(),
        invert: first["invert"].as_bool().unwrap(),
    };
    let load = |stage| -> Project {
        let file = directory.join(format!("stage-{stage:03}.clay"));
        serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap()
    };
    let mut source = load(18);
    source.mesh_settings = settings.topology.clone();
    source.symmetry = settings.symmetry.clone();
    let mut fixed = App::default();
    fixed
        .document
        .replace_project(source, "ring-source.clay")
        .unwrap();
    let seam_normal = fixed.document.path(id).unwrap().points[0].normal;
    fixed.camera.yaw = seam_normal.0.atan2(seam_normal.2) - 0.35;
    fixed.camera.pitch = -0.22;
    fixed.camera.zoom = 0.92;
    fixed.camera.pan = Vec2::ZERO;
    fixed.camera.perspective = false;
    let use_brush_size = std::env::var_os("CLAY_RING_REVIEW_ACTIVE_BRUSH_SIZE").is_some();
    let fixed_name = if use_brush_size {
        "brush-size"
    } else {
        "fixed"
    };
    if use_brush_size {
        // Match the active brush footprint in this capture's viewport, excluding
        // the central panel's eight-point margins, using the shared projection.
        options.radius = settings.active_brush.as_ref().unwrap().radius
            / fixed.camera.scale_at(
                Vec2::new(1184., 884.),
                fixed.document.path(id).unwrap().points[0].position,
            );
    }
    let before = fixed.document.geometry_snapshot();
    let started = std::time::Instant::now();
    let preview = fixed
        .document
        .prepare_path_stroke(id, settings.active_brush.as_ref().unwrap(), options)
        .unwrap()
        .run(&|| false)
        .unwrap();
    assert_eq!(fixed.document.geometry_snapshot(), before);
    eprintln!(
        "Replayed ring in {:?}: radius {}, {} dabs, {} vertices, attachment warning: {:?}",
        started.elapsed(),
        options.radius,
        preview.sample_count(),
        preview.mesh().vertices.len(),
        preview.attachment_warning()
    );
    assert!(fixed.document.apply_path_stroke(preview).unwrap());
    std::fs::write(
        format!("/tmp/clay-ring-{fixed_name}.clay"),
        serde_json::to_vec(&fixed.document.project()).unwrap(),
    )
    .unwrap();
    let mut exported = App::default();
    exported
        .document
        .replace_project(load(19), "ring-exported.clay")
        .unwrap();

    let mut capture = UiCapture::new([1200, 900]);
    let ctx = egui::Context::default();
    // Face the actual seam, slightly off-axis so the ring relief catches light.
    // Identical projection and lighting make the old/new captures comparable.
    let mut images = Vec::new();
    for (name, app) in [("exported", &mut exported), (fixed_name, &mut fixed)] {
        app.viewport.initialize_renderer(&capture.state).unwrap();
        app.camera.yaw = seam_normal.0.atan2(seam_normal.2) - 0.35;
        app.camera.pitch = -0.22;
        app.camera.zoom = 0.92;
        app.camera.pan = Vec2::ZERO;
        app.camera.perspective = false;
        app.ui.show_polygroups = false;
        app.ui.guide_paths = false;
        app.ui.show_symmetry_guides = false;
        for wire in [false, true] {
            app.ui.wire = wire;
            let mut rgba = Vec::new();
            for _ in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1200., 900.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                    },
                );
                rgba = capture.render(&ctx, output);
            }
            let suffix = if wire { "-wire" } else { "" };
            capture.save(format!("/tmp/clay-ring-{name}{suffix}.png"), &rgba);
            if !wire {
                images.push(rgba);
            }
        }
    }
    let changed = images[0]
        .as_chunks::<4>()
        .0
        .iter()
        .zip(images[1].as_chunks::<4>().0)
        .filter(|(old, new)| old[..3] != new[..3])
        .count();
    assert!(
        changed > 100,
        "the replay must visibly differ from the exported artifact: {changed}"
    );
}
