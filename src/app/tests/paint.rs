use super::*;
use crate::sculpt::Kind;

#[test]
fn material_brush_menu_old_project_pointer_release_escape_and_undo() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut project = app.document.project();
    project.brushes.retain(|b| b.kind != Kind::Paint);
    app.document.replace_project(project, "old.clay").unwrap();
    assert!(!app.document.mesh().vertex_groups.is_empty());
    assert!(app.ui.show_polygroups);
    app.dispatch(Command::Action(Action::NewBrush("Paint".into())), &ctx);
    assert_eq!(app.document.brushes()[app.ui.selected].kind, Kind::Paint);
    assert!(
        !app.ui.show_polygroups,
        "Paint must show the painted material"
    );
    let mut project = app.document.project();
    project.mesh_settings.dynamic = true;
    project.mesh_settings.auto_remesh = true;
    app.document
        .replace_project(project, "dynamic.clay")
        .unwrap();
    app.camera.yaw = 0.;
    app.camera.pitch = 0.;
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let start = rect.center() + Vec2::new(70., 0.);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let draw = |app: &mut App| {
        navigation_frame(
            app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        for i in 1..=4 {
            navigation_frame(
                app,
                &ctx,
                vec![egui::Event::PointerMoved(
                    start + Vec2::new(0., i as f32 * 12.),
                )],
            );
        }
    };
    draw(&mut app);
    assert_ne!(app.document.geometry_snapshot(), before);
    // A temporary Smooth modifier on release must not remesh the Paint result.
    visibility_frame(
        &mut app,
        &ctx,
        egui::Modifiers::SHIFT,
        vec![primary_event(start, false)],
    );
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    assert!(
        app.mesh_job.is_none(),
        "Paint must not launch automatic voxel remeshing"
    );
    let after = app.document.geometry_snapshot();
    assert_eq!(after.mesh.triangles, before.mesh.triangles);
    assert_eq!(after.mesh.vertices, before.mesh.vertices);
    assert!(!after.mesh.materials.is_empty());
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    let history = app.document.history_counts();
    draw(&mut app);
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
    navigation_frame(&mut app, &ctx, vec![primary_event(start, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
}

#[test]
fn material_sampler_does_not_paint_and_p_toggles_group_view() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.dispatch(Command::AddBrush(Kind::Paint), &ctx);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let point = rect.center();
    app.ui.materials.sampling = true;
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(point, false)]);
    assert!(!app.ui.materials.sampling);
    let colour = app.document.brushes()[app.ui.selected].material.base_color;
    for (a, b) in colour
        .into_iter()
        .zip(crate::sculpt::Material::default().base_color)
    {
        assert!((a - b).abs() < 1e-6);
    }
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), history);
    app.ui.materials.sampling = true;
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    };
    navigation_frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);
    assert!(!app.ui.materials.sampling);
    for expected in [true, false] {
        let _ = ctx.run(
            egui::RawInput {
                events: vec![key(egui::Key::P)],
                ..Default::default()
            },
            |ctx| app.shortcuts(ctx),
        );
        assert_eq!(app.ui.show_polygroups, expected);
        let _ = ctx.run(Default::default(), |_| {});
    }
    let _ = ctx.run(
        egui::RawInput {
            events: vec![key(egui::Key::P)],
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.text_edit_singleline(&mut app.ui.path).request_focus();
            });
            app.shortcuts(ctx);
            assert!(!app.ui.show_polygroups, "P must not toggle while typing");
        },
    );
}

#[test]
fn material_removal_preserves_paint_other_bindings_levels_and_library_undo() {
    use crate::sculpt::{Brush, Material, material::layers::Asset};
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mut cube = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube);
    cube.divisions = [1; 3];
    app.document
        .replace_geometry(crate::sculpt::Geometry::new(cube.mesh().unwrap()))
        .unwrap();
    let base = Material::examples()[7].1.clone();
    let index = app
        .ui
        .preferences
        .materials
        .copy("Wax", base.clone())
        .unwrap();
    let copy = app.ui.preferences.materials.copies[index].clone();
    let asset = Asset {
        id: copy.id,
        name: copy.name.clone(),
        material: base.clone(),
        original: copy.original.clone(),
    };
    app.dispatch(Command::SaveMaterial(asset.clone()), &ctx);
    let other = Asset {
        id: 5,
        name: "Copper".into(),
        material: Material::examples()[4].1.clone(),
        original: None,
    };
    app.dispatch(Command::SaveMaterial(other.clone()), &ctx);
    let mut brush = Brush::new(Kind::Paint);
    brush.material = other.material.with_link(other.id);
    app.dispatch(Command::FillAllMaterial(brush.clone()), &ctx);
    brush.material = base.with_link(asset.id);
    brush.strength = 0.5;
    app.dispatch(Command::FillMaterial(brush), &ctx);
    let mut geometry = app.document.geometry_snapshot();
    geometry.subdivide().unwrap();
    geometry.subdivide().unwrap();
    app.document.replace_geometry(geometry).unwrap();
    app.document.set_mesh_level(1).unwrap();
    app.document.duplicate_model(false).unwrap();
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::RemoveMaterial(asset.id), &ctx);
    assert!(app.ui.preferences.materials.copies.is_empty());
    assert_eq!(app.document.materials().len(), 1);
    let meshes = |g: &crate::sculpt::Geometry| {
        let mut meshes = vec![g.mesh.clone()];
        meshes.extend(g.settings.lower.clone());
        meshes.extend(g.settings.upper.clone());
        for model in &g.submodels {
            meshes.push(model.mesh.clone());
            meshes.extend(model.mesh_settings.lower.clone());
            meshes.extend(model.mesh_settings.upper.clone());
        }
        meshes
    };
    let after = app.document.geometry_snapshot();
    for (old, new) in meshes(&before).iter().zip(meshes(&after)) {
        assert_eq!(old.vertices, new.vertices);
        for (old, new) in old.materials.iter().zip(&new.materials) {
            assert_eq!(
                old.values(),
                new.values(),
                "Removal preserves the rendered surface"
            );
            assert!(new.links.iter().all(|l| l.id != asset.id));
            assert_eq!(
                new.links.iter().find(|l| l.id == other.id),
                old.links.iter().find(|l| l.id == other.id)
            );
        }
    }
    let project = app.document.project();
    project.validate().unwrap();
    let restored: crate::sculpt::Project =
        serde_json::from_slice(&serde_json::to_vec(&project).unwrap()).unwrap();
    assert_eq!(restored.scene.materials, project.scene.materials);
    assert_eq!(restored.mesh.materials, project.mesh.materials);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.ui.preferences.materials.copies, vec![copy]);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
    assert!(app.ui.preferences.materials.copies.is_empty());
    // Built-ins remain hidden across preferences saves and Undo restores them too.
    app.dispatch(Command::RemoveMaterial(other.id), &ctx);
    assert!(
        app.ui
            .preferences
            .materials
            .hidden_builtins
            .contains(&other.id)
    );
    let prefs: crate::preferences::Preferences =
        serde_json::from_slice(&serde_json::to_vec(&app.ui.preferences).unwrap()).unwrap();
    prefs.validate().unwrap();
    assert_eq!(prefs.materials.hidden_builtins, vec![other.id]);
    app.dispatch(Command::Undo, &ctx);
    assert!(app.ui.preferences.materials.hidden_builtins.is_empty());
}

#[test]
fn lighting_widget_does_not_block_fresh_sculpt_strokes_outside_it() {
    for show in [false, true] {
        let mut app = App::default();
        app.ui.preferences.lighting.show_controls = show;
        let ctx = egui::Context::default();
        let before = app.document.geometry_snapshot();
        let lighting = app.ui.preferences.lighting.clone();
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let start = rect.center() - Vec2::new(0., 130.);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        let end = start + Vec2::new(0., 40.);
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_ne!(
            app.document.mesh().vertices,
            before.mesh.vertices,
            "show_controls={show}"
        );
        assert_eq!(app.ui.preferences.lighting, lighting);
    }
}
