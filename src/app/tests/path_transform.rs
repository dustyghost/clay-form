use super::*;
use crate::{
    command::PathCommand as C,
    sculpt::{
        Geometry, Primitive, PrimitiveKind, Vec3,
        paths::{PathMode, PathPoint},
    },
    viewport::paths::{
        PathTool,
        transform::{Operation, Pivot},
    },
};

#[test]
fn whole_path_gizmos_move_rotate_scale_cancel_and_keep_one_save_step() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [1; 3];
    let mut geometry = Geometry::new(primitive.mesh().unwrap());
    geometry.scene.active_metadata.visible = false;
    app.document.replace_geometry(geometry).unwrap();
    let id = app
        .document
        .create_path(
            "Transform route".into(),
            PathMode::Free,
            [Vec3(-0.7, -0.3, 0.), Vec3(0., 0.4, 0.), Vec3(0.7, -0.3, 0.)]
                .map(|position| PathPoint {
                    position,
                    normal: Vec3(0., 0., 1.),
                })
                .to_vec(),
            false,
            1.,
        )
        .unwrap();
    let before = app.document.project();
    let history = app.document.history_counts();
    app.dispatch(Command::Path(C::Open), &ctx);
    app.viewport.paths.tool = PathTool::Whole;
    let controls = |ctx: &egui::Context| {
        ctx.data(|d| {
            d.get_temp::<(Pos2, Vec<(String, Pos2)>)>(egui::Id::new(
                "whole_path_transform_controls",
            ))
        })
        .unwrap()
    };
    let drag = |app: &mut App, from: Pos2, to: Pos2| {
        navigation_frame(
            app,
            &ctx,
            vec![egui::Event::PointerMoved(from), primary_event(from, true)],
        );
        navigation_frame(app, &ctx, vec![egui::Event::PointerMoved(to)]);
        navigation_frame(
            app,
            &ctx,
            vec![primary_event(to, false), egui::Event::PointerGone],
        );
    };
    navigation_frame(&mut app, &ctx, vec![]);
    let (center, _) = controls(&ctx);
    drag(&mut app, center, center + Vec2::new(30., -15.));
    let moved = app.viewport.paths.draft.clone().unwrap();
    let delta = moved.points[0].position - before.paths[0].points[0].position;
    assert!(delta.len() > 0.01);
    for (a, b) in before.paths[0].points.iter().zip(&moved.points) {
        assert!((b.position - a.position - delta).len() < 1e-5);
    }
    app.viewport.paths.transform.operation = Operation::Rotate;
    navigation_frame(&mut app, &ctx, vec![]);
    let (center, _) = controls(&ctx);
    drag(
        &mut app,
        center + Vec2::new(100., 0.),
        center + Vec2::new(0., -100.),
    );
    let rotated = app.viewport.paths.draft.clone().unwrap();
    assert_ne!(moved.points, rotated.points);
    let length = |path: &crate::sculpt::paths::SurfacePath| {
        (path.points[1].position - path.points[0].position).len()
    };
    assert!((length(&moved) - length(&rotated)).abs() < 1e-5);
    app.viewport.paths.transform.operation = Operation::Scale;
    navigation_frame(&mut app, &ctx, vec![]);
    let (_, handles) = controls(&ctx);
    let scale = handles
        .iter()
        .find(|(name, _)| name == "Scale path")
        .unwrap()
        .1;
    drag(
        &mut app,
        scale,
        scale + Vec2::new(1., -1.).normalized() * 50.,
    );
    let scaled = app.viewport.paths.draft.clone().unwrap();
    assert!((length(&scaled) / length(&rotated) - 1.5).abs() < 1e-5);
    assert!((app.viewport.paths.transform.pending_section_scale - 1.5).abs() < 1e-5);
    assert!(app.document.project() == before);
    assert_eq!(app.document.history_counts(), history);
    navigation_frame(&mut app, &ctx, vec![]);
    let (_, handles) = controls(&ctx);
    let scale = handles[0].1;
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(scale), primary_event(scale, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(scale + Vec2::new(20., -20.))],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(scale, false)]);
    assert_eq!(app.viewport.paths.draft.as_ref().unwrap(), &scaled);
    assert!((app.viewport.paths.transform.pending_section_scale - 1.5).abs() < 1e-5);
    app.dispatch(Command::Path(C::Save), &ctx);
    assert_eq!(app.document.path(id).unwrap(), &scaled, "{}", app.ui.status);
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    app.dispatch(Command::Undo, &ctx);
    assert!(app.document.project() == before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.path(id).unwrap(), &scaled);
    app.viewport.paths.tool = PathTool::Move;
    app.viewport.paths.draft.as_mut().unwrap().points[1]
        .position
        .2 += 0.1;
    app.dispatch(Command::Path(C::Save), &ctx);
    assert!(app.document.path(id).unwrap().points[1].position.2 > scaled.points[1].position.2);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.path(id).unwrap(), &scaled);
    navigation_frame(&mut app, &ctx, vec![]);
    let editor = &mut app.viewport.paths;
    editor.transform.operation = Operation::Scale;
    editor.transform.pivot = Pivot::Start;
    editor.transform.scale_sections = false;
    editor.transform.scale = 0.5;
    editor
        .transform
        .apply_numeric(editor.draft.as_mut().unwrap())
        .unwrap();
    let route_only = editor.draft.as_ref().unwrap();
    assert_eq!(route_only.points[0].position, scaled.points[0].position);
    assert!((length(route_only) / length(&scaled) - 0.5).abs() < 1e-5);
    assert_eq!(editor.transform.pending_section_scale, 1.);
    app.dispatch(Command::Path(C::Discard), &ctx);
    assert_eq!(app.viewport.paths.draft.as_ref().unwrap(), &scaled);
}

#[test]
fn whole_path_surface_detachment_is_explicit_and_undoable() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mesh = crate::sculpt::Mesh {
        vertices: vec![
            Vec3(-1., -1., 0.),
            Vec3(1., -1., 0.),
            Vec3(1., 1., 0.),
            Vec3(-1., 1., 0.),
        ],
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        mask: vec![0.; 4],
        ..Default::default()
    };
    app.document.replace_geometry(Geometry::new(mesh)).unwrap();
    let id = app
        .document
        .create_path(
            "Attached route".into(),
            PathMode::Surface,
            [Vec3(-0.5, 0., 0.), Vec3(0.5, 0., 0.)]
                .map(|position| PathPoint {
                    position,
                    normal: Vec3(0., 0., 1.),
                })
                .to_vec(),
            false,
            0.,
        )
        .unwrap();
    let before = app.document.project();
    let route = app.document.path(id).unwrap().dense_curve();
    let history = app.document.history_counts().0;
    app.dispatch(Command::Path(C::Open), &ctx);
    app.viewport.paths.transform.translation = Vec3(0., 0., 1.);
    assert!(
        app.viewport
            .paths
            .transform
            .apply_numeric(app.viewport.paths.draft.as_mut().unwrap())
            .is_err()
    );
    assert!(app.document.project() == before);
    app.dispatch(Command::Path(C::Detach), &ctx);
    assert_eq!(
        app.document.path(id).unwrap().mode,
        PathMode::Free,
        "{}",
        app.ui.status
    );
    assert_eq!(app.document.path(id).unwrap().dense_curve(), route);
    assert_eq!(app.document.history_counts().0, history + 1);
    assert_eq!(app.document.mesh(), &before.mesh);
    app.dispatch(Command::Undo, &ctx);
    assert!(app.document.project() == before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.path(id).unwrap().mode, PathMode::Free);
}
