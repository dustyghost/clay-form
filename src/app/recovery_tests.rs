use super::*;
use crate::recovery::{
    self,
    tests::{Directory, document},
};

fn drain(app: &mut App, ctx: &egui::Context) {
    let start = std::time::Instant::now();
    while app.recovery.as_ref().unwrap().busy {
        app.tick_recovery(ctx);
        assert!(start.elapsed().as_secs() < 5);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn recovery_waits_for_edits_restores_dirty_and_requires_unsaved_decision() {
    let directory = Directory::new();
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.initialize_recovery(directory.0.clone());
    assert!(recovery::entries(&directory.0).unwrap().is_empty());
    assert!(!app.recovery_ready(&ctx));
    app.document = document();
    app.document
        .add_reference(crate::reference::tests::image())
        .unwrap();
    app.document.begin_stroke();
    assert!(!app.recovery_ready(&ctx));
    app.document.end_stroke();
    app.dispatch(Command::MaskIntensity(0.5), &ctx);
    assert!(!app.recovery_ready(&ctx));
    app.document.end_edit();
    assert!(app.recovery_ready(&ctx));
    let original = app.document.project();
    let history = app.document.history_counts();
    app.save_recovery(&ctx);
    drain(&mut app, &ctx);
    assert!(app.document.project() == original);
    assert_eq!(app.document.history_counts(), history);
    assert!(app.document.is_dirty());
    assert_eq!(app.document.current_path(), "sculpt.clay");
    let entry = recovery::entries(&directory.0).unwrap().remove(0);
    // Simulate restart after an interruption, without retiring the old worker.
    drop(app);
    let mut restarted = App::default();
    restarted.initialize_recovery(directory.0.clone());
    assert!(restarted.ui.recovery_open);
    assert!(!restarted.document.is_dirty());
    restarted.execute_pending(PendingAction::Recover(entry.clone()), &ctx);
    assert!(restarted.document.project() == original);
    assert!(restarted.document.is_dirty());
    assert_eq!(restarted.document.current_path(), "recovered.clay");
    assert_eq!(restarted.ui.path, "recovered.clay");
    assert!(!restarted.document.can_undo());
    assert!(
        entry.path.exists(),
        "keep recovered copy until save or discard"
    );
    restarted.request(PendingAction::New, &ctx);
    assert!(restarted.ui.pending.is_some());
    restarted.dispatch(Command::CancelPending, &ctx);
    assert!(entry.path.exists());
    let saved = directory.0.join("restored.clay");
    assert!(restarted.save_to(&saved.to_string_lossy()));
    drop(restarted);
    assert!(recovery::entries(&directory.0).unwrap().is_empty());
    assert!(persistence::load_project(&saved.to_string_lossy()).unwrap() == original);
}
#[test]
fn invalid_recovery_keeps_current_document_and_visible_error() {
    let directory = Directory::new();
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.initialize_recovery(directory.0.clone());
    app.document = document();
    app.save_recovery(&ctx);
    drain(&mut app, &ctx);
    let entry = recovery::entries(&directory.0).unwrap().remove(0);
    let before = app.document.project();
    std::fs::write(&entry.path, b"{broken").unwrap();
    app.execute_pending(PendingAction::Recover(entry), &ctx);
    assert!(app.document.project() == before);
    assert!(app.recovery_error.is_some());
    assert!(app.ui.recovery_open);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires an installed EGL OpenGL implementation"]
fn recovery_and_menu_placement_offscreen_preview() {
    use crate::{
        preferences::{BuiltinMenu, Dock, Tool},
        viewport::UiCapture,
    };
    let mut capture = UiCapture::new([1200, 850]);
    let ctx = egui::Context::default();
    ctx.style_mut(|style| style.animation_time = 0.);
    let directory = Directory::new();
    let mut app = App {
        document: document(),
        ..Default::default()
    };
    app.initialize_recovery(directory.0.clone());
    app.viewport.initialize_renderer(&capture.gl).unwrap();
    for panel in &mut app.ui.preferences.panels {
        panel.dock = Dock::Hidden;
    }
    app.ui
        .preferences
        .dock_menu(BuiltinMenu::Transform, Dock::Left);
    app.ui.preferences.pin_menu(BuiltinMenu::Mask, [820., 170.]);
    app.ui
        .preferences
        .pin_menu(BuiltinMenu::Slice, [300., 580.]);
    app.dispatch(
        Command::TransformMode(crate::viewport::TransformMode::World),
        &ctx,
    );
    assert!(
        app.ui
            .preferences
            .panels
            .iter()
            .find(|p| p.tool == Tool::Transform)
            .unwrap()
            .dock
            == Dock::Hidden
    );
    app.save_recovery(&ctx);
    drain(&mut app, &ctx);
    for step in 0..6 {
        if step == 3 {
            app.open_recovery();
        }
        let output = ctx.run(
            egui::RawInput {
                time: Some(step as f64),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200., 850.),
                )),
                ..Default::default()
            },
            |ctx| {
                let _ = app.ui.toolbar(ctx, &app.document, app.camera.zoom);
                let _ = app
                    .ui
                    .brush_panel(ctx, &app.document, "Preview", 1., app.camera.zoom);
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                app.show_recovery(ctx);
            },
        );
        let rgba = capture.render(&ctx, output);
        if step == 2 {
            capture.save("/tmp/clay-menu-placement.png", &rgba);
        }
        if step == 5 {
            capture.save("/tmp/clay-recovery-dialog.png", &rgba);
        }
    }
    app.viewport.destroy_renderer(&capture.gl);
}
