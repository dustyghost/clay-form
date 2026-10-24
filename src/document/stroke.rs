//! Sculpt stroke boundaries, symmetry, and dab application.
use super::*;

impl Document {
    pub fn begin_stroke(&mut self) {
        self.end_edit();
        if !self.stroke_active {
            self.stroke_before = Some(self.geometry.clone());
            self.stroke_active = true;
            self.stroke_deformed = false;
            self.stroke_flatten_planes.clear();
            self.adaptive_budget = AdaptiveBudget::default();
        }
    }
    pub fn end_stroke(&mut self) -> bool {
        let deformed = self.stroke_active && self.stroke_deformed;
        if let Some((_, settings)) = self.relax_stroke.take()
            && deformed
            && let Some(before) = self.stroke_before.take()
        {
            self.history
                .record_owned(before, "Relax stroke", Some(settings));
        }
        self.stroke_active = false;
        self.stroke_deformed = false;
        self.stroke_before = None;
        self.stroke_flatten_planes.clear();
        deformed
    }
    pub fn symmetry(&self) -> &Symmetry {
        &self.symmetry
    }
    pub fn set_symmetry(&mut self, symmetry: Symmetry) -> Result<(), String> {
        symmetry.validate()?;
        if self.symmetry != symmetry {
            self.end_stroke();
            self.symmetry = symmetry;
            self.dirty = true;
        }
        Ok(())
    }
    pub fn apply_dab(&mut self, brush: &Brush, dab: Dab) -> bool {
        self.apply_dab_stenciled(brush, dab, None)
    }
    pub(crate) fn apply_dab_stenciled(
        &mut self,
        brush: &Brush,
        dab: Dab,
        stencil: Option<&crate::sculpt::alpha::Projection>,
    ) -> bool {
        if !self.active_visible() || self.primitive().is_some() {
            return false;
        }
        if stencil.is_some() && matches!(brush.kind, Kind::Grab | Kind::Snake | Kind::Detail) {
            return false;
        }
        if brush.kind == Kind::Relax {
            return self.apply_relax_dab(brush, dab, stencil).unwrap_or(false);
        }
        if self.relax_stroke.is_some() {
            self.end_stroke();
        }
        self.begin_stroke();
        let dabs = self.symmetry.dabs(dab);
        if brush.kind == Kind::Flatten && self.stroke_flatten_planes.is_empty() {
            // Capture before adaptive edits or the first deformation. Each
            // symmetry copy gets the initial surface patch on its own side.
            self.stroke_flatten_planes = self.geometry.mesh.surface_planes(&dabs);
        }
        let settings = &self.geometry.settings.adaptive;
        let stencils = stencil.map_or_else(Vec::new, |s| s.copies(&self.symmetry));
        let region = crate::sculpt::AdaptiveRegion {
            dabs: &dabs,
            stencils: &stencils,
        };
        let mut before_topology = self.geometry.mesh.adapt_stenciled(
            brush,
            region,
            settings,
            &mut self.adaptive_budget,
            true,
        );
        before_topology |= self
            .geometry
            .mesh
            .prepare_stenciled_stamp(brush, &dabs, &stencils, settings);
        let old_mask = (brush.kind == Kind::Mask).then(|| self.geometry.mesh.mask.clone());
        let deformed = self.geometry.mesh.stamp_many_with_flatten_planes(
            brush,
            &dabs,
            &stencils,
            &self.stroke_flatten_planes,
        );
        let mut after_dabs = dabs.clone();
        if brush.kind == Kind::Snake {
            for dab in &mut after_dabs {
                dab.center = dab.center + brush.snake_motion(*dab);
            }
        }
        let after_topology = self.geometry.mesh.adapt_stenciled(
            brush,
            crate::sculpt::AdaptiveRegion {
                dabs: &after_dabs,
                stencils: &stencils,
            },
            settings,
            &mut self.adaptive_budget,
            false,
        );
        self.stroke_deformed |= deformed || before_topology || after_topology;
        if deformed
            || before_topology
            || after_topology
            || old_mask.is_some_and(|mask| mask != self.geometry.mesh.mask)
        {
            if let Some(before) = self.stroke_before.take() {
                let mut captured = self.capture_edit_settings();
                captured.active_brush = Some(brush.clone());
                captured.first_dab = Some(serde_json::json!({
                    "pressure": dab.pressure, "invert": dab.invert,
                }));
                self.history.record_owned(
                    before,
                    if settings.enabled {
                        "Adaptive brush stroke"
                    } else if brush.kind == Kind::Snake {
                        "Snake stroke"
                    } else {
                        match brush.effective_stroke_mode() {
                            crate::sculpt::StrokeMode::Freehand => "Brush stroke",
                            crate::sculpt::StrokeMode::Line => "Line stroke",
                            crate::sculpt::StrokeMode::DragRect => "Drag Rect stamp",
                            crate::sculpt::StrokeMode::DragDot => "Drag Dot stamp",
                        }
                    },
                    Some(captured),
                );
            }
            self.dirty = true;
            self.display_changed();
        }
        deformed || before_topology || after_topology
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, Vec3};

    fn patch() -> Mesh {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Plane);
        primitive.divisions = [32, 32, 1];
        let mut mesh = primitive.mesh().unwrap();
        for p in &mut mesh.vertices {
            p.2 = 0.2 * p.0 + 0.025 * (p.0 * 20.).cos() * (p.1 * 20.).cos();
        }
        mesh
    }

    fn dab() -> Dab {
        Dab {
            center: Vec3(0., 0., 0.025),
            normal: Vec3(-0.2, 0., 1.).norm(),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.8,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        }
    }

    #[test]
    fn flatten_keeps_its_initial_plane_when_surface_hits_change_and_reverse() {
        let mesh = patch();
        let mut steady = Document::default();
        steady
            .replace_geometry(Geometry::new(mesh.clone()))
            .unwrap();
        let mut changing = Document::default();
        changing.replace_geometry(Geometry::new(mesh)).unwrap();
        let brush = Brush::new(Kind::Flatten);
        steady.apply_dab(&brush, dab());
        changing.apply_dab(&brush, dab());
        for step in 0..24 {
            let x = ((step % 8) as f32 - 3.5) * 0.04;
            let base = Dab {
                center: Vec3(x, 0., 0.),
                ..dab()
            };
            steady.apply_dab(&brush, base);
            changing.apply_dab(
                &brush,
                Dab {
                    center: Vec3(x, 0., if step % 2 == 0 { 0.04 } else { -0.04 }),
                    normal: Vec3(if step % 2 == 0 { 0.3 } else { -0.4 }, 0., 1.).norm(),
                    ..base
                },
            );
        }
        for (a, b) in steady.mesh().vertices.iter().zip(&changing.mesh().vertices) {
            assert!(
                (*a - *b).len() < 1e-6,
                "surface hits retargeted Flatten: {a:?} vs {b:?}"
            );
        }
    }

    #[test]
    fn flatten_converges_without_overshoot_and_preserves_protection_and_history() {
        let mut mesh = patch();
        for (p, mask) in mesh.vertices.iter().zip(&mut mesh.mask) {
            if p.0 < -0.35 {
                *mask = 1.;
            }
        }
        mesh.hidden_faces = mesh
            .triangles
            .iter()
            .map(|t| t.iter().any(|&i| mesh.vertices[i].1 > 0.4))
            .collect();
        let eligible = mesh.sculptable_vertices();
        let (center, normal) = mesh.surface_planes(&[dab()])[0];
        assert!(
            normal.dot(Vec3(-0.2, 0., 1.).norm()) > 0.995,
            "sample the inclined patch, not a world axis"
        );
        let mut doc = Document::default();
        doc.replace_geometry(Geometry::new(mesh.clone())).unwrap();
        let history = doc.history_counts();
        let brush = Brush {
            strength: 0.35,
            ..Brush::new(Kind::Flatten)
        };
        let error = |mesh: &Mesh| {
            mesh.vertices
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    eligible[*i] && mesh.mask[*i] == 0. && p.0.abs() < 0.3 && p.1.abs() < 0.3
                })
                .map(|(_, p)| (*p - center).dot(normal).powi(2))
                .sum::<f32>()
        };
        let initial_error = error(&mesh);
        let mut previous_error = initial_error;
        doc.apply_dab(&brush, dab());
        for step in 0..32 {
            doc.apply_dab(
                &brush,
                Dab {
                    center: Vec3(if step % 2 == 0 { -0.1 } else { 0.1 }, 0., 0.025),
                    ..dab()
                },
            );
            let (fixed_center, fixed_normal) = doc.stroke_flatten_planes[0];
            for (i, p) in doc.mesh().vertices.iter().enumerate() {
                if !eligible[i] || mesh.mask[i] == 1. {
                    assert_eq!(*p, mesh.vertices[i]);
                }
                let before = (mesh.vertices[i] - fixed_center).dot(fixed_normal);
                let after = (*p - fixed_center).dot(fixed_normal);
                assert!(before * after >= -1e-8, "Flatten crossed its target plane");
                assert!(after.abs() <= before.abs() + 1e-6);
            }
            let current_error = error(doc.mesh());
            assert!(current_error <= previous_error + 1e-6);
            previous_error = current_error;
        }
        assert!(previous_error < initial_error * 0.1);
        doc.end_stroke();
        assert_eq!(doc.history_counts().0, history.0 + 1);
        let result = doc.geometry_snapshot();
        doc.undo();
        assert_eq!(doc.mesh(), &mesh);
        doc.redo();
        assert_eq!(doc.geometry_snapshot(), result);
        let saved: Project =
            serde_json::from_str(&serde_json::to_string(&doc.project()).unwrap()).unwrap();
        assert_eq!(saved.mesh, *doc.mesh());
        let new_dab = Dab {
            center: Vec3(0.5, 0., 0.1),
            ..dab()
        };
        let expected_plane = doc.mesh().surface_planes(&[new_dab])[0];
        doc.apply_dab(&brush, new_dab);
        assert_eq!(
            doc.stroke_flatten_planes[0], expected_plane,
            "a new stroke must resample"
        );
    }

    #[test]
    fn flatten_pressure_inversion_and_symmetry_keep_their_meaning() {
        let mesh = patch();
        let brush = Brush::new(Kind::Flatten);
        let run = |pressure, invert, mirrored| {
            let mut doc = Document::default();
            doc.replace_geometry(Geometry::new(mesh.clone())).unwrap();
            doc.set_symmetry(Symmetry {
                enabled: mirrored,
                mirror: [false, true, false],
                ..Default::default()
            })
            .unwrap();
            doc.apply_dab(
                &brush,
                Dab {
                    pressure,
                    invert,
                    ..dab()
                },
            );
            doc.mesh().clone()
        };
        let full = run(1., false, false);
        let half = run(0.5, false, false);
        let inverse = run(1., true, false);
        let copies = run(1., false, true);
        for (i, p) in mesh.vertices.iter().enumerate() {
            let delta = full.vertices[i] - *p;
            assert!((half.vertices[i] - *p - delta * 0.5).len() < 1e-6);
            assert!((inverse.vertices[i] - *p + delta).len() < 1e-6);
            assert!((copies.vertices[i] - full.vertices[i]).len() < 1e-5);
        }
    }
}
