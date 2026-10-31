use super::*;
use crate::{
    command::InstanceCommand as C,
    sculpt::{
        Geometry, Vec3,
        paths::{PathMode, PathPoint},
    },
};

#[test]
#[ignore = "requires Vulkan; checks selectable path bending, neighbour joining and baking"]
fn reusable_path_bend_join_offscreen_review() {
    use crate::{
        preferences::{Dock, Tool},
        sculpt::{Primitive, PrimitiveKind},
    };
    let directory = std::path::PathBuf::from(
        std::env::var("CLAY_GUIDE_PREVIEW").unwrap_or_else(|_| "/tmp/clay-path-join".into()),
    );
    std::fs::create_dir_all(&directory).unwrap();
    let mut g = Guide::new(&directory, [1280, 900]);
    g.interactive = true;
    g.panel(Tool::Submodels, Dock::Right);
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [16, 2, 2];
    let mut source = cube.mesh().unwrap();
    for p in &mut source.vertices {
        p.0 *= 0.18;
        p.1 *= 0.075;
        p.2 *= 0.075;
    }
    let mut geometry = Geometry::new(source.clone());
    geometry.scene.active_metadata.visible = false;
    g.app.document.replace_geometry(geometry).unwrap();
    g.app
        .document
        .create_path(
            "Curved rail".into(),
            PathMode::Free,
            vec![
                Vec3(-1.25, -0.45, 0.),
                Vec3(-0.5, 0.6, 0.),
                Vec3(0.5, 0.6, 0.),
                Vec3(1.25, -0.45, 0.),
            ]
            .into_iter()
            .map(|position| PathPoint {
                position,
                normal: Vec3(0., 0., 1.),
            })
            .collect(),
            false,
            1.,
        )
        .unwrap();
    g.app.dispatch(Command::Instances(C::NewPath), &g.ctx);
    assert!(
        g.app
            .document
            .instance_placements()
            .unwrap()
            .meshes
            .is_empty()
    );
    g.save("06-path-rigid");
    click_guide_label(&mut g, "Bend along path");
    click_guide_label(&mut g, "Fit section lengths to spacing");
    click_guide_label(&mut g, "Rigid");
    click_guide_label(&mut g, "Bend along path");
    assert!(
        matches!(&g.app.ui.instances.editor.as_ref().unwrap().layout, crate::sculpt::instances::Layout::Path(a) if !a.fit_sections)
    );
    click_guide_label(&mut g, "Fit section lengths to spacing");
    assert_eq!(
        g.app.document.instance_placements().unwrap().meshes.len(),
        8,
        "{}",
        g.app.ui.status
    );
    g.save("07-path-bent-separate");
    click_guide_label(&mut g, "Join neighbours");
    assert_eq!(
        g.app.document.instance_placements().unwrap().len(),
        1,
        "{}",
        g.app.ui.status
    );
    click_guide_label(&mut g, "Separate");
    click_guide_label(&mut g, "Rigid");
    assert!(
        g.app
            .document
            .instance_placements()
            .unwrap()
            .meshes
            .is_empty()
    );
    click_guide_label(&mut g, "Bend along path");
    click_guide_label(&mut g, "Join neighbours");
    let before = g.app.document.history_counts().0;
    g.save("08-path-bent-joined");
    click_guide_label(&mut g, "Apply instances");
    assert!(
        !g.app.document.instance_preview_active(),
        "{}",
        g.app.ui.status
    );
    assert_eq!(g.app.document.history_counts().0, before + 1);
    let resolved = g.app.document.instance_placements().unwrap();
    let expected = resolved.meshes[&(1, 0)].clone();
    click_guide_label(&mut g, "Bake joined model");
    finish_mesh_job(&mut g.app);
    assert!(
        g.app.document.instances().sets.is_empty(),
        "{}",
        g.app.ui.status
    );
    assert_eq!(g.app.document.model_count(), 2);
    assert_eq!(g.app.document.mesh(), expected.as_ref());
    assert_eq!(g.app.document.model(0).unwrap().mesh, &source);
    g.save("09-path-joined-baked");
    g.app.dispatch(Command::Undo, &g.ctx);
    assert!(g.app.document.instances().sets[0].joined());
    g.app.dispatch(Command::Redo, &g.ctx);
    g.app.document.project().validate().unwrap();
}
