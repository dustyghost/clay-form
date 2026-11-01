use super::*;
use crate::{
    sculpt::{
        Geometry, Mesh, Primitive, PrimitiveKind, Vec3,
        cage::{Adjustment, Cage},
    },
    viewport::TransformOperation,
};
pub(super) fn key_escape() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}
#[test]
fn cage_viewport_reverses_and_cancels_push_bend_twist_in_both_views() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    let mesh = Primitive {
        divisions: [3, 5, 2],
        size: Vec3(1., 1.7, 0.8),
        ..Default::default()
    }
    .mesh()
    .unwrap();
    app.document.replace_geometry(Geometry::new(mesh)).unwrap();
    app.camera.yaw = 0.32;
    app.camera.pitch = 0.2;
    app.camera.zoom = 0.8;
    let before = app.document.mesh().clone();
    let fitted = Cage::fit(&before).unwrap();
    let count = app.document.history_counts();
    app.dispatch(Command::TransformOperation(TransformOperation::Cage), &ctx);
    for perspective in [false, true] {
        app.camera.perspective = perspective;
        let rect = navigation_frame(&mut app, &ctx, vec![]);
        let project = |p: Vec3, camera: crate::camera::Camera| {
            let (r, u, _) = camera.basis();
            rect.center()
                + camera.pan
                + egui::vec2(p.dot(r), -p.dot(u)) * camera.scale_at(rect.size(), p)
        };
        let end = Vec3(0., fitted.high.1, 0.);
        let axis = crate::sculpt::Axis::Y;
        for (pos, edit) in [
            (project(fitted.corner(5), app.camera), None),
            (
                project(end, app.camera) + egui::vec2(-40., -22.),
                Some(Adjustment::Bend(axis, 0.72)),
            ),
            (
                project(end, app.camera) + egui::vec2(40., -22.),
                Some(Adjustment::Twist(axis, 0.72)),
            ),
        ] {
            let to = pos + egui::vec2(60., 0.);
            for cancel in [true, false] {
                navigation_frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(pos), primary_event(pos, true)],
                );
                navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
                assert!(
                    app.viewport.cage_previewing(),
                    "drag did not start at {pos:?}"
                );
                assert_eq!(app.document.mesh(), &before);
                navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos)]);
                navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
                navigation_frame(&mut app, &ctx, vec![]);
                if cancel {
                    navigation_frame(&mut app, &ctx, vec![key_escape()]);
                }
                navigation_frame(&mut app, &ctx, vec![primary_event(to, false)]);
                if cancel {
                    assert_eq!(app.document.history_counts().0, count.0);
                } else {
                    let expected = edit.unwrap_or_else(|| {
                        Adjustment::Pull(
                            5,
                            app.camera.basis().0
                                * (60. / app.camera.scale_at(rect.size(), fitted.corner(5))),
                        )
                    });
                    for (&p, &q) in before.vertices.iter().zip(&app.document.mesh().vertices) {
                        assert!((fitted.point(p, expected) - q).len() < 1e-4);
                    }
                    assert_ne!(app.document.mesh(), &before);
                    assert_eq!(app.document.history_counts().0, count.0 + 1);
                    app.dispatch(Command::Undo, &ctx);
                }
                assert_eq!(app.document.mesh(), &before);
            }
        }
    }
    app.dispatch(Command::TransformOperation(TransformOperation::Cage), &ctx);
    assert_eq!(app.document.mesh(), &before);
    assert_eq!(app.document.history_counts().0, count.0);
}

pub(super) fn boolean_fixture() -> Geometry {
    use crate::sculpt::{BooleanRole::*, ModelMetadata, Submodel};
    let cube = |size, center| {
        Primitive {
            size,
            center,
            divisions: [3; 3],
            ..Default::default()
        }
        .mesh()
        .unwrap()
    };
    let cylinder = |diameter, depth| {
        Primitive {
            kind: PrimitiveKind::Cylinder,
            size: Vec3(diameter, depth, diameter),
            center: Vec3(-0.6, 0., 0.),
            basis: [Vec3(1., 0., 0.), Vec3(0., 0., 1.), Vec3(0., -1., 0.)],
            ..Default::default()
        }
        .mesh()
        .unwrap()
    };
    let mut g = Geometry::new(cylinder(0.60, 1.5));
    g.name = "Bore cutter".into();
    g.scene.active_metadata.boolean_role = Subtract;
    for (name, mesh) in [
        ("Round boss", cylinder(1.42, 0.75)),
        ("Body", cube(Vec3(1.65, 1.1, 0.42), Vec3(0.2, 0., 0.))),
    ] {
        g.submodels.push(Submodel {
            name: name.into(),
            mesh,
            mesh_settings: Default::default(),
            metadata: ModelMetadata {
                boolean_role: Add,
                ..Default::default()
            },
        });
    }
    g
}
#[test]
fn boolean_stack_bore_keeps_a_sharp_rim_at_preview_resolution() {
    let mut doc = Document::default();
    doc.replace_geometry(boolean_fixture()).unwrap();
    let mesh =
        crate::sculpt::boolean::evaluate(&doc.boolean_operands().unwrap(), 48, &|| false).unwrap();
    mesh.validate_closed().unwrap();
    let mut cap = vec![false; mesh.vertices.len()];
    let mut wall = cap.clone();
    for face in &mesh.triangles {
        let [a, b, c] = face.map(|v| mesh.vertices[v]);
        let normal = (b - a).cross(c - a).norm();
        for &v in face {
            cap[v] |= normal.2.abs() > 0.985;
            wall[v] |= normal.2.abs() < 0.2;
        }
    }
    // The source bore has radius 0.3, centred at x = -0.6, with the cap at z = 0.375.
    // Every sector must have a vertex where the planar cap and cylindrical wall meet.
    let mut sectors = [false; 24];
    for (i, &Vec3(x, y, z)) in mesh.vertices.iter().enumerate() {
        let x = x + 0.6;
        if cap[i] && wall[i] && (x.hypot(y) - 0.3).abs() < 0.004 && (z - 0.375).abs() < 0.004 {
            let turn = y.atan2(x).rem_euclid(std::f32::consts::TAU) / std::f32::consts::TAU;
            sectors[(turn * 24.) as usize % 24] = true;
        }
    }
    assert!(
        sectors.iter().all(|v| *v),
        "incomplete bore rim: {sectors:?}"
    );
}

#[test]
fn boolean_stack_smooth_shading_respects_caps_walls_and_shallow_joins() {
    let mut doc = Document::default();
    doc.replace_geometry(boolean_fixture()).unwrap();
    let mesh =
        crate::sculpt::boolean::evaluate(&doc.boolean_operands().unwrap(), 48, &|| false).unwrap();
    let shading = mesh.surface_shading();
    let mut flat_faces = [0; 3];
    let mut curved_faces = [0; 2];
    for (triangle, fans) in mesh.triangles.iter().zip(&shading.triangles) {
        let [a, b, c] = triangle.map(|v| mesh.vertices[v]);
        let center = (a + b + c) * (1. / 3.);
        let face = (b - a).cross(c - a).norm();
        let normals = fans.map(|v| shading.vertices[v].1);
        // Check both sides of the boss cap, the body front/back, and its shallow
        // junction with the boss along the upper/lower face of the block.
        for (i, (distance, normal)) in [
            (center.2.abs() - 0.375, Vec3(0., 0., center.2.signum())),
            (center.2.abs() - 0.21, Vec3(0., 0., center.2.signum())),
            (center.1.abs() - 0.55, Vec3(0., center.1.signum(), 0.)),
        ]
        .into_iter()
        .enumerate()
        {
            if distance.abs() < 0.001 && face.dot(normal) > 0.9998 {
                assert!(
                    normals
                        .iter()
                        .all(|n| n.dot(normal) > 2_f32.to_radians().cos()),
                    "tilted lighting normal on flat face at {center:?}"
                );
                flat_faces[i] += 1;
            }
        }
        // Lighting must interpolate around both cylinders in smooth view.
        if (0.24..0.34).contains(&center.2.abs()) && face.2.abs() < 0.02 {
            let radius = (center.0 + 0.6).hypot(center.1);
            for (i, (r, sign)) in [(0.71, 1.), (0.30, -1.)].into_iter().enumerate() {
                if (radius - r).abs() >= 0.01 {
                    continue;
                }
                for (v, n) in [a, b, c].into_iter().zip(normals) {
                    let radial = Vec3(v.0 + 0.6, v.1, 0.).norm() * sign;
                    assert!(n.dot(radial) > 4_f32.to_radians().cos());
                }
                if normals[0].dot(normals[1]) < 1_f32.to_radians().cos() {
                    curved_faces[i] += 1;
                }
            }
        }
    }
    assert!(flat_faces.iter().all(|&count| count > 100));
    assert!(curved_faces.iter().all(|&count| count > 100));
}

#[test]
fn boolean_stack_live_result_refresh_and_picking() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    app.document.replace_geometry(boolean_fixture()).unwrap();
    app.document.set_boolean_resolution(16).unwrap();
    app.dispatch(Command::ToggleBooleanResult, &ctx);
    fn settle(app: &mut App, ctx: &egui::Context) -> Mesh {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        loop {
            navigation_frame(app, ctx, vec![]);
            if let Some(mesh) = app.boolean.result() {
                return mesh.clone();
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{}",
                app.ui.boolean_notice
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    let cut = settle(&mut app, &ctx);
    let cached = app.viewport.boolean_display.clone().unwrap();
    app.camera.yaw += 0.2;
    navigation_frame(&mut app, &ctx, vec![]);
    assert!(std::sync::Arc::ptr_eq(
        &cached,
        app.viewport.boolean_display.as_ref().unwrap()
    ));
    app.camera.yaw -= 0.2;
    // Result triangles never act as editable operand faces. Switch back to pick the body.
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    let point = rect.center() + egui::vec2(0.65, 0.40) * (rect.size().y * 0.35);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(point, false)]);
    assert_eq!(app.document.active_model_index(), 0);
    let camera_before = app.camera.basis();
    app.dispatch(Command::ToggleBooleanResult, &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    navigation_frame(&mut app, &ctx, vec![primary_event(point, false)]);
    assert_eq!(app.camera.basis(), camera_before);
    assert_eq!(app.document.active_model_index(), 2);
    app.dispatch(Command::SelectSubmodel(0), &ctx);
    app.dispatch(Command::ToggleBooleanResult, &ctx);
    app.dispatch(Command::TransformOperation(TransformOperation::Move), &ctx);
    settle(&mut app, &ctx);
    let center = rect.center() + egui::vec2(-0.6, 0.) * (rect.size().y * 0.35);
    let end = center + egui::vec2(160., 0.);
    let vertices = app.document.mesh().vertices.clone();
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(center),
            primary_event(center, true),
        ],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert!(app.ui.boolean_enabled);
    assert_ne!(app.document.mesh().vertices, vertices);
    app.dispatch(Command::BakeBoolean, &ctx);
    assert_eq!(app.document.model_count(), 3);
    let filled = settle(&mut app, &ctx);
    assert_ne!(cut, filled);
    let before = app.document.geometry_snapshot();
    app.dispatch(Command::BakeBoolean, &ctx);
    assert_eq!(app.document.mesh(), &filled);
    assert_eq!(app.document.model_count(), 4);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}
