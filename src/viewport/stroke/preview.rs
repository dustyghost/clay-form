//! Cheap live placement, with cancellable full-detail previews after a short pause.
use super::{PendingStroke, Scene};
use crate::{
    document::Document,
    sculpt::{AdaptiveBudget, AdaptiveSettings, Brush, Dab, Kind, Mesh, alpha::Projection},
    viewport::gpu::DisplayMesh,
};
use eframe::egui;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
    },
    time::{Duration, Instant},
};

const SETTLE: Duration = Duration::from_millis(120);

#[derive(Clone, PartialEq)]
pub(super) struct Request {
    revision: u64,
    brush: Brush,
    settings: AdaptiveSettings,
    dabs: Vec<Vec<Dab>>,
    stencils: Vec<Projection>,
}
impl Request {
    pub fn new(stroke: &PendingStroke, document: &Document, scene: &Scene) -> Self {
        Self {
            revision: document.display_revision(),
            brush: stroke.posed_brush(),
            settings: document.mesh_settings().adaptive.clone(),
            dabs: stroke
                .dabs(scene)
                .into_iter()
                .map(|d| document.symmetry().dabs(d))
                .collect(),
            stencils: stroke
                .stencil
                .as_ref()
                .map_or_else(Vec::new, |s| s.copies(document.symmetry())),
        }
    }
    pub fn render(
        &self,
        source: &Mesh,
        detailed: bool,
        cancelled: &impl Fn() -> bool,
    ) -> Option<Mesh> {
        if cancelled() {
            return None;
        }
        let mut mesh = source.clone();
        if !self.stencils.is_empty()
            && matches!(self.brush.kind, Kind::Grab | Kind::Snake | Kind::Detail)
        {
            return Some(mesh);
        }
        let mut budget = AdaptiveBudget::default();
        for dabs in &self.dabs {
            if cancelled() {
                return None;
            }
            let region = crate::sculpt::AdaptiveRegion {
                dabs,
                stencils: &self.stencils,
            };
            if detailed {
                mesh.adapt_stenciled(&self.brush, region, &self.settings, &mut budget, true);
                mesh.prepare_stenciled_stamp_cancellable(
                    &self.brush,
                    dabs,
                    &self.stencils,
                    &self.settings,
                    cancelled,
                );
            }
            if cancelled() {
                return None;
            }
            mesh.stamp_many_stenciled(&self.brush, dabs, &self.stencils);
            if detailed {
                mesh.adapt_stenciled(&self.brush, region, &self.settings, &mut budget, false);
            }
        }
        (!cancelled()).then_some(mesh)
    }
}

struct Job {
    request: Request,
    receiver: Receiver<Option<Mesh>>,
    cancel: Arc<AtomicBool>,
}
impl Job {
    fn start(request: Request, source: Arc<Mesh>, ctx: egui::Context) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let work = request.clone();
        std::thread::Builder::new()
            .name("clay-stamp-preview".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    work.render(&source, true, &|| flag.load(Ordering::Relaxed))
                }))
                .ok()
                .flatten();
                let _ = sender.send(result);
                ctx.request_repaint();
            })?;
        Ok(Self {
            request,
            receiver,
            cancel,
        })
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[derive(Default)]
struct State {
    request: Option<Request>,
    source: Option<Arc<Mesh>>,
    display: Option<Arc<DisplayMesh>>,
    changed: Option<Instant>,
    job: Option<Job>,
    finished: bool,
}

#[derive(Default)]
pub(crate) struct Preview(RefCell<State>);
impl Preview {
    pub fn cancel(&self) {
        if let Some(job) = &self.0.borrow().job {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }
    #[cfg(test)]
    pub fn refining(&self) -> bool {
        let state = self.0.borrow();
        state.request.is_some() && !state.finished
    }
    pub fn hint(&self) -> &'static str {
        let state = self.0.borrow();
        if state.finished || state.request.is_none() {
            "Release to apply · Esc to cancel"
        } else if state.job.is_some() {
            "Refining preview · Release to apply · Esc to cancel"
        } else {
            "Pause for detail · Release to apply · Esc to cancel"
        }
    }
    pub fn display(
        &self,
        stroke: &PendingStroke,
        document: &Document,
        scene: &Scene,
        ctx: &egui::Context,
    ) -> Arc<DisplayMesh> {
        self.update(
            Request::new(stroke, document, scene),
            document,
            ctx,
            Instant::now(),
        )
    }
    fn update(
        &self,
        request: Request,
        document: &Document,
        ctx: &egui::Context,
        now: Instant,
    ) -> Arc<DisplayMesh> {
        let mut state = self.0.borrow_mut();
        if state.request.as_ref() != Some(&request) {
            if let Some(job) = &state.job {
                job.cancel.store(true, Ordering::Relaxed);
            }
            if state
                .request
                .as_ref()
                .is_none_or(|r| r.revision != request.revision)
            {
                state.source = Some(Arc::new(document.mesh().clone()));
            }
            // Keep placement responsive; the final stamp still uses the complete
            // refinement path. Each pose is rebuilt from the original surface.
            let mesh = request
                .render(state.source.as_ref().unwrap(), false, &|| false)
                .unwrap();
            state.display = Some(Arc::new(DisplayMesh::with_active_mesh(document, &mesh)));
            state.finished = !request.settings.enabled;
            state.request = Some(request.clone());
            state.changed = Some(now);
        }
        if let Some(job) = &state.job {
            let result = match job.receiver.try_recv() {
                Ok(mesh) => Some(mesh),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => Some(None),
            };
            if let Some(result) = result {
                // Old pressure, invert, pose, stencil, view, or geometry must
                // never overwrite the current preview, even if a job just finished.
                if job.request == request && !job.cancel.load(Ordering::Relaxed) {
                    if let Some(mesh) = result {
                        state.display =
                            Some(Arc::new(DisplayMesh::with_active_mesh(document, &mesh)));
                    }
                    state.finished = true;
                }
                state.job = None;
            }
        }
        if !state.finished && state.job.is_none() {
            let elapsed = now.saturating_duration_since(state.changed.unwrap());
            if elapsed >= SETTLE {
                match Job::start(request, state.source.as_ref().unwrap().clone(), ctx.clone()) {
                    Ok(job) => state.job = Some(job),
                    Err(_) => state.finished = true,
                }
            } else {
                ctx.request_repaint_after(SETTLE - elapsed);
            }
        }
        state.display.as_ref().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Alpha, Geometry, Primitive, PrimitiveKind, StrokeMode, Vec3};

    fn setup() -> (Document, Request) {
        let mut document = Document::default();
        let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
        cube.divisions = [6; 3];
        document
            .replace_geometry(Geometry::new(cube.mesh().unwrap()))
            .unwrap();
        document
            .set_adaptive(AdaptiveSettings {
                enabled: true,
                ..Default::default()
            })
            .unwrap();
        let mut brush = Brush::new(Kind::Standard);
        brush.stroke_mode = StrokeMode::DragRect;
        brush.alpha = Alpha::Hard;
        let request = Request {
            revision: document.display_revision(),
            brush,
            settings: document.mesh_settings().adaptive.clone(),
            dabs: vec![vec![Dab {
                center: Vec3(0., 0., 1.),
                normal: Vec3(0., 0., 1.),
                right: Vec3(1., 0., 0.),
                up: Vec3(0., 1., 0.),
                radius: 0.6,
                pressure: 1.,
                delta: Vec3::default(),
                invert: false,
            }]],
            stencils: vec![],
        };
        (document, request)
    }

    #[test]
    fn stamp_preview_is_immediate_cached_and_refines_without_document_edits() {
        let (mut document, request) = setup();
        let original = document.geometry_snapshot();
        let history = document.history_counts();
        let ctx = egui::Context::default();
        let preview = Preview::default();
        let now = Instant::now();
        let coarse = preview.update(request.clone(), &document, &ctx, now);
        assert_eq!(coarse.mesh.vertices.len(), document.mesh().vertices.len());
        assert_ne!(coarse.mesh.vertices, document.mesh().vertices);
        assert!(
            preview.0.borrow().job.is_none(),
            "no refinement while moving"
        );
        let held = preview.update(request.clone(), &document, &ctx, now);
        assert!(
            Arc::ptr_eq(&coarse, &held),
            "idle frames reuse mesh and shading"
        );
        preview.update(request.clone(), &document, &ctx, now + SETTLE);
        let deadline = Instant::now() + Duration::from_secs(30);
        while preview.refining() {
            assert!(Instant::now() < deadline, "preview worker did not finish");
            std::thread::sleep(Duration::from_millis(5));
            preview.update(request.clone(), &document, &ctx, now + SETTLE);
        }
        let detailed = preview.update(request.clone(), &document, &ctx, now + SETTLE);
        assert!(detailed.mesh.vertices.len() > coarse.mesh.vertices.len());
        assert!(Arc::ptr_eq(
            &detailed,
            &preview.update(request.clone(), &document, &ctx, now + SETTLE)
        ));
        assert_eq!(document.geometry_snapshot(), original);
        assert_eq!(document.history_counts(), history);
        document.apply_dab(&request.brush, request.dabs[0][0]);
        document.end_stroke();
        assert_eq!(detailed.mesh.vertices, document.mesh().vertices);
        assert_eq!(detailed.mesh.triangles, document.mesh().triangles);
        document.undo();
        assert_eq!(document.geometry_snapshot(), original);
    }

    #[test]
    fn stamp_preview_discards_stale_work_and_cancels_on_drop() {
        let (document, request) = setup();
        let history = document.history_counts();
        let ctx = egui::Context::default();
        let now = Instant::now();
        let preview = Preview::default();
        let first = preview.update(request.clone(), &document, &ctx, now);
        let source = preview.0.borrow().source.clone().unwrap();
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        preview.0.borrow_mut().job = Some(Job {
            request: request.clone(),
            receiver,
            cancel: cancel.clone(),
        });
        let mut latest = request.clone();
        for i in 1..=20 {
            latest.dabs[0][0].radius = 0.6 + i as f32 * 0.01;
            latest.dabs[0][0].pressure = 0.7;
            latest.dabs[0][0].invert = true;
            preview.update(latest.clone(), &document, &ctx, now);
            assert!(Arc::ptr_eq(
                &source,
                preview.0.borrow().source.as_ref().unwrap()
            ));
            assert!(
                preview
                    .0
                    .borrow()
                    .job
                    .as_ref()
                    .unwrap()
                    .cancel
                    .load(Ordering::Relaxed)
            );
        }
        sender.send(Some(first.mesh.clone())).unwrap();
        let current = preview.update(latest.clone(), &document, &ctx, now);
        let expected = latest.render(document.mesh(), false, &|| false).unwrap();
        assert_eq!(current.mesh.vertices, expected.vertices);
        assert!(preview.0.borrow().job.is_none());
        assert!(cancel.load(Ordering::Relaxed));
        let (_sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        preview.0.borrow_mut().job = Some(Job {
            request: latest,
            receiver,
            cancel: cancel.clone(),
        });
        drop(preview);
        assert!(cancel.load(Ordering::Relaxed));
        assert_eq!(document.history_counts(), history);
    }

    #[test]
    fn stamp_preview_invalidates_source_and_projection_inputs() {
        let (mut document, mut request) = setup();
        let preview = Preview::default();
        let ctx = egui::Context::default();
        let now = Instant::now();
        let first = preview.update(request.clone(), &document, &ctx, now);
        let source = preview.0.borrow().source.clone().unwrap();
        request.dabs[0][0].right = Vec3(0., 1., 0.);
        request.dabs[0][0].up = Vec3(-1., 0., 0.);
        let rotated = preview.update(request.clone(), &document, &ctx, now);
        assert!(!Arc::ptr_eq(&first, &rotated));
        request.stencils.push(Projection {
            image: Alpha::Soft,
            frame: crate::sculpt::alpha::Frame {
                origin: Vec3(0., 0., 1.),
                right: Vec3(1., 0., 0.),
                up: Vec3(0., 1., 0.),
            },
            view: Vec3(0., 0., 1.),
            distance: 0.,
            position: [0.; 2],
            size: [1.; 2],
            rotation: 0.,
        });
        let stencil = preview.update(request.clone(), &document, &ctx, now);
        assert!(!Arc::ptr_eq(&rotated, &stencil));
        request.settings.detail = 4.;
        let settings = preview.update(request.clone(), &document, &ctx, now);
        assert!(!Arc::ptr_eq(&settings, &stencil));
        document.mask_region(|p| p.0 < 0., false).unwrap();
        request.revision = document.display_revision();
        let masked = preview.update(request.clone(), &document, &ctx, now);
        assert!(!Arc::ptr_eq(
            &source,
            preview.0.borrow().source.as_ref().unwrap()
        ));
        let expected = request.render(document.mesh(), false, &|| false).unwrap();
        assert_eq!(masked.mesh.vertices, expected.vertices);
        assert_eq!(masked.mesh.mask, expected.mask);
    }

    #[test]
    fn stamp_preview_cancellation_discards_partial_refinement() {
        let (document, request) = setup();
        let calls = std::cell::Cell::new(0);
        let result = request.render(document.mesh(), true, &|| {
            calls.set(calls.get() + 1);
            calls.get() >= 4
        });
        assert!(result.is_none());
        assert!(calls.get() >= 4, "exercise cancellation during refinement");
    }

    #[test]
    #[ignore = "manual release-mode stamp preview timings"]
    fn stamp_preview_profile() {
        let (mut document, mut request) = setup();
        document
            .replace_geometry(Geometry::new(Mesh::sphere()))
            .unwrap();
        request.revision = document.display_revision();
        request.brush.radius = 252.10057;
        request.brush.rotation = 55.801712;
        request.brush.alpha = crate::sculpt::alpha::builtins()
            .iter()
            .find(|a| a.name == "Reactor rings")
            .unwrap()
            .image
            .clone();
        request.dabs[0][0].center = Vec3(-0.18910993, 0.22439388, 0.9556);
        request.dabs[0][0].normal = Vec3(-0.18930681, 0.22462495, 0.955_88);
        request.dabs[0][0].radius = 0.8365707;
        let started = Instant::now();
        request.render(document.mesh(), true, &|| false).unwrap();
        eprintln!(
            "full refinement: {:.2} ms",
            started.elapsed().as_secs_f64() * 1000.
        );
        let ctx = egui::Context::default();
        let preview = Preview::default();
        let now = Instant::now();
        for i in 0..5 {
            request.dabs[0][0].radius += 0.001;
            let started = Instant::now();
            preview.update(request.clone(), &document, &ctx, now);
            let moving = started.elapsed();
            let started = Instant::now();
            preview.update(request.clone(), &document, &ctx, now);
            eprintln!(
                "frame {i}: moving {:.2} ms, cached {:.3} ms",
                moving.as_secs_f64() * 1000.,
                started.elapsed().as_secs_f64() * 1000.
            );
        }
    }
}
