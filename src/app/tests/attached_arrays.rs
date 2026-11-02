use super::*;
use crate::{
    command::InstanceCommand as C,
    sculpt::{Geometry, Primitive, PrimitiveKind, instances::Layout},
};
fn setup() -> (App, egui::Context) {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut p = Primitive::for_kind(PrimitiveKind::Cube);
    p.divisions = [1; 3];
    app.document
        .replace_geometry(Geometry::new(p.mesh().unwrap()))
        .unwrap();
    app.dispatch(Command::Instances(C::NewArray), &ctx);
    for _ in 0..3 {
        navigation_frame(&mut app, &ctx, vec![]);
    }
    (app, ctx)
}
fn handle(ctx: &egui::Context, name: &str) -> Pos2 {
    ctx.data(|d| d.get_temp::<Vec<(&'static str, Pos2)>>(egui::Id::new("array_inline_handles")))
        .unwrap()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}
fn spacing(app: &App) -> f32 {
    let Layout::Array(a) = &app.document.attached_array(0).unwrap().layout else {
        panic!()
    };
    a.spacing.0
}
#[test]
fn attached_array_inline_drag_release_hover_escape_and_undo_are_one_edit() {
    let (mut app, ctx) = setup();
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts().0;
    let p = handle(&ctx, "X spacing");
    let end = p + egui::vec2(50., 0.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(p), primary_event(p, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(spacing(&app) > 1.05);
    assert_eq!(app.document.history_counts().0, history);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            primary_event(end, false),
            egui::Event::PointerMoved(end + egui::vec2(130., 0.)),
        ],
    );
    let after = app.document.geometry_snapshot();
    assert_eq!(app.document.history_counts().0, history + 1);
    assert!(!app.document.instance_preview_active());
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(p)]);
    assert_eq!(app.document.geometry_snapshot(), after);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
    navigation_frame(&mut app, &ctx, vec![]);
    let p = handle(&ctx, "X spacing");
    let history = app.document.history_counts();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(p), primary_event(p, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(p + egui::vec2(40., 0.))],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(p, false)]);
    assert_eq!(app.document.geometry_snapshot(), after);
    assert_eq!(app.document.history_counts(), history);
}
#[test]
fn attached_array_modes_navigation_copy_handles_and_custom_controls_follow_owner() {
    let (mut app, ctx) = setup();
    assert_eq!(Action::ArrayControls.label(), "Active array controls");
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let empty = rect.left_bottom() + egui::vec2(50., -70.);
    let yaw = app.camera.yaw;
    let history = app.document.history_counts();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(empty), primary_event(empty, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(empty + egui::vec2(30., 10.))],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(empty + egui::vec2(30., 10.), false)],
    );
    assert_ne!(app.camera.yaw, yaw);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::Instances(C::ArrayMode(true)), &ctx);
    assert!(app.document.editing_array_source());
    assert!(app.document.require_sculpt_mesh().is_ok());
    app.dispatch(Command::Instances(C::ArrayMode(false)), &ctx);
    assert!(!app.document.editing_array_source());
    assert!(app.document.require_sculpt_mesh().is_err());
    let id = app.document.attached_array(0).unwrap().id;
    app.dispatch(Command::Instances(C::EditCopy(id, 1)), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    let p = ctx
        .data(|d| d.get_temp::<Vec<(&'static str, Pos2)>>(egui::Id::new("instance_move_handles")))
        .unwrap()
        .into_iter()
        .find(|(n, _)| *n == "X")
        .unwrap()
        .1;
    let before = app.document.instance_placements().unwrap();
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(p), primary_event(p, true)],
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(p + egui::vec2(20., 0.))],
    );
    assert_ne!(
        app.document.instance_placements().unwrap()[1],
        before[1],
        "copy drag starts at {p:?}; status: {}",
        app.ui.status
    );
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            primary_event(p + egui::vec2(20., 0.), false),
            egui::Event::PointerMoved(p + egui::vec2(180., 0.)),
        ],
    );
    let after = app.document.instance_placements().unwrap();
    assert_eq!(after[0], before[0]);
    assert_ne!(after[1], before[1]);
    assert!((after[1].2.transform.origin - before[1].2.transform.origin).len() < 1.);
    navigation_frame(&mut app, &ctx, vec![]);
    assert!(
        ctx.data(
            |d| d.get_temp::<Vec<(&'static str, Pos2)>>(egui::Id::new("instance_move_handles"))
        )
        .is_some()
    );
    app.dispatch(Command::Instances(C::ArrayDone), &ctx);
    assert!(app.document.attached_array(0).is_some());
    assert!(app.document.selected_instance.is_none());
}
