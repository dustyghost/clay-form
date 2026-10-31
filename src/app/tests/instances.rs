use super::*;
use crate::{
    command::InstanceCommand as C,
    sculpt::{Geometry, Mesh, Primitive, PrimitiveKind, Vec3},
};
fn setup() -> (App, egui::Context) {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [1; 3];
    app.document
        .replace_geometry(Geometry::new(cube.mesh().unwrap()))
        .unwrap();
    app.dispatch(
        Command::Instances(C::Capture("Stud".into(), Vec3(0., 0., -1.), 0, 2)),
        &ctx,
    );
    let mut g = app.document.geometry_snapshot();
    g.mesh = Mesh::starting_sphere();
    app.document.replace_geometry(g).unwrap();
    app.dispatch(Command::Instances(C::StartBrush(1)), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    (app, ctx)
}
#[test]
fn reusable_object_tool_resets_when_changing_projects() {
    let (mut app, ctx) = setup();
    assert!(app.viewport.object_brush.is_some());
    app.execute_pending(crate::command::PendingAction::New, &ctx);
    assert!(app.viewport.object_brush.is_none());
    assert!(app.ui.instances.asset.is_none());
    assert!(app.document.instances().is_empty());
}
#[test]
fn reusable_objects_real_pointer_stroke_and_cancel_are_one_transaction() {
    let (mut app, ctx) = setup();
    let before = app.document.project();
    let count = app.document.history_counts().0;
    let start = Pos2::new(450., 400.);
    let end = Pos2::new(550., 400.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    assert!(app.document.instance_preview_active(), "{}", app.ui.status);
    assert!(app.document.project() == before);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(!app.document.instance_preview_active());
    assert_eq!(app.document.history_counts().0, count + 1);
    assert!(app.document.instance_placements().unwrap().len() >= 2);
    assert_eq!(app.document.mesh(), &before.mesh);
    app.document.undo();
    assert!(app.document.project() == before);
    navigation_frame(&mut app, &ctx, vec![]);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
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
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert!(app.document.project() == before);
    assert!(!app.document.instance_preview_active());
}
#[test]
fn reusable_array_preview_blocks_sculpt_and_supports_custom_actions() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    navigation_frame(&mut app, &ctx, vec![]);
    let before = app.document.project();
    app.dispatch(Command::Action(Action::CreateInstanceArray), &ctx);
    assert!(app.document.instance_preview_active(), "{}", app.ui.status);
    let p = Pos2::new(500., 400.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(p), primary_event(p, true)],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(p, false)]);
    assert_eq!(app.document.mesh(), &before.mesh);
    app.dispatch(Command::Instances(C::Apply), &ctx);
    assert_eq!(app.document.history_counts(), (1, 0));
    let set = app.document.instances().sets[0].clone();
    app.dispatch(Command::Instances(C::Edit(set.id)), &ctx);
    app.dispatch(Command::Instances(C::Cancel), &ctx);
    assert_eq!(app.document.instances().sets[0], set);
}
#[test]
fn reusable_object_spacing_is_independent_of_pointer_event_count() {
    let run = |steps: usize| {
        let (mut app, ctx) = setup();
        let start = Pos2::new(430., 400.);
        let end = Pos2::new(570., 400.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        for i in 1..=steps {
            navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(
                    start.lerp(end, i as f32 / steps as f32),
                )],
            );
        }
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        app.document
            .instance_placements()
            .unwrap()
            .iter()
            .map(|(_, _, p)| p.transform.origin)
            .collect::<Vec<_>>()
    };
    let coarse = run(1);
    let fine = run(20);
    assert_eq!(coarse.len(), fine.len());
    assert!(coarse.len() > 2);
    for (a, b) in coarse.iter().zip(fine) {
        assert!((*a - b).len() < 0.02, "{a:?} {b:?}");
    }
}
