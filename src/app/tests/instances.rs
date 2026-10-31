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
fn object_kit_picks_a_part_per_stroke_and_reuses_personal_brushes_across_projects() {
    use crate::sculpt::instances::{Layout, Source};
    let (mut app, ctx) = setup();
    let dir = std::env::temp_dir().join(format!("clay-brush-shelf-{}", std::process::id()));
    app.preferences_path = Some(dir.join("preferences.json"));
    app.dispatch(Command::Instances(C::StopBrush), &ctx);
    app.dispatch(
        Command::Instances(C::CreateBrush("Ship / fittings".into())),
        &ctx,
    );
    app.dispatch(Command::Instances(C::BrushMember(1, 1, true)), &ctx);
    app.dispatch(
        Command::Instances(C::Capture("Dome".into(), Vec3(0., -1., 0.), 0, 1)),
        &ctx,
    );
    app.dispatch(Command::Instances(C::ChooseObject(1)), &ctx);
    app.dispatch(Command::Instances(C::StartBrush(1)), &ctx);
    let start = Pos2::new(450., 400.);
    let end = Pos2::new(550., 400.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    app.dispatch(Command::Instances(C::ChooseObject(2)), &ctx);
    assert_eq!(app.ui.instances.asset, Some(1));
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end), primary_event(end, false)],
    );
    let first = app.document.instances().sets[0].clone();
    let Layout::Manual(items) = &first.layout else {
        panic!()
    };
    assert!(
        !items.is_empty()
            && items
                .iter()
                .all(|p| p.piece.source == Source::Asset(1) && p.piece.up == 2)
    );
    app.dispatch(Command::Instances(C::ChooseObject(2)), &ctx);
    assert_eq!(
        app.viewport.object_brush.as_ref().unwrap().piece.source,
        Source::Asset(2)
    );
    let next = Pos2::new(500., 350.);
    app.ui.instances.brush.rotation = 42.;
    let history = app.document.history_counts().0;
    navigation_frame(&mut app, &ctx, vec![]);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(next), primary_event(next, true)],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(next, false)]);
    assert_eq!(app.document.instances().sets[0], first);
    let Layout::Manual(items) = &app.document.instances().sets[1].layout else {
        panic!()
    };
    assert_eq!(items[0].piece.source, Source::Asset(2));
    assert_eq!(items[0].piece.up, 1);
    assert!((items[0].point(Vec3(0., -1., 0.)) - items[0].transform.origin).len() < 1e-6);
    assert_eq!(app.document.instances().assets[1].options.rotation, 42.);
    assert_eq!(app.document.history_counts().0, history + 1);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.instances().sets.len(), 1);
    assert_eq!(app.ui.instances.brush.rotation, 0.);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.ui.instances.brush.rotation, 42.);
    app.dispatch(Command::Instances(C::StopBrush), &ctx);
    app.dispatch(Command::Instances(C::SavePersonalBrush(1)), &ctx);
    assert_eq!(app.ui.instances.personal_brushes, vec!["Ship / fittings"]);
    let mut app = App {
        preferences_path: Some(dir.join("preferences.json")),
        ..Default::default()
    };
    assert!(app.document.instances().assets.is_empty());
    app.dispatch(Command::Instances(C::ListPersonalBrushes), &ctx);
    app.dispatch(
        Command::Instances(C::LoadPersonalBrush("Ship / fittings".into())),
        &ctx,
    );
    assert_eq!(app.document.instances().brushes[0].objects.len(), 2);
    assert_eq!(app.ui.instances.asset, Some(1));
    assert_eq!(app.document.instances().assets[1].pivot, Vec3(0., -1., 0.));
    app.document.project().validate().unwrap();
    let project = app.document.project();
    app.dispatch(
        Command::Instances(C::RemovePersonalBrush("Ship / fittings".into())),
        &ctx,
    );
    assert!(app.ui.instances.personal_brushes.is_empty());
    assert!(app.document.project() == project);
    std::fs::remove_dir_all(dir).unwrap();
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
fn reusable_object_tool_yields_to_sculpt_brush_and_transform_selection() {
    let (mut app, ctx) = setup();
    app.ui.select_brush(0, &app.document);
    navigation_frame(&mut app, &ctx, vec![]);
    assert!(app.viewport.object_brush.is_none());
    app.dispatch(Command::Instances(C::StartBrush(1)), &ctx);
    app.dispatch(
        Command::TransformMode(crate::viewport::TransformMode::World),
        &ctx,
    );
    navigation_frame(&mut app, &ctx, vec![]);
    assert!(app.viewport.object_brush.is_none());
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
fn reusable_path_inline_thickness_handles_share_preview_and_preserve_navigation() {
    use crate::sculpt::{
        instances::*,
        paths::{PathMode, PathPoint},
    };
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
    cube.divisions = [8, 2, 2];
    let mut mesh = cube.mesh().unwrap();
    for p in &mut mesh.vertices {
        p.0 *= 0.15;
        p.1 *= 0.04;
        p.2 *= 0.04;
    }
    let mut geometry = Geometry::new(mesh);
    geometry.scene.active_metadata.visible = false;
    app.document.replace_geometry(geometry).unwrap();
    app.document
        .create_path(
            "Handle curve".into(),
            PathMode::Free,
            [Vec3(-0.8, -0.3, 0.), Vec3(0., 0.5, 0.), Vec3(0.8, -0.3, 0.)]
                .map(|position| PathPoint {
                    position,
                    normal: Vec3(0., 0., 1.),
                })
                .to_vec(),
            false,
            1.,
        )
        .unwrap();
    let before = app.document.project();
    let history = app.document.history_counts();
    app.dispatch(Command::Instances(C::NewPath), &ctx);
    let mut set = app.ui.instances.editor.clone().unwrap();
    let Layout::Path(a) = &mut set.layout else {
        unreachable!()
    };
    a.bend = true;
    a.join = JoinMode::Weld;
    app.ui.instances.editor = Some(set.clone());
    app.dispatch(Command::Instances(C::Preview(Box::new(set.clone()))), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    let handle = |ctx: &egui::Context, label: &str| {
        ctx.data(|d| {
            d.get_temp::<Vec<(String, Pos2, Vec2, f32)>>(egui::Id::new("path_thickness_handles"))
        })
        .unwrap()
        .into_iter()
        .find(|h| h.0 == label)
        .unwrap_or_else(|| panic!("{label} handle must be visible"))
    };
    let (_, start, _, _) = handle(&ctx, "Start");
    for pressed in [true, false] {
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(start),
                primary_event(start, pressed),
            ],
        );
    }
    assert_eq!(
        app.document.instances().sets[0],
        set,
        "selecting a handle must not change thickness"
    );
    let builds = app.viewport.path_thickness.cache_builds;
    for (label, amount) in [
        ("Start", -0.3),
        ("End", 0.5),
        ("Path point 2", 0.6),
        ("Section 4", 0.25),
    ] {
        navigation_frame(&mut app, &ctx, vec![]);
        let (_, position, axis, t) = handle(&ctx, label);
        let Layout::Path(a) = &app.document.instances().sets[0].layout else {
            unreachable!()
        };
        let old = a.thickness.at(t, false);
        let end = position + axis * (60. * amount);
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(position),
                primary_event(position, true),
            ],
        );
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        navigation_frame(
            &mut app,
            &ctx,
            vec![primary_event(end, false), egui::Event::PointerGone],
        );
        let Layout::Path(a) = &app.document.instances().sets[0].layout else {
            unreachable!()
        };
        assert!(
            (a.thickness.at(t, false) - old - amount).abs() < 1e-4,
            "{label} updates the shared profile"
        );
        assert_eq!(
            app.ui.instances.editor.as_ref().unwrap(),
            &app.document.instances().sets[0]
        );
        assert!(app.document.project() == before);
        assert_eq!(app.document.history_counts(), history);
        assert_eq!(
            app.viewport.path_thickness.cache_builds, builds,
            "thickness-only edits reuse sampled path positions"
        );
    }
    let edited = app.document.instances().sets[0].clone();
    let (_, position, _, _) = handle(&ctx, "Path point 2");
    let camera = app.camera;
    let end = position + Vec2::new(25., 10.);
    let secondary = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(position),
            secondary(position, true),
        ],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![secondary(end, false)]);
    assert_ne!(app.camera.yaw, camera.yaw);
    assert_eq!(app.document.instances().sets[0], edited);
    assert_eq!(app.viewport.path_thickness.cache_builds, builds);
    app.dispatch(Command::Instances(C::Apply), &ctx);
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    app.document.undo();
    assert!(app.document.project() == before);
    app.document.redo();
    assert_eq!(app.document.instances().sets[0], edited);
    app.dispatch(Command::Instances(C::Edit(1)), &ctx);
    app.dispatch(Command::Instances(C::Cancel), &ctx);
    assert_eq!(app.document.instances().sets[0], edited);
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
