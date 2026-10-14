//! Controller/input regressions for the shared transform controls.
use super::*;
use crate::{
    sculpt::{Primitive, PrimitiveKind, Vec3},
    viewport::{TransformMode as Mode, TransformOperation as Op},
};

fn app() -> App {
    let mut app = App::default();
    let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
    primitive.divisions = [1; 3];
    primitive.size = Vec3(1.2, 0.8, 0.6);
    let mut project = app.document.project();
    project.mesh = primitive.mesh().unwrap();
    app.document
        .replace_project(project, "rotation.clay")
        .unwrap();
    app
}
fn frame(
    app: &mut App,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    focused: bool,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            events,
            focused,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    )
}
fn arc(center: Pos2, angle: f32) -> Pos2 {
    center + Vec2::new(angle.cos(), -angle.sin()) * 85.
}
fn quarter_turn(app: &mut App, ctx: &egui::Context, center: Pos2) {
    let start = arc(center, std::f32::consts::FRAC_PI_4);
    navigation_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    assert!(app.viewport.gesture == Some(Gesture::Transform));
    let middle = arc(center, std::f32::consts::FRAC_PI_2);
    let output = frame(app, ctx, vec![egui::Event::PointerMoved(middle)], true);
    assert!(
        output
            .shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.text() == "+45.0°"))
    );
    // The last movement exists only on release, and must still be applied.
    let end = arc(center, 3. * std::f32::consts::FRAC_PI_4);
    navigation_frame(app, ctx, vec![primary_event(end, false)]);
}
fn expect_quarter_turn(before: &[Vec3], after: &[Vec3], pivot: Vec3) {
    for (&p, &q) in before.iter().zip(after) {
        let r = p - pivot;
        assert!(
            (q - (pivot + Vec3(-r.1, r.0, r.2))).len() < 1e-5,
            "{p:?} -> {q:?}, pivot {pivot:?}"
        );
    }
}
#[test]
fn rotation_world_ring_has_angle_feedback_one_undo_and_separate_camera_orbit() {
    let mut app = app();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::Action(Action::Rotate), &ctx);
    assert_eq!(app.ui.transform_mode, Mode::World);
    quarter_turn(&mut app, &ctx, rect.center());
    expect_quarter_turn(
        &before.mesh.vertices,
        &app.document.mesh().vertices,
        Vec3::default(),
    );
    assert_eq!(app.document.history_counts(), (1, 0));
    assert_eq!((app.camera.yaw, app.camera.pitch), (0., 0.));
    let rotated = app.document.geometry_snapshot();
    let start = Pos2::new(900., 600.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start + Vec2::new(20., 15.))],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert!(app.camera.yaw.abs() > 0.1);
    assert_eq!(app.document.geometry_snapshot(), rotated);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), rotated);
}
#[test]
fn rotation_pivot_fast_placement_preserves_redo_survives_operations_and_resets_to_center() {
    let mut app = app();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    app.document.invert_mask().unwrap();
    app.document.undo();
    app.document.mark_saved("rotation.clay");
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::Action(Action::PlacePivot), &ctx);
    let pos = rect.center() + Vec2::new(-220., 140.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(pos),
            primary_event(pos, true),
            primary_event(pos, false),
        ],
    );
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), (0, 1));
    assert!(!app.document.is_dirty());
    app.dispatch(Command::TransformOperation(Op::Scale), &ctx);
    app.dispatch(Command::TransformOperation(Op::Rotate), &ctx);
    quarter_turn(&mut app, &ctx, pos);
    let scale = rect.width().min(rect.height()) * 0.35;
    let pivot = Vec3(-220. / scale, -140. / scale, 0.);
    expect_quarter_turn(&before.mesh.vertices, &app.document.mesh().vertices, pivot);
    let rotated = app.document.geometry_snapshot();
    app.dispatch(Command::Action(Action::ResetPivot), &ctx);
    assert_eq!(app.document.geometry_snapshot(), rotated);
    assert_eq!(app.document.history_counts(), (1, 0));
    let center = rotated
        .mesh
        .vertices
        .iter()
        .fold(Vec3::default(), |a, &p| a + p)
        * (1. / rotated.mesh.vertices.len() as f32);
    let projected = rect.center() + Vec2::new(center.0, -center.1) * scale;
    quarter_turn(&mut app, &ctx, projected);
    expect_quarter_turn(
        &rotated.mesh.vertices,
        &app.document.mesh().vertices,
        center,
    );
    assert_eq!(app.document.history_counts(), (2, 0));
}
#[test]
fn rotation_action_line_reuses_its_start_and_direction_without_moving_the_line() {
    let mut app = app();
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::TransformMode(Mode::ActionLine), &ctx);
    let start = rect.center() + Vec2::new(-50., 0.);
    let end = rect.center() + Vec2::new(50., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::TransformOperation(Op::Rotate), &ctx);
    let press = start + Vec2::new(0., -50.);
    let release = press + Vec2::new(0., 42.5);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(press), primary_event(press, true)],
    );
    assert!(app.viewport.gesture == Some(Gesture::Transform));
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(release)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(release, false)]);
    assert_eq!(app.document.history_counts(), (1, 0));
    assert_ne!(app.document.geometry_snapshot(), before);
    for (&p, &q) in before
        .mesh
        .vertices
        .iter()
        .zip(&app.document.mesh().vertices)
    {
        assert!((q.0 - p.0).abs() < 1e-6);
        let distance = |p: Vec3| (p.1 * p.1 + (p.2 - 0.3).powi(2)).sqrt();
        assert!((distance(p) - distance(q)).abs() < 1e-5);
    }
    assert_eq!((app.camera.yaw, app.camera.pitch), (0., 0.));
    let center_before = app
        .document
        .mesh()
        .vertices
        .iter()
        .fold(Vec3::default(), |sum, &p| sum + p)
        * (1. / app.document.mesh().vertices.len() as f32);
    let placed = app.document.geometry_snapshot();
    app.dispatch(Command::Action(Action::ResetPivot), &ctx);
    assert_eq!(app.document.geometry_snapshot(), placed);
    let scale = rect.width().min(rect.height()) * 0.35;
    let press =
        rect.center() + Vec2::new(center_before.0, -center_before.1) * scale + Vec2::new(0., -50.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(press), primary_event(press, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(press + Vec2::new(0., 20.), false)],
    );
    assert_eq!(app.document.history_counts(), (2, 0));
    let center_after = app
        .document
        .mesh()
        .vertices
        .iter()
        .fold(Vec3::default(), |sum, &p| sum + p)
        * (1. / app.document.mesh().vertices.len() as f32);
    assert!(
        (center_after - center_before).len() < 1e-5,
        "reset line rotates around model center"
    );
    for (&p, &q) in placed
        .mesh
        .vertices
        .iter()
        .zip(&app.document.mesh().vertices)
    {
        assert!((p.0 - q.0).abs() < 1e-6, "reset keeps the line direction");
    }
    app.dispatch(Command::Undo, &ctx);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}
#[test]
fn rotation_focus_camera_and_operation_changes_finish_drag_and_wait_for_release() {
    for interruption in 0..3 {
        let mut app = app();
        let ctx = egui::Context::default();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        app.dispatch(Command::TransformOperation(Op::Rotate), &ctx);
        let start = arc(rect.center(), 0.6);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(arc(rect.center(), 1.))],
        );
        let before = app.document.geometry_snapshot();
        assert_eq!(app.document.history_counts(), (1, 0));
        if interruption == 1 {
            frame(&mut app, &ctx, vec![], false);
        } else if interruption == 2 {
            app.dispatch(Command::ZoomIn, &ctx);
        } else {
            app.dispatch(Command::TransformOperation(Op::Scale), &ctx);
        }
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(arc(rect.center(), 2.))],
        );
        navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), (1, 0));
    }
}
#[test]
fn rotation_actions_roundtrip_in_preferences_and_obey_background_job_guard() {
    use crate::preferences::Shortcut;
    let mut app = app();
    let ctx = egui::Context::default();
    for (action, key) in [
        (Action::Rotate, egui::Key::F6),
        (Action::PlacePivot, egui::Key::F7),
        (Action::ResetPivot, egui::Key::F8),
    ] {
        app.ui
            .preferences
            .bind(action, Shortcut::new(key, egui::Modifiers::NONE))
            .unwrap();
    }
    app.ui.preferences =
        serde_json::from_slice(&serde_json::to_vec(&app.ui.preferences).unwrap()).unwrap();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::F6,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| app.shortcuts(ctx),
    );
    assert_eq!(app.ui.transform_operation, Op::Rotate);
    app.dispatch(Command::Subdivide, &ctx);
    for action in [Action::Rotate, Action::PlacePivot, Action::ResetPivot] {
        assert!(!app.ui.action_enabled(&action, &app.document));
        app.dispatch(Command::Action(action), &ctx);
        assert_eq!(app.ui.transform_operation, Op::Rotate);
    }
    app.dispatch(Command::CancelMeshJob, &ctx);
}

#[test]
fn rotation_shared_controls_report_rejected_transforms_without_changing_geometry_or_history() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.document.initialize_primitive().unwrap();
    let mut p = app.document.primitive().unwrap().clone();
    p.basis = p.basis.map(|v| v * 0.0002);
    app.document.update_primitive(p).unwrap();
    app.document.end_primitive_edit();
    app.document.mark_saved("small.clay");
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    app.dispatch(Command::TransformOperation(Op::Scale), &ctx);
    let press = rect.center();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(press), primary_event(press, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(press - Vec2::new(250., 0.))],
    );
    assert!(
        app.ui.status.contains("Primitive placement"),
        "{}",
        app.ui.status
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(press, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    assert!(!app.document.is_dirty());
}
