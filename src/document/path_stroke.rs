//! Saved-path brush execution uses the ordinary sculpt pipeline on a private document.
use super::*;
use crate::sculpt::{
    Vec3,
    paths::{PathSample, SurfacePath},
};

const MAX_PATH_DABS: usize = 4096;
const MAX_VERTEX_DABS: usize = 100_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathStrokeOptions {
    /// Model-space radius, fixed at launch and independent of the camera.
    pub radius: f32,
    /// Constant pressure for every dab; the selected brush's pressure curve still applies.
    pub pressure: f32,
    pub reverse: bool,
    pub invert: bool,
}
impl Default for PathStrokeOptions {
    fn default() -> Self {
        Self {
            radius: 0.1,
            pressure: 1.,
            reverse: false,
            invert: false,
        }
    }
}
impl PathStrokeOptions {
    fn validate(self) -> Result<(), String> {
        if !self.radius.is_finite()
            || !(0.00001..=1000.).contains(&self.radius)
            || !self.pressure.is_finite()
            || !(0. ..=1.).contains(&self.pressure)
        {
            return Err(
                "Path strokes need a positive model-space radius and pressure between 0 and 1"
                    .into(),
            );
        }
        Ok(())
    }
}

pub struct PathStrokeRequest {
    source: Geometry,
    revision: u64,
    path: SurfacePath,
    brush: Brush,
    symmetry: Symmetry,
    options: PathStrokeOptions,
    settings: crate::diagnostics::EditSettings,
    stencil: Option<crate::sculpt::alpha::Projection>,
}

pub struct PathStrokePreview {
    source: Geometry,
    result: Geometry,
    revision: u64,
    sample_count: usize,
    settings: crate::diagnostics::EditSettings,
    attachment_warning: Option<String>,
}
impl PathStrokePreview {
    pub fn mesh(&self) -> &Mesh {
        &self.result.mesh
    }
    pub fn sample_count(&self) -> usize {
        self.sample_count
    }
    pub fn attachment_warning(&self) -> Option<&str> {
        self.attachment_warning.as_deref()
    }
}

/// Private live-stroke transaction. Dropping it cancels geometry and path changes.
pub struct PathStrokeSession {
    source: Geometry,
    work: Document,
    revision: u64,
    path_id: u64,
    brush: Brush,
    options: PathStrokeOptions,
    stencil: Option<crate::sculpt::alpha::Projection>,
    attachment_radius: f32,
    previous: Option<Vec3>,
    snake: Option<SnakeHandle>,
    sample_count: usize,
    vertex_dabs: usize,
    vertex_dab_limit: usize,
    settings: crate::diagnostics::EditSettings,
}
struct SnakeHandle {
    center: Vec3,
    normal: Vec3,
    mobility: f32,
}
impl PathStrokeSession {
    /// Temporary Smooth and preset changes remain in this stroke's transaction.
    /// Radius is captured in model units when the new preset is selected.
    pub fn set_brush(&mut self, brush: &Brush, radius: f32) -> Result<(), String> {
        validate_brush(brush)?;
        PathStrokeOptions {
            radius,
            ..self.options
        }
        .validate()?;
        if self.stencil.is_some() && matches!(brush.kind, Kind::Grab | Kind::Snake | Kind::Detail) {
            return Err("Grab, Snake and Detail do not support stencil projection".into());
        }
        if self.brush.kind != brush.kind {
            self.snake = None;
            // A newly selected motion brush starts a fresh handle where the next
            // sample lands; it must not inherit an earlier relief brush's delta.
            if matches!(brush.kind, Kind::Grab | Kind::Snake) {
                self.previous = None;
            }
        }
        self.brush = brush.clone();
        self.brush.stroke_mode = crate::sculpt::StrokeMode::Freehand;
        self.options.radius = radius;
        self.attachment_radius = self.attachment_radius.max(radius);
        self.settings.active_brush = Some(brush.clone());
        Ok(())
    }
    pub fn mesh(&self) -> &Mesh {
        self.work.mesh()
    }
    pub fn fork_mesh(&self) -> std::sync::Arc<Mesh> {
        std::sync::Arc::new(self.mesh().clone())
    }
    pub fn begin_frame(&mut self) {
        self.work.begin_sculpt_frame();
        // Live input spreads work over frames; automated replay never calls this
        // boundary and retains the same whole-operation limit.
        self.vertex_dabs = 0;
    }
    /// A live pressure/modifier sample. The caller supplies consecutive constrained
    /// positions, subdividing motion at brush spacing (at most radius/10 for Snake).
    pub fn apply_sample(
        &mut self,
        sample: PathSample,
        pressure: f32,
        invert: bool,
    ) -> Result<bool, String> {
        let finite = |p: Vec3| p.0.is_finite() && p.1.is_finite() && p.2.is_finite();
        if !pressure.is_finite()
            || !(0. ..=1.).contains(&pressure)
            || !finite(sample.position)
            || !finite(sample.normal)
            || !finite(sample.tangent)
            || !finite(sample.binormal)
            || sample.normal.len() < 0.5
            || sample.tangent.len() < 0.5
            || sample.binormal.len() < 0.5
        {
            return Err("Invalid constrained brush sample".into());
        }
        let radius = self.options.radius
            * if self.brush.pressure_radius {
                pressure.powf(self.brush.pressure_curve).max(0.05)
            } else {
                1.
            };
        let delta = self
            .previous
            .map_or(Vec3::default(), |previous| sample.position - previous);
        let mut dab = Dab {
            center: sample.position,
            normal: sample.normal,
            right: sample.tangent,
            up: sample.binormal,
            radius,
            pressure,
            delta,
            invert,
        };
        let cost = self
            .work
            .mesh()
            .vertices
            .len()
            .saturating_mul(self.work.symmetry.dabs(dab).len());
        if self.sample_count >= MAX_PATH_DABS
            || self.vertex_dabs.saturating_add(cost) > self.vertex_dab_limit
        {
            return Err("Path stroke cancelled; no changes kept. The work limit was exceeded; use shorter strokes or increase brush spacing".into());
        }
        self.vertex_dabs += cost;
        self.sample_count += 1;
        if self.sample_count == 1 {
            self.settings.first_dab = Some(serde_json::json!({
                "path_id": self.path_id, "radius_world": self.options.radius,
                "pressure": pressure, "invert": invert, "reverse": self.options.reverse,
                "pressure_policy": "sampled",
            }));
        }
        self.previous = Some(sample.position);
        if self.brush.kind == Kind::Snake {
            if self.snake.is_none() {
                self.snake = self
                    .work
                    .mesh()
                    .vertices
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| self.work.mesh().mask[*i] < 0.999)
                    .min_by(|(_, a), (_, b)| {
                        (**a - sample.position)
                            .len()
                            .total_cmp(&(**b - sample.position).len())
                    })
                    .filter(|(_, point)| (**point - sample.position).len() < radius)
                    .map(|(i, &center)| SnakeHandle {
                        center,
                        normal: sample.normal,
                        mobility: 1. - self.work.mesh().mask[i],
                    });
            }
            let Some(handle) = &self.snake else {
                return Ok(false);
            };
            dab.center = handle.center;
            dab.normal = handle.normal;
        }
        let revision = self.work.display_revision();
        let deformed = self
            .work
            .apply_dab_stenciled(&self.brush, dab, self.stencil.as_ref());
        if self.brush.kind == Kind::Snake
            && deformed
            && let Some(handle) = &mut self.snake
        {
            handle.center = handle.center + self.brush.snake_motion(dab) * handle.mobility;
        }
        Ok(revision != self.work.display_revision())
    }
    pub fn finish(self) -> Result<PathStrokePreview, String> {
        self.finish_cancelled(&|| false)
    }
    fn finish_cancelled(mut self, cancel: &impl Fn() -> bool) -> Result<PathStrokePreview, String> {
        if cancel() {
            return Err("Path stroke cancelled".into());
        }
        self.work.end_stroke();
        self.work.geometry.mesh.validate()?;
        self.work
            .geometry
            .settings
            .validate(&self.work.geometry.mesh)?;
        let mut attachment_warning = None;
        if self.work.geometry.mesh != self.source.mesh
            && let Some(path) = self
                .work
                .geometry
                .paths
                .iter_mut()
                .find(|p| p.id == self.path_id)
        {
            // A stroke transports only its own guide. Failure keeps its old,
            // explicitly stale attachment; the valid sculpt is still committable.
            if let Err(error) = path.reattach_after_stroke(
                &self.source.mesh,
                &self.work.geometry.mesh,
                self.attachment_radius * 2.,
                cancel,
            ) {
                attachment_warning = Some(error);
            }
        }
        if cancel() {
            return Err("Path stroke cancelled".into());
        }
        Ok(PathStrokePreview {
            source: self.source,
            result: self.work.geometry,
            revision: self.revision,
            sample_count: self.sample_count,
            settings: self.settings,
            attachment_warning,
        })
    }
}
impl PathStrokeRequest {
    /// Starts a streaming transaction without sampling or replaying the whole route.
    pub fn begin_live(self) -> PathStrokeSession {
        let work = Document {
            geometry: self.source.clone(),
            symmetry: self.symmetry,
            ..Document::default()
        };
        let mut brush = self.brush;
        // The path supplies motion; pointer gesture policies do not turn every
        // constrained sample into a separate Drag Rect/Drag Dot stamp.
        brush.stroke_mode = crate::sculpt::StrokeMode::Freehand;
        PathStrokeSession {
            source: self.source,
            work,
            revision: self.revision,
            path_id: self.path.id,
            brush,
            options: self.options,
            stencil: self.stencil,
            attachment_radius: self.options.radius,
            previous: None,
            snake: None,
            sample_count: 0,
            vertex_dabs: 0,
            vertex_dab_limit: MAX_VERTEX_DABS,
            settings: self.settings,
        }
    }
    /// Cancellation and failures discard all private geometry, including adaptive topology.
    pub fn run(self, cancel: &impl Fn() -> bool) -> Result<PathStrokePreview, String> {
        self.run_with_vertex_budget(cancel, MAX_VERTEX_DABS)
    }
    fn run_with_vertex_budget(
        self,
        cancel: &impl Fn() -> bool,
        vertex_dab_limit: usize,
    ) -> Result<PathStrokePreview, String> {
        if cancel() {
            return Err("Path stroke cancelled".into());
        }
        let radius = self.options.radius
            * if self.brush.pressure_radius {
                self.options
                    .pressure
                    .powf(self.brush.pressure_curve)
                    .max(0.05)
            } else {
                1.
            };
        let spacing = if self.brush.kind == Kind::Snake {
            self.brush.spacing.min(0.1)
        } else {
            self.brush.spacing
        };
        let mut samples =
            self.path
                .samples_on_mesh_cancelled(&self.source.mesh, radius * spacing, cancel)?;
        if samples.len() > MAX_PATH_DABS {
            return Err("This path stroke exceeds the work limit; increase radius or spacing, or shorten the path".into());
        }
        if self.options.reverse {
            if self.path.closed {
                samples[1..].reverse();
            } else {
                samples.reverse();
            }
            for sample in &mut samples {
                sample.tangent = sample.tangent * -1.;
                sample.binormal = sample.binormal * -1.;
            }
        }
        // Drag brushes begin with zero motion; include the final closing motion
        // of a loop. Relief brushes omit that duplicate endpoint exposure.
        if self.path.closed && matches!(self.brush.kind, Kind::Grab | Kind::Snake) {
            samples.push(samples[0]);
        }
        let options = self.options;
        let mut session = self.begin_live();
        session.vertex_dab_limit = vertex_dab_limit;
        for sample in samples {
            if cancel() {
                return Err("Path stroke cancelled".into());
            }
            // Automated dabs have no rendered-frame boundary. Give every part
            // of the route the ordinary local refinement allowance instead of
            // spending one frame's topology budget near the start of the path.
            // Keep cumulative vertex/dab work counters intact for this job.
            session.work.begin_sculpt_frame();
            session.apply_sample(sample, options.pressure, options.invert)?;
        }
        if let Some(settings) = &mut session.settings.first_dab {
            settings["pressure_policy"] = "constant".into();
        }
        session.finish_cancelled(cancel)
    }
}

impl Document {
    pub fn prepare_path_stroke(
        &self,
        path_id: u64,
        brush: &Brush,
        options: PathStrokeOptions,
    ) -> Result<PathStrokeRequest, String> {
        self.prepare_live_path_stroke(path_id, brush, options, None)
    }

    /// Live guided strokes can carry the same frozen stencil projector as ordinary strokes.
    pub fn prepare_live_path_stroke(
        &self,
        path_id: u64,
        brush: &Brush,
        options: PathStrokeOptions,
        stencil: Option<crate::sculpt::alpha::Projection>,
    ) -> Result<PathStrokeRequest, String> {
        self.require_sculpt_mesh()?;
        options.validate()?;
        validate_brush(brush)?;
        if self.edit_in_progress() {
            return Err("Finish the current edit before previewing a path stroke".into());
        }
        if !self.active_visible() {
            return Err("Show the target submodel before applying a path stroke".into());
        }
        if self.geometry.scene.active_metadata.temporary_thickness > 0. {
            return Err("Apply or remove Thickness before applying a path stroke".into());
        }
        if stencil.is_some() && matches!(brush.kind, Kind::Grab | Kind::Snake | Kind::Detail) {
            return Err("Grab, Snake and Detail do not support stencil projection".into());
        }
        if stencil.is_none() && self.stencil().is_some_and(|stencil| stencil.enabled) {
            return Err(
                "Disable the stencil or provide its viewport projection for a guided stroke".into(),
            );
        }
        let path = self
            .path(path_id)
            .ok_or("The selected path no longer exists")?;
        if path.target != self.active_model_id() {
            return Err(
                "Select the path's target submodel before applying its brush stroke".into(),
            );
        }
        if path.mode != crate::sculpt::paths::PathMode::Surface {
            return Err(
                "Attach this free-space path to the surface before applying a brush".into(),
            );
        }
        if path.is_stale(self.mesh()) {
            return Err(
                "The target surface changed; reproject the path before applying a brush".into(),
            );
        }
        let mut settings = self.capture_edit_settings();
        settings.active_brush = Some(brush.clone());
        settings.first_dab = Some(serde_json::json!({
            "path_id": path_id, "radius_world": options.radius,
            "pressure": options.pressure, "pressure_policy": "constant",
            "reverse": options.reverse, "invert": options.invert,
        }));
        Ok(PathStrokeRequest {
            source: self.geometry.clone(),
            revision: self.display_revision(),
            path: path.clone(),
            brush: brush.clone(),
            symmetry: self.symmetry.clone(),
            options,
            settings,
            stencil,
        })
    }

    pub fn apply_path_stroke(&mut self, preview: PathStrokePreview) -> Result<bool, String> {
        if self.edit_in_progress()
            || preview.revision != self.display_revision()
            || preview.source != self.geometry
        {
            return Err("The scene changed; preview the path stroke again".into());
        }
        if preview.result == self.geometry {
            return Ok(false);
        }
        self.replace_geometry_recorded(
            preview.result,
            "Path brush stroke",
            Some(preview.settings),
        )?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::paths::{PathMode, PathPoint, mesh_signature};
    use std::cell::Cell;

    fn plane() -> Document {
        let n = 20;
        let mut mesh = Mesh::default();
        for y in 0..=n {
            for x in 0..=n {
                mesh.vertices
                    .push(Vec3(x as f32 * 0.1 - 1., y as f32 * 0.1 - 1., 0.));
                mesh.mask.push(0.);
            }
        }
        for y in 0..n {
            for x in 0..n {
                let a = y * (n + 1) + x;
                mesh.triangles
                    .extend([[a, a + 1, a + n + 2], [a, a + n + 2, a + n + 1]]);
            }
        }
        Document {
            geometry: Geometry::new(mesh),
            ..Document::default()
        }
    }
    fn request(doc: &Document, kind: Kind) -> PathStrokeRequest {
        let path = SurfacePath {
            id: 1,
            name: "Reusable curve".into(),
            target: 1,
            mode: PathMode::Surface,
            smoothing: 0.5,
            closed: false,
            seam: 0,
            surface_signature: mesh_signature(doc.mesh()),
            attached_route: None,
            corner_rounding: Vec::new(),
            points: [Vec3(-0.5, -0.2, 0.), Vec3(0., 0.2, 0.), Vec3(0.5, -0.2, 0.)]
                .into_iter()
                .map(|position| PathPoint {
                    position,
                    normal: Vec3(0., 0., 1.),
                })
                .collect(),
        };
        PathStrokeRequest {
            source: doc.geometry.clone(),
            revision: doc.display_revision(),
            path,
            brush: Brush::new(kind),
            symmetry: doc.symmetry.clone(),
            options: PathStrokeOptions {
                radius: 0.2,
                ..Default::default()
            },
            settings: doc.capture_edit_settings(),
            stencil: None,
        }
    }

    #[test]
    fn preview_is_private_cancel_discards_and_apply_is_one_undo_step() {
        let mut doc = plane();
        let original = doc.geometry.clone();
        let checks = Cell::new(0);
        let cancelled = request(&doc, Kind::Standard).run(&|| {
            checks.set(checks.get() + 1);
            checks.get() > 5
        });
        assert!(cancelled.err().unwrap().contains("cancelled"));
        assert_eq!(doc.geometry, original);
        assert_eq!(doc.history_counts(), (0, 0));
        assert!(!doc.is_dirty());

        let preview = request(&doc, Kind::Standard).run(&|| false).unwrap();
        assert!(preview.sample_count() > 3);
        assert_ne!(preview.mesh(), doc.mesh());
        assert_eq!(doc.geometry, original);
        assert!(doc.apply_path_stroke(preview).unwrap());
        assert_eq!(doc.history_counts(), (1, 0));
        assert_eq!(doc.undo_label(), Some("Path brush stroke"));
        let edited = doc.geometry.clone();
        doc.undo();
        assert_eq!(doc.geometry, original);
        doc.redo();
        assert_eq!(doc.geometry, edited);
    }

    #[test]
    fn no_op_retains_redo_and_stale_preview_cannot_overwrite_scene() {
        let mut doc = plane();
        let preview = request(&doc, Kind::Standard).run(&|| false).unwrap();
        doc.apply_path_stroke(preview).unwrap();
        doc.undo();
        doc.mark_saved("test.clay");
        let mut zero = request(&doc, Kind::Standard);
        zero.options.pressure = 0.;
        assert!(!doc.apply_path_stroke(zero.run(&|| false).unwrap()).unwrap());
        assert_eq!(doc.history_counts(), (0, 1));
        assert!(!doc.is_dirty());
        let preview = request(&doc, Kind::Standard).run(&|| false).unwrap();
        // Some persistent settings changes do not increment the display revision.
        // Full source comparison must still protect them.
        doc.geometry.settings.resolution += 1;
        let unchanged = doc.geometry.clone();
        assert!(
            doc.apply_path_stroke(preview)
                .unwrap_err()
                .contains("scene changed")
        );
        assert_eq!(doc.geometry, unchanged);
        assert_eq!(doc.history_counts(), (0, 1));
    }

    #[test]
    fn masks_hidden_faces_and_symmetry_use_the_existing_brush_rules() {
        let mut doc = plane();
        let masked = 10 * 21 + 10;
        doc.geometry.mesh.mask[masked] = 1.;
        doc.geometry.mesh.hidden_faces = vec![false; doc.mesh().triangles.len()];
        let hidden_face = 2 * (5 * 20 + 14);
        doc.geometry.mesh.hidden_faces[hidden_face] = true;
        let protected = doc.mesh().triangles[hidden_face];
        doc.symmetry.enabled = true;
        let mut req = request(&doc, Kind::Standard);
        // Author on just one side; symmetry must also edit the opposite side.
        req.path.points = [Vec3(0.4, -0.5, 0.), Vec3(0.4, 0.5, 0.)]
            .into_iter()
            .map(|position| PathPoint {
                position,
                normal: Vec3(0., 0., 1.),
            })
            .collect();
        req.options.radius = 0.55;
        let preview = req.run(&|| false).unwrap();
        assert_eq!(preview.mesh().vertices[masked], doc.mesh().vertices[masked]);
        for v in protected {
            assert_eq!(preview.mesh().vertices[v], doc.mesh().vertices[v]);
        }
        assert!(preview.mesh().vertices[10 * 21 + 6].2 > 0.);
        assert!(preview.mesh().vertices[10 * 21 + 14].2 > 0.);
        assert!(
            (preview.mesh().vertices[10 * 21 + 6].2 - preview.mesh().vertices[10 * 21 + 14].2)
                .abs()
                < 1e-5
        );
        assert_eq!(preview.mesh().mask, doc.mesh().mask);
    }

    #[test]
    fn mask_path_can_reuse_geometry_and_another_brush_preserves_target() {
        let mut doc = plane();
        let original = doc.mesh().vertices.clone();
        let preview = request(&doc, Kind::Mask).run(&|| false).unwrap();
        assert_eq!(preview.mesh().vertices, original);
        assert!(preview.mesh().mask.iter().any(|&mask| mask > 0.));
        doc.apply_path_stroke(preview).unwrap();
        let preview = request(&doc, Kind::Inflate).run(&|| false).unwrap();
        assert_ne!(preview.mesh().vertices, original);
        doc.apply_path_stroke(preview).unwrap();
        assert_eq!(doc.history_counts(), (2, 0));
        doc.undo();
        assert_eq!(doc.mesh().vertices, original);
        assert!(doc.mesh().mask.iter().any(|&mask| mask > 0.));
    }

    #[test]
    fn invalid_options_and_excess_work_fail_without_editing() {
        for radius in [f32::NAN, 0., -1.] {
            assert!(
                PathStrokeOptions {
                    radius,
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        let doc = plane();
        let mut req = request(&doc, Kind::Standard);
        req.options.radius = 0.001;
        req.brush.spacing = 0.1;
        assert!(req.run(&|| false).is_err());
        assert!(!doc.is_dirty());
        assert_eq!(doc.history_counts(), (0, 0));
    }

    #[test]
    fn saved_path_follows_its_own_strokes_and_reuses_without_reprojection() {
        let mut doc = plane();
        let points = request(&doc, Kind::Standard).path.points;
        let id = doc
            .create_path(
                "Surface curve".into(),
                PathMode::Surface,
                points,
                false,
                0.5,
            )
            .unwrap();
        let original_path = doc.path(id).unwrap().clone();
        let options = PathStrokeOptions {
            radius: 0.2,
            ..Default::default()
        };
        let preview = doc
            .prepare_path_stroke(id, &Brush::new(Kind::Standard), options)
            .unwrap()
            .run(&|| false)
            .unwrap();
        doc.apply_path_stroke(preview).unwrap();
        assert_ne!(doc.path(id).unwrap().points, original_path.points);
        assert!(!doc.path(id).unwrap().is_stale(doc.mesh()));
        let refreshed_path = doc.path(id).unwrap().clone();
        assert_eq!(refreshed_path.target, original_path.target);
        let before = doc.geometry.clone();
        let preview = doc
            .prepare_path_stroke(id, &Brush::new(Kind::Clay), options)
            .unwrap()
            .run(&|| false)
            .unwrap();
        assert!(doc.apply_path_stroke(preview).unwrap());
        assert_eq!(doc.path(id).unwrap().target, refreshed_path.target);
        assert!(!doc.path(id).unwrap().is_stale(doc.mesh()));
        doc.project().validate().unwrap();
        doc.undo();
        assert_eq!(doc.geometry, before);
    }

    #[test]
    fn all_brush_kinds_support_paths_but_free_paths_and_other_targets_report_clear_errors() {
        let mut doc = plane();
        let points = request(&doc, Kind::Standard).path.points;
        let free = doc
            .create_path(
                "Free curve".into(),
                PathMode::Free,
                points.clone(),
                false,
                0.,
            )
            .unwrap();
        let surface = doc
            .create_path("Surface curve".into(), PathMode::Surface, points, false, 0.)
            .unwrap();
        for kind in Kind::ALL {
            let preview = doc
                .prepare_path_stroke(
                    surface,
                    &Brush::new(kind),
                    PathStrokeOptions {
                        radius: 0.2,
                        ..Default::default()
                    },
                )
                .unwrap()
                .run(&|| false)
                .unwrap_or_else(|error| panic!("{kind:?}: {error}"));
            preview.mesh().validate().unwrap();
            if matches!(kind, Kind::Grab | Kind::Snake | Kind::Detail) {
                assert_ne!(
                    preview.mesh(),
                    doc.mesh(),
                    "{kind:?} must move geometry along the path"
                );
            }
        }
        let brush = Brush::new(Kind::Standard);
        assert!(
            doc.prepare_path_stroke(free, &brush, PathStrokeOptions::default())
                .err()
                .unwrap()
                .contains("free-space")
        );
        doc.duplicate_model(false).unwrap();
        assert!(
            doc.prepare_path_stroke(surface, &brush, PathStrokeOptions::default())
                .err()
                .unwrap()
                .contains("target submodel")
        );
    }

    #[test]
    fn closed_cylinder_stroke_covers_the_back_and_does_not_double_the_seam() {
        let mut primitive =
            crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cylinder);
        primitive.segments = 96;
        primitive.divisions[1] = 8;
        let mut doc = Document {
            geometry: Geometry::new(primitive.mesh().unwrap()),
            ..Document::default()
        };
        let points = (0..32)
            .map(|i| {
                let angle = std::f32::consts::TAU * i as f32 / 32.;
                let position = Vec3(angle.cos(), 0., angle.sin());
                PathPoint {
                    position,
                    normal: position,
                }
            })
            .collect();
        let id = doc
            .create_path("Wrap".into(), PathMode::Surface, points, true, 0.5)
            .unwrap();
        let mut brush = Brush::new(Kind::Standard);
        brush.strength = 0.03;
        brush.spacing = 0.125;
        let options = PathStrokeOptions {
            radius: 0.2,
            ..Default::default()
        };
        let run = |reverse| {
            doc.prepare_path_stroke(id, &brush, PathStrokeOptions { reverse, ..options })
                .unwrap()
                .run(&|| false)
                .unwrap()
        };
        let forward = run(false);
        let backward = run(true);
        assert!((240..260).contains(&forward.sample_count()));
        assert_eq!(forward.sample_count(), backward.sample_count());
        for preview in [&forward, &backward] {
            let movement: Vec<f32> = doc
                .mesh()
                .vertices
                .iter()
                .zip(&preview.mesh().vertices)
                .filter(|(old, _)| old.1.abs() < 1e-5)
                .map(|(old, new)| (*new - *old).dot(old.norm()))
                .collect();
            assert_eq!(movement.len(), 96);
            let lo = movement.iter().copied().fold(f32::INFINITY, f32::min);
            let hi = movement.iter().copied().fold(0_f32, f32::max);
            assert!(
                lo > 0.,
                "The entire cylinder ring, including its back, must be sculpted"
            );
            assert!(
                hi / lo < 1.15,
                "The seam must receive the same exposure as the rest: {lo}..{hi}"
            );
        }
        assert_eq!(doc.path(id).unwrap().seam, 0);
    }

    #[test]
    fn automatic_stroke_keeps_total_work_limit_across_topology_budget_refreshes() {
        let doc = plane();
        let before = doc.project();
        let limit = doc.mesh().vertices.len() * 3;
        let error = request(&doc, Kind::Clay)
            .run_with_vertex_budget(&|| false, limit)
            .err()
            .unwrap();
        assert!(error.contains("work limit was exceeded"));
        assert!(doc.project() == before);
        assert_eq!(doc.history_counts(), (0, 0));
        assert!(!doc.is_dirty());
    }

    #[test]
    fn automatic_adaptive_stroke_matches_the_same_samples_with_live_frame_budgets() {
        let mut primitive =
            crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cylinder);
        primitive.segments = 32;
        primitive.divisions[1] = 4;
        let mut doc = Document {
            geometry: Geometry::new(primitive.mesh().unwrap()),
            ..Document::default()
        };
        doc.geometry.settings.adaptive.enabled = true;
        doc.geometry.settings.adaptive.detail = 16.;
        let make_request = || {
            let mut request = request(&doc, Kind::Clay);
            request.path.smoothing = 0.;
            request.path.points = (0..=8)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::PI / 16.;
                    let position = Vec3(angle.cos(), 0., angle.sin());
                    PathPoint {
                        position,
                        normal: position,
                    }
                })
                .collect();
            request
        };
        let automatic = make_request().run(&|| false).unwrap();
        let manual_request = make_request();
        let samples = manual_request
            .path
            .samples_on_mesh(
                doc.mesh(),
                manual_request.options.radius * manual_request.brush.spacing,
            )
            .unwrap();
        let options = manual_request.options;
        let mut manual = manual_request.begin_live();
        for sample in samples {
            manual.begin_frame();
            manual
                .apply_sample(sample, options.pressure, options.invert)
                .unwrap();
        }
        let manual = manual.finish().unwrap();
        eprintln!(
            "Adaptive same-samples comparison: original={}, automatic={}, live={}",
            doc.mesh().vertices.len(),
            automatic.mesh().vertices.len(),
            manual.mesh().vertices.len()
        );
        assert!(
            automatic.mesh() == manual.mesh(),
            "A whole-path stroke must not consume one frame's adaptive allowance for the entire route"
        );
    }

    #[test]
    #[ignore = "requires CLAY_RING_DIAGNOSTICS pointing to the exported ring sculpt session"]
    fn exported_ring_startup_footprint_diagnostic() {
        let directory = std::env::var("CLAY_RING_DIAGNOSTICS").unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{directory}/manifest.json")).unwrap(),
        )
        .unwrap();
        let transition = manifest["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|edit| edit["from"] == 18 && edit["to"] == 19)
            .unwrap();
        let settings: crate::diagnostics::EditSettings =
            serde_json::from_value(transition["settings_at_edit_start"].clone()).unwrap();
        let first = settings.first_dab.as_ref().unwrap();
        let brush = settings.active_brush.as_ref().unwrap();
        let options = PathStrokeOptions {
            radius: first["radius_world"].as_f64().unwrap() as f32,
            pressure: first["pressure"].as_f64().unwrap() as f32,
            invert: first["invert"].as_bool().unwrap(),
            reverse: false,
        };
        let filename = format!("{directory}/stage-018.clay");
        let mut project = crate::persistence::load_project(&filename).unwrap();
        project.mesh_settings = settings.topology.clone();
        project.symmetry = settings.symmetry.clone();
        let mut doc = Document::default();
        doc.replace_project(project, &filename).unwrap();
        let id = first["path_id"].as_u64().unwrap();
        let samples = doc
            .path_samples(id, options.radius * brush.spacing)
            .unwrap();
        let samples = &samples[..30];

        // Cast along each source normal, returning the first outer-surface hit.
        // This compares identical locations even when adaptive topology differs.
        let relief = |mesh: &Mesh, sample: &PathSample| {
            let reach = options.radius * 2.;
            let origin = sample.position + sample.normal * reach;
            let direction = sample.normal * -1.;
            let mut nearest = f32::INFINITY;
            for &[a, b, c] in &mesh.triangles {
                let edge1 = mesh.vertices[b] - mesh.vertices[a];
                let edge2 = mesh.vertices[c] - mesh.vertices[a];
                let p = direction.cross(edge2);
                let determinant = edge1.dot(p);
                if determinant.abs() < 1e-12 {
                    continue;
                }
                let offset = origin - mesh.vertices[a];
                let u = offset.dot(p) / determinant;
                let q = offset.cross(edge1);
                let v = direction.dot(q) / determinant;
                let distance = edge2.dot(q) / determinant;
                if u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001 && distance >= 0. {
                    nearest = nearest.min(distance);
                }
            }
            assert!(
                nearest.is_finite(),
                "missing surface at {:?}",
                sample.position
            );
            reach - nearest
        };
        let source_relief: Vec<_> = samples.iter().map(|s| relief(doc.mesh(), s)).collect();
        for prepare in [false, true] {
            let started = std::time::Instant::now();
            let mut session = doc
                .prepare_path_stroke(id, brush, options)
                .unwrap()
                .begin_live();
            let before = session.mesh().vertices.len();
            if prepare {
                let first = samples[0];
                let dab = Dab {
                    center: first.position,
                    normal: first.normal,
                    right: first.tangent,
                    up: first.binormal,
                    radius: options.radius,
                    pressure: options.pressure,
                    delta: Vec3::default(),
                    invert: options.invert,
                };
                // Experiment only: use the existing bounded footprint resolver,
                // then keep the actual deformation brush in Freehand mode.
                let placed = Brush {
                    stroke_mode: crate::sculpt::StrokeMode::DragDot,
                    ..brush.clone()
                };
                let dabs = session.work.symmetry.dabs(dab);
                let adaptive = session.work.geometry.settings.adaptive.clone();
                session
                    .work
                    .geometry
                    .mesh
                    .prepare_stenciled_stamp(&placed, &dabs, &[], &adaptive);
            }
            let prepared = session.mesh().vertices.len();
            for sample in samples {
                session.begin_frame();
                session
                    .apply_sample(*sample, options.pressure, options.invert)
                    .unwrap();
            }
            let mesh = session.mesh();
            mesh.validate().unwrap();
            let depth: Vec<_> = samples
                .iter()
                .zip(&source_relief)
                .map(|(s, source)| relief(mesh, s) - source)
                .collect();
            let curvature = depth[4..26]
                .windows(3)
                .map(|v| (v[0] - 2. * v[1] + v[2]).abs())
                .fold(0_f32, f32::max);
            let mut examined = 0;
            let mut reversed = 0;
            let mut steep = 0;
            let mut worst_aspect = 0_f32;
            for &[a, b, c] in &mesh.triangles {
                let [a, b, c] = [a, b, c].map(|i| mesh.vertices[i]);
                let center = (a + b + c) * (1. / 3.);
                let sample = samples
                    .iter()
                    .min_by(|a, b| {
                        (center - a.position)
                            .len()
                            .total_cmp(&(center - b.position).len())
                    })
                    .unwrap();
                let offset = center - sample.position;
                if offset.len() > options.radius * 1.5 || offset.dot(sample.normal) < -0.05 {
                    continue;
                }
                let cross = (b - a).cross(c - a);
                let area = cross.len();
                if area < 1e-12 {
                    continue;
                }
                examined += 1;
                let alignment = cross.dot(sample.normal) / area;
                reversed += usize::from(alignment < 0.);
                steep += usize::from(alignment < 0.2);
                let edge = (b - a).len().max((c - b).len()).max((a - c).len());
                worst_aspect = worst_aspect.max(edge * edge / area);
            }
            let name = if prepare { "prepared" } else { "ordinary" };
            eprintln!(
                "Ring startup {name}: elapsed={:?}, vertices={before}->{prepared}->{}, faces examined={examined}, reversed={reversed}, steep={steep}, worst aspect={worst_aspect}, max relief second difference={curvature}, relief={depth:?}",
                started.elapsed(),
                mesh.vertices.len()
            );
            let output = format!("/tmp/clay-ring-startup-{name}.clay");
            serde_json::to_writer(
                std::fs::File::create(&output).unwrap(),
                &session.work.project(),
            )
            .unwrap();
            eprintln!("Saved {output}");
        }
    }

    #[test]
    #[ignore = "requires CLAY_RING_DIAGNOSTICS pointing to the exported ring sculpt session"]
    fn exported_automatic_ring_refines_the_complete_trajectory_like_live_strokes() {
        let directory = std::env::var("CLAY_RING_DIAGNOSTICS").unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{directory}/manifest.json")).unwrap(),
        )
        .unwrap();
        let settings =
            &manifest["transitions"].as_array().unwrap().last().unwrap()["settings_at_edit_start"];
        let dab = &settings["first_dab"];
        let brush: Brush = serde_json::from_value(settings["active_brush"].clone()).unwrap();
        let options = PathStrokeOptions {
            radius: dab["radius_world"].as_f64().unwrap() as f32,
            pressure: dab["pressure"].as_f64().unwrap() as f32,
            invert: dab["invert"].as_bool().unwrap(),
            reverse: false,
        };
        let mut doc = Document::default();
        let filename = format!("{directory}/stage-018.clay");
        doc.replace_project(
            crate::persistence::load_project(&filename).unwrap(),
            &filename,
        )
        .unwrap();
        let id = dab["path_id"].as_u64().unwrap();
        let samples = doc
            .path_samples(id, options.radius * brush.spacing)
            .unwrap();
        let automatic = doc
            .prepare_path_stroke(id, &brush, options)
            .unwrap()
            .run(&|| false)
            .unwrap();
        let mut manual = doc
            .prepare_path_stroke(id, &brush, options)
            .unwrap()
            .begin_live();
        for sample in &samples {
            manual.begin_frame();
            manual
                .apply_sample(*sample, options.pressure, options.invert)
                .unwrap();
        }
        let manual = manual.finish().unwrap();
        eprintln!(
            "Export ring: dabs={}, original={}, automatic={}, live={}, auto attachment={:?}, live attachment={:?}",
            samples.len(),
            doc.mesh().vertices.len(),
            automatic.mesh().vertices.len(),
            manual.mesh().vertices.len(),
            automatic.attachment_warning(),
            manual.attachment_warning()
        );
        assert!(automatic.mesh() == manual.mesh());
        assert!(automatic.attachment_warning().is_none());
    }

    #[test]
    fn detail_creates_a_real_groove_in_both_automated_and_live_strokes() {
        let mut doc = plane();
        let points = [Vec3(-0.5, 0., 0.), Vec3(0.5, 0., 0.)]
            .into_iter()
            .map(|position| PathPoint {
                position,
                normal: Vec3(0., 0., 1.),
            })
            .collect();
        let id = doc
            .create_path("Detail guide".into(), PathMode::Surface, points, false, 0.)
            .unwrap();
        let brush = Brush::new(Kind::Detail);
        let options = PathStrokeOptions {
            radius: 0.3,
            ..Default::default()
        };
        let preview = doc
            .prepare_path_stroke(id, &brush, options)
            .unwrap()
            .run(&|| false)
            .unwrap();
        assert!(
            preview.mesh().vertices.iter().any(|p| p.2 < -0.005),
            "Detail must cut a visible groove"
        );
        let samples = doc
            .path_samples(id, options.radius * brush.spacing)
            .unwrap();
        let mut live = doc
            .prepare_path_stroke(id, &brush, options)
            .unwrap()
            .begin_live();
        let before = doc.geometry_snapshot();
        for sample in samples.iter().take(12) {
            live.apply_sample(*sample, 0.8, false).unwrap();
        }
        assert!(live.mesh().vertices.iter().any(|p| p.2 < -0.005));
        for (old, new) in doc.mesh().vertices.iter().zip(&live.mesh().vertices) {
            if old.0 > 0.2 {
                assert_eq!(old, new, "Unvisited route must not receive a stroke");
            }
        }
        assert_eq!(doc.geometry_snapshot(), before);
        let preview = live.finish().unwrap();
        assert!(
            preview.attachment_warning().is_none(),
            "{:?}",
            preview.attachment_warning()
        );
        doc.apply_path_stroke(preview).unwrap();
        assert!(!doc.path(id).unwrap().is_stale(doc.mesh()));
        assert!(
            doc.prepare_path_stroke(id, &brush, options).is_ok(),
            "Guide remains reusable immediately"
        );
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
    }

    #[test]
    fn live_pressure_and_motion_preserve_masks_symmetry_and_private_cancellation() {
        let mut doc = plane();
        doc.symmetry.enabled = true;
        let protected = 10 * 21 + 6;
        doc.geometry.mesh.mask[protected] = 1.;
        let req = request(&doc, Kind::Grab);
        let samples = req.path.samples_on_mesh(doc.mesh(), 0.03).unwrap();
        let before = doc.geometry_snapshot();
        let mut session = req.begin_live();
        session.apply_sample(samples[0], 0., false).unwrap();
        assert_eq!(session.mesh(), doc.mesh());
        for sample in samples.iter().skip(1).take(10) {
            session.apply_sample(*sample, 0.6, false).unwrap();
        }
        assert_ne!(
            session.mesh(),
            doc.mesh(),
            "Grab requires actual constrained motion"
        );
        assert_eq!(
            session.mesh().vertices[protected],
            doc.mesh().vertices[protected]
        );
        assert!(
            doc.mesh()
                .vertices
                .iter()
                .zip(&session.mesh().vertices)
                .any(|(old, new)| old.0 > 0.2 && old != new),
            "Symmetry follows constrained motion"
        );
        drop(session);
        assert_eq!(doc.geometry_snapshot(), before);
        assert!(!doc.is_dirty());
    }

    #[test]
    fn guided_stencil_uses_normal_projection_protection_and_existing_brush_limits() {
        use crate::sculpt::alpha::{Frame, Projection};
        let mut doc = plane();
        let points = request(&doc, Kind::Standard).path.points;
        let id = doc
            .create_path("Stencil guide".into(), PathMode::Surface, points, false, 0.)
            .unwrap();
        let stencil = Projection {
            image: Alpha::Image {
                width: 2,
                height: 1,
                pixels: vec![0., 0.].into(),
            },
            frame: Frame {
                origin: Vec3::default(),
                right: Vec3(1., 0., 0.),
                up: Vec3(0., 1., 0.),
            },
            view: Vec3(0., 0., 1.),
            distance: 0.,
            position: [0.; 2],
            size: [2.; 2],
            rotation: 0.,
        };
        let preview = doc
            .prepare_live_path_stroke(
                id,
                &Brush::new(Kind::Standard),
                PathStrokeOptions::default(),
                Some(stencil.clone()),
            )
            .unwrap()
            .run(&|| false)
            .unwrap();
        assert_eq!(
            preview.mesh(),
            doc.mesh(),
            "Black stencil protects the guided route"
        );
        for kind in [Kind::Grab, Kind::Snake, Kind::Detail] {
            assert!(
                doc.prepare_live_path_stroke(
                    id,
                    &Brush::new(kind),
                    PathStrokeOptions::default(),
                    Some(stencil.clone())
                )
                .err()
                .unwrap()
                .contains("stencil projection")
            );
        }
    }

    #[test]
    fn live_temporary_smooth_changes_brush_and_radius_with_one_undo() {
        let mut doc = plane();
        let points = [Vec3(-0.5, 0., 0.), Vec3(0.5, 0., 0.)]
            .into_iter()
            .map(|position| PathPoint {
                position,
                normal: Vec3(0., 0., 1.),
            })
            .collect();
        let id = doc
            .create_path("Smooth guide".into(), PathMode::Surface, points, false, 0.)
            .unwrap();
        let samples = doc.path_samples(id, 0.025).unwrap();
        let options = PathStrokeOptions {
            radius: 0.25,
            ..Default::default()
        };
        let before = doc.geometry_snapshot();
        let history = doc.history_counts();
        let mut session = doc
            .prepare_path_stroke(id, &Brush::new(Kind::Standard), options)
            .unwrap()
            .begin_live();
        for sample in samples.iter().take(20) {
            session.apply_sample(*sample, 1., false).unwrap();
        }
        let raised = session.mesh().clone();
        let peak = raised.vertices.iter().map(|p| p.2).fold(0_f32, f32::max);
        assert!(peak > 0.);
        let mut smooth = Brush::new(Kind::Smooth);
        smooth.strength = 0.8;
        session.set_brush(&smooth, 0.35).unwrap();
        for sample in samples.iter().take(20).rev() {
            session.apply_sample(*sample, 1., false).unwrap();
        }
        assert_ne!(session.mesh(), &raised);
        assert!(
            session
                .mesh()
                .vertices
                .iter()
                .map(|p| p.2)
                .fold(0_f32, f32::max)
                < peak
        );
        assert!(session.set_brush(&smooth, f32::NAN).is_err());
        assert_eq!(session.options.radius, 0.35);
        assert_eq!(doc.geometry_snapshot(), before);
        doc.apply_path_stroke(session.finish().unwrap()).unwrap();
        assert_eq!(doc.history_counts().0, history.0 + 1);
        assert_eq!(doc.undo_label(), Some("Path brush stroke"));
        doc.undo();
        assert_eq!(doc.geometry_snapshot(), before);
    }

    #[test]
    fn live_frame_renews_work_budget_but_one_frame_and_total_dabs_remain_bounded() {
        let doc = plane();
        let req = request(&doc, Kind::Standard);
        let sample = req.path.samples_on_mesh(doc.mesh(), 0.05).unwrap()[0];
        let mut session = req.begin_live();
        session.vertex_dabs = MAX_VERTEX_DABS;
        let error = session.apply_sample(sample, 1., false).unwrap_err();
        assert!(error.contains("cancelled") && error.contains("no changes kept"));
        assert_eq!(session.mesh(), doc.mesh());
        session.begin_frame();
        assert!(session.apply_sample(sample, 1., false).unwrap());
        assert_ne!(session.mesh(), doc.mesh());
        let after_first = session.mesh().clone();
        session.vertex_dabs = MAX_VERTEX_DABS;
        assert!(session.apply_sample(sample, 1., false).is_err());
        assert_eq!(session.mesh(), &after_first);
        session.begin_frame();
        session.sample_count = MAX_PATH_DABS;
        assert!(session.apply_sample(sample, 1., false).is_err());
        assert_eq!(session.mesh(), &after_first);
        drop(session);
        assert!(!doc.is_dirty());
        assert_eq!(doc.history_counts(), (0, 0));
    }

    #[test]
    fn repeated_detail_keeps_the_center_of_its_groove() {
        let mut doc = plane();
        let points = [Vec3(-0.6, 0., 0.), Vec3(0.6, 0., 0.)]
            .into_iter()
            .map(|position| PathPoint {
                position,
                normal: Vec3(0., 0., 1.),
            })
            .collect();
        let id = doc
            .create_path(
                "Repeated incision".into(),
                PathMode::Surface,
                points,
                false,
                0.,
            )
            .unwrap();
        let brush = Brush::new(Kind::Detail);
        let options = PathStrokeOptions {
            radius: 0.3,
            ..Default::default()
        };
        let center = doc
            .mesh()
            .vertices
            .iter()
            .position(|p| p.0.abs() < 1e-5 && p.1.abs() < 1e-5)
            .unwrap();
        let mut previous_depth = 0.;
        let mut first_gain = 0.;
        for pass in 0..4 {
            let route = doc
                .path_samples(id, options.radius * brush.spacing)
                .unwrap();
            let drift = route
                .iter()
                .filter(|p| p.position.0.abs() < 0.3)
                .map(|p| p.position.1.abs())
                .fold(0_f32, f32::max);
            let preview = doc
                .prepare_path_stroke(id, &brush, options)
                .unwrap()
                .run(&|| false)
                .unwrap();
            let warning = preview.attachment_warning().map(str::to_owned);
            doc.apply_path_stroke(preview).unwrap();
            let depth = -doc.mesh().vertices[center].2;
            let gain = depth - previous_depth;
            if pass == 0 {
                first_gain = gain;
            }
            eprintln!(
                "Detail pass {pass}: drift={drift:.6}, depth={depth:.6}, gain={gain:.6}, first_gain={first_gain:.6}, warning={warning:?}"
            );
            assert!(
                warning.is_none(),
                "Pass {pass} lost its attachment: {warning:?}"
            );
            assert!(
                drift < 0.005,
                "Repeated route moved onto a groove wall: {drift}"
            );
            assert!(
                gain > first_gain * 0.65,
                "Repeated guide lost most of its incision effect: {gain} versus {first_gain}"
            );
            previous_depth = depth;
        }
    }

    /// Local diagnostic exports contain private geometry and stay outside fixtures.
    #[test]
    #[ignore = "requires CLAY_PATH_DIAGNOSTICS pointing to an exported sculpt session"]
    fn exported_detail_path_samples_remain_attached() {
        let directory = std::env::var("CLAY_PATH_DIAGNOSTICS").unwrap();
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{directory}/manifest.json")).unwrap(),
        )
        .unwrap();
        for stage in 10..=15 {
            let filename = format!("{directory}/stage-{stage:03}.clay");
            let mut doc = Document::default();
            doc.replace_project(
                crate::persistence::load_project(&filename).unwrap(),
                &filename,
            )
            .unwrap();
            let id = doc.paths()[0].id;
            for spacing in [0.005, 0.186945 * 0.08] {
                let samples = doc
                    .path_samples(id, spacing)
                    .unwrap_or_else(|e| panic!("stage {stage}, spacing {spacing}: {e}"));
                let maximum = crate::sculpt::paths::sample_surface_error(doc.mesh(), &samples);
                eprintln!(
                    "Export stage {stage}, spacing {spacing}, samples {}, max surface error {maximum}",
                    samples.len()
                );
                assert!(
                    maximum < 1e-5,
                    "stage {stage} route floats off the surface by {maximum}"
                );
            }
            if stage == 13 {
                let transition = manifest["transitions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|t| t["id"] == 57)
                    .unwrap();
                let settings = &transition["settings_at_edit_start"];
                let dab = &settings["first_dab"];
                let brush: Brush =
                    serde_json::from_value(settings["active_brush"].clone()).unwrap();
                let options = PathStrokeOptions {
                    radius: dab["radius_world"].as_f64().unwrap() as f32,
                    pressure: dab["pressure"].as_f64().unwrap() as f32,
                    invert: dab["invert"].as_bool().unwrap(),
                    reverse: dab["reverse"].as_bool().unwrap(),
                };
                let preview = doc
                    .prepare_path_stroke(id, &brush, options)
                    .unwrap()
                    .run(&|| false)
                    .unwrap();
                eprintln!(
                    "Export stage13 full-path replay attachment: {:?}",
                    preview.attachment_warning()
                );
                assert!(preview.attachment_warning().is_none());
                doc.apply_path_stroke(preview).unwrap();
                assert!(doc.path_samples(id, 0.005).is_ok());
            }
        }
    }
}
