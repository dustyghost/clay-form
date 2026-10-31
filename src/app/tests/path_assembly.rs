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
    drag_thickness_handle(&mut g, "Start", -0.5);
    drag_thickness_handle(&mut g, "End", 1.);
    g.save("10-thickness-taper-inline");
    drag_thickness_handle(&mut g, "Path point 2", 0.8);
    drag_thickness_handle(&mut g, "Section 4", -0.3);
    g.save("11-thickness-profile-inline");
    click_guide_label(&mut g, "Thickness along path");
    click_guide_label(&mut g, "Uniform");
    click_guide_label(&mut g, "End-to-end taper");
    click_guide_label(&mut g, "Along-curve profile");
    for _ in 0..3 {
        g.frame(vec![]);
    }
    let (plot, max) = g
        .ctx
        .data(|d| d.get_temp::<(egui::Rect, f32)>(egui::Id::new("path_thickness_plot")))
        .unwrap();
    let crate::sculpt::instances::Layout::Path(a) =
        &g.app.ui.instances.editor.as_ref().unwrap().layout
    else {
        unreachable!()
    };
    let point = a.thickness.points[1];
    let start = egui::pos2(
        plot.left() + point.position * plot.width(),
        plot.bottom() - point.factor / max * plot.height(),
    );
    let end = start - egui::vec2(0., 0.25 / max * plot.height());
    g.frame(vec![
        egui::Event::PointerMoved(start),
        primary_event(start, true),
    ]);
    g.frame(vec![egui::Event::PointerMoved(end)]);
    g.frame(vec![primary_event(end, false), egui::Event::PointerGone]);
    let crate::sculpt::instances::Layout::Path(a) =
        &g.app.ui.instances.editor.as_ref().unwrap().layout
    else {
        unreachable!()
    };
    assert!(
        (a.thickness.points[1].factor - point.factor - 0.25).abs() < 1e-3,
        "graph drag changes the same profile as the inline handles"
    );
    let count = a.thickness.points.len();
    g.save("12-thickness-profile-panel");
    click_guide_label(&mut g, "Add point");
    click_guide_label(&mut g, "Remove point");
    let crate::sculpt::instances::Layout::Path(a) =
        &g.app.ui.instances.editor.as_ref().unwrap().layout
    else {
        unreachable!()
    };
    assert_eq!(a.thickness.points.len(), count);
    click_guide_label(&mut g, "Inline thickness handles");
    g.save("12-thickness-profile-panel");
    click_guide_label(&mut g, "Thickness along path");
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
    g.save("13-thickness-baked");
    g.app.dispatch(Command::Undo, &g.ctx);
    assert!(g.app.document.instances().sets[0].joined());
    g.app.dispatch(Command::Redo, &g.ctx);
    g.app.document.project().validate().unwrap();
}

fn drag_thickness_handle(g: &mut Guide, label: &str, amount: f32) {
    g.frame(vec![]);
    let (_, position, axis, t) = g
        .ctx
        .data(|d| {
            d.get_temp::<Vec<(String, egui::Pos2, egui::Vec2, f32)>>(egui::Id::new(
                "path_thickness_handles",
            ))
        })
        .unwrap()
        .into_iter()
        .find(|h| h.0 == label)
        .unwrap_or_else(|| panic!("{label} handle is visible"));
    let crate::sculpt::instances::Layout::Path(a) =
        &g.app.ui.instances.editor.as_ref().unwrap().layout
    else {
        unreachable!()
    };
    let old = a.thickness.at(t, false);
    let end = position + axis * (amount * 60.);
    g.frame(vec![
        egui::Event::PointerMoved(position),
        primary_event(position, true),
    ]);
    for step in 1..=3 {
        g.frame(vec![egui::Event::PointerMoved(
            position.lerp(end, step as f32 / 3.),
        )]);
    }
    g.frame(vec![primary_event(end, false), egui::Event::PointerGone]);
    let crate::sculpt::instances::Layout::Path(a) =
        &g.app.ui.instances.editor.as_ref().unwrap().layout
    else {
        unreachable!()
    };
    assert!(
        (a.thickness.at(t, false) - old - amount).abs() < 1e-3,
        "{label}: {}",
        g.app.ui.status
    );
}
