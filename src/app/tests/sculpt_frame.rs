use super::*;
use crate::viewport::frame_trace::{Capture, Sample, Stage};

fn exercise(
    mut render: impl FnMut(&egui::Context, egui::FullOutput),
    app: &mut App,
) -> Vec<Sample> {
    let ctx = egui::Context::default();
    // Floating controls fade in by input time; exclude that unrelated animation
    // from exact same-geometry framebuffer comparisons.
    ctx.style_mut(|style| style.animation_time = 0.);
    let before = app.document.geometry_snapshot();
    let history = app.document.history_counts();
    let capture = Capture::start();
    let start = Pos2::new(460., 400.);
    let end = Pos2::new(540., 400.);
    let frames = [
        vec![],
        vec![egui::Event::PointerMoved(start), primary_event(start, true)],
        vec![egui::Event::PointerMoved(end)],
        vec![primary_event(end, false)],
        vec![],
    ];
    for (frame, events) in frames.into_iter().enumerate() {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1000., 800.),
                )),
                time: Some(frame as f64 / 60.),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
            },
        );
        render(&ctx, output);
        crate::viewport::frame_trace::record(Stage::Complete {
            revision: app.document.display_revision(),
            stroke_active: app.document.stroke_active(),
        });
    }
    let samples = capture.finish();
    let dabs = |frame| {
        samples
            .iter()
            .filter(|s| s.frame == frame && matches!(s.stage, Stage::Dab { .. }))
            .count()
    };
    assert_eq!(dabs(2), 1, "first press produces one dab");
    assert!(
        dabs(3) > 1 && dabs(3) <= 128,
        "pointer jump must exercise a burst"
    );
    assert_eq!(
        dabs(4),
        0,
        "release without movement creates no extra Standard dab"
    );
    assert_eq!(dabs(5), 0);
    for frame in 1..=5 {
        let input = samples
            .iter()
            .find_map(|s| match s.stage {
                Stage::Input { revision, .. } if s.frame == frame => Some(revision),
                _ => None,
            })
            .unwrap();
        let scene = samples
            .iter()
            .find_map(|s| match s.stage {
                Stage::Scene { revision } if s.frame == frame => Some(revision),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            scene, input,
            "picking/display snapshot precedes freehand edits"
        );
        for s in samples.iter().filter(|s| s.frame == frame) {
            if let Stage::Callback { revision } = s.stage {
                assert_eq!(
                    revision,
                    Some(scene),
                    "GPU callback owns the prepared snapshot"
                );
            }
        }
        let (end_revision, stroke_active) = samples
            .iter()
            .find_map(|s| match s.stage {
                Stage::Complete {
                    revision,
                    stroke_active,
                } if s.frame == frame => Some((revision, stroke_active)),
                _ => None,
            })
            .unwrap();
        if frame == 2 || frame == 3 {
            assert!(end_revision > scene);
        }
        if frame >= 4 {
            assert!(!stroke_active);
            assert_eq!(end_revision, scene);
        }
    }
    let after = app.document.geometry_snapshot();
    assert_ne!(after, before);
    assert_eq!(app.document.history_counts().0, history.0 + 1);
    app.document.undo();
    assert_eq!(app.document.geometry_snapshot(), before);
    // Keep the pointer and overlays identical for history pixel comparisons.
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            time: Some(5. / 60.),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
    render(&ctx, output);
    app.document.redo();
    assert_eq!(app.document.geometry_snapshot(), after);
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1000., 800.),
            )),
            time: Some(6. / 60.),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
        },
    );
    render(&ctx, output);
    samples
}

#[test]
fn sculpt_frame_revision_trace_cpu() {
    exercise(|_, _| {}, &mut App::default());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires Vulkan; exercises actual freehand input and queued render callbacks"]
fn sculpt_frame_revision_trace_gpu() {
    let mut app = App::default();
    let mut gpu = crate::viewport::UiCapture::new([1000, 800]);
    app.viewport.initialize_renderer(&gpu.state).unwrap();
    let mut frames = Vec::new();
    let samples = exercise(|ctx, output| frames.push(gpu.render(ctx, output)), &mut app);
    if let Some(path) = std::env::var_os("CLAY_SCULPT_FRAME_TRACE") {
        let path = std::path::PathBuf::from(path);
        for (i, pixels) in frames.iter().enumerate() {
            gpu.save(path.with_extension(format!("{i}.png")), pixels);
        }
    }
    assert_eq!(
        samples
            .iter()
            .filter(|s| matches!(s.stage, Stage::Callback { .. }))
            .count(),
        5
    );
    assert!(
        frames[5] != frames[6],
        "same-pointer Undo and Redo must render different surfaces"
    );
    let differences: Vec<_> = frames[4]
        .as_chunks::<4>()
        .0
        .iter()
        .zip(frames[6].as_chunks::<4>().0)
        .enumerate()
        .filter_map(|(i, (a, b))| (a != b).then_some(i))
        .collect();
    assert!(
        differences.is_empty(),
        "Redo framebuffer differs at {} pixels; first {:?}, last {:?}",
        differences.len(),
        differences.first(),
        differences.last()
    );
    if let Some(path) = std::env::var_os("CLAY_SCULPT_FRAME_TRACE") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({
            "schema": 1,
            "scope": "CPU frame/input/callback sequence; offscreen readback follows each frame. No OS presentation measurement.",
            "adapter": format!("{:?}", gpu.state.adapter.get_info()),
            "samples": samples,
        })).unwrap()).unwrap();
    }
    app.viewport.destroy_renderer();
}
