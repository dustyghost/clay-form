use super::*;
use crate::{
    command::{InstanceCommand as C, PendingAction},
    sculpt::{
        Geometry, Mesh, Vec3,
        instances::{Layout, Source},
    },
};

fn plate(extent: Vec3, offset: Vec3) -> Mesh {
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [1; 3];
    let mut mesh = cube.mesh().unwrap();
    for p in &mut mesh.vertices {
        *p = Vec3(p.0 * extent.0, p.1 * extent.1, p.2 * extent.2) + offset;
    }
    mesh
}
#[test]
#[ignore = "requires Vulkan; creates a personal multi-object brush and paints two different parts"]
fn object_kit_library_and_strokes_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").unwrap_or_else(|_| "/tmp/clay-object-kits".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let personal = directory.join(format!("test-profile-{}", std::process::id()));
    let mut g = Guide::new(&directory, [1440, 1000]);
    g.interactive = true;
    g.panel(Tool::Submodels, Dock::Right);
    g.app.ui.preferences.right_width = 345.;
    g.app.preferences_path = Some(personal.join("preferences.json"));
    let mut vent = plate(Vec3(0.15, 0.095, 0.02), Vec3::default());
    vent.materials =
        vec![crate::sculpt::Material::from_srgb([0.23, 0.27, 0.33], 0.5, 0.2); vent.vertices.len()];
    for i in -2..=2 {
        let mut slat = plate(Vec3(0.009, 0.077, 0.015), Vec3(i as f32 * 0.05, 0., 0.034));
        slat.materials = vec![
            crate::sculpt::Material::from_srgb([0.66, 0.73, 0.79], 0.4, 0.15);
            slat.vertices.len()
        ];
        vent = vent.merged_with(&slat).unwrap();
    }
    let mut geometry = Geometry::new(vent);
    geometry.name = "Hull vent".into();
    g.app.document.replace_geometry(geometry).unwrap();
    g.app.camera.zoom = 3.;
    g.app.camera.yaw = 0.32;
    g.app.camera.pitch = -0.28;
    click_guide_label(&mut g, "Manage brushes…");
    g.app.ui.instances.group_name = "Ship fittings".into();
    g.frame(vec![]);
    g.frame(vec![]);
    click_guide_label(&mut g, "New brush");
    assert_eq!(g.app.ui.instances.group, Some(1), "{}", g.app.ui.status);
    click_guide_label(&mut g, "Capture a model into this brush");
    click_guide_label(&mut g, "Use base centre");
    g.save("17-object-capture-orientation");
    for label in ["Contact", "Forward +X", "Up +Z"] {
        assert!(
            g.text_positions.iter().any(|(text, _)| text == label),
            "{label} visible on the model"
        );
    }
    click_guide_label(&mut g, "Capture active model");
    assert_eq!(
        g.app.document.instances().assets[0].pivot,
        Vec3(0., 0., -0.02)
    );
    // The second part has a different source-space Up axis.
    let mut dome = Primitive::for_kind(PrimitiveKind::UvSphere).mesh().unwrap();
    for p in &mut dome.vertices {
        *p = Vec3(p.0 * 0.10, p.1 * 0.052 - 0.028, p.2 * 0.10);
    }
    dome.materials = vec![
        crate::sculpt::Material::from_srgb([0.86, 0.56, 0.26], 0.3, 0.12);
        dome.vertices.len()
    ];
    let mut mount = Primitive::for_kind(PrimitiveKind::Cylinder).mesh().unwrap();
    for p in &mut mount.vertices {
        *p = Vec3(p.0 * 0.11, p.1 * 0.012 - 0.072, p.2 * 0.11);
    }
    mount.materials = vec![
        crate::sculpt::Material::from_srgb([0.38, 0.44, 0.51], 0.45, 0.2);
        mount.vertices.len()
    ];
    dome = mount.merged_with(&dome).unwrap();
    let mut geometry = g.app.document.geometry_snapshot();
    geometry.mesh = dome;
    geometry.name = "Sensor dome".into();
    g.app.document.replace_geometry(geometry).unwrap();
    g.app.ui.instances.up = 1;
    click_guide_label(&mut g, "Use base centre");
    click_guide_label(&mut g, "Capture active model");
    assert_eq!(g.app.document.instances().brushes[0].objects, vec![1, 2]);
    assert!(g.app.document.instances().assets[1].pivot.1 < -0.07);
    click_guide_label(&mut g, "Capture a model into this brush");
    g.app.ui.instances.library_open = false;
    let mut geometry = g.app.document.geometry_snapshot();
    geometry.mesh = Mesh::starting_sphere();
    geometry.name = "Practice hull".into();
    g.app.document.replace_geometry(geometry).unwrap();
    g.app.camera = Default::default();
    click_guide_label(&mut g, "Hull vent");
    g.app.ui.instances.brush.scale = 1.;
    g.app.ui.instances.brush.spacing = 0.38;
    click_guide_label(&mut g, "Place / brush objects");
    let rect = g.frame(vec![]).0;
    let centre = rect.center();
    g.stroke(
        centre + egui::vec2(-150., -45.),
        centre + egui::vec2(150., -45.),
    );
    let first = g.app.document.instances().sets[0].clone();
    click_guide_label(&mut g, "Sensor dome");
    assert_eq!(
        g.app.viewport.object_brush.as_ref().unwrap().piece.source,
        Source::Asset(2)
    );
    g.app.ui.instances.brush.scale = 0.95;
    g.app.ui.instances.brush.spacing = 0.27;
    g.stroke(
        centre + egui::vec2(-150., 52.),
        centre + egui::vec2(150., 52.),
    );
    assert_eq!(g.app.document.instances().sets[0], first);
    for (i, set) in g.app.document.instances().sets.iter().enumerate() {
        let Layout::Manual(items) = &set.layout else {
            panic!()
        };
        assert!(
            items.len() >= 3
                && items
                    .iter()
                    .all(|p| p.piece.source == Source::Asset(i as u64 + 1))
        );
    }
    assert_eq!(g.app.document.instances().sets.len(), 2);
    g.app.camera.yaw = 0.16;
    g.app.camera.pitch = -0.16;
    g.save("18-ship-fittings-strokes");
    click_guide_label(&mut g, "Manage brushes…");
    g.frame(vec![]);
    g.frame(vec![]);
    click_guide_label(&mut g, "Save brush to my library");
    assert_eq!(g.app.ui.instances.personal_brushes, vec!["Ship fittings"]);
    g.save("19-ship-fittings-library");
    g.app.dispatch(Command::Instances(C::StopBrush), &g.ctx);
    g.app.document.project().validate().unwrap();
    g.app.execute_pending(PendingAction::New, &g.ctx);
    click_guide_label(&mut g, "Manage brushes…");
    g.frame(vec![]);
    g.frame(vec![]);
    click_guide_label(&mut g, "Add to project");
    let library = g.app.document.instances();
    assert_eq!(library.brushes[0].name, "Ship fittings");
    assert_eq!(library.assets[0].options.spacing, 0.38);
    assert_eq!(library.assets[1].up, 1);
    assert_eq!(library.assets[1].options.spacing, 0.27);
    assert!(library.sets.is_empty());
    std::fs::remove_dir_all(personal).unwrap();
}
