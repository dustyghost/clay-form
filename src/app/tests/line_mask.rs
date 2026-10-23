//! Through-depth line masking uses the actual viewport event routing.
use super::*;
use crate::viewport::SelectionTool;

const START: Pos2 = Pos2::new(500., 100.);
const END: Pos2 = Pos2::new(500., 740.);
fn begin(app: &mut App, ctx: &egui::Context, erase: bool, flip: bool) {
    app.dispatch(
        Command::SelectTool(Some(SelectionTool::MaskLine { erase, flip })),
        ctx,
    );
    navigation_frame(app, ctx, vec![]);
    navigation_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(START), primary_event(START, true)],
    );
    navigation_frame(app, ctx, vec![egui::Event::PointerMoved(END)]);
    assert!(app.viewport.gesture == Some(Gesture::Select));
}

#[test]
fn line_mask_projects_both_depths_preserves_geometry_and_commits_once() {
    for perspective in [false, true] {
        for flip in [false, true] {
            for erase in [false, true] {
                let mut app = App::default();
                app.camera.perspective = perspective;
                app.camera.yaw = 0.7;
                app.camera.pitch = 0.3;
                if erase {
                    app.document.invert_mask().unwrap();
                }
                app.document.duplicate_model(false).unwrap();
                let before = app.document.geometry_snapshot();
                let count = app.document.history_counts().0;
                let ctx = egui::Context::default();
                begin(&mut app, &ctx, erase, flip);
                assert_eq!(app.document.geometry_snapshot(), before);
                navigation_frame(&mut app, &ctx, vec![primary_event(END, false)]);
                let after = app.document.geometry_snapshot();
                let mut restored = after.clone();
                restored.mesh.mask.clone_from(&before.mesh.mask);
                assert_eq!(restored, before, "mask weights are the only changed data");
                let (right, _, view) = app.camera.basis();
                for (i, &p) in before.mesh.vertices.iter().enumerate() {
                    let affected = if flip {
                        p.dot(right) >= 0.
                    } else {
                        p.dot(right) <= 0.
                    };
                    let expected = if affected {
                        if erase { 0. } else { 1. }
                    } else {
                        before.mesh.mask[i]
                    };
                    assert_eq!(
                        after.mesh.mask[i], expected,
                        "vertex {i}, perspective {perspective}, flip {flip}"
                    );
                }
                for sign in [-1., 1.] {
                    assert!(
                        after
                            .mesh
                            .vertices
                            .iter()
                            .enumerate()
                            .any(|(i, p)| p.dot(view) * sign > 0.3
                                && after.mesh.mask[i] != before.mesh.mask[i])
                    );
                }
                assert_eq!(app.document.history_counts().0, count + 1);
                assert_eq!(app.camera.yaw, 0.7);
                assert_eq!(app.camera.pitch, 0.3);
                assert_eq!(app.camera.zoom, 1.);
                assert_eq!(
                    app.document.undo_label(),
                    Some(if erase {
                        "Erase mask line"
                    } else {
                        "Mask line"
                    })
                );
                let json = serde_json::to_string(&app.document.project()).unwrap();
                let mut loaded = Document::default();
                loaded
                    .replace_project(serde_json::from_str(&json).unwrap(), "line.clay")
                    .unwrap();
                assert_eq!(loaded.geometry_snapshot(), after);
                app.dispatch(Command::Undo, &ctx);
                assert_eq!(app.document.geometry_snapshot(), before);
                app.dispatch(Command::Redo, &ctx);
                assert_eq!(app.document.geometry_snapshot(), after);
            }
        }
    }
}

#[test]
fn line_mask_interruptions_and_empty_side_preserve_history() {
    for interruption in [
        "escape",
        "tool",
        "toggle_off",
        "focus",
        "popup",
        "menu",
        "pointer_gone",
        "no_hit",
        "tap",
    ] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.document.invert_mask().unwrap();
        app.document.undo();
        app.document.mark_saved("line.clay");
        let before = app.document.geometry_snapshot();
        begin(&mut app, &ctx, false, false);
        match interruption {
            "escape" => {
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
            }
            "toggle_off" => app.dispatch(Command::Action(Action::MaskLine), &ctx),
            "tool" => app.dispatch(Command::SelectTool(Some(SelectionTool::MaskLasso)), &ctx),
            "focus" => {
                let _ = ctx.run(
                    egui::RawInput {
                        focused: false,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                    },
                );
            }
            "menu" => {
                egui::Popup::open_id(&ctx, egui::Id::new("line-test-menu"));
                navigation_frame(&mut app, &ctx, vec![]);
                egui::Popup::close_all(&ctx);
            }
            "popup" => {
                app.ui.settings_open = true;
                navigation_frame(&mut app, &ctx, vec![]);
                app.ui.settings_open = false;
            }
            "pointer_gone" => {
                navigation_frame(&mut app, &ctx, vec![egui::Event::PointerGone]);
            }
            "no_hit" => {
                // Cancel, then draw outside the left silhouette with the outward side selected.
                app.dispatch(Command::SelectTool(None), &ctx);
                navigation_frame(&mut app, &ctx, vec![primary_event(END, false)]);
                app.dispatch(Command::Action(Action::MaskLine), &ctx);
                navigation_frame(
                    &mut app,
                    &ctx,
                    vec![
                        primary_event(Pos2::new(100., 100.), true),
                        egui::Event::PointerMoved(Pos2::new(100., 740.)),
                        primary_event(Pos2::new(100., 740.), false),
                    ],
                );
            }
            "tap" => {
                navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(START)]);
            }
            _ => unreachable!(),
        }
        let release = if interruption == "tap" { START } else { END };
        navigation_frame(&mut app, &ctx, vec![primary_event(release, false)]);
        assert_eq!(app.document.geometry_snapshot(), before, "{interruption}");
        assert_eq!(app.document.history_counts(), (0, 1), "{interruption}");
        assert!(!app.document.is_dirty());
        assert_eq!(app.camera.yaw, 0.);
    }
}

#[test]
fn line_mask_fast_pen_samples_flip_and_remapped_navigation() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    begin(&mut app, &ctx, false, false);
    app.ui
        .preferences
        .bind(
            Action::FlipMaskLine,
            crate::preferences::Shortcut::new(egui::Key::F9, egui::Modifiers::NONE),
        )
        .unwrap();
    let json = serde_json::to_string(&app.ui.preferences).unwrap();
    app.ui.preferences = serde_json::from_str(&json).unwrap();
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::F9,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| app.shortcuts(ctx),
    );
    assert!(matches!(
        app.ui.selection_tool,
        Some(SelectionTool::MaskLine { flip: true, .. })
    ));
    app.ui.preferences.clear_binding(&Action::FlipMaskLine);
    let json = serde_json::to_string(&app.ui.preferences).unwrap();
    app.ui.preferences = serde_json::from_str(&json).unwrap();
    assert!(app.ui.preferences.shortcut(&Action::FlipMaskLine).is_none());
    navigation_frame(&mut app, &ctx, vec![primary_event(END, false)]);
    let flipped = app.document.geometry_snapshot();
    app.document.undo();
    app.dispatch(
        Command::SelectTool(Some(SelectionTool::MaskLine {
            erase: false,
            flip: true,
        })),
        &ctx,
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            primary_event(START, true),
            egui::Event::PointerMoved(END),
            primary_event(END, false),
        ],
    );
    assert_eq!(
        app.document.geometry_snapshot(),
        flipped,
        "batched pen samples match live preview and flip"
    );
    for pan in [false, true] {
        let before = app.document.geometry_snapshot();
        if pan {
            app.ui.preferences.pan_hold = crate::preferences::Hold::Alt;
        } else {
            app.ui.preferences.zoom_hold = crate::preferences::Hold::Alt;
        }
        visibility_frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![egui::Event::PointerMoved(START), primary_event(START, true)],
        );
        assert!(app.viewport.gesture == Some(Gesture::Orbit));
        visibility_frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![egui::Event::PointerMoved(START + Vec2::new(40., 40.))],
        );
        visibility_frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![primary_event(START + Vec2::new(40., 40.), false)],
        );
        assert_eq!(app.document.geometry_snapshot(), before);
        if pan {
            assert_ne!(app.camera.pan, Vec2::ZERO);
        } else {
            assert_ne!(app.camera.zoom, 1.);
            app.ui.preferences.zoom_hold = crate::preferences::Hold::Ctrl;
        }
    }
}

#[test]
fn line_mask_off_center_uses_perspective_projection_at_each_vertex_depth() {
    let mut app = App::default();
    app.camera.perspective = true;
    app.camera.yaw = 0.5;
    let ctx = egui::Context::default();
    app.dispatch(Command::Action(Action::MaskLine), &ctx);
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let x = rect.center().x + 70.;
    let start = Pos2::new(x, 100.);
    let end = Pos2::new(x, 740.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            primary_event(start, true),
            egui::Event::PointerMoved(end),
            primary_event(end, false),
        ],
    );
    let right = app.camera.basis().0;
    let mut differs_from_ortho = false;
    for (i, &p) in app.document.mesh().vertices.iter().enumerate() {
        let projected_x = rect.center().x + p.dot(right) * app.camera.scale_at(rect.size(), p);
        let ortho_x = rect.center().x + p.dot(right) * rect.size().min_elem() * 0.35;
        assert_eq!(
            app.document.mesh().mask[i],
            if projected_x <= x { 1. } else { 0. }
        );
        differs_from_ortho |= (projected_x <= x) != (ortho_x <= x);
    }
    assert!(
        differs_from_ortho,
        "fixture must distinguish perspective projection from a parallel plane"
    );
}
