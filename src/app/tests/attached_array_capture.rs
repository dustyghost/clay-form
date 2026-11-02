use super::*;
use crate::sculpt::{Geometry, Vec3, instances::*};

fn array(g: &Guide) -> &Array {
    let Layout::Array(a) = &g.app.document.attached_array(0).unwrap().layout else {
        panic!()
    };
    a
}
fn grip(g: &Guide, name: &str) -> Pos2 {
    g.ctx
        .data(|d| d.get_temp::<Vec<(&'static str, Pos2)>>(egui::Id::new("array_inline_handles")))
        .unwrap()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}
fn visible_click(g: &mut Guide, label: &str) {
    g.frame(vec![]);
    let top = g.app.ui.viewport_menu.viewport_rect.unwrap().top();
    let point = g
        .text_positions
        .iter()
        .find(|(text, p)| {
            text == label && p.y > top && p.y < g.size[1] as f32 && p.x < g.size[0] as f32
        })
        .unwrap_or_else(|| panic!("{label} is inside the visible workspace"))
        .1;
    g.frame(vec![
        egui::Event::PointerMoved(point),
        primary_event(point, true),
    ]);
    g.frame(vec![primary_event(point, false), egui::Event::PointerGone]);
}
fn drag(g: &mut Guide, name: &str, delta: Vec2) {
    g.frame(vec![]);
    let p = grip(g, name);
    g.frame(vec![egui::Event::PointerMoved(p), primary_event(p, true)]);
    g.frame(vec![egui::Event::PointerMoved(p + delta)]);
    g.frame(vec![
        primary_event(p + delta, false),
        egui::Event::PointerMoved(p + delta * 3.),
    ]);
    assert!(!g.app.document.instance_preview_active());
}
#[test]
#[ignore = "requires Vulkan; clicks and renders attached arrays and inline handles"]
fn attached_array_controls_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").unwrap_or_else(|_| "/tmp/clay-attached-arrays".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1440, 1000]);
    g.interactive = true;
    g.panel(Tool::Submodels, Dock::Right);
    g.app.ui.preferences.right_width = 355.;
    let mut p = Primitive::for_kind(PrimitiveKind::Cube);
    p.divisions = [2; 3];
    let mut mesh = p.mesh().unwrap();
    for v in &mut mesh.vertices {
        *v = Vec3(v.0 * 0.22, v.1 * 0.13, v.2 * 0.10);
    }
    g.app
        .document
        .replace_geometry(Geometry::new(mesh))
        .unwrap();
    click_guide_label(&mut g, "New array…");
    assert_eq!(g.app.document.model_count(), 1);
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 2);
    g.save("01-attached-linear");
    let (_, before) = g.frame(vec![]);
    let spacing = array(&g).spacing.0;
    let history = g.app.document.history_counts().0;
    drag(&mut g, "X spacing", egui::vec2(35., 0.));
    assert_ne!(array(&g).spacing.0, spacing);
    assert_eq!(g.app.document.history_counts().0, history + 1);
    let (rect, after) = g.frame(vec![]);
    let changed = (rect.top() as usize + 200..rect.bottom() as usize - 80)
        .flat_map(|y| {
            (rect.left() as usize..rect.right() as usize).map(move |x| (y * 1440 + x) * 4)
        })
        .filter(|&i| before[i..i + 3] != after[i..i + 3])
        .count();
    assert!(
        changed > 100,
        "Dragging spacing changes the rendered array surface"
    );
    click_guide_label(&mut g, "Radial");
    g.app.finish_parameter_input(&g.ctx);
    click_guide_label(&mut g, "Frame");
    g.save("02-attached-radial");
    assert_eq!(array(&g).kind, ArrayKind::Radial);
    let radius = array(&g).radius;
    drag(&mut g, "Radius", egui::vec2(30., 0.));
    assert_ne!(array(&g).radius, radius);
    drag(&mut g, "Arc", egui::vec2(-25., 30.));
    assert!(array(&g).angle < 360.);
    click_guide_label(&mut g, "Grid");
    g.app.finish_parameter_input(&g.ctx);
    click_guide_label(&mut g, "Frame");
    g.save("03-attached-grid");
    assert_eq!(array(&g).kind, ArrayKind::Grid);
    assert_eq!(array(&g).count % array(&g).columns, 0);
    drag(&mut g, "Y spacing", egui::vec2(0., -25.));
    visible_click(&mut g, "Edit source");
    assert!(g.app.document.editing_array_source());
    g.save("04-edit-source");
    visible_click(&mut g, "Edit array");
    assert!(!g.app.document.editing_array_source());
    visible_click(&mut g, "Done");
    assert!(!g.app.ui.instances.array_edit);
    g.frame(vec![]);
    assert!(!g.app.ui.instances.array_edit);
    visible_click(&mut g, "Make editable…");
    let before = g.app.document.geometry_snapshot();
    let faces = g.app.document.mesh().triangles.len() * array(&g).count;
    g.save("05-make-editable");
    visible_click(&mut g, "Bake now");
    finish_mesh_job(&mut g.app);
    assert!(g.app.document.attached_array(0).is_none());
    assert_eq!(g.app.document.mesh().triangles.len(), faces);
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.geometry_snapshot(), before);
    g.size = [1100, 800];
    visible_click(&mut g, "Edit array");
    g.app.dispatch(
        Command::Instances(crate::command::InstanceCommand::FrameArray),
        &g.ctx,
    );
    g.save("06-small-window");
}
