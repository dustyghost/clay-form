//! Exercise edits through command dispatch and input boundaries, not just Document.
use super::*;
use crate::sculpt::{Geometry, Primitive, PrimitiveKind};

fn frame(
    app: &mut App,
    ctx: &egui::Context,
    mut commands: Vec<Command>,
    events: Vec<egui::Event>,
    focused: bool,
) {
    let _ = ctx.run(
        egui::RawInput {
            events,
            focused,
            ..Default::default()
        },
        |ctx| {
            for command in commands.drain(..) {
                app.dispatch(command, ctx);
            }
            app.finish_parameter_input(ctx);
        },
    );
}
fn button(pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: egui::pos2(10., 10.),
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn masked_plane() -> App {
    let mut primitive = Primitive::for_kind(PrimitiveKind::Plane);
    primitive.divisions = [2, 2, 1];
    let mut mesh = primitive.mesh().unwrap();
    mesh.mask = mesh
        .vertices
        .iter()
        .map(|p| if p.0 < 0. { 0.2 } else { 0.8 })
        .collect();
    mesh.create_vertex_group("Saved weights".into(), 0.1, false)
        .unwrap();
    let mut geometry = Geometry::new(mesh);
    geometry.subdivide().unwrap();
    geometry.subdivide().unwrap();
    geometry.set_level(2).unwrap();
    let mut app = App::default();
    app.document.replace_geometry(geometry).unwrap();
    app.document.duplicate_model(false).unwrap();
    app
}
#[test]
fn parameter_mask_drag_recovers_from_zero_and_release_starts_a_new_undo_step() {
    let mut app = masked_plane();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    for (i, amount) in [0.4, 0., 0.8].into_iter().enumerate() {
        frame(
            &mut app,
            &ctx,
            vec![Command::MaskIntensity(amount)],
            if i == 0 { vec![button(true)] } else { vec![] },
            true,
        );
    }
    assert_eq!(app.document.history_counts().0, count + 1);
    let mut after = app.document.geometry_snapshot();
    for (edited, old) in std::iter::once(&mut after.mesh)
        .chain(after.settings.lower.iter_mut())
        .chain(after.settings.upper.iter_mut())
        .zip(
            std::iter::once(&before.mesh)
                .chain(before.settings.lower.iter())
                .chain(before.settings.upper.iter()),
        )
    {
        for (a, b) in edited.mask.iter().zip(&old.mask) {
            assert!((a - b).abs() < 1e-6);
        }
        edited.mask.clone_from(&old.mask);
    }
    assert_eq!(after, before, "no geometry, group or inactive-model edits");
    frame(&mut app, &ctx, vec![], vec![button(false)], true);
    frame(
        &mut app,
        &ctx,
        vec![Command::MaskIntensity(0.3)],
        vec![button(true)],
        true,
    );
    frame(&mut app, &ctx, vec![], vec![button(false)], true);
    assert_eq!(app.document.history_counts().0, count + 2);
    app.dispatch(Command::Undo, &ctx);
    assert!((app.document.mask_peak() - 0.8).abs() < 1e-6);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    app.dispatch(Command::Redo, &ctx);
    assert!((app.document.mask_peak() - 0.3).abs() < 1e-6);
}
#[test]
fn parameter_thickness_and_threshold_drags_have_one_step_each_and_focus_ends_them() {
    let mut app = masked_plane();
    let ctx = egui::Context::default();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    frame(
        &mut app,
        &ctx,
        vec![Command::SetTemporaryThickness(0.1)],
        vec![button(true)],
        true,
    );
    for v in [0.2, 0.3] {
        frame(
            &mut app,
            &ctx,
            vec![Command::SetTemporaryThickness(v)],
            vec![],
            true,
        );
    }
    assert_eq!(app.document.history_counts().0, count + 1);
    for v in [0.2, 0.4, 0.6] {
        frame(
            &mut app,
            &ctx,
            vec![Command::MaskThreshold(v)],
            vec![],
            true,
        );
    }
    assert_eq!(
        app.document.history_counts().0,
        count + 2,
        "switching controls ends the previous edit"
    );
    frame(&mut app, &ctx, vec![], vec![], false);
    frame(
        &mut app,
        &ctx,
        vec![Command::MaskThreshold(0.7)],
        vec![button(true)],
        true,
    );
    assert_eq!(app.document.history_counts().0, count + 3);
    for _ in 0..3 {
        app.dispatch(Command::Undo, &ctx);
    }
    assert_eq!(app.document.geometry_snapshot(), before);
}
#[test]
fn parameter_primitive_drag_groups_updates_and_switching_models_ends_mask_baseline() {
    let mut app = masked_plane();
    let ctx = egui::Context::default();
    app.dispatch(Command::MaskIntensity(0.), &ctx);
    app.dispatch(Command::SelectSubmodel(1), &ctx);
    app.dispatch(Command::MaskIntensity(0.4), &ctx);
    assert!((app.document.mask_peak() - 0.4).abs() < 1e-6);
    app.document.initialize_primitive().unwrap();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    let mut primitive = app.document.primitive().unwrap().clone();
    for i in 0..3 {
        primitive.center.0 += 0.1;
        frame(
            &mut app,
            &ctx,
            vec![Command::UpdatePrimitive(primitive.clone())],
            if i == 0 { vec![button(true)] } else { vec![] },
            true,
        );
    }
    assert_eq!(app.document.history_counts().0, count + 1);
    frame(&mut app, &ctx, vec![], vec![button(false)], true);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}
#[test]
fn parameter_noop_and_empty_clear_preserve_redo_and_clean_state() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.invert_mask().unwrap();
    app.document.undo();
    app.document.mark_saved("unchanged.clay");
    let before = app.document.geometry_snapshot();
    for command in [
        Command::ClearMask,
        Command::MaskIntensity(0.),
        Command::MaskThreshold(0.5),
        Command::SetTemporaryThickness(0.),
    ] {
        app.dispatch(command, &ctx);
        assert_eq!(app.document.history_counts(), (0, 1));
        assert_eq!(app.document.geometry_snapshot(), before);
        assert!(!app.document.is_dirty());
    }
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.mask_peak(), 1.);
}
#[test]
fn parameter_clear_checks_hidden_masks_on_stored_levels_and_undo_restores_them() {
    let mut app = masked_plane();
    let ctx = egui::Context::default();
    let mut geometry = app.document.geometry_snapshot();
    geometry.mesh.mask.fill(0.);
    for level in geometry
        .settings
        .lower
        .iter_mut()
        .chain(geometry.settings.upper.iter_mut())
    {
        level.hidden_faces = vec![true; level.triangles.len()];
    }
    app.document.replace_geometry(geometry).unwrap();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    app.dispatch(Command::ClearMask, &ctx);
    assert_eq!(app.document.mask_peak(), 0.);
    assert_eq!(app.document.history_counts().0, count + 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}
