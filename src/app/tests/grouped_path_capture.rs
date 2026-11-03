use super::*;
use crate::sculpt::instances::Layout;

#[test]
#[ignore = "requires Vulkan; assigns regions of one connected source and bakes the path object"]
fn grouped_path_object_controls_offscreen_review() {
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").unwrap_or_else(|_| "/tmp/clay-grouped-path".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1440, 1000]);
    g.interactive = true;
    g.panel(Tool::Submodels, Dock::Right);
    g.app.ui.preferences.right_width = 365.;
    crate::app::tests::grouped_path::prepare(&mut g.app);
    // Custom role names are assigned after opening the editor. The source is
    // upright, so this path must discover Y even though the initial piece uses X.
    let mut geometry = g.app.document.geometry_snapshot();
    for p in &mut geometry.mesh.vertices {
        *p = crate::sculpt::Vec3(-p.1, p.0, p.2);
    }
    g.app.document.replace_geometry(geometry).unwrap();
    g.app
        .document
        .rename_vertex_group(0, "Foot".into())
        .unwrap();
    g.app
        .document
        .rename_vertex_group(1, "Repeat".into())
        .unwrap();
    g.app.document.rename_vertex_group(2, "Tip".into()).unwrap();
    let source = g.app.document.mesh().clone();
    let history = g.app.document.history_counts().0;
    click_guide_label(&mut g, "Create path object…");
    assert!(g.app.ui.instances.invalid);
    for (role, name) in [("Start", "Foot"), ("Middle", "Repeat"), ("End", "Tip")] {
        click_guide_label(&mut g, &format!("{role}: Choose group…"));
        for _ in 0..3 {
            g.frame(vec![]);
        }
        click_guide_label(&mut g, name);
    }
    assert!(!g.app.ui.instances.invalid, "{}", g.app.ui.status);
    let layout = &g.app.ui.instances.editor.as_ref().unwrap().layout;
    assert!(matches!(layout,Layout::Path(a) if a.regions.as_ref().unwrap().middle=="Repeat"));
    assert!(matches!(layout, Layout::Path(a) if a.middle.forward == 1 && a.middle.up == 2));
    // Switch Up directly in the editor, then change thickness and apply.
    g.frame(vec![]);
    let up = g
        .text_positions
        .iter()
        .find(|(label, _)| label == "Up")
        .unwrap()
        .1;
    let x = g
        .text_positions
        .iter()
        .filter(|(label, pos)| label == "X" && pos.x > up.x && (pos.y - up.y).abs() < 3.)
        .min_by(|(_, a), (_, b)| a.x.total_cmp(&b.x))
        .unwrap()
        .1;
    g.frame(vec![egui::Event::PointerMoved(x), primary_event(x, true)]);
    g.frame(vec![primary_event(x, false), egui::Event::PointerGone]);
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 1);
    g.app.dispatch(Command::ResetView, &g.ctx);
    g.save("01-grouped-path-preview");
    super::path_assembly::drag_thickness_handle(&mut g, "End", 0.5);
    g.save("02-grouped-path-taper");
    click_guide_label(&mut g, "Apply instances");
    assert!(
        matches!(&g.app.document.instances().sets[0].layout, Layout::Path(a) if a.middle.forward == 1 && a.middle.up == 0)
    );
    assert_eq!(g.app.document.history_counts().0, history + 1);
    assert_eq!(g.app.document.mesh(), &source);
    crate::persistence::save_project(
        directory
            .join("Grouped path example.clay")
            .to_str()
            .unwrap(),
        &g.app.document.project(),
    )
    .unwrap();
    let mesh = g.app.document.instance_placements().unwrap().meshes[&(1, 0)].clone();
    click_guide_label(&mut g, "Bake set…");
    click_guide_label(&mut g, "One new submodel");
    click_guide_label(&mut g, "Bake now");
    finish_mesh_job(&mut g.app);
    assert!(
        g.app.document.instances().sets.is_empty(),
        "{}",
        g.app.ui.status
    );
    assert_eq!(g.app.document.mesh(), mesh.as_ref());
    g.save("03-grouped-path-baked");
    g.app.dispatch(Command::Undo, &g.ctx);
    assert_eq!(g.app.document.instances().sets.len(), 1);
    g.app.dispatch(Command::Redo, &g.ctx);
    assert!(g.app.document.instances().sets.is_empty());
}
