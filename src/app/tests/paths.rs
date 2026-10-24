use super::*;
use crate::command::PathCommand;
use crate::sculpt::paths::{PathMode, PathPoint};
use crate::viewport::paths::PathTool;

fn click(app: &mut App, ctx: &egui::Context, point: Pos2) {
    navigation_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(point), primary_event(point, true)],
    );
    navigation_frame(app, ctx, vec![primary_event(point, false)]);
}

#[test]
fn path_authoring_orbit_move_cancel_and_save_preserve_transaction_boundaries() {
    let mut app = App::default();
    let ctx = egui::Context::default();
    // Keep the authored surface beside the floating path palette.
    app.camera.pan = Vec2::new(180., 0.);
    navigation_frame(&mut app, &ctx, vec![]);
    let original = app.document.geometry_snapshot();
    app.dispatch(Command::Path(PathCommand::New(PathMode::Surface)), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    let first = Pos2::new(650., 430.);
    click(&mut app, &ctx, first);
    click(&mut app, &ctx, Pos2::new(720., 390.));
    assert_eq!(app.viewport.paths.draft.as_ref().unwrap().points.len(), 2);
    assert_eq!(app.document.geometry_snapshot(), original);
    assert_eq!(app.document.history_counts(), (0, 0));

    // Cancelling a point drag restores the complete pre-drag draft.
    let before = app.viewport.paths.draft.clone();
    app.viewport.paths.tool = PathTool::Move;
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(first), primary_event(first, true)],
    );
    let moved = first + Vec2::new(30., 30.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(moved)]);
    assert_ne!(app.viewport.paths.draft, before);
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
    navigation_frame(&mut app, &ctx, vec![primary_event(moved, false)]);
    assert_eq!(app.viewport.paths.draft, before);

    let orbit = Pos2::new(780., 500.);
    let secondary = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let yaw = app.camera.yaw;
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(orbit), secondary(orbit, true)],
    );
    let end = orbit + Vec2::new(55., 25.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![secondary(end, false)]);
    assert!((app.camera.yaw - yaw).abs() > 0.1);
    assert_eq!(app.viewport.paths.draft, before);
    assert_eq!(app.document.geometry_snapshot(), original);

    app.dispatch(Command::Path(PathCommand::Save), &ctx);
    assert_eq!(app.document.paths().len(), 1, "{}", app.ui.status);
    assert_eq!(app.document.history_counts(), (1, 0));
    let saved = app.document.geometry_snapshot();
    app.viewport.paths.draft.as_mut().unwrap().name = "Uncommitted rename".into();
    app.dispatch(Command::Path(PathCommand::Close), &ctx);
    assert!(!app.viewport.paths.active);
    assert_eq!(app.document.geometry_snapshot(), saved);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), original);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn closed_cylinder_path_offscreen_preview() {
    use crate::sculpt::{Geometry, Primitive, PrimitiveKind, Vec3};
    let mut capture = crate::viewport::UiCapture::new([1000, 800]);
    let ctx = egui::Context::default();
    let mut app = App::default();
    app.viewport.initialize_renderer(&capture.state).unwrap();
    let mut cylinder = Primitive::for_kind(PrimitiveKind::Cylinder);
    cylinder.segments = 96;
    app.document
        .replace_geometry(Geometry::new(cylinder.mesh().unwrap()))
        .unwrap();
    let points = (0..32)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / 32.;
            let radial = Vec3(angle.cos(), 0., angle.sin());
            PathPoint {
                position: radial,
                normal: radial,
            }
        })
        .collect();
    let id = app
        .document
        .create_path("Cylinder ring".into(), PathMode::Surface, points, true, 1.)
        .unwrap();
    app.dispatch(Command::Path(PathCommand::Open), &ctx);
    app.dispatch(Command::Path(PathCommand::Select(id)), &ctx);
    app.viewport.paths.selected = Some(4);
    app.camera.pitch = 0.35;
    app.camera.zoom = 0.85;
    app.camera.pan = Vec2::new(160., 0.);
    let before = app.document.geometry_snapshot();
    let prefix = std::env::var("CLAY_PATHS_PREVIEW").unwrap_or_else(|_| "/tmp/clay-paths".into());
    for (name, yaw) in [("front", 0.5), ("back", 3.6)] {
        app.camera.yaw = yaw;
        let mut images = Vec::new();
        for guide in [false, true] {
            let draft = app.viewport.paths.draft.clone();
            if !guide {
                app.viewport.paths.draft = None;
            }
            for frame in 0..3 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1000., 800.),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                    },
                );
                let rgba = capture.render(&ctx, output);
                if frame == 2 {
                    if guide {
                        capture.save(format!("{prefix}-{name}.png"), &rgba);
                    }
                    images.push(rgba);
                }
            }
            app.viewport.paths.draft = draft;
        }
        let changed = (200..650)
            .flat_map(|y| (450..950).map(move |x| (y * 1000 + x) * 4))
            .filter(|&i| images[0][i..i + 3] != images[1][i..i + 3])
            .count();
        assert!(
            changed > 400,
            "the closed guide must be visible over the rendered cylinder: {changed}"
        );
    }
    assert_eq!(app.document.geometry_snapshot(), before);
    app.viewport.destroy_renderer();
}

fn live_guide_setup(kind: crate::sculpt::Kind) -> (App, egui::Context, u64, egui::Rect) {
    use crate::sculpt::{Geometry, Primitive, PrimitiveKind, Vec3};
    let mut app = App::default();
    let mut grid = Primitive::for_kind(PrimitiveKind::Grid);
    grid.divisions = [60; 3];
    app.document
        .replace_geometry(Geometry::new(grid.mesh().unwrap()))
        .unwrap();
    app.ui.selected = app
        .document
        .brushes()
        .iter()
        .position(|b| b.kind == kind)
        .unwrap();
    let mut brush = app.document.brushes()[app.ui.selected].clone();
    brush.radius = 25.;
    brush.strength = 0.45;
    brush.spacing = 0.2;
    brush.pressure_radius = false;
    brush.stabilize = false;
    app.document.update_brush(app.ui.selected, brush).unwrap();
    let id = app
        .document
        .create_path(
            "Live guide".into(),
            PathMode::Surface,
            [-0.7, 0., 0.7]
                .into_iter()
                .map(|x| PathPoint {
                    position: Vec3(x, 0., 0.),
                    normal: Vec3(0., 0., 1.),
                })
                .collect(),
            false,
            0.,
        )
        .unwrap();
    app.document.mark_saved("guided-live.clay");
    let ctx = egui::Context::default();
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    assert!(
        !app.viewport.paths.active,
        "saved guides operate in normal sculpt mode"
    );
    (app, ctx, id, rect)
}
fn guide_screen(rect: egui::Rect, x: f32, y: f32) -> Pos2 {
    rect.center() + Vec2::new(x, -y) * (rect.width().min(rect.height()) * 0.35)
}
fn live_drag(
    app: &mut App,
    ctx: &egui::Context,
    rect: egui::Rect,
    start_y: f32,
    end_y: f32,
) -> Pos2 {
    let start = guide_screen(rect, -0.3, start_y);
    navigation_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    let mut end = start;
    for i in 1..=20 {
        let t = i as f32 / 20.;
        end = guide_screen(rect, -0.3 + 0.55 * t, start_y + (end_y - start_y) * t);
        navigation_frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
    }
    end
}
#[test]
fn live_saved_guide_constrains_partial_drag_and_commits_once_on_release() {
    let (mut app, ctx, id, rect) = live_guide_setup(crate::sculpt::Kind::Standard);
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    let end = live_drag(&mut app, &ctx, rect, 0.02, 0.3);
    assert_eq!(
        app.document.geometry_snapshot(),
        before,
        "guided preview stays private until release"
    );
    assert_eq!(app.document.history_counts(), counts);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    let after = app.document.geometry_snapshot();
    assert_ne!(after.mesh, before.mesh, "{}", app.ui.status);
    assert_eq!(app.document.history_counts().0, counts.0 + 1);
    assert!(
        !app.document.path(id).unwrap().is_stale(app.document.mesh()),
        "{}",
        app.ui.status
    );
    let moved: Vec<_> = before
        .mesh
        .vertices
        .iter()
        .zip(&after.mesh.vertices)
        .filter(|(a, b)| (**a - **b).len() > 1e-6)
        .map(|(a, _)| *a)
        .collect();
    assert!(!moved.is_empty());
    assert!(
        moved.iter().all(|p| p.1.abs() < 0.13),
        "the brush must stay on the saved guide when the pointer moves away"
    );
    assert!(
        moved.iter().all(|p| p.0 < 0.5 && p.0 > -0.55),
        "a partial gesture must not replay the untouched path ends"
    );
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
    app.dispatch(Command::Redo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), after);
}
#[test]
fn live_saved_guide_escape_discards_mesh_path_and_history() {
    let (mut app, ctx, _, rect) = live_guide_setup(crate::sculpt::Kind::Standard);
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    let end = live_drag(&mut app, &ctx, rect, 0.02, 0.2);
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
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), counts);
    assert!(!app.document.is_dirty());
}
#[test]
fn off_guide_pointer_stroke_remains_freehand() {
    let (mut app, ctx, _, rect) = live_guide_setup(crate::sculpt::Kind::Standard);
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts().0;
    let end = live_drag(&mut app, &ctx, rect, 0.5, 0.5);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(app.document.history_counts().0, count + 1);
    let moved: Vec<_> = before
        .mesh
        .vertices
        .iter()
        .zip(&app.document.mesh().vertices)
        .filter(|(a, b)| (**a - **b).len() > 1e-6)
        .map(|(a, _)| *a)
        .collect();
    assert!(!moved.is_empty());
    assert!(
        moved.iter().all(|p| (p.1 - 0.5).abs() < 0.13),
        "off-guide strokes retain the pointer's freehand trajectory"
    );
}
#[test]
fn live_detail_brush_uses_the_guide_and_can_repeat_without_manual_reprojection() {
    let (mut app, ctx, id, rect) = live_guide_setup(crate::sculpt::Kind::Detail);
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    let end = live_drag(&mut app, &ctx, rect, 0.02, 0.25);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_ne!(
        app.document.mesh(),
        &before.mesh,
        "Detail must sculpt along a live path: {}",
        app.ui.status
    );
    assert_eq!(app.document.history_counts().0, counts.0 + 1);
    assert!(
        !app.document.path(id).unwrap().is_stale(app.document.mesh()),
        "{}",
        app.ui.status
    );
    let first = app.document.geometry_snapshot();
    let end = live_drag(&mut app, &ctx, rect, 0.02, 0.2);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(app.document.history_counts().0, counts.0 + 2);
    assert_ne!(app.document.mesh(), &first.mesh);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), first);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), before);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn live_detail_path_offscreen_preview() {
    let (mut app, _, _, rect) = live_guide_setup(crate::sculpt::Kind::Detail);
    // Capture the first font-atlas upload as well as the mesh callbacks.
    let ctx = egui::Context::default();
    app.camera.pitch = -0.45;
    let mut capture = crate::viewport::UiCapture::new([1000, 800]);
    app.viewport.initialize_renderer(&capture.state).unwrap();
    let prefix =
        std::env::var("CLAY_LIVE_PATH_PREVIEW").unwrap_or_else(|_| "/tmp/clay-live-path".into());
    let capture_stage = |app: &mut App, capture: &mut crate::viewport::UiCapture, name: &str| {
        for frame in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1000., 800.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
            let rgba = capture.render(&ctx, output);
            if frame == 2 {
                let guide_pixels = (300..500)
                    .flat_map(|y| (280..720).map(move |x| (y * 1000 + x) * 4))
                    .filter(|&i| rgba[i] < 130 && rgba[i + 1] > 150 && rgba[i + 2] > 120)
                    .count();
                assert!(
                    guide_pixels > 100,
                    "saved guide must remain visible during {name}: {guide_pixels}"
                );
                capture.save(format!("{prefix}-{name}.png"), &rgba);
            }
        }
    };
    capture_stage(&mut app, &mut capture, "before");
    let before = app.document.geometry_snapshot();
    let end = live_drag(&mut app, &ctx, rect, 0.02, 0.25);
    assert_eq!(app.document.geometry_snapshot(), before);
    capture_stage(&mut app, &mut capture, "during");
    navigation_frame(
        &mut app,
        &ctx,
        vec![primary_event(end, false), egui::Event::PointerGone],
    );
    assert_ne!(app.document.mesh(), &before.mesh);
    capture_stage(&mut app, &mut capture, "after");
    for (index, name) in ["after-second", "after-third"].into_iter().enumerate() {
        let before = app.document.mesh().clone();
        let id = app.document.paths()[0].id;
        repeat_guide_drag(&mut app, &ctx, rect, id, if index == 0 { -1. } else { 1. });
        let (bins, _) = guide_coverage(&before, app.document.mesh());
        assert!(
            bins.iter().all(|&motion| motion > 1e-6),
            "repeat capture {name} stalled: {bins:?}"
        );
        capture_stage(&mut app, &mut capture, name);
    }
    app.viewport.destroy_renderer();
}

fn project_guide_world(app: &App, rect: egui::Rect, point: crate::sculpt::Vec3) -> Pos2 {
    let (right, up, _) = app.camera.basis();
    rect.center()
        + app.camera.pan
        + Vec2::new(point.dot(right), -point.dot(up))
            * (rect.width().min(rect.height()) * 0.35 * app.camera.zoom)
}

/// Repeat the pointer trajectory against the current attached guide, so this
/// checks brush traversal rather than an assumption that a deformed guide stays
/// at its original screen coordinates. Alternate the side used to capture it.
fn repeat_guide_drag(app: &mut App, ctx: &egui::Context, rect: egui::Rect, id: u64, side: f32) {
    let samples = app.document.path_samples(id, 0.003).unwrap();
    let route_point = |x: f32| {
        samples
            .iter()
            .min_by(|a, b| {
                (a.position.0 - x)
                    .abs()
                    .total_cmp(&(b.position.0 - x).abs())
            })
            .unwrap()
            .position
    };
    let start = project_guide_world(app, rect, route_point(-0.3)) + Vec2::new(0., side * 5.);
    navigation_frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    assert!(
        app.viewport.session.guided.dragging(),
        "a repeated stroke must capture the visible guide: {}",
        app.ui.status
    );
    let mut end = start;
    for i in 1..=24 {
        let t = i as f32 / 24.;
        end = project_guide_world(app, rect, route_point(-0.3 + 0.55 * t))
            + Vec2::new(0., side * (5. + 15. * t));
        navigation_frame(app, ctx, vec![egui::Event::PointerMoved(end)]);
        assert!(
            app.viewport.session.guided.dragging(),
            "guided traversal ended at {t}: {}",
            app.ui.status
        );
    }
    navigation_frame(app, ctx, vec![primary_event(end, false)]);
}

fn guide_coverage(before: &crate::sculpt::Mesh, after: &crate::sculpt::Mesh) -> ([f32; 8], f32) {
    assert_eq!(
        before.vertices.len(),
        after.vertices.len(),
        "nonadaptive fixture keeps vertex identity"
    );
    let mut bins = [0_f32; 8];
    let mut total = 0.;
    for (a, b) in before.vertices.iter().zip(&after.vertices) {
        if !(-0.3..=0.25).contains(&a.0) || a.1.abs() > 0.11 {
            continue;
        }
        let index = (((a.0 + 0.3) / 0.55 * 8.) as usize).min(7);
        let motion = (*b - *a).len();
        bins[index] = bins[index].max(motion);
        total += motion;
    }
    (bins, total)
}

#[test]
fn repeated_live_guides_cover_the_complete_trajectory_after_surface_deformation() {
    for kind in [crate::sculpt::Kind::Standard, crate::sculpt::Kind::Detail] {
        for oblique in [false, true] {
            let (mut app, ctx, id, rect) = live_guide_setup(kind);
            if oblique {
                app.camera.pitch = -0.55;
                app.camera.yaw = 0.25;
            }
            navigation_frame(&mut app, &ctx, vec![]);
            let initial_count = app.document.history_counts().0;
            let mut previous_total = 0.;
            for pass in 0..3 {
                let before = app.document.mesh().clone();
                repeat_guide_drag(
                    &mut app,
                    &ctx,
                    rect,
                    id,
                    if pass % 2 == 0 { 1. } else { -1. },
                );
                let (bins, total) = guide_coverage(&before, app.document.mesh());
                eprintln!(
                    "repeat guide {kind:?} oblique={oblique} pass={pass} bins={bins:?} motion={total}"
                );
                assert_eq!(app.document.history_counts().0, initial_count + pass + 1);
                assert!(
                    !app.document.path(id).unwrap().is_stale(app.document.mesh()),
                    "{}",
                    app.ui.status
                );
                assert!(
                    bins.iter().all(|&motion| motion > 1e-6),
                    "{kind:?} oblique={oblique} pass={pass} must sculpt every traversed section, not just a start dab: {bins:?}"
                );
                if pass > 0 {
                    assert!(
                        total > previous_total * 0.2,
                        "{kind:?} oblique={oblique} pass={pass} lost most of its effect: {total} versus {previous_total}"
                    );
                }
                previous_total = total;
            }
        }
    }
}

#[test]
fn path_only_start_policy_latches_rejection_until_release_and_allows_near_guide() {
    let (mut app, ctx, _, rect) = live_guide_setup(crate::sculpt::Kind::Standard);
    app.dispatch(Command::Action(Action::TogglePathOnlyStrokes), &ctx);
    navigation_frame(&mut app, &ctx, vec![]);
    assert!(app.ui.path_starts_only);
    let before = app.document.geometry_snapshot();
    let counts = app.document.history_counts();
    // Beginning outside the guide remains ignored even after crossing onto it.
    let end = live_drag(&mut app, &ctx, rect, 0.5, 0.);
    assert!(!app.viewport.session.guided.dragging());
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), counts);
    assert!(!app.document.is_dirty());
    assert!(
        app.ui.status.contains("Start near a surface path"),
        "{}",
        app.ui.status
    );
    navigation_frame(&mut app, &ctx, vec![]);
    let end = live_drag(&mut app, &ctx, rect, 0.02, 0.15);
    assert!(app.viewport.session.guided.dragging());
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    assert_ne!(app.document.mesh(), &before.mesh);
    assert_eq!(app.document.history_counts().0, counts.0 + 1);
}

#[test]
fn freehand_is_available_when_path_only_policy_or_global_guidance_is_off() {
    for disable_global in [false, true] {
        let (mut app, ctx, _, rect) = live_guide_setup(crate::sculpt::Kind::Standard);
        if disable_global {
            app.dispatch(Command::Action(Action::TogglePathOnlyStrokes), &ctx);
            app.dispatch(Command::Action(Action::TogglePathGuides), &ctx);
            assert!(app.ui.path_starts_only);
            assert!(!app.ui.guide_paths);
        } else {
            assert!(!app.ui.path_starts_only);
            assert!(app.ui.guide_paths);
        }
        navigation_frame(&mut app, &ctx, vec![]);
        let before = app.document.geometry_snapshot();
        let count = app.document.history_counts().0;
        let end = live_drag(&mut app, &ctx, rect, 0.5, 0.5);
        assert!(!app.viewport.session.guided.dragging());
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_ne!(
            app.document.mesh(),
            &before.mesh,
            "global guidance disabled={disable_global}: {}",
            app.ui.status
        );
        assert_eq!(app.document.history_counts().0, count + 1);
    }
}

fn ring_setup(mode: crate::viewport::paths::RingMode) -> (App, egui::Context, egui::Rect) {
    use crate::sculpt::{Geometry, Primitive, PrimitiveKind};
    let mut app = App::default();
    let mut cylinder = Primitive::for_kind(PrimitiveKind::Cylinder);
    cylinder.segments = 48;
    cylinder.divisions[1] = 8;
    app.document
        .replace_geometry(Geometry::new(cylinder.mesh().unwrap()))
        .unwrap();
    let ctx = egui::Context::default();
    app.dispatch(Command::Path(PathCommand::NewRing), &ctx);
    app.viewport.paths.ring_mode = mode;
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    assert!(app.viewport.paths.tool == PathTool::Ring);
    (app, ctx, rect)
}
fn assert_complete_cylinder_ring(points: &[PathPoint]) {
    assert!(
        points.len() >= 16,
        "a whole circumference needs a resolved route"
    );
    let components: [fn(&PathPoint) -> f32; 2] = [|p| p.position.0, |p| p.position.2];
    for component in components {
        assert!(points.iter().map(component).fold(f32::INFINITY, f32::min) < -0.9);
        assert!(
            points
                .iter()
                .map(component)
                .fold(f32::NEG_INFINITY, f32::max)
                > 0.9
        );
    }
    assert_ne!(
        points.first().unwrap().position,
        points.last().unwrap().position
    );
}
#[test]
fn one_pen_drag_creates_and_positions_both_complete_ring_modes_without_document_edits() {
    use crate::viewport::paths::RingMode;
    for mode in [RingMode::Level, RingMode::Shortest] {
        let (mut app, ctx, rect) = ring_setup(mode);
        let before = app.document.geometry_snapshot();
        let count = app.document.history_counts();
        let start = guide_screen(rect, 0.25, -0.25);
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        let first = app.viewport.paths.draft.as_ref().unwrap().clone();
        assert!(first.closed, "{}", app.ui.status);
        assert_complete_cylinder_ring(&first.points);
        assert!(
            first
                .points
                .iter()
                .all(|p| (p.position.1 + 0.25).abs() < 0.02)
        );
        let end = guide_screen(rect, 0.25, 0.5);
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        let moved = app.viewport.paths.draft.as_ref().unwrap().clone();
        assert_complete_cylinder_ring(&moved.points);
        assert!(
            moved
                .points
                .iter()
                .all(|p| (p.position.1 - 0.5).abs() < 0.02),
            "ring drag must reposition the entire loop: {}",
            app.ui.status
        );
        assert_ne!(moved.points, first.points);
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), count);
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        assert_eq!(app.viewport.paths.draft.as_ref(), Some(&moved));
        assert_eq!(app.document.geometry_snapshot(), before);
        app.dispatch(Command::Path(PathCommand::SaveAndSculpt), &ctx);
        assert!(!app.viewport.paths.active, "{}", app.ui.status);
        assert_eq!(app.document.paths().len(), 1);
        assert!(app.document.paths()[0].closed);
        assert_eq!(app.document.history_counts().0, count.0 + 1);
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.geometry_snapshot(), before);
    }
}
#[test]
fn ring_drag_escape_restores_the_draft_and_failed_motion_keeps_last_valid_preview() {
    use crate::viewport::paths::RingMode;
    let (mut app, ctx, rect) = ring_setup(RingMode::Level);
    let original = app.viewport.paths.draft.clone();
    let before = app.document.geometry_snapshot();
    let count = app.document.history_counts();
    let start = guide_screen(rect, 0.25, -0.25);
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
    );
    let valid = app.viewport.paths.draft.clone();
    assert!(valid.as_ref().unwrap().closed);
    let outside = guide_screen(rect, 1.4, 0.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(outside)]);
    // show_paths delivers the previous viewport frame's notice before drawing
    // the next frame. Let the off-model notice reach the status bar.
    navigation_frame(&mut app, &ctx, vec![]);
    assert_eq!(app.viewport.paths.draft, valid);
    assert!(
        app.ui.status.contains("last valid ring"),
        "{}",
        app.ui.status
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
    navigation_frame(&mut app, &ctx, vec![primary_event(outside, false)]);
    assert_eq!(app.viewport.paths.draft, original);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert_eq!(app.document.history_counts(), count);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn one_drag_ring_authoring_offscreen_preview() {
    use crate::viewport::paths::RingMode;
    let (mut app, _, rect) = ring_setup(RingMode::Level);
    let ctx = egui::Context::default();
    let mut capture = crate::viewport::UiCapture::new([1000, 800]);
    app.viewport.initialize_renderer(&capture.state).unwrap();
    app.camera.pan = Vec2::new(180., 0.);
    app.camera.pitch = 0.3;
    app.camera.yaw = 0.5;
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let render = |app: &mut App, capture: &mut crate::viewport::UiCapture, name: Option<&str>| {
        for frame in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1000., 800.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            );
            let rgba = capture.render(&ctx, output);
            if frame == 2
                && let Some(name) = name
            {
                capture.save(format!("/tmp/clay-ring-{name}.png"), &rgba);
            }
        }
    };
    render(&mut app, &mut capture, None);
    for (mode, name) in [(RingMode::Level, "level"), (RingMode::Shortest, "shortest")] {
        app.viewport.paths.ring_mode = mode;
        let start = rect.center() + app.camera.pan;
        navigation_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        );
        let end = start + Vec2::new(0., -65.);
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        assert!(
            app.viewport.paths.draft.as_ref().unwrap().closed,
            "{}",
            app.ui.status
        );
        assert_complete_cylinder_ring(&app.viewport.paths.draft.as_ref().unwrap().points);
        render(&mut app, &mut capture, Some(name));
        navigation_frame(
            &mut app,
            &ctx,
            vec![primary_event(end, false), egui::Event::PointerGone],
        );
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.history_counts(), history);
    }
    app.viewport.destroy_renderer();
}

fn corner_path_setup() -> (App, egui::Context, egui::Rect) {
    use crate::sculpt::{Geometry, Primitive, PrimitiveKind, Vec3};
    let mut app = App::default();
    let mut grid = Primitive::for_kind(PrimitiveKind::Grid);
    grid.divisions = [12; 3];
    app.document
        .replace_geometry(Geometry::new(grid.mesh().unwrap()))
        .unwrap();
    app.camera.pan = Vec2::new(180., 0.);
    let ctx = egui::Context::default();
    app.dispatch(Command::Path(PathCommand::New(PathMode::Surface)), &ctx);
    let draft = app.viewport.paths.draft.as_mut().unwrap();
    draft.name = "Individual rounded corner".into();
    draft.points = [(-0.65, -0.45), (0., 0.45), (0.65, -0.45)]
        .into_iter()
        .map(|(x, y)| PathPoint {
            position: Vec3(x, y, 0.),
            normal: Vec3(0., 0., 1.),
        })
        .collect();
    draft.smoothing = 0.;
    app.viewport.paths.tool = PathTool::Move;
    app.viewport.paths.selected = Some(1);
    let rect = navigation_frame(&mut app, &ctx, vec![]);
    (app, ctx, rect)
}

#[test]
fn path_corner_handle_drag_cancel_navigation_and_save_are_transactional() {
    let (mut app, ctx, rect) = corner_path_setup();
    let original = app.viewport.paths.draft.clone();
    let geometry = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let tip = project_guide_world(&app, rect, original.as_ref().unwrap().points[1].position);
    let diamond = tip + Vec2::new(0., 22.);
    let end = diamond + Vec2::new(0., 40.);
    for cancel in [true, false] {
        navigation_frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::PointerMoved(diamond),
                primary_event(diamond, true),
            ],
        );
        navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
        let draft = app.viewport.paths.draft.as_ref().unwrap();
        assert!(
            draft.corner_rounding(1) > 0.2,
            "diamond drag must round the selected corner"
        );
        assert_eq!(
            draft.points,
            original.as_ref().unwrap().points,
            "rounding keeps control points fixed"
        );
        assert_eq!(draft.corner_rounding(0), 0.);
        assert_eq!(draft.corner_rounding(2), 0.);
        assert_eq!(app.document.geometry_snapshot(), geometry);
        assert_eq!(app.document.history_counts(), history);
        if cancel {
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
        navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
        if cancel {
            assert_eq!(app.viewport.paths.draft, original);
            navigation_frame(
                &mut app,
                &ctx,
                vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }
    let rounded = app.viewport.paths.draft.clone();
    let secondary = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let yaw = app.camera.yaw;
    navigation_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(end), secondary(end, true)],
    );
    let orbit_end = end + Vec2::new(50., 25.);
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(orbit_end)]);
    navigation_frame(&mut app, &ctx, vec![secondary(orbit_end, false)]);
    assert_ne!(app.camera.yaw, yaw);
    assert_eq!(
        app.viewport.paths.draft, rounded,
        "orbit must not move a rounding handle"
    );
    app.dispatch(Command::Path(PathCommand::Save), &ctx);
    assert_eq!(app.document.paths().len(), 1, "{}", app.ui.status);
    assert_eq!(
        app.document.paths()[0].corner_rounding,
        rounded.unwrap().corner_rounding
    );
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    // Closing the editor also leaves the saved edit available to ordinary Undo.
    app.dispatch(Command::Path(PathCommand::Close), &ctx);
    app.dispatch(Command::Undo, &ctx);
    assert_eq!(app.document.geometry_snapshot(), geometry);
}

#[test]
fn closed_path_seam_corner_has_an_editable_rounding_handle() {
    let (mut app, ctx, rect) = corner_path_setup();
    let draft = app.viewport.paths.draft.as_mut().unwrap();
    draft.set_closed(true).unwrap();
    draft.set_seam(1).unwrap();
    let original = draft.clone();
    let tip = project_guide_world(&app, rect, original.points[1].position);
    let diamond = tip + Vec2::new(0., 22.);
    let end = diamond + Vec2::new(0., 32.);
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(diamond),
            primary_event(diamond, true),
        ],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(end)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(end, false)]);
    let rounded = app.viewport.paths.draft.as_ref().unwrap();
    assert!(rounded.corner_rounding(1) > 0.15);
    assert_eq!(rounded.seam, 1);
    assert_eq!(rounded.points, original.points);
    let before_amount = rounded.corner_rounding(1);
    let direction = (rounded.points[2].position - rounded.points[1].position).norm();
    let edge_handle = project_guide_world(
        &app,
        rect,
        rounded.points[1].position + direction * before_amount,
    );
    let screen_direction = (edge_handle - tip).normalized();
    let reduced = edge_handle - screen_direction * 14.;
    navigation_frame(
        &mut app,
        &ctx,
        vec![
            egui::Event::PointerMoved(edge_handle),
            primary_event(edge_handle, true),
        ],
    );
    navigation_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(reduced)]);
    navigation_frame(&mut app, &ctx, vec![primary_event(reduced, false)]);
    let rounded = app.viewport.paths.draft.as_ref().unwrap();
    assert!(
        rounded.corner_rounding(1) < before_amount - 0.02,
        "the outgoing trim handle must also adjust rounding"
    );
    assert_eq!(rounded.points, original.points);
    assert!(app.document.paths().is_empty());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Vulkan adapter (software lavapipe supported)"]
fn rounded_path_corners_offscreen_preview() {
    let (mut app, _, _) = corner_path_setup();
    app.viewport
        .paths
        .draft
        .as_mut()
        .unwrap()
        .set_corner_rounding(1, 0.45)
        .unwrap();
    let before = app.document.geometry_snapshot();
    let ctx = egui::Context::default();
    let mut capture = crate::viewport::UiCapture::new([1000, 800]);
    app.viewport.initialize_renderer(&capture.state).unwrap();
    for frame in 0..4 {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000., 800.),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
        let rgba = capture.render(&ctx, output);
        if frame == 3 {
            capture.save("/tmp/clay-rounded-path-corners.png", &rgba);
        }
    }
    let path = app.viewport.paths.draft.as_ref().unwrap();
    assert!(path.corner_rounding(1) > 0.4);
    let max_y = path
        .dense_curve()
        .iter()
        .map(|point| point.position.1)
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max_y < 0.40,
        "the rendered route must cut around the sharp corner"
    );
    assert_eq!(app.document.geometry_snapshot(), before);
    app.viewport.destroy_renderer();
}
