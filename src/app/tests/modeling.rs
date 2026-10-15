use super::*;
use crate::{
    sculpt::{Geometry, Primitive, PrimitiveKind},
    viewport::{ComponentMode, ModelTool, SelectionChange},
};
fn setup() -> (App, egui::Context, egui::Rect) {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut p = Primitive::for_kind(PrimitiveKind::Cube);
    p.divisions = [3; 3];
    app.document
        .replace_geometry(Geometry::new(p.mesh().unwrap()))
        .unwrap();
    app.camera.yaw = 0.;
    app.camera.pitch = 0.;
    app.dispatch(Command::ModelingMode(true), &ctx);
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    (app, ctx, rect)
}
fn tap(app: &mut App, ctx: &egui::Context, p: Pos2) {
    navigation_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(p), primary_event(p, true)],
    );
    navigation_frame(app, ctx, vec![primary_event(p, false)]);
}
#[test]
fn modeling_face_edge_vertex_selection_and_groups_are_explicit() {
    let (mut app, ctx, rect) = setup();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    tap(&mut app, &ctx, rect.center());
    assert_eq!(app.ui.modeling.count, 1);
    app.ui.modeling.change = SelectionChange::Add;
    tap(&mut app, &ctx, rect.center() + Vec2::new(100., 0.));
    assert_eq!(app.ui.modeling.count, 2);
    app.ui.modeling.change = SelectionChange::Remove;
    tap(&mut app, &ctx, rect.center() + Vec2::new(100., 0.));
    assert_eq!(app.ui.modeling.count, 1);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::GroupModelSelection(None, false), &ctx);
    assert_eq!(
        app.document.mesh().vertex_groups[0]
            .weights
            .iter()
            .filter(|&&w| w == 1.)
            .count(),
        4
    );
    assert_eq!(app.document.mesh().mask, before.mesh.mask);
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    app.dispatch(Command::GroupModelSelection(Some(0), true), &ctx);
    assert!(
        app.document.mesh().vertex_groups[0]
            .weights
            .iter()
            .all(|&w| w == 0.)
    );
    let scale = rect.width().min(rect.height()) * 0.35;
    app.dispatch(Command::ModelingComponent(ComponentMode::Edge), &ctx);
    app.ui.modeling.change = SelectionChange::Replace;
    tap(&mut app, &ctx, rect.center() + Vec2::new(scale / 3., 0.));
    assert_eq!(app.ui.modeling.count, 1);
    app.dispatch(Command::ModelSelectLoop, &ctx);
    assert!(app.viewport.selected_edges(&app.document).len() > 1);
    app.dispatch(Command::ModelingComponent(ComponentMode::Vertex), &ctx);
    tap(
        &mut app,
        &ctx,
        rect.center() + Vec2::new(scale / 3., scale / 3.),
    );
    assert_eq!(app.ui.modeling.count, 1);
    app.dispatch(Command::GroupModelSelection(None, false), &ctx);
    assert_eq!(
        app.document.mesh().vertex_groups[1]
            .weights
            .iter()
            .filter(|&&w| w == 1.)
            .count(),
        1
    );
}
#[test]
fn modeling_extrusion_previews_cancel_commit_and_undo_once() {
    for perspective in [false, true] {
        let (mut app, ctx, rect) = setup();
        if perspective {
            app.dispatch(Command::SetPerspective(true), &ctx);
        }
        app.ui.modeling.tool = ModelTool::Extrude;
        let before = app.document.geometry_snapshot();
        let history = app.document.history_counts();
        let start = rect.center();
        let end = start + Vec2::new(0., -70.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert!(app.viewport.modeling.preview().is_some());
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), history);
        navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Escape, true)]);
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                primary_event(end, false),
                slice_key(egui::Key::Escape, false),
            ],
        );
        assert!(app.ui.modeling.active);
        assert_eq!(app.document.geometry_snapshot(), before);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_ne!(
            app.document.geometry_snapshot(),
            before,
            "{}",
            app.ui.status
        );
        assert_eq!(app.document.history_counts().0, history.0 + 1);
        let after = app.document.geometry_snapshot();
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Redo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), after);
        let project: crate::sculpt::Project =
            serde_json::from_slice(&serde_json::to_vec(&app.document.project()).unwrap()).unwrap();
        project.validate().unwrap();
    }
}
#[test]
fn modeling_loop_preview_commits_and_navigation_does_not_sculpt() {
    let (mut app, ctx, rect) = setup();
    app.dispatch(Command::ModelingComponent(ComponentMode::Edge), &ctx);
    app.ui.modeling.tool = ModelTool::LoopCut;
    let before = app.document.geometry_snapshot();
    let scale = rect.width().min(rect.height()) * 0.35;
    let point = rect.center() + Vec2::new(scale / 3., 0.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(point)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    tap(&mut app, &ctx, point);
    assert!(
        app.document.mesh().vertices.len() > before.mesh.vertices.len(),
        "{}",
        app.ui.status
    );
    let after = app.document.geometry_snapshot();
    let yaw = app.camera.yaw;
    let start = rect.left_center() + Vec2::new(10., 0.);
    let end = start + Vec2::new(60., 10.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_ne!(app.camera.yaw, yaw);
    assert_eq!(app.document.geometry_snapshot(), after);
}
#[test]
fn modeling_stale_selection_and_disabled_preview_cannot_commit() {
    let (mut app, ctx, rect) = setup();
    tap(&mut app, &ctx, rect.center());
    app.dispatch(Command::Undo, &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    assert_eq!(app.ui.modeling.count, 0);
    let (mut app, ctx, rect) = setup();
    app.ui.modeling.tool = ModelTool::Extrude;
    let before = app.document.geometry_snapshot();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(rect.center()),
            primary_event(rect.center(), true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(
            rect.center() + Vec2::new(0., -40.),
        )],
    );
    app.ui.settings_open = true;
    navigation_frame(&mut app, &ctx, vec![primary_event(rect.center(), false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.viewport.modeling.dragging());
}

#[test]
fn modeling_partial_quad_group_preserves_quad_labels_and_masks() {
    let (mut app, _, _) = setup();
    let quad = app.document.mesh().quads[0];
    let before = app.document.mesh().mask.clone();
    app.document
        .group_model_selection(&quad[..3], "Corners", None, false)
        .unwrap();
    app.document.mesh().validate().unwrap();
    assert_eq!(
        app.document.mesh().vertex_groups[0]
            .weights
            .iter()
            .filter(|&&w| w > 0.)
            .count(),
        3
    );
    assert_eq!(app.document.mesh().mask, before);
}

#[test]
fn modeling_lost_pointer_discards_draft_and_shortcut_keeps_mode_through_undo() {
    let (mut app, ctx, rect) = setup();
    app.dispatch(Command::ModelingMode(false), &ctx);
    app.ui
        .preferences
        .bind(
            Action::PenModeling,
            crate::preferences::Shortcut::new(egui::Key::F8, egui::Modifiers::NONE),
        )
        .unwrap();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![slice_key(egui::Key::F8, true)],
            ..Default::default()
        },
        |ctx| app.shortcuts(ctx),
    );
    assert!(app.ui.modeling.active);
    app.ui.modeling.tool = ModelTool::Extrude;
    let before = app.document.geometry_snapshot();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(rect.center()),
            primary_event(rect.center(), true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(
            rect.center() + Vec2::new(0., -40.),
        )],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
    assert!(!app.viewport.modeling.dragging());
    navigation_frame(&mut app, &ctx, vec![primary_event(rect.center(), false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Undo, &ctx);
    assert!(app.ui.modeling.active);
}

#[test]
fn modeling_loop_drag_slides_counts_cancels_and_commits_once() {
    let (mut app, ctx, rect) = setup();
    app.dispatch(Command::Action(Action::ModelLoopCut), &ctx);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let scale = rect.width().min(rect.height()) * 0.35;
    let start = rect.center() + Vec2::new(scale / 3., 15.);
    tap(&mut app, &ctx, start);
    let centered = app.document.mesh().clone();
    assert!(centered.vertices.len() > before.mesh.vertices.len());
    app.dispatch(Command::Undo, &ctx);
    let end = start + Vec2::new(0., 30.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.viewport.modeling.dragging());
    assert_eq!(app.document.geometry_snapshot(), before);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_ne!(app.document.mesh().vertices, centered.vertices);
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    app.dispatch(Command::Undo, &ctx);
    let zoom = app.camera.zoom;
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    let wheel = |y| egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0., y),
        modifiers: egui::Modifiers::NONE,
    };
    for _ in 0..3 {
        navigation_frame(&mut app, &ctx, vec![wheel(30.)]);
    }
    navigation_frame(&mut app, &ctx, vec![wheel(-30.)]);
    assert_eq!(app.ui.modeling.loop_count, 3);
    assert_eq!(app.camera.zoom, zoom);
    assert_eq!(app.document.geometry_snapshot(), before);
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert_eq!(
        app.document.mesh().vertices.len() - before.mesh.vertices.len(),
        3 * (centered.vertices.len() - before.mesh.vertices.len())
    );
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    app.dispatch(Command::Undo, &ctx);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Escape, true)]);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            primary_event(end, false),
            slice_key(egui::Key::Escape, false),
        ],
    );
    assert!(app.ui.modeling.active);
    assert!(!app.viewport.modeling.dragging());
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[test]
fn modeling_custom_actions_activate_tools_and_preserve_group_selection() {
    let (mut app, ctx, rect) = setup();
    tap(&mut app, &ctx, rect.center());
    app.dispatch(Command::Action(Action::ModelExtrude), &ctx);
    assert_eq!(app.ui.modeling.tool, ModelTool::Extrude);
    assert_eq!(app.viewport.modeling.selected.len(), 1);
    app.dispatch(Command::Action(Action::ModelCreateGroup), &ctx);
    assert_eq!(app.document.mesh().vertex_groups.len(), 1);
    app.dispatch(Command::Action(Action::ModelRemoveFromGroup), &ctx);
    assert!(
        app.document.mesh().vertex_groups[0]
            .weights
            .iter()
            .all(|&w| w == 0.)
    );
    app.dispatch(Command::Action(Action::ModelAddToGroup), &ctx);
    assert_eq!(
        app.document.mesh().vertex_groups[0]
            .weights
            .iter()
            .filter(|&&w| w == 1.)
            .count(),
        4
    );
    app.dispatch(Command::ModelingMode(false), &ctx);
    app.dispatch(Command::Action(Action::ModelLoopCut), &ctx);
    assert!(app.ui.modeling.active);
    assert_eq!(app.ui.modeling.mode, ComponentMode::Edge);
    assert_eq!(app.ui.modeling.tool, ModelTool::LoopCut);
    assert_eq!(app.viewport.modeling.selected.len(), 4); // Face selection converts to its edges.
    app.dispatch(Command::Action(Action::ModelMoreLoops), &ctx);
    assert_eq!(app.ui.modeling.loop_count, 2);
    app.dispatch(Command::Action(Action::ModelFewerLoops), &ctx);
    assert_eq!(app.ui.modeling.loop_count, 1);
    app.dispatch(Command::Action(Action::ModelVertices), &ctx);
    assert_eq!(app.ui.modeling.mode, ComponentMode::Vertex);
    assert_eq!(app.ui.modeling.tool, ModelTool::Select);
    app.dispatch(Command::Action(Action::ModelAdd), &ctx);
    assert_eq!(app.ui.modeling.change, SelectionChange::Add);
    app.ui.settings_open = true;
    app.dispatch(Command::Action(Action::ModelExtrude), &ctx);
    assert_eq!(app.ui.modeling.mode, ComponentMode::Vertex);
}

fn selection_grid() -> (App, egui::Context, egui::Rect) {
    let (mut app, ctx, _) = setup();
    let mut p = Primitive::for_kind(PrimitiveKind::Grid);
    p.divisions = [5, 5, 1];
    app.document
        .replace_geometry(Geometry::new(p.mesh().unwrap()))
        .unwrap();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    (app, ctx, rect)
}
fn pen_drag(app: &mut App, ctx: &egui::Context, start: Pos2, end: Pos2) {
    navigation_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(app, ctx, vec![primary_event(end, false)]);
}
#[test]
fn modeling_drag_selects_crossed_faces_once_and_cancels_without_geometry_edits() {
    let (mut app, ctx, rect) = selection_grid();
    let scale = rect.width().min(rect.height()) * 0.35;
    let p = |x: f32, y: f32| rect.center() + Vec2::new(x * scale, -y * scale);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    tap(&mut app, &ctx, p(0., 0.8));
    let original = app.viewport.modeling.selected.clone();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(p(-0.8, 0.)),
            primary_event(p(-0.8, 0.), true),
        ],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(p(0.8, 0.))]);
    assert_eq!(
        app.ui.modeling.count, 5,
        "Fast drag must collect the intermediate faces"
    );
    assert_eq!(app.viewport.modeling.selected, original);
    navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Escape, true)]);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            primary_event(p(0.8, 0.), false),
            slice_key(egui::Key::Escape, false),
        ],
    );
    assert_eq!(app.viewport.modeling.selected, original);
    assert!(app.ui.modeling.active);
    pen_drag(&mut app, &ctx, p(-0.8, 0.), p(0.8, 0.));
    assert_eq!(app.ui.modeling.count, 5, "Replace clears only at the start");
    app.ui.modeling.change = SelectionChange::Add;
    pen_drag(&mut app, &ctx, p(-0.8, 0.4), p(0.8, 0.4));
    assert_eq!(app.ui.modeling.count, 10);
    app.ui.modeling.change = SelectionChange::Remove;
    pen_drag(&mut app, &ctx, p(0., 0.), p(0., 0.4));
    assert_eq!(app.ui.modeling.count, 8);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    let original = app.viewport.modeling.selected.clone();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(p(-0.8, 0.)),
            primary_event(p(-0.8, 0.), true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(p(0.8, 0.)),
            egui::Event::PointerGone,
        ],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(p(0.8, 0.), false)]);
    assert_eq!(app.viewport.modeling.selected, original);
}
#[test]
fn modeling_selection_keeps_coalesced_pen_path_and_paints_edges_and_vertices() {
    let (mut app, ctx, rect) = selection_grid();
    let scale = rect.width().min(rect.height()) * 0.35;
    let p = |x: f32, y: f32| rect.center() + Vec2::new(x * scale, -y * scale);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(p(-0.8, 0.)),
            primary_event(p(-0.8, 0.), true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(p(0.8, 0.)),
            egui::Event::PointerMoved(p(0.8, 0.8)),
        ],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(p(0.8, 0.8), false)]);
    assert_eq!(app.ui.modeling.count, 7);
    app.dispatch(Command::Action(Action::ModelEdges), &ctx);
    pen_drag(&mut app, &ctx, p(-0.6, 0.), p(0.6, 0.));
    assert_eq!(app.ui.modeling.count, 4);
    assert_eq!(app.viewport.selected_edges(&app.document).len(), 4);
    app.dispatch(Command::Action(Action::ModelVertices), &ctx);
    pen_drag(&mut app, &ctx, p(-0.6, 0.2), p(0.6, 0.2));
    assert_eq!(app.ui.modeling.count, 4);
}
#[test]
fn modeling_face_loop_direction_and_parallel_edge_selection_are_explicit() {
    let (mut app, ctx, rect) = selection_grid();
    let scale = rect.width().min(rect.height()) * 0.35;
    let p = |x: f32, y: f32| rect.center() + Vec2::new(x * scale, -y * scale);
    app.dispatch(Command::Action(Action::ModelSelectLoops), &ctx);
    tap(&mut app, &ctx, p(0.18, 0.));
    let row = app.viewport.modeling.selected.clone();
    assert_eq!(row.len(), 5);
    tap(&mut app, &ctx, p(0., 0.18));
    assert_eq!(app.ui.modeling.count, 5);
    assert_eq!(row.intersection(&app.viewport.modeling.selected).count(), 1);
    app.dispatch(Command::Action(Action::ModelSelectRing), &ctx);
    tap(&mut app, &ctx, p(0.2, 0.));
    let ring = app.viewport.selected_edges(&app.document).to_vec();
    assert_eq!(ring.len(), 6);
    app.dispatch(Command::Action(Action::ModelSelectLoops), &ctx);
    tap(&mut app, &ctx, p(0.2, 0.));
    let line = app.viewport.selected_edges(&app.document);
    assert_eq!(line.len(), 5);
    assert_eq!(line.iter().filter(|e| ring.contains(e)).count(), 1);
    app.dispatch(Command::Action(Action::ModelSelectSingle), &ctx);
    tap(&mut app, &ctx, p(0.2, 0.));
    assert_eq!(app.ui.modeling.count, 1);
}

#[test]
fn modeling_symmetry_selects_groups_and_dissolves_atomically_with_undo() {
    let (mut app, ctx, rect) = selection_grid();
    let scale = rect.width().min(rect.height()) * 0.35;
    let p = |x: f32, y: f32| rect.center() + Vec2::new(x * scale, -y * scale);
    app.dispatch(
        Command::SetSymmetry(crate::sculpt::Symmetry {
            enabled: true,
            ..Default::default()
        }),
        &ctx,
    );
    tap(&mut app, &ctx, p(0.4, 0.));
    assert_eq!(app.ui.modeling.count, 2);
    app.dispatch(Command::Action(Action::ModelCreateGroup), &ctx);
    assert_eq!(
        app.document.mesh().vertex_groups[0]
            .weights
            .iter()
            .filter(|&&w| w > 0.)
            .count(),
        8
    );
    app.dispatch(Command::Undo, &ctx);
    app.dispatch(Command::Action(Action::ModelEdges), &ctx);
    app.dispatch(Command::Action(Action::ModelSelectLoops), &ctx);
    tap(&mut app, &ctx, p(0.2, 0.));
    assert_eq!(app.ui.modeling.count, 10);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.dispatch(Command::Action(Action::ModelDissolveLoops), &ctx);
    assert_eq!(
        (
            app.document.mesh().vertices.len(),
            app.document.mesh().quads.len()
        ),
        (24, 15),
        "{}",
        app.ui.status
    );
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    assert_eq!(app.document.undo_label(), Some("Dissolve selected loops"));
    assert!(app.viewport.selected_edges(&app.document).is_empty());
    let after = app.document.geometry_snapshot();
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
    app.document.project().validate().unwrap();
}
#[test]
fn modeling_symmetry_extrudes_both_sides_and_mismatch_blocks_edits() {
    let (mut app, ctx, rect) = setup();
    let scale = rect.width().min(rect.height()) * 0.35;
    let p = rect.center() + Vec2::new(scale * 2. / 3., 0.);
    app.dispatch(
        Command::SetSymmetry(crate::sculpt::Symmetry {
            enabled: true,
            ..Default::default()
        }),
        &ctx,
    );
    app.dispatch(Command::Action(Action::ModelExtrude), &ctx);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    pen_drag(&mut app, &ctx, p, p + Vec2::new(0., -30.));
    assert_eq!(
        app.document.mesh().quads.len(),
        before.mesh.quads.len() + 8,
        "{}",
        app.ui.status
    );
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    crate::sculpt::ModelingSymmetry::new(app.document.mesh(), app.document.symmetry()).unwrap();
    app.dispatch(Command::Undo, &ctx);
    let mut geometry = app.document.geometry_snapshot();
    geometry.mesh.vertices[0].0 += 0.1;
    app.document.replace_geometry(geometry).unwrap();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    pen_drag(&mut app, &ctx, p, p + Vec2::new(0., -30.));
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::Action(Action::ModelSelect), &ctx);
    tap(&mut app, &ctx, p);
    assert_eq!(app.ui.modeling.count, 0);
}
#[test]
fn modeling_dissolve_partial_loop_is_atomic_and_keeps_selection() {
    let (mut app, ctx, rect) = selection_grid();
    app.dispatch(Command::Action(Action::ModelEdges), &ctx);
    let scale = rect.width().min(rect.height()) * 0.35;
    tap(&mut app, &ctx, rect.center() + Vec2::new(scale * 0.2, 0.));
    let selection = app.viewport.modeling.selected.clone();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    app.dispatch(Command::Action(Action::ModelDissolveLoops), &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert_eq!(app.viewport.modeling.selected, selection);
    assert!(app.ui.status.contains("complete"), "{}", app.ui.status);
}

#[test]
fn modeling_symmetry_change_expands_existing_selection_and_cancels_held_preview() {
    let (mut app, ctx, rect) = selection_grid();
    let scale = rect.width().min(rect.height()) * 0.35;
    app.dispatch(Command::Action(Action::ModelEdges), &ctx);
    tap(&mut app, &ctx, rect.center() + Vec2::new(scale * 0.2, 0.));
    assert_eq!(app.viewport.selected_edges(&app.document).len(), 1);
    app.dispatch(
        Command::SetSymmetry(crate::sculpt::Symmetry {
            enabled: true,
            ..Default::default()
        }),
        &ctx,
    );
    navigation_frame(&mut app, &ctx, vec![]);
    assert_eq!(app.ui.modeling.count, 2);
    assert_eq!(app.viewport.selected_edges(&app.document).len(), 2);
    app.dispatch(Command::Action(Action::ModelLoopCut), &ctx);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let start = rect.center() + Vec2::new(0., scale * 0.2);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start + Vec2::new(10., 0.))],
    );
    assert!(app.viewport.modeling.dragging());
    app.dispatch(
        Command::SetSymmetry(crate::sculpt::Symmetry::default()),
        &ctx,
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert!(!app.viewport.modeling.dragging());
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
}

#[test]
fn modeling_shared_targets_conversion_and_measured_move() {
    use crate::viewport::{ModelAxis, SelectionSpan};
    let (mut app, ctx, rect) = setup();
    app.ui.modeling.span = SelectionSpan::Flat;
    tap(&mut app, &ctx, rect.center());
    assert_eq!(app.viewport.modeling.selected.len(), 9);
    app.dispatch(Command::ModelBoundary, &ctx);
    assert_eq!(app.viewport.modeling.selected.len(), 12);
    app.dispatch(Command::ModelingComponent(ComponentMode::Vertex), &ctx);
    assert_eq!(app.viewport.modeling.selected.len(), 12);
    app.ui.modeling.tool = ModelTool::Move;
    app.ui.modeling.axis = ModelAxis::Z;
    app.ui.modeling.amount = 0.1;
    let before = app.document.mesh().clone();
    let history = app.document.history_counts().0;
    app.dispatch(Command::ModelApply, &ctx);
    assert_ne!(app.document.mesh(), &before);
    assert_eq!(app.viewport.modeling.selected.len(), 12);
    assert_eq!(app.document.history_counts().0, history + 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.mesh(), &before);
}
#[test]
fn modeling_component_drag_is_transactional() {
    let (mut app, ctx, rect) = setup();
    app.ui.modeling.tool = ModelTool::Move;
    let before = app.document.mesh().clone();
    let p = rect.center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(p), primary_event(p, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(p + Vec2::new(0., -30.))],
    );
    assert_eq!(app.document.mesh(), &before);
    assert_ne!(app.viewport.modeling.preview().unwrap(), &before);
    navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Escape, true)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(p, false)]);
    assert_eq!(app.document.mesh(), &before);
    pen_drag(&mut app, &ctx, p, p + Vec2::new(0., -30.));
    assert_ne!(app.document.mesh(), &before);
    assert!(!app.viewport.modeling.selected.is_empty());
}
#[test]
fn modeling_construction_numeric_and_pen_share_valid_results() {
    for tool in [ModelTool::Inset, ModelTool::Bevel, ModelTool::Fuse] {
        let (mut app, ctx, rect) = setup();
        let mut p = Primitive::for_kind(PrimitiveKind::Cube);
        p.divisions = [1; 3];
        app.document
            .replace_geometry(Geometry::new(p.mesh().unwrap()))
            .unwrap();
        navigation_frame(&mut app, &ctx, vec![]);
        tap(&mut app, &ctx, rect.center());
        app.dispatch(Command::Action(Action::ModelTool(tool)), &ctx);
        app.ui.modeling.amount = 0.1;
        let before = app.document.mesh().clone();
        let history = app.document.history_counts().0;
        app.dispatch(Command::ModelApply, &ctx);
        assert_ne!(
            app.document.mesh(),
            &before,
            "{}: {}",
            tool.label(),
            app.ui.status
        );
        assert_eq!(app.document.history_counts().0, history + 1);
        if tool != ModelTool::Fuse {
            assert_eq!(app.viewport.modeling.selected.len(), 1);
        }
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.mesh(), &before);
        pen_drag(
            &mut app,
            &ctx,
            rect.center(),
            rect.center() + Vec2::new(0., -20.),
        );
        assert_ne!(
            app.document.mesh(),
            &before,
            "{}: {}",
            tool.label(),
            app.ui.status
        );
    }
}
#[test]
fn modeling_topology_commands_are_atomic_and_undoable() {
    use crate::sculpt::TopologyEdit;
    let (mut app, ctx, rect) = setup();
    tap(&mut app, &ctx, rect.center());
    let before = app.document.mesh().clone();
    let history = app.document.history_counts().0;
    app.dispatch(Command::ModelTopology(TopologyEdit::DeleteFaces), &ctx);
    assert_eq!(
        app.document.mesh().polygons().len(),
        before.polygons().len() - 1
    );
    assert_eq!(app.document.history_counts().0, history + 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.mesh(), &before);
    navigation_frame(&mut app, &ctx, vec![]);
    tap(&mut app, &ctx, rect.center());
    app.dispatch(Command::ModelTopology(TopologyEdit::Bridge), &ctx);
    assert_eq!(app.document.mesh(), &before);
    assert_eq!(app.viewport.modeling.selected.len(), 1);
}
#[test]
fn modeling_presets_roundtrip_repeat_and_smooth_preview_do_not_lose_the_cage() {
    use crate::viewport::ModelAxis;
    let (mut app, ctx, rect) = setup();
    tap(&mut app, &ctx, rect.center());
    app.dispatch(Command::Action(Action::ModelTool(ModelTool::Move)), &ctx);
    app.ui.modeling.axis = ModelAxis::Z;
    app.ui.modeling.amount = 0.1;
    app.ui.modeling.preset_name = "Measured lift".into();
    let original = app.document.mesh().clone();
    app.dispatch(Command::Action(Action::ModelSavePreset), &ctx);
    let id = app.ui.modeling.preset.unwrap();
    let serialized = serde_json::to_string(&app.ui.preferences).unwrap();
    let prefs: crate::preferences::Preferences = serde_json::from_str(&serialized).unwrap();
    prefs.validate().unwrap();
    app.ui.preferences = prefs;
    app.ui.modeling.amount = 0.9;
    app.dispatch(Command::Action(Action::ModelPreset(id)), &ctx);
    assert_eq!(app.ui.modeling.amount, 0.1);
    assert_eq!(app.document.mesh(), &original);
    app.dispatch(Command::ModelApply, &ctx);
    let once = app.document.mesh().clone();
    app.ui.modeling.amount = 0.8;
    app.dispatch(Command::ModelRepeat, &ctx);
    let twice = app.document.mesh().clone();
    assert_eq!(app.ui.modeling.amount, 0.1);
    for v in app.viewport.modeling.selected_vertices(&twice) {
        assert!((twice.vertices[v].2 - once.vertices[v].2 - 0.1).abs() < 1e-5);
    }
    navigation_frame(&mut app, &ctx, vec![]);
    let history = app.document.history_counts();
    app.ui.modeling.amount = 0.;
    app.dispatch(Command::ModelApply, &ctx);
    tap(&mut app, &ctx, rect.center());
    assert_eq!(app.viewport.modeling.last_edit.unwrap().amount, 0.1);
    assert_eq!(app.document.history_counts(), history);
    app.ui.modeling.smooth_preview = true;
    navigation_frame(&mut app, &ctx, vec![]);
    assert_eq!(app.document.mesh(), &twice);
    assert_eq!(app.document.history_counts(), history);
    assert!(app.viewport.modeling.preview().unwrap().quads.len() > twice.quads.len());
    app.dispatch(Command::ModelingMode(false), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    assert!(app.viewport.modeling.preview().is_none());
    app.dispatch(Command::Action(Action::ModelDeletePreset(id)), &ctx);
    app.dispatch(Command::ModelingMode(true), &ctx);
    app.dispatch(Command::Action(Action::ModelSavePreset), &ctx);
    assert!(app.ui.modeling.preset.unwrap() > id);
}
#[test]
fn modeling_tool_amounts_are_remembered_without_brush_size() {
    let (mut app, ctx, _) = setup();
    app.dispatch(Command::Action(Action::ModelTool(ModelTool::Inset)), &ctx);
    app.ui.modeling.amount = 0.07;
    app.dispatch(Command::Action(Action::ModelTool(ModelTool::Scale)), &ctx);
    assert_eq!(app.ui.modeling.amount, 1.);
    app.ui.modeling.amount = 1.3;
    app.dispatch(Command::Action(Action::ModelTool(ModelTool::Inset)), &ctx);
    assert_eq!(app.ui.modeling.amount, 0.07);
    app.dispatch(Command::Action(Action::ModelTool(ModelTool::Scale)), &ctx);
    assert_eq!(app.ui.modeling.amount, 1.3);
}

#[test]
fn modeling_taps_apply_visible_settings_like_numeric_edits() {
    use crate::viewport::ModelAxis;
    for tool in [
        ModelTool::Extrude,
        ModelTool::Inset,
        ModelTool::Bevel,
        ModelTool::Fuse,
        ModelTool::Move,
        ModelTool::Rotate,
        ModelTool::Scale,
    ] {
        let (mut app, ctx, rect) = setup();
        let mut p = Primitive::for_kind(PrimitiveKind::Cube);
        p.divisions = [1; 3];
        app.document
            .replace_geometry(Geometry::new(p.mesh().unwrap()))
            .unwrap();
        navigation_frame(&mut app, &ctx, vec![]);
        tap(&mut app, &ctx, rect.center());
        app.dispatch(Command::Action(Action::ModelTool(tool)), &ctx);
        app.ui.modeling.axis = if tool == ModelTool::Scale {
            ModelAxis::Uniform
        } else {
            ModelAxis::Normal
        };
        app.ui.modeling.amount = match tool {
            ModelTool::Scale => 1.1,
            ModelTool::Rotate => 10.,
            _ => 0.08,
        };
        let before = app.document.mesh().clone();
        app.dispatch(Command::ModelApply, &ctx);
        let expected = app.document.mesh().clone();
        assert_ne!(expected, before, "{}: {}", tool.label(), app.ui.status);
        app.dispatch(Command::Undo, &ctx);
        navigation_frame(&mut app, &ctx, vec![]);
        let history = app.document.history_counts().0;
        // A small amount of pen jitter still counts as a tap.
        pen_drag(
            &mut app,
            &ctx,
            rect.center(),
            rect.center() + Vec2::new(1., 1.),
        );
        assert_eq!(
            app.document.mesh(),
            &expected,
            "{}: {}",
            tool.label(),
            app.ui.status
        );
        assert_eq!(app.document.history_counts().0, history + 1);
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.mesh(), &before);
    }
}

#[test]
fn modeling_tap_repeat_retargets_remembers_amount_and_respects_cancellation() {
    let (mut app, ctx, rect) = setup();
    let scale = rect.width().min(rect.height()) * 0.35;
    let center = rect.center();
    let next = center + Vec2::new(scale * 2. / 3., 0.);
    app.dispatch(Command::Action(Action::ModelExtrude), &ctx);
    pen_drag(&mut app, &ctx, center, center + Vec2::new(0., -30.));
    let amount = app.ui.modeling.amount;
    let after_drag = app.document.mesh().clone();
    app.dispatch(Command::Action(Action::ModelSelect), &ctx);
    tap(&mut app, &ctx, next);
    assert_eq!(app.document.mesh(), &after_drag);
    app.dispatch(Command::Action(Action::ModelExtrude), &ctx);
    assert_eq!(app.ui.modeling.amount, amount);
    let selected = app.viewport.modeling.selected_vertices(app.document.mesh());
    tap(&mut app, &ctx, next);
    let after_tap = app.document.mesh().clone();
    assert_eq!(after_tap.quads.len(), after_drag.quads.len() + 4);
    // Original vertices stay put, and each new cap corner is displaced by the remembered amount.
    for v in selected {
        let target = after_drag.vertices[v] + crate::sculpt::Vec3(0., 0., amount);
        assert!(
            after_tap
                .vertices
                .iter()
                .any(|p| (*p - target).len() < 1e-5)
        );
    }
    let history = app.document.history_counts();
    app.dispatch(Command::Action(Action::ModelTapRepeat), &ctx);
    assert!(!app.ui.modeling.tap_repeat);
    tap(&mut app, &ctx, next);
    assert_eq!(app.document.mesh(), &after_tap);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::Action(Action::ModelTapRepeat), &ctx);
    navigation_frame(&mut app, &ctx, vec![primary_event(next, true)]);
    navigation_frame(&mut app, &ctx, vec![slice_key(egui::Key::Escape, true)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(next, false)]);
    assert_eq!(app.document.mesh(), &after_tap);
    assert_eq!(app.document.history_counts(), history);
    // A real drag returning to its origin is a zero-distance drag, not a repeat tap.
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            slice_key(egui::Key::Escape, false),
            primary_event(next, true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(next + Vec2::new(0., -30.))],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(next)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(next, false)]);
    assert_eq!(app.document.mesh(), &after_tap);
    assert_eq!(app.document.history_counts(), history);
}

#[test]
fn modeling_repeat_taps_obey_symmetry_and_protection() {
    let (mut app, ctx, rect) = setup();
    let scale = rect.width().min(rect.height()) * 0.35;
    let p = rect.center() + Vec2::new(scale * 2. / 3., 0.);
    app.dispatch(
        Command::SetSymmetry(crate::sculpt::Symmetry {
            enabled: true,
            ..Default::default()
        }),
        &ctx,
    );
    app.dispatch(Command::Action(Action::ModelExtrude), &ctx);
    let before = app.document.mesh().clone();
    let history = app.document.history_counts().0;
    tap(&mut app, &ctx, p);
    assert_eq!(app.document.mesh().quads.len(), before.quads.len() + 8);
    assert_eq!(app.document.history_counts().0, history + 1);
    crate::sculpt::ModelingSymmetry::new(app.document.mesh(), app.document.symmetry()).unwrap();
    app.dispatch(Command::Undo, &ctx);
    let mut geometry = app.document.geometry_snapshot();
    geometry.mesh.mask.fill(1.);
    app.document.replace_geometry(geometry).unwrap();
    let protected = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    tap(&mut app, &ctx, p);
    assert_eq!(app.document.geometry_snapshot(), protected);
    assert_eq!(app.document.history_counts(), history);
}

#[test]
fn modeling_presets_store_tap_repeat_and_accept_older_settings() {
    let (mut app, ctx, _) = setup();
    let mut old = serde_json::to_value(app.ui.modeling.options()).unwrap();
    old.as_object_mut().unwrap().remove("tap_repeat");
    let restored: crate::viewport::ModelingOptions = serde_json::from_value(old).unwrap();
    assert!(restored.tap_repeat);
    app.ui.modeling.tap_repeat = false;
    app.ui.modeling.preset_name = "Drag only".into();
    app.dispatch(Command::Action(Action::ModelSavePreset), &ctx);
    let id = app.ui.modeling.preset.unwrap();
    app.ui.modeling.tap_repeat = true;
    app.dispatch(Command::Action(Action::ModelPreset(id)), &ctx);
    assert!(!app.ui.modeling.tap_repeat);
}
