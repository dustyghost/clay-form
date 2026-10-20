use super::*;
use crate::sculpt::{
    Alpha, Brush, Kind, StrokeMode,
    alpha::{Asset, Stencil},
};
fn asset() -> Asset {
    Asset {
        name: "Test alpha".into(),
        category: "Tests".into(),
        image: Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![0., 1., 0., 1.].into(),
        },
    }
}
#[test]
fn alpha_import_preview_is_private_cancellable_and_blocks_viewport_edits() {
    let dir = std::env::temp_dir().join(format!("clay-alpha-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("alpha.png");
    crate::persistence::save_alpha(path.to_str().unwrap(), &asset().image).unwrap();
    let mut app = App::default();
    let ctx = egui::Context::default();
    let before = app.document.project();
    let history = app.document.history_counts();
    app.ui.alpha_path = path.to_string_lossy().into();
    app.dispatch(Command::LoadAlpha, &ctx);
    assert!(app.ui.alphas.draft.is_some());
    assert!(app.document.project() == before);
    let rect = navigation_frame(&mut app, &ctx, vec![]);
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
            rect.center() + Vec2::new(40., 0.),
        )],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(rect.center(), false)]);
    assert!(app.document.project() == before);
    assert_eq!(app.document.history_counts(), history);
    // Modal Escape discards the draft, and has no persistent side effects.
    let _ = ctx.run(
        egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        },
        |ctx| {
            assert!(app.ui.alpha_popup(ctx).is_none());
        },
    );
    assert!(app.ui.alphas.draft.is_none());
    assert!(!app.document.is_dirty());
    app.dispatch(Command::SaveAlpha(None, asset()), &ctx);
    app.dispatch(Command::UseAlpha(0, 0), &ctx);
    app.dispatch(Command::UseAlpha(0, 1), &ctx);
    assert_eq!(app.document.brushes()[0].alpha, asset().image);
    assert_eq!(app.document.brushes()[1].alpha, asset().image);
    app.ui.alphas.export_path = path.to_string_lossy().into();
    let original = std::fs::read(&path).unwrap();
    app.dispatch(Command::ExportAlpha(0, false), &ctx);
    assert!(app.ui.alphas.overwrite.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn alpha_stencil_stamp_escape_and_attachment_escape_leave_geometry_untouched() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::SetStencil(Some(Stencil::new(asset()))), &ctx);
    let mut brush = Brush::new(Kind::Standard);
    brush.stroke_mode = StrokeMode::DragRect;
    app.document.update_brush(0, brush).unwrap();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let center = rect.center();
    app.document.mark_saved("test.clay");
    let before = app.document.project();
    let history = app.document.history_counts();
    let escape = || egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    };
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(center + Vec2::new(80., 0.))],
    );
    navigation_frame(&mut app, &ctx, vec![escape()]);
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(center + Vec2::new(80., 0.), false)],
    );
    assert!(app.document.project() == before);
    assert_eq!(app.document.history_counts(), history);
    assert!(!app.document.is_dirty());
    app.dispatch(Command::AttachStencil, &ctx);
    navigation_frame(&mut app, &ctx, vec![escape()]);
    assert!(!app.viewport.attach_stencil);
    assert!(app.document.project() == before);
    for action in [
        Action::ToggleStencil,
        Action::AttachStencil,
        Action::RemoveStencil,
    ] {
        assert!(app.ui.action_enabled(&action, &app.document));
        assert!(Action::ALL.contains(&action));
    }
    app.execute_action(Action::ToggleStencil, &ctx);
    assert!(!app.document.stencil().unwrap().enabled);
    app.execute_action(Action::RemoveStencil, &ctx);
    assert!(app.document.stencil().is_none());
    assert!(!app.ui.action_enabled(&Action::AttachStencil, &app.document));
}
