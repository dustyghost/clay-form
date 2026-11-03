use super::*;
use crate::sculpt::{Geometry, Vec3, cage::Cage};
#[test]
#[ignore = "requires Vulkan to compare viewport shading"]
fn flat_shading_viewport_toggle() {
    let directory = std::env::var("CLAY_GUIDE_PREVIEW")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("clay-flat-shading"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1280, 900]);
    g.interactive = true;
    g.app
        .ui
        .preferences
        .dock_menu(BuiltinMenu::View, Dock::Right);
    g.app
        .document
        .replace_geometry(crate::app::tests::deformation_stack::boolean_fixture())
        .unwrap();
    g.app.document.set_boolean_resolution(48).unwrap();
    g.app.camera.zoom = 0.73;
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = 0.22;
    g.app.camera.pan = egui::vec2(78., 0.);
    let revision = g.app.document.display_revision();
    let history = g.app.document.history_counts();
    assert!(!g.app.ui.flat_shading);

    fn pixels(g: &mut Guide) -> Vec<u8> {
        // The docked View panel lays out its field-of-view control on the next frame.
        g.frame(vec![egui::Event::PointerGone]);
        let (rect, image) = g.frame(vec![]);
        let mut viewport = Vec::new();
        for y in rect.top().ceil() as usize..rect.bottom() as usize {
            let start = (y * g.size[0] as usize + rect.left().ceil() as usize) * 4;
            viewport.extend_from_slice(&image[start..start + (rect.width().floor() as usize) * 4]);
        }
        viewport
    }
    fn visibly_different(a: &[u8], b: &[u8]) -> bool {
        a.iter().zip(b).filter(|(a, b)| a.abs_diff(**b) > 3).count() > 100
    }
    g.frame(vec![]);
    let sources = pixels(&mut g);
    click_guide_label(&mut g, "Flat shading");
    assert!(g.app.ui.flat_shading);
    assert!(visibly_different(&sources, &pixels(&mut g)));
    g.app.dispatch(Command::Action(Action::FlatShading), &g.ctx);
    assert_eq!(sources, pixels(&mut g));

    g.app.dispatch(Command::ToggleBooleanResult, &g.ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while g.app.boolean.result().is_none() {
        g.frame(vec![]);
        assert!(std::time::Instant::now() < deadline);
    }
    let cached = g.app.viewport.boolean_display.clone().unwrap();
    let smooth = pixels(&mut g);
    g.save("flat-shading-off");
    click_guide_label(&mut g, "Flat shading");
    let flat = pixels(&mut g);
    g.save("flat-shading-on");
    assert!(visibly_different(&smooth, &flat));
    click_guide_label(&mut g, "Flat shading");
    assert_eq!(smooth, pixels(&mut g));
    assert!(std::sync::Arc::ptr_eq(
        &cached,
        g.app.viewport.boolean_display.as_ref().unwrap()
    ));
    assert_eq!(g.app.document.display_revision(), revision);
    assert_eq!(g.app.document.history_counts(), history);

    // A fully creased reference independently supplies one face normal per triangle.
    let mut reference = g.app.boolean.result().unwrap().clone();
    reference.creases = reference.polygon_edges().into_iter().collect();
    reference.creases.sort_unstable();
    g.app
        .viewport
        .set_boolean_result(&g.app.document, &reference);
    fn assert_face_lighting(expected: &[u8], actual: &[u8]) {
        let max_error = expected
            .iter()
            .zip(actual)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(
            max_error <= 2,
            "flat shading differs from face normals by {max_error}"
        );
    }
    assert_face_lighting(&pixels(&mut g), &flat);
    g.app.camera.perspective = true;
    let reference_perspective = pixels(&mut g);
    g.app.viewport.boolean_display = Some(cached);
    g.app.dispatch(Command::Action(Action::FlatShading), &g.ctx);
    assert_face_lighting(&reference_perspective, &pixels(&mut g));
}

#[test]
#[ignore = "Vulkan interaction and visual review of deformation and Boolean tools"]
fn deformation_stack_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW")
            .unwrap_or_else(|_| "/tmp/clay-deformation-stack".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1280, 900]);
    g.interactive = true;
    g.panel(Tool::Submodels, Dock::Right);
    let mesh = Primitive {
        size: Vec3(0.85, 2.0, 0.62),
        divisions: [4, 16, 4],
        ..Default::default()
    }
    .mesh()
    .unwrap();
    g.app
        .document
        .replace_geometry(Geometry::new(mesh.clone()))
        .unwrap();
    g.app.camera.yaw = 0.48;
    g.app.camera.pitch = 0.16;
    g.app.camera.zoom = 0.73;
    let count = g.app.document.history_counts().0;
    click_guide_label(&mut g, "Deformation frame");
    assert_eq!(g.app.document.history_counts().0, count);
    let rect = g.frame(vec![]).0;
    let cage = Cage::fit(&mesh).unwrap();
    let c = cage.corner(5);
    let (r, u, _) = g.app.camera.basis();
    let corner =
        rect.center() + egui::vec2(c.dot(r), -c.dot(u)) * g.app.camera.scale_at(rect.size(), c);
    for (i, handle, file, delta) in [
        (0, "", "01-pull", egui::vec2(75., -25.)),
        (1, "B", "02-bend", egui::vec2(94., 0.)),
        (2, "T", "03-twist", egui::vec2(105., 0.)),
    ] {
        g.frame(vec![]);
        let p = if i == 0 {
            corner
        } else {
            g.text_positions
                .iter()
                .find(|(text, _)| text == handle)
                .unwrap()
                .1
        };
        g.frame(vec![egui::Event::PointerMoved(p), primary_event(p, true)]);
        let end = p + delta;
        g.frame(vec![egui::Event::PointerMoved(end)]);
        assert!(g.app.viewport.cage_previewing());
        assert_eq!(g.app.document.mesh(), &mesh);
        g.save(file);
        if i == 0 {
            g.frame(vec![crate::app::tests::deformation_stack::key_escape()]);
            g.frame(vec![primary_event(end, false)]);
            assert_eq!(g.app.document.history_counts().0, count);
            g.frame(vec![egui::Event::PointerMoved(p), primary_event(p, true)]);
            g.frame(vec![egui::Event::PointerMoved(end)]);
        }
        g.frame(vec![primary_event(end, false)]);
        assert_ne!(g.app.document.mesh(), &mesh);
        g.app.dispatch(Command::Undo, &g.ctx);
        assert_eq!(g.app.document.mesh(), &mesh);
    }
    click_guide_label(&mut g, "Done");
    g.app
        .document
        .replace_geometry(crate::app::tests::deformation_stack::boolean_fixture())
        .unwrap();
    g.app.document.set_boolean_resolution(48).unwrap();
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = 0.22;
    g.app.camera.pan = egui::vec2(78., 0.);
    click_guide_label(&mut g, "Boolean result");
    fn settle(g: &mut Guide) {
        g.frame(vec![]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while g.app.boolean.result().is_none() {
            g.frame(vec![]);
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                g.app.ui.boolean_notice
            );
        }
    }
    for resolution in [24, 96, 48] {
        g.app.document.set_boolean_resolution(resolution).unwrap();
        settle(&mut g);
        g.save(&format!("boolean-resolution-{resolution}"));
    }
    g.save("04-boolean-preview");
    let unmodified = g.app.document.geometry_snapshot();
    let undo = g.app.document.history_counts();
    click_guide_label(&mut g, "Boolean result");
    assert!(!g.app.ui.boolean_enabled);
    g.save("06-boolean-operands");
    click_guide_label(&mut g, "Boolean result");
    settle(&mut g);
    assert_eq!(g.app.document.geometry_snapshot(), unmodified);
    assert_eq!(g.app.document.history_counts(), undo);
    g.app.dispatch(Command::SelectSubmodel(2), &g.ctx);
    click_guide_label(&mut g, "Subtract");
    click_guide_label(&mut g, "Intersect");
    assert_eq!(g.app.document.active_model_index(), 2);
    assert_eq!(
        g.app.document.model(0).unwrap().metadata.boolean_role,
        crate::sculpt::BooleanRole::Intersect
    );
    settle(&mut g);
    g.save("07-boolean-intersect");
    click_guide_label(&mut g, "Intersect");
    click_guide_label(&mut g, "Subtract");
    settle(&mut g);
    let before = g.app.document.geometry_snapshot();
    click_guide_label(&mut g, "Bake result");
    assert_eq!(g.app.document.model_count(), 4);
    g.save("05-boolean-bake");
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), before);
}

#[test]
#[ignore = "requires Vulkan to click the Boolean method and inspect the preview"]
fn surface_boolean_method_offscreen_review() {
    let directory = std::env::var_os("CLAY_GUIDE_PREVIEW")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("clay-surface-boolean"));
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1280, 900]);
    g.interactive = true;
    g.panel(Tool::Submodels, Dock::Right);
    g.app
        .document
        .replace_geometry(crate::app::tests::deformation_stack::boolean_fixture())
        .unwrap();
    g.app.camera.zoom = 0.73;
    g.app.camera.yaw = 0.35;
    g.app.camera.pitch = 0.22;
    click_guide_label(&mut g, "Boolean result");
    click_guide_label(&mut g, "Voxel");
    click_guide_label(&mut g, "Preserve surfaces");
    assert_eq!(
        g.app.document.boolean_method(),
        crate::sculpt::BooleanMethod::Surface
    );
    for (name, role) in [
        ("01-bore", "Subtract"),
        ("02-intersection", "Intersect"),
        ("03-union", "Add"),
    ] {
        if role != "Subtract" {
            let previous = g
                .app
                .document
                .model(0)
                .unwrap()
                .metadata
                .boolean_role
                .label();
            click_guide_label(&mut g, previous);
            // Lower rows also say Add. Select the item painted in the open popup.
            g.frame(vec![]);
            let point = g
                .text_positions
                .iter()
                .rev()
                .find(|(text, _)| text == role)
                .unwrap()
                .1;
            g.frame(vec![
                egui::Event::PointerMoved(point),
                primary_event(point, true),
            ]);
            g.frame(vec![primary_event(point, false), egui::Event::PointerGone]);
        }
        assert_eq!(
            g.app
                .document
                .model(0)
                .unwrap()
                .metadata
                .boolean_role
                .label(),
            role
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            g.frame(vec![]);
            if g.app.ui.boolean_ready {
                break;
            }
            assert!(
                g.app.ui.boolean_notice.contains("Updating")
                    && std::time::Instant::now() < deadline,
                "{}",
                g.app.ui.boolean_notice
            );
        }
        g.save(name);
    }
    let stack = g.app.document.geometry_snapshot();
    click_guide_label(&mut g, "Bake result");
    assert_eq!(g.app.document.model_count(), 4);
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), stack);
}
