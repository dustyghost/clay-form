use super::*;
use crate::sculpt::{BorderTarget, EdgeEdit, Geometry, ModelMetadata, Primitive, PrimitiveKind};
fn plane_mesh() -> crate::sculpt::Mesh {
    Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap()
}

#[test]
fn edges_actions_preserve_undo_save_and_noop_history() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document
        .replace_geometry(Geometry::new(plane_mesh()))
        .unwrap();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    app.ui.edges.target = BorderTarget::Holes;
    app.dispatch(Command::Action(Action::CreaseEdges), &ctx);
    finish_mesh_job(&mut app);
    let creased = app.document.geometry_snapshot();
    assert_eq!(creased.mesh.creases.len(), 4, "{}", app.ui.status);
    assert_eq!(app.document.history_counts(), (count + 1, 0));
    app.dispatch(Command::Action(Action::CreaseEdges), &ctx);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.history_counts(), (count + 1, 0));
    app.dispatch(Command::Action(Action::SelectCreases), &ctx);
    assert_eq!(app.ui.edges.selected.len(), 4);
    app.dispatch(Command::Action(Action::UncreaseEdges), &ctx);
    finish_mesh_job(&mut app);
    assert!(app.document.mesh().creases.is_empty(), "{}", app.ui.status);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), creased);
    let project: crate::sculpt::Project =
        serde_json::from_slice(&serde_json::to_vec(&app.document.project()).unwrap()).unwrap();
    project.validate().unwrap();
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), creased);
    app.dispatch(Command::Action(Action::SelectCreases), &ctx);
    app.ui.edges.smoothness = 1.;
    app.dispatch(Command::Action(Action::SmoothSubdivide), &ctx);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.mesh().creases.len(), 8, "{}", app.ui.status);
    assert_eq!(app.document.mesh_settings().levels(), 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), creased);
}

#[test]
fn edges_click_selection_is_separate_from_sculpt_and_clears_after_topology_changes() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document
        .replace_geometry(Geometry::new(plane_mesh()))
        .unwrap();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts();
    app.dispatch(Command::Action(Action::SelectEdges), &ctx);
    assert!(app.ui.edges.selecting);
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let point = rect.center() + Vec2::new(0., -rect.width().min(rect.height()) * 0.35);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(point, false)]);
    assert_eq!(app.ui.edges.selected.len(), 1);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), count);
    app.dispatch(Command::Action(Action::ExpandEdgeLoop), &ctx);
    assert_eq!(app.ui.edges.selected.len(), 4);
    app.dispatch(Command::Action(Action::AddSupportLoops), &ctx);
    finish_mesh_job(&mut app);
    assert!(
        app.document.mesh().vertices.len() > before.mesh.vertices.len(),
        "{}",
        app.ui.status
    );
    navigation_frame(&mut app, &ctx, vec![]);
    assert!(app.ui.edges.selected.is_empty());
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn edges_cancel_guards_and_modifier_application_are_atomic() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document
        .replace_geometry(Geometry::new(plane_mesh()))
        .unwrap();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts();
    app.dispatch(Command::EditEdges(EdgeEdit::SmoothSubdivide(1.)), &ctx);
    app.dispatch(Command::CancelMeshJob, &ctx);
    finish_mesh_job(&mut app);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), count);
    app.ui.settings_open = true;
    app.dispatch(Command::EditEdges(EdgeEdit::ClearCreases), &ctx);
    assert!(app.mesh_job.is_none());
    app.ui.settings_open = false;
    let mut geometry = before.clone();
    geometry.scene.active_metadata = ModelMetadata {
        temporary_thickness: 0.2,
        thickness_loops: 3,
        thickness_crease: true,
        ..Default::default()
    };
    app.document.replace_geometry(geometry).unwrap();
    let preview = app
        .document
        .mesh()
        .solidify_with_metadata(&app.document.geometry_snapshot().scene.active_metadata)
        .unwrap();
    assert!(!app.ui.action_enabled(&Action::CreaseEdges, &app.document));
    app.dispatch(Command::ApplyThickness, &ctx);
    assert_eq!(*app.document.mesh(), preview);
    assert!(!app.document.mesh().creases.is_empty());
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.mesh().vertices, before.mesh.vertices);
}

#[test]
fn edges_shift_add_and_ctrl_remove_do_not_zoom_or_hide() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document
        .replace_geometry(Geometry::new(plane_mesh()))
        .unwrap();
    app.dispatch(Command::Action(Action::SelectEdges), &ctx);
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let scale = rect.width().min(rect.height()) * 0.35;
    let first = rect.center() + Vec2::new(0., -scale);
    let second = rect.center() + Vec2::new(-scale, 0.);
    let before = app.document.geometry_snapshot();
    for (point, modifiers, expected) in [
        (first, egui::Modifiers::NONE, 1),
        (second, egui::Modifiers::SHIFT, 2),
        (first, egui::Modifiers::CTRL, 1),
    ] {
        for pressed in [true, false] {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1000., 800.),
                    )),
                    modifiers,
                    events: vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers,
                        },
                    ],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
        }
        assert_eq!(app.ui.edges.selected.len(), expected);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.camera.zoom, 1.);
    }
}

#[test]
fn edges_empty_space_drag_matches_normal_navigation_and_keeps_selection() {
    for modifiers in [
        egui::Modifiers::NONE,
        egui::Modifiers::ALT,
        egui::Modifiers::CTRL,
        egui::Modifiers::SHIFT,
    ] {
        let mut results = Vec::new();
        for selecting in [false, true] {
            let mut app = App::default();
            let ctx = egui::Context::default();
            app.document
                .replace_geometry(Geometry::new(plane_mesh()))
                .unwrap();
            let before = app.document.geometry_snapshot();
            if selecting {
                app.dispatch(Command::Action(Action::SelectEdges), &ctx);
                let edges = app.document.mesh().polygon_edges();
                app.viewport.set_edge_selection(edges, &app.document);
            }
            let rect = navigation_frame(&mut app, &ctx, vec![]);
            let selected = app.ui.edges.selected.clone();
            let start = rect.min + Vec2::splat(60.);
            // Cross onto the model without switching from navigation to selection.
            let end = rect.center();
            for (pos, pressed) in [(start, Some(true)), (end, None), (end, Some(false))] {
                let mut events = vec![egui::Event::PointerMoved(pos)];
                if let Some(pressed) = pressed {
                    events.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers,
                    });
                }
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1000., 800.),
                        )),
                        modifiers,
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                    },
                );
                if pressed == Some(true) {
                    assert!(app.viewport.session.gesture == Some(Gesture::Orbit));
                }
            }
            assert_eq!(app.document.geometry_snapshot(), before);
            assert_eq!(app.ui.edges.selected, selected);
            results.push((
                app.camera.yaw,
                app.camera.pitch,
                app.camera.pan,
                app.camera.zoom,
            ));
        }
        assert_eq!(results[0], results[1], "{modifiers:?}");
        assert_ne!(results[1], (0., 0., Vec2::ZERO, 1.));
    }
}

#[test]
fn edges_surface_drag_does_not_turn_into_orbit_or_sculpt() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document
        .replace_geometry(Geometry::new(plane_mesh()))
        .unwrap();
    app.dispatch(Command::Action(Action::SelectEdges), &ctx);
    let before = app.document.geometry_snapshot();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let start = rect.center();
    let end = rect.min + Vec2::splat(60.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    assert!(app.viewport.session.gesture == Some(Gesture::Edges));
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.viewport.session.gesture == Some(Gesture::Edges));
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(
        (app.camera.yaw, app.camera.pitch, app.camera.zoom),
        (0., 0., 1.)
    );
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn edges_selection_commands_enter_picking_before_the_next_surface_click() {
    for action in [
        Action::SelectCreases,
        Action::SelectBorderEdges,
        Action::ExpandEdgeLoop,
    ] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        let mut mesh = plane_mesh();
        mesh.creases = mesh.polygon_edges();
        let seed = mesh.creases[0];
        app.document.replace_geometry(Geometry::new(mesh)).unwrap();
        let before = app.document.geometry_snapshot();
        app.viewport.set_edge_selection(vec![seed], &app.document);
        app.ui.transform_mode = crate::viewport::TransformMode::World;
        app.dispatch(Command::Action(action), &ctx);
        assert!(app.ui.edges.selecting);
        assert!(app.ui.transform_mode == crate::viewport::TransformMode::Sculpt);
        let center = navigation_frame(&mut app, &ctx, vec![]).center();
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(center),
                primary_event(center, true),
            ],
        );
        navigation_frame(&mut app, &ctx, vec![primary_event(center, false)]);
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Action(Action::SelectEdges), &ctx);
        app.dispatch(Command::Action(Action::ClearEdgeSelection), &ctx);
        assert!(
            !app.ui.edges.selecting,
            "Clearing a highlight does not enter a mode"
        );
    }
}
