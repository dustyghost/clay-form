use super::*;
use crate::preferences::{VisibilitySettings, VisibilityShape};

fn tool(app: &mut App, ctx: &egui::Context, shape: VisibilityShape, hide: bool, flip: bool) {
    let settings = VisibilitySettings { shape, hide, flip };
    app.ui.preferences.visibility_selection = settings;
    app.dispatch(Command::SelectTool(Some(settings.tool())), ctx);
    navigation_frame(app, ctx, vec![]);
}

#[test]
fn visibility_line_preserves_topology_attributes_and_roundtrips_undo() {
    for perspective in [false, true] {
        for hide in [false, true] {
            for flip in [false, true] {
                let mut app = App::default();
                let ctx = egui::Context::default();
                app.camera.perspective = perspective;
                app.camera.yaw = 0.4;
                app.document.invert_mask().unwrap();
                app.document.duplicate_model(false).unwrap();
                tool(&mut app, &ctx, VisibilityShape::Line, hide, flip);
                let rect = navigation_frame(&mut app, &ctx, vec![]);
                let before = app.document.geometry_snapshot();
                let count = app.document.history_counts().0;
                let start = Pos2::new(560., 100.);
                let end = Pos2::new(560., 740.);
                navigation_frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(start), primary_event(start, true)],
                );
                navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
                assert_eq!(
                    app.document.geometry_snapshot(),
                    before,
                    "preview never edits"
                );
                navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
                let after = app.document.geometry_snapshot();
                let mut restored = after.clone();
                restored
                    .mesh
                    .hidden_faces
                    .clone_from(&before.mesh.hidden_faces);
                assert_eq!(
                    restored, before,
                    "only visibility changes on active submodel"
                );
                assert!(after.mesh.hidden_face_count() > 0);
                assert!(after.mesh.hidden_face_count() < after.mesh.triangles.len());
                let right = app.camera.basis().0;
                let selected = before.mesh.face_region_selection(|p| {
                    let x = rect.center().x + p.dot(right) * app.camera.scale_at(rect.size(), p);
                    (x <= start.x) != flip
                });
                for (i, selected) in selected.into_iter().enumerate() {
                    assert_eq!(after.mesh.face_visible(i), selected != hide);
                }
                assert_eq!(app.document.history_counts().0, count + 1);
                let json = serde_json::to_string(&app.document.project()).unwrap();
                let mut loaded = Document::default();
                loaded
                    .replace_project(serde_json::from_str(&json).unwrap(), "visibility.clay")
                    .unwrap();
                assert_eq!(loaded.geometry_snapshot(), after);
                app.document.undo();
                assert_eq!(app.document.geometry_snapshot(), before);
                app.document.redo();
                assert_eq!(app.document.geometry_snapshot(), after);
                app.document.show_all_faces().unwrap();
                assert_eq!(app.document.geometry_snapshot(), before);
            }
        }
    }
}

fn add_point(app: &mut App, ctx: &egui::Context, pos: Pos2) {
    navigation_frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(pos),
            primary_event(pos, true),
            primary_event(pos, false),
        ],
    );
}

#[test]
fn visibility_path_edits_preview_then_commits_once_and_cancels() {
    for cancel in [false, true] {
        let mut app = App::default();
        let ctx = egui::Context::default();
        tool(&mut app, &ctx, VisibilityShape::Path, true, false);
        let before = app.document.geometry_snapshot();
        let count = app.document.history_counts().0;
        for p in [
            Pos2::new(400., 230.),
            Pos2::new(680., 230.),
            Pos2::new(680., 600.),
            Pos2::new(400., 600.),
        ] {
            add_point(&mut app, &ctx, p);
        }
        let start = Pos2::new(680., 230.);
        let end = Pos2::new(590., 280.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_eq!(app.document.geometry_snapshot(), before);
        if cancel {
            app.camera.yaw += 0.1;
            navigation_frame(&mut app, &ctx, vec![]);
        }
        app.dispatch(Command::Action(Action::ApplyVisibilityPath), &ctx);
        navigation_frame(&mut app, &ctx, vec![]);
        if cancel {
            assert_eq!(app.document.geometry_snapshot(), before);
            assert_eq!(app.document.history_counts().0, count);
        } else {
            assert!(app.document.mesh().hidden_face_count() > 0);
            assert_eq!(app.document.history_counts().0, count + 1);
            app.document.undo();
            assert_eq!(app.document.geometry_snapshot(), before);
        }
    }
}

#[test]
fn visibility_new_choices_and_bindings_persist_and_clear() {
    let mut preferences = crate::preferences::Preferences::default();
    preferences.visibility_selection = VisibilitySettings {
        shape: VisibilityShape::Path,
        hide: true,
        flip: true,
    };
    for action in [
        Action::SelectVisibilityLine,
        Action::SelectVisibilityPath,
        Action::VisibilityHide,
        Action::VisibilityIsolate,
        Action::FlipVisibilitySide,
        Action::ApplyVisibilityPath,
        Action::ClearVisibilityPath,
        Action::DeleteVisibilityPoint,
    ] {
        preferences
            .bind(
                action.clone(),
                crate::preferences::Shortcut::new(egui::Key::F8, egui::Modifiers::ALT),
            )
            .unwrap();
        let json = serde_json::to_string(&preferences).unwrap();
        preferences = serde_json::from_str(&json).unwrap();
        assert_eq!(preferences.shortcut(&action).unwrap().key, "F8");
        preferences.clear_binding(&action);
        let json = serde_json::to_string(&preferences).unwrap();
        preferences = serde_json::from_str(&json).unwrap();
        assert!(preferences.shortcut(&action).is_none());
    }
    assert_eq!(
        preferences.visibility_selection.shape,
        VisibilityShape::Path
    );
    assert!(preferences.visibility_selection.hide && preferences.visibility_selection.flip);
}

#[test]
fn visibility_alt_hold_keeps_explicit_operation_and_cancels_every_shape_on_release() {
    for shape in VisibilityShape::ALL {
        let mut app = App::default();
        let ctx = egui::Context::default();
        app.ui.preferences.visibility_hold = crate::preferences::HoldBinding {
            alt: true,
            ..Default::default()
        };
        app.ui.preferences.visibility_selection = VisibilitySettings {
            shape,
            hide: false,
            flip: false,
        };
        let original_tool = app.ui.selection_tool;
        let before = app.document.geometry_snapshot();
        navigation_frame(&mut app, &ctx, vec![]);
        let start = Pos2::new(500., 300.);
        let end = Pos2::new(680., 650.);
        visibility_frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        visibility_frame(
            &mut app,
            &ctx,
            egui::Modifiers::ALT,
            vec![egui::Event::PointerMoved(end)],
        );
        if shape == VisibilityShape::Path {
            visibility_frame(
                &mut app,
                &ctx,
                egui::Modifiers::ALT,
                vec![primary_event(end, false)],
            );
            for point in [Pos2::new(400., 400.), Pos2::new(600., 400.)] {
                visibility_frame(
                    &mut app,
                    &ctx,
                    egui::Modifiers::ALT,
                    vec![
                        egui::Event::PointerMoved(point),
                        primary_event(point, true),
                        primary_event(point, false),
                    ],
                );
            }
        }
        visibility_frame(&mut app, &ctx, egui::Modifiers::NONE, vec![]);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_eq!(
            app.document.geometry_snapshot(),
            before,
            "hold release cancels {shape:?}"
        );
        assert_eq!(app.document.history_counts(), (0, 0));
        assert_eq!(app.ui.selection_tool, original_tool);
        if shape == VisibilityShape::Path {
            visibility_frame(&mut app, &ctx, egui::Modifiers::ALT, vec![]);
            app.dispatch(Command::Action(Action::ApplyVisibilityPath), &ctx);
            visibility_frame(&mut app, &ctx, egui::Modifiers::ALT, vec![]);
            assert_eq!(
                app.document.geometry_snapshot(),
                before,
                "old path must not reappear"
            );
        }
    }
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.ui.preferences.visibility_hold = crate::preferences::HoldBinding {
        alt: true,
        ..Default::default()
    };
    app.ui.preferences.visibility_selection = VisibilitySettings {
        shape: VisibilityShape::Line,
        hide: false,
        flip: false,
    };
    navigation_frame(&mut app, &ctx, vec![]);
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::ALT,
        vec![
            egui::Event::PointerMoved(Pos2::new(500., 100.)),
            primary_event(Pos2::new(500., 100.), true),
        ],
    );
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::ALT,
        vec![
            egui::Event::PointerMoved(Pos2::new(500., 740.)),
            primary_event(Pos2::new(500., 740.), false),
        ],
    );
    assert_eq!(
        app.document.undo_label(),
        Some("Isolate selection"),
        "Alt used as hold never switches Isolate to Hide"
    );
    assert_eq!(app.camera.pan, Vec2::ZERO);
    assert_eq!(app.camera.zoom, 1.);
}
