//! Temporary interaction state. Dropping a session cancels private stroke previews.
use super::*;

pub(super) struct SnakeStroke {
    pub center: Vec3,
    pub normal: Vec3,
    pub mobility: f32,
}
#[derive(Default)]
pub(crate) struct InteractionSession {
    pub(super) sculpt: SculptSession,
    pub(crate) visibility_hold: Option<crate::preferences::HoldBinding>,
    pub(crate) guided: guided::GuidedPaths,
    pub(crate) attach_stencil: bool,
    pub(super) stencil_overlay: stencil::Overlay,
    pub(crate) projection_preview: Option<std::sync::Arc<crate::sculpt::Mesh>>,
    pub(crate) projection_mode: bool,
    pub(crate) gesture: Option<Gesture>,
    pub(super) gizmo_dragged: bool,
    pub(super) gizmo_start: Option<Pos2>,
    pub(super) gizmo_snap: Option<(f32, f32)>,
    pub(super) transform: transform::TransformControls,
    pub(super) primitive: primitive::PrimitiveControls,
    pub(super) slice: Option<slice::SliceDrag>,
    pub(super) selection: Option<visibility::SelectionDrag>,
    pub(super) visibility_path: Option<visibility_path::VisibilityPath>,
    pub(crate) visibility_path_action: Option<crate::preferences::Action>,
    pub(super) slice_path: Option<slice::SlicePathDraft>,
}
#[derive(Default)]
pub(super) struct SculptSession {
    pub stroke: bool,
    pub stroke_brush: Option<String>,
    pub decimate_removed: Option<(usize, usize)>,
    pub pending_stroke: Option<stroke::PendingStroke>,
    pub stabilizer: Option<stabilizer::Stabilizer>,
    pub last: Option<Pos2>,
    pub paint: PaintSampler,
    pub snake: Option<SnakeStroke>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PaintSample {
    pub point: Pos2,
    pub pressure: f32,
    pub seconds: f32,
}
#[derive(Default)]
pub(super) struct PaintSampler {
    previous: Option<(Pos2, f32, f64)>,
    distance: f32,
    deposited: Option<(Pos2, f32, f64)>,
}
impl PaintSampler {
    pub fn sample(
        &mut self,
        point: Pos2,
        pressure: f32,
        now: f64,
        spacing: f32,
        continuous: bool,
        finish: bool,
    ) -> Vec<PaintSample> {
        let spacing = spacing.max(1.);
        let Some((previous, old_pressure, old_time)) =
            self.previous.replace((point, pressure, now))
        else {
            self.deposited = Some((point, pressure, now));
            return vec![PaintSample {
                point,
                pressure,
                seconds: if continuous { 0.05 } else { 0. },
            }];
        };
        let mut samples = Vec::new();
        if continuous {
            // Fixed time boundaries keep spray seeds and deposition independent of repaint rate.
            let mut deposit = self.deposited.unwrap();
            loop {
                let tick = deposit.2 + 0.05;
                let time = if tick <= now + 1e-9 {
                    tick.min(now)
                } else if finish && now > deposit.2 {
                    now
                } else {
                    break;
                };
                let fraction = if now > old_time {
                    ((time - old_time) / (now - old_time)).clamp(0., 1.) as f32
                } else {
                    1.
                };
                let end = previous + (point - previous) * fraction;
                let end_pressure = old_pressure + (pressure - old_pressure) * fraction;
                let steps = (end.distance(deposit.0) / spacing).ceil().max(1.) as usize;
                for step in 1..=steps {
                    let t = step as f32 / steps as f32;
                    samples.push(PaintSample {
                        point: deposit.0 + (end - deposit.0) * t,
                        pressure: deposit.1 + (end_pressure - deposit.1) * t,
                        seconds: ((time - deposit.2) / steps as f64) as f32,
                    });
                }
                deposit = (end, end_pressure, time);
                if time >= now {
                    break;
                }
            }
            self.deposited = Some(deposit);
        } else {
            let length = point.distance(previous);
            let mut along = (spacing - self.distance).max(0.);
            while length > 0. && along <= length + 1e-5 {
                let t = (along / length).clamp(0., 1.);
                samples.push(PaintSample {
                    point: previous + (point - previous) * t,
                    pressure: old_pressure + (pressure - old_pressure) * t,
                    seconds: 0.,
                });
                along += spacing;
            }
            self.distance = if samples.is_empty() {
                self.distance + length
            } else {
                (length - (along - spacing)).max(0.)
            };
        }
        samples
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancel_drops_transient_state_without_resetting_persistent_selection() {
        let mut viewport = Viewport {
            edge_selection: vec![[0, 1]],
            edge_revision: 12,
            ..Default::default()
        };
        viewport.paths.active = true;
        viewport.session.gesture = Some(Gesture::Sculpt);
        viewport.session.sculpt.stroke = true;
        viewport.session.sculpt.last = Some(Pos2::new(1., 2.));
        viewport.session.sculpt.stroke_brush = Some("test".into());
        viewport.session.attach_stencil = true;
        let mut document = Document::default();
        document.begin_stroke();
        viewport.cancel(&mut document);
        assert!(viewport.session.gesture.is_none());
        assert!(!viewport.session.sculpt.stroke);
        assert!(viewport.session.sculpt.last.is_none());
        assert!(viewport.session.sculpt.stroke_brush.is_none());
        assert!(!viewport.session.attach_stencil);
        assert!(viewport.paths.active);
        assert_eq!(viewport.edge_selection, vec![[0, 1]]);
        assert_eq!(viewport.edge_revision, 12);
        assert!(!document.edit_in_progress());
    }
}

#[cfg(test)]
mod paint_sampling_tests {
    use super::*;
    fn line(events: usize, continuous: bool) -> Vec<PaintSample> {
        let mut sampler = PaintSampler::default();
        let mut samples = Vec::new();
        for i in 0..=events {
            let t = i as f32 / events as f32;
            samples.extend(sampler.sample(
                Pos2::new(t * 100., 0.),
                0.2 + 0.8 * t,
                t as f64,
                10.,
                continuous,
                i == events,
            ));
        }
        samples
    }
    #[test]
    fn paint_spacing_and_pressure_do_not_depend_on_event_partition() {
        for events in [1, 8, 100] {
            let samples = line(events, false);
            assert_eq!(samples.len(), 11);
            for (i, sample) in samples.iter().enumerate() {
                assert!((sample.point.x - i as f32 * 10.).abs() < 1e-4);
                assert!((sample.pressure - (0.2 + 0.08 * i as f32)).abs() < 1e-5);
            }
        }
    }
    #[test]
    fn paint_time_substeps_preserve_slow_frames_and_release_remainder() {
        for events in [1, 5, 20, 100] {
            let samples = line(events, true);
            assert!((samples.iter().map(|s| s.seconds).sum::<f32>() - 1.05).abs() < 1e-5);
            assert!(samples.iter().all(|s| s.seconds <= 0.050001));
        }
        let mut sampler = PaintSampler::default();
        sampler.sample(Pos2::ZERO, 1., 0., 10., true, false);
        assert!(
            sampler
                .sample(Pos2::ZERO, 1., 0.023, 10., true, false)
                .is_empty()
        );
        let final_sample = sampler.sample(Pos2::ZERO, 1., 0.027, 10., true, true);
        assert_eq!(final_sample.len(), 1);
        assert!((final_sample[0].seconds - 0.027).abs() < 1e-6);
    }
}
