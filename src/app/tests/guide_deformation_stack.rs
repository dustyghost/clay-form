use super::*;
use crate::sculpt::{Geometry, Vec3, cage::Cage};
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
