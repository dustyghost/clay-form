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
    assert_eq!(g.app.document.instance_placements().unwrap().len(), 1);
    g.app.dispatch(Command::ResetView, &g.ctx);
    g.save("01-grouped-path-preview");
    super::path_assembly::drag_thickness_handle(&mut g, "End", 0.5);
    g.save("02-grouped-path-taper");
    click_guide_label(&mut g, "Apply instances");
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
