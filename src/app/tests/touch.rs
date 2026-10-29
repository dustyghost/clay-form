use super::*;
use egui::{Event, MouseWheelUnit, TouchPhase};

fn touch(id: u64, phase: TouchPhase, x: f32, y: f32) -> Event {
    Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos: Pos2::new(x, y),
        force: None,
    }
}

fn frame(app: &mut App, ctx: &egui::Context, time: f64, events: Vec<Event>) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            time: Some(time),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
}

fn setup() -> (App, egui::Context) {
    let mut app = App::default();
    let ctx = egui::Context::default();
    for n in 0..3 {
        frame(&mut app, &ctx, n as f64 * 0.01, vec![]);
    }
    (app, ctx)
}

fn tap(app: &mut App, ctx: &egui::Context, fingers: u64, time: f64) {
    frame(
        app,
        ctx,
        time,
        (1..=fingers)
            .map(|id| touch(id, TouchPhase::Start, 400. + id as f32 * 60., 400.))
            .collect(),
    );
    // Real integrations emulate a primary pointer only for the first contact.
    for id in 1..=fingers {
        frame(
            app,
            ctx,
            time + 0.04 + id as f64 * 0.02,
            vec![touch(id, TouchPhase::End, 400. + id as f32 * 60., 400.)],
        );
    }
}

#[test]
fn touch_taps_undo_redo_once_and_use_configured_actions() {
    let (mut app, ctx) = setup();
    app.document.mask_region(|_| true, false).unwrap();
    assert!(app.document.can_undo());
    tap(&mut app, &ctx, 2, 1.);
    assert!(!app.document.can_undo());
    assert!(app.document.can_redo());
    tap(&mut app, &ctx, 3, 2.);
    assert!(app.document.can_undo());
    assert!(!app.document.can_redo());
    app.ui.preferences.touch.two_tap = Some(Action::Wireframe);
    let before = app.ui.wire;
    tap(&mut app, &ctx, 2, 3.);
    assert_ne!(app.ui.wire, before);
    app.ui.preferences.touch.two_tap = None;
    tap(&mut app, &ctx, 2, 4.);
    assert_ne!(app.ui.wire, before);
}

#[test]
fn touch_navigation_and_staggered_releases_never_sculpt_or_undo() {
    let (mut app, ctx) = setup();
    let before = app.document.geometry_snapshot();
    let p = Pos2::new(460., 400.);
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            touch(1, TouchPhase::Start, p.x, p.y),
            Event::PointerMoved(p),
            primary_event(p, true),
        ],
    );
    assert!(!app.document.can_undo());
    frame(
        &mut app,
        &ctx,
        1.02,
        vec![touch(2, TouchPhase::Start, 540., 400.)],
    );
    frame(
        &mut app,
        &ctx,
        1.04,
        vec![
            touch(1, TouchPhase::Move, 490., 420.),
            touch(2, TouchPhase::Move, 570., 420.),
            Event::PointerMoved(Pos2::new(490., 420.)),
        ],
    );
    assert_eq!(app.camera.pan, Vec2::new(30., 20.));
    assert_eq!(app.camera.zoom, 1.);
    assert_eq!(app.camera.yaw, 0.);
    frame(
        &mut app,
        &ctx,
        1.06,
        vec![
            touch(1, TouchPhase::Move, 450., 420.),
            touch(2, TouchPhase::Move, 610., 420.),
        ],
    );
    assert_eq!(app.camera.pan, Vec2::new(30., 20.));
    assert!((app.camera.zoom - 2.).abs() < 1e-5);
    frame(
        &mut app,
        &ctx,
        1.08,
        vec![
            touch(1, TouchPhase::End, 450., 420.),
            primary_event(p, false),
            Event::PointerGone,
        ],
    );
    frame(
        &mut app,
        &ctx,
        1.1,
        vec![
            touch(2, TouchPhase::Move, 660., 470.),
            Event::PointerMoved(Pos2::new(660., 470.)),
        ],
    );
    frame(
        &mut app,
        &ctx,
        1.12,
        vec![
            touch(2, TouchPhase::End, 660., 470.),
            primary_event(p, false),
        ],
    );
    assert_eq!(app.camera.yaw, 0.);
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_undo());
    assert!(app.viewport.session.gesture.is_none());
}

#[test]
fn touch_one_finger_orbits_and_count_changes_do_not_jump() {
    let (mut app, ctx) = setup();
    frame(
        &mut app,
        &ctx,
        1.,
        vec![touch(1, TouchPhase::Start, 400., 400.)],
    );
    frame(
        &mut app,
        &ctx,
        1.03,
        vec![touch(1, TouchPhase::Move, 440., 420.)],
    );
    assert!(app.camera.yaw.abs() > 0.1);
    let before = app.camera;
    frame(
        &mut app,
        &ctx,
        1.05,
        vec![touch(2, TouchPhase::Start, 700., 500.)],
    );
    assert_eq!(app.camera.pan, before.pan);
    assert_eq!(app.camera.zoom, before.zoom);
    assert_eq!(app.camera.yaw, before.yaw);
    frame(
        &mut app,
        &ctx,
        1.06,
        vec![touch(3, TouchPhase::Start, 500., 500.)],
    );
    frame(
        &mut app,
        &ctx,
        1.07,
        vec![touch(3, TouchPhase::End, 500., 500.)],
    );
    frame(
        &mut app,
        &ctx,
        1.08,
        vec![
            touch(1, TouchPhase::Move, 480., 420.),
            touch(2, TouchPhase::Move, 740., 500.),
        ],
    );
    assert_eq!(app.camera.pan, before.pan);
    assert!(!app.document.can_undo());
}

#[test]
fn touch_long_moved_cancelled_extra_finger_and_outside_taps_do_not_act() {
    for case in 0..6 {
        let (mut app, ctx) = setup();
        app.document.mask_region(|_| true, false).unwrap();
        let x = if case == 4 { -20. } else { 460. };
        frame(
            &mut app,
            &ctx,
            1.,
            vec![
                touch(1, TouchPhase::Start, x, 400.),
                touch(2, TouchPhase::Start, 540., 400.),
            ],
        );
        if case == 1 {
            frame(
                &mut app,
                &ctx,
                1.02,
                vec![touch(1, TouchPhase::Move, 500., 400.)],
            );
            frame(
                &mut app,
                &ctx,
                1.03,
                vec![touch(1, TouchPhase::Move, 460., 400.)],
            );
        }
        if case == 3 {
            frame(
                &mut app,
                &ctx,
                1.02,
                vec![
                    touch(3, TouchPhase::Start, 600., 400.),
                    touch(4, TouchPhase::Start, 650., 400.),
                ],
            );
            frame(
                &mut app,
                &ctx,
                1.03,
                vec![
                    touch(3, TouchPhase::End, 600., 400.),
                    touch(4, TouchPhase::End, 650., 400.),
                ],
            );
        }
        if case == 5 {
            app.ui.settings_open = true;
        }
        let time = if case == 0 { 2. } else { 1.1 };
        let phase = if case == 2 {
            TouchPhase::Cancel
        } else {
            TouchPhase::End
        };
        frame(
            &mut app,
            &ctx,
            time,
            vec![
                touch(1, phase, x, 400.),
                touch(2, TouchPhase::End, 540., 400.),
            ],
        );
        assert!(app.document.can_undo(), "case {case}");
        assert!(!app.document.can_redo(), "case {case}");
    }
}

fn wheel(unit: MouseWheelUnit, delta: Vec2) -> Event {
    Event::MouseWheel {
        unit,
        delta,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn touch_trackpad_scroll_pans_without_a_zoom_tail_and_wheel_still_zooms() {
    let (mut app, ctx) = setup();
    let p = Pos2::new(500., 400.);
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            Event::PointerMoved(p),
            wheel(MouseWheelUnit::Point, Vec2::new(40., 25.)),
        ],
    );
    assert_eq!(app.camera.pan, Vec2::new(40., 25.));
    for i in 1..20 {
        frame(&mut app, &ctx, 1. + i as f64 * 0.02, vec![]);
    }
    assert_eq!(app.camera.zoom, 1.);
    frame(
        &mut app,
        &ctx,
        2.,
        vec![wheel(MouseWheelUnit::Line, Vec2::new(0., 1.))],
    );
    assert!(app.camera.zoom > 1.);
    let before = app.camera.zoom;
    frame(&mut app, &ctx, 3., vec![Event::Zoom(1.5)]);
    assert!(app.camera.zoom >= before * 1.5);
    assert!(!app.document.can_undo());
}

#[test]
fn touch_trackpad_button_taps_are_opt_in_and_drags_do_not_undo() {
    let (mut app, ctx) = setup();
    app.document.mask_region(|_| true, false).unwrap();
    let p = Pos2::new(500., 400.);
    let button = |button, pressed, pos| Event::PointerButton {
        button,
        pressed,
        pos,
        modifiers: egui::Modifiers::NONE,
    };
    for (time, enabled) in [(1., false), (2., true)] {
        app.ui.preferences.touch.button_taps = enabled;
        frame(
            &mut app,
            &ctx,
            time,
            vec![
                Event::PointerMoved(p),
                button(egui::PointerButton::Secondary, true, p),
            ],
        );
        frame(
            &mut app,
            &ctx,
            time + 0.05,
            vec![button(egui::PointerButton::Secondary, false, p)],
        );
        assert_eq!(app.document.can_redo(), enabled);
    }
    frame(
        &mut app,
        &ctx,
        3.,
        vec![
            button(egui::PointerButton::Middle, true, p),
            button(egui::PointerButton::Middle, false, p),
        ],
    );
    assert!(app.document.can_undo());
    frame(
        &mut app,
        &ctx,
        4.,
        vec![button(egui::PointerButton::Secondary, true, p)],
    );
    let end = p + Vec2::new(50., 0.);
    frame(&mut app, &ctx, 4.05, vec![Event::PointerMoved(end)]);
    frame(
        &mut app,
        &ctx,
        4.1,
        vec![button(egui::PointerButton::Secondary, false, end)],
    );
    assert!(app.camera.yaw.abs() > 0.1);
    assert!(!app.document.can_redo());
}

#[test]
fn touch_pinch_configuration_clamps_zoom_and_ctrl_wheel_mapping_works() {
    let (mut app, ctx) = setup();
    app.ui.preferences.touch.reverse_pinch = true;
    app.ui.preferences.touch.pinch_speed = 2.;
    frame(
        &mut app,
        &ctx,
        1.,
        vec![Event::PointerMoved(Pos2::new(500., 400.)), Event::Zoom(2.)],
    );
    assert_eq!(app.camera.zoom, 0.25);
    frame(&mut app, &ctx, 1.1, vec![Event::Zoom(100.)]);
    // Large imports can be framed below the former 20% minimum. Keep the
    // sensitivity/reversal check separate from reaching the actual limit.
    assert!((app.camera.zoom - 2.5e-5).abs() < 1e-10);
    frame(&mut app, &ctx, 1.15, vec![Event::Zoom(100.)]);
    assert_eq!(app.camera.zoom, Camera::MIN_ZOOM);
    frame(
        &mut app,
        &ctx,
        1.2,
        vec![Event::Zoom(f32::NAN), Event::Zoom(-1.)],
    );
    assert_eq!(app.camera.zoom, Camera::MIN_ZOOM);
    app.ui.preferences.touch.reverse_pinch = false;
    frame(
        &mut app,
        &ctx,
        1.3,
        vec![Event::MouseWheel {
            unit: MouseWheelUnit::Line,
            delta: Vec2::new(0., 2.),
            modifiers: egui::Modifiers::CTRL,
        }],
    );
    assert!(app.camera.zoom > Camera::MIN_ZOOM);
    let before = app.camera.zoom;
    app.ui.preferences.touch.pinch = false;
    frame(&mut app, &ctx, 1.4, vec![Event::Zoom(2.)]);
    assert_eq!(app.camera.zoom, before);
}

#[test]
fn touch_does_not_start_on_overlay_and_cancelled_contacts_can_restart() {
    let (mut app, ctx) = setup();
    app.document.mask_region(|_| true, false).unwrap();
    let button = ctx
        .data(|d| d.get_temp::<egui::Rect>(egui::Id::new(("viewport_control_response", 0))))
        .unwrap()
        .center();
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            touch(1, TouchPhase::Start, button.x, button.y),
            touch(2, TouchPhase::Start, 500., 400.),
        ],
    );
    frame(
        &mut app,
        &ctx,
        1.1,
        vec![
            touch(1, TouchPhase::End, button.x, button.y),
            touch(2, TouchPhase::End, 500., 400.),
        ],
    );
    assert!(!app.document.can_redo());
    frame(
        &mut app,
        &ctx,
        2.,
        vec![
            touch(1, TouchPhase::Start, 460., 400.),
            touch(2, TouchPhase::Start, 540., 400.),
        ],
    );
    frame(
        &mut app,
        &ctx,
        2.1,
        vec![
            touch(1, TouchPhase::Cancel, 460., 400.),
            touch(2, TouchPhase::End, 540., 400.),
        ],
    );
    assert!(!app.document.can_redo());
    tap(&mut app, &ctx, 2, 3.);
    assert!(app.document.can_redo());
}

#[test]
fn touch_disabled_restores_existing_pointer_navigation() {
    let (mut app, ctx) = setup();
    app.ui.preferences.touch.enabled = false;
    let p = Pos2::new(100., 100.);
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            touch(1, TouchPhase::Start, p.x, p.y),
            Event::PointerMoved(p),
            primary_event(p, true),
        ],
    );
    frame(
        &mut app,
        &ctx,
        1.1,
        vec![
            touch(1, TouchPhase::Move, 160., 120.),
            Event::PointerMoved(Pos2::new(160., 120.)),
        ],
    );
    assert!(app.camera.yaw.abs() > 0.1);
    frame(
        &mut app,
        &ctx,
        1.2,
        vec![
            touch(1, TouchPhase::End, 160., 120.),
            primary_event(p, false),
        ],
    );
    assert!(!app.document.can_undo());
}

#[test]
fn touch_batched_drag_keeps_final_movement_and_never_becomes_a_tap() {
    let (mut app, ctx) = setup();
    app.document.mask_region(|_| true, false).unwrap();
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            touch(1, TouchPhase::Start, 460., 400.),
            touch(2, TouchPhase::Start, 540., 400.),
            touch(1, TouchPhase::Move, 500., 420.),
            touch(2, TouchPhase::Move, 580., 420.),
            touch(1, TouchPhase::End, 500., 420.),
            touch(2, TouchPhase::End, 580., 420.),
        ],
    );
    assert_eq!(app.camera.pan, Vec2::new(40., 20.));
    assert_eq!(app.camera.zoom, 1.);
    assert!(!app.document.can_redo());
}

#[test]
fn touch_focus_loss_cancels_taps_without_leaving_navigation_stuck() {
    let (mut app, ctx) = setup();
    app.document.mask_region(|_| true, false).unwrap();
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            touch(1, TouchPhase::Start, 460., 400.),
            touch(2, TouchPhase::Start, 540., 400.),
        ],
    );
    let _ = ctx.run(
        egui::RawInput {
            focused: false,
            time: Some(1.05),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
    frame(
        &mut app,
        &ctx,
        1.1,
        vec![
            touch(1, TouchPhase::End, 460., 400.),
            touch(2, TouchPhase::End, 540., 400.),
        ],
    );
    assert!(!app.document.can_redo());
    tap(&mut app, &ctx, 2, 2.);
    assert!(app.document.can_redo());
}

#[test]
fn touch_twist_rolls_in_screen_space_without_orbiting_editing_or_undoing() {
    let (mut app, ctx) = setup();
    app.document.mask_region(|_| true, false).unwrap();
    let before = app.document.geometry_snapshot();
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            touch(2, TouchPhase::Start, 580., 400.),
            touch(1, TouchPhase::Start, 420., 400.),
        ],
    );
    frame(
        &mut app,
        &ctx,
        1.04,
        vec![
            touch(1, TouchPhase::Move, 500., 320.),
            touch(2, TouchPhase::Move, 500., 480.),
        ],
    );
    assert!((app.camera.roll - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    assert_eq!(app.camera.yaw, 0.);
    assert_eq!(app.camera.pitch, 0.);
    assert_eq!(app.camera.pan, Vec2::ZERO);
    assert_eq!(app.camera.zoom, 1.);
    // Continue twisting while panning and pinching, crossing the -pi/pi seam.
    for (i, degrees) in [170_f32, 190., 270.].into_iter().enumerate() {
        let angle = degrees.to_radians();
        let arm = Vec2::new(angle.cos(), angle.sin()) * 120.;
        frame(
            &mut app,
            &ctx,
            1.06 + i as f64 * 0.02,
            vec![
                touch(1, TouchPhase::Move, 520. - arm.x, 420. - arm.y),
                touch(2, TouchPhase::Move, 520. + arm.x, 420. + arm.y),
            ],
        );
    }
    assert!((app.camera.roll - 270_f32.to_radians()).abs() < 1e-5);
    assert_eq!(app.camera.pan, Vec2::splat(20.));
    assert!((app.camera.zoom - 1.5).abs() < 1e-5);
    frame(
        &mut app,
        &ctx,
        1.2,
        vec![
            touch(1, TouchPhase::End, 520., 540.),
            touch(2, TouchPhase::End, 520., 300.),
        ],
    );
    assert_eq!(app.document.geometry_snapshot(), before);
    assert!(!app.document.can_redo());
}

#[test]
fn touch_twist_configuration_and_native_trackpad_rotation_share_direction() {
    let (mut app, ctx) = setup();
    app.ui.preferences.touch.twist_speed = 2.;
    app.ui.preferences.touch.reverse_twist = true;
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            Event::PointerMoved(Pos2::new(500., 400.)),
            Event::Rotate(0.3),
        ],
    );
    assert!((app.camera.roll + 0.6).abs() < 1e-6);
    app.ui.preferences.touch.twist = false;
    frame(&mut app, &ctx, 1.1, vec![Event::Rotate(0.3)]);
    assert!((app.camera.roll + 0.6).abs() < 1e-6);
    frame(
        &mut app,
        &ctx,
        2.,
        vec![
            touch(1, TouchPhase::Start, 420., 400.),
            touch(2, TouchPhase::Start, 580., 400.),
        ],
    );
    frame(
        &mut app,
        &ctx,
        2.1,
        vec![
            touch(1, TouchPhase::Move, 500., 320.),
            touch(2, TouchPhase::Move, 500., 480.),
        ],
    );
    assert!((app.camera.roll + 0.6).abs() < 1e-6);
    assert!(!app.document.can_undo());
}

#[test]
fn touch_twist_near_coincident_contacts_rebases_without_flipping() {
    let (mut app, ctx) = setup();
    frame(
        &mut app,
        &ctx,
        1.,
        vec![
            touch(1, TouchPhase::Start, 420., 400.),
            touch(2, TouchPhase::Start, 580., 400.),
        ],
    );
    frame(
        &mut app,
        &ctx,
        1.1,
        vec![
            touch(1, TouchPhase::Move, 495., 400.),
            touch(2, TouchPhase::Move, 505., 400.),
        ],
    );
    frame(
        &mut app,
        &ctx,
        1.2,
        vec![
            touch(1, TouchPhase::Move, 580., 400.),
            touch(2, TouchPhase::Move, 420., 400.),
        ],
    );
    assert_eq!(app.camera.roll, 0.);
}

#[test]
fn touch_roll_axis_snap_returns_to_an_upright_view() {
    let (mut app, ctx) = setup();
    app.camera.roll = 0.7;
    frame(&mut app, &ctx, 1., vec![]);
    let rect = app.ui.viewport_menu.viewport_rect.unwrap();
    let center = Pos2::new(rect.right() - 79., rect.top() + 79.);
    let point = center + Vec2::new(0.7_f32.cos(), 0.7_f32.sin()) * 42.;
    frame(
        &mut app,
        &ctx,
        1.1,
        vec![Event::PointerMoved(point), primary_event(point, true)],
    );
    frame(&mut app, &ctx, 1.2, vec![primary_event(point, false)]);
    assert_eq!(app.camera.roll, 0.);
    assert!((app.camera.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert!(!app.document.can_undo());
}
