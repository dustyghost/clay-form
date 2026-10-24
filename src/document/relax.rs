//! Local redistribution retains one surface across consecutive Relax strokes.
use super::*;
use crate::sculpt::{RelaxationOptions, RelaxationSurface};

impl Document {
    pub(crate) fn relax_stroke_active(&self) -> bool {
        self.relax_stroke.is_some()
    }
    pub(super) fn apply_relax_dab(
        &mut self,
        brush: &Brush,
        dab: Dab,
        stencil: Option<&crate::sculpt::alpha::Projection>,
    ) -> Result<bool, String> {
        if self.relax_stroke.is_none() {
            self.end_stroke();
            let dirty = self.dirty;
            self.begin_stroke();
            let mut settings = self.capture_edit_settings();
            settings.active_brush = Some(brush.clone());
            settings.first_dab = Some(serde_json::json!({
                "pressure": dab.pressure, "invert": dab.invert, "radius_world": dab.radius,
            }));
            self.relax_stroke = Some((dirty, settings));
        }
        let result = (|| {
            if self
                .relax_surface
                .as_ref()
                .is_none_or(|(revision, _)| *revision != self.display_revision)
            {
                self.relax_surface = Some((
                    self.display_revision,
                    RelaxationSurface::new(self.mesh(), &|| false)?,
                ));
            }
            let dabs = self.symmetry.dabs(dab);
            let stencils = stencil.map_or_else(Vec::new, |s| s.copies(&self.symmetry));
            let mesh = &self.geometry.mesh;
            let normals = mesh.normals();
            let weights: Vec<_> = mesh
                .vertices
                .iter()
                .enumerate()
                .map(|(i, &p)| {
                    dabs.iter()
                        .enumerate()
                        .map(|(j, dab)| {
                            let offset = p - dab.center;
                            if dab.radius <= 0.
                                || offset.len() > dab.radius * 1.5
                                || normals[i].dot(dab.normal) < 0.1
                            {
                                return 0.;
                            }
                            let pressure = if brush.pressure {
                                dab.pressure.powf(brush.pressure_curve)
                            } else {
                                1.
                            };
                            brush.weight(
                                offset.dot(dab.right) / dab.radius,
                                offset.dot(dab.up) / dab.radius,
                            ) * pressure
                                * stencils.get(j).map_or(1., |s| s.weight(p))
                        })
                        .fold(0_f32, f32::max)
                        .clamp(0., 1.)
                })
                .collect();
            let (mesh, report) = self.relax_surface.as_mut().unwrap().1.apply(
                mesh,
                RelaxationOptions {
                    strength: brush.strength,
                    iterations: 1,
                    max_deviation: brush.relax_tolerance,
                },
                Some(&weights),
                &|| false,
            )?;
            let changed = report.moved > 0;
            self.relax_status = Some(format!(
                "Relax: {}. Surface tolerance and fixed topology limit redistribution.",
                report.summary()
            ));
            if changed {
                self.geometry.mesh = mesh;
                self.stroke_deformed = true;
                self.dirty = true;
                self.display_changed();
            }
            self.relax_surface.as_mut().unwrap().0 = self.display_revision;
            Ok::<_, String>(changed)
        })();
        if let Err(error) = &result {
            self.relax_status = Some(error.clone());
        }
        result
    }

    /// Relax delays history recording until release, so cancellation preserves redo.
    pub(crate) fn cancel_relax_stroke(&mut self) -> bool {
        let Some((dirty, _)) = self.relax_stroke.take() else {
            return false;
        };
        if let Some(before) = self.stroke_before.take() {
            self.geometry = before;
            self.display_changed();
            if let Some((revision, _)) = &mut self.relax_surface {
                *revision = self.display_revision;
            }
        }
        self.dirty = dirty;
        self.stroke_deformed = false;
        self.end_stroke();
        self.relax_status = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, Vec3};

    fn scene() -> Document {
        let mut p = Primitive::for_kind(PrimitiveKind::Plane);
        p.divisions = [24, 24, 1];
        let mut mesh = p.mesh().unwrap();
        for v in &mut mesh.vertices {
            if v.0.abs() < 0.9 && v.1.abs() < 0.9 {
                v.0 += 0.025 * (v.1 * 15.).sin();
                v.1 += 0.02 * (v.0 * 13.).cos();
            }
        }
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(mesh)).unwrap();
        d.mark_saved("relax.clay");
        d.history.reset();
        d
    }
    fn dab() -> Dab {
        Dab {
            center: Vec3(0.25, 0., 0.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.55,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        }
    }
    #[test]
    fn relax_brush_stroke_boundaries_do_not_recapture_or_accumulate_surface_drift() {
        let mut continuous = scene();
        let mut separate = scene();
        let source = continuous.mesh().clone();
        let brush = Brush::new(Kind::Relax);
        for _ in 0..12 {
            continuous.apply_dab(&brush, dab());
            separate.apply_dab(&brush, dab());
            separate.end_stroke();
        }
        continuous.end_stroke();
        assert_eq!(continuous.mesh(), separate.mesh());
        assert_ne!(continuous.mesh(), &source);
        assert_eq!(continuous.history_counts(), (1, 0));
        assert_eq!(continuous.undo_label(), Some("Relax stroke"));
        let after = continuous.geometry_snapshot();
        continuous.undo();
        assert_eq!(continuous.mesh(), &source);
        continuous.redo();
        assert_eq!(continuous.geometry_snapshot(), after);
        let project: Project =
            serde_json::from_slice(&serde_json::to_vec(&continuous.project()).unwrap()).unwrap();
        let mut reopened = Document::default();
        reopened.replace_project(project, "relax.clay").unwrap();
        assert_eq!(reopened.geometry_snapshot(), after);
        assert!(reopened.brushes().iter().any(|b| b.kind == Kind::Relax));
        let mut attributes = continuous.mesh().clone();
        attributes.vertices = source.vertices.clone();
        assert_eq!(attributes, source);
        assert!(continuous.mesh().vertices.iter().all(|p| p.2 == 0.));
        for (&before, &after) in source.vertices.iter().zip(&continuous.mesh().vertices) {
            if (before - dab().center).len() >= dab().radius {
                assert_eq!(before, after);
            }
        }
    }
    #[test]
    fn relax_brush_consecutive_groove_strokes_keep_the_original_surface() {
        let source = crate::sculpt::relaxation_fixture();
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(source.clone())).unwrap();
        let brush = Brush::new(Kind::Relax);
        let stamp = Dab {
            center: Vec3(0., 0., 0.9),
            radius: 0.9,
            ..dab()
        };
        for _ in 0..12 {
            d.apply_dab(&brush, stamp);
            d.end_stroke();
        }
        assert_ne!(d.mesh(), &source);
        let (_, report) = d
            .mesh()
            .relax_mesh(
                &source,
                RelaxationOptions {
                    strength: 0.,
                    iterations: 1,
                    max_deviation: brush.relax_tolerance,
                },
                &|| false,
            )
            .unwrap();
        assert!(report.max_deviation <= brush.relax_tolerance);
        let floor = |m: &Mesh| {
            m.vertices
                .iter()
                .filter(|p| p.2 > 0.5)
                .map(|p| p.2)
                .fold(f32::INFINITY, f32::min)
        };
        assert!((floor(d.mesh()) - floor(&source)).abs() <= brush.relax_tolerance);
        d.mesh().validate_closed().unwrap();
    }

    #[test]
    fn relax_brush_other_surface_edits_invalidate_the_reference() {
        let mut d = scene();
        let brush = Brush::new(Kind::Relax);
        d.apply_dab(&brush, dab());
        d.end_stroke();
        let old_revision = d.relax_surface.as_ref().unwrap().0;
        d.apply_dab(&Brush::new(Kind::Inflate), dab());
        d.end_stroke();
        assert_ne!(old_revision, d.display_revision);
        let raised = d.mesh().clone();
        d.apply_dab(&brush, dab());
        d.end_stroke();
        assert_eq!(d.relax_surface.as_ref().unwrap().0, d.display_revision);
        d.mesh()
            .relax_mesh(
                &raised,
                RelaxationOptions {
                    strength: 0.,
                    iterations: 1,
                    max_deviation: brush.relax_tolerance,
                },
                &|| false,
            )
            .unwrap();
    }

    #[test]
    fn relax_brush_cancel_preserves_dirty_history_and_redo() {
        let mut d = scene();
        let brush = Brush::new(Kind::Relax);
        let original = d.geometry_snapshot();
        d.apply_dab(&brush, dab());
        d.end_stroke();
        d.undo();
        d.mark_saved("relax.clay");
        let counts = d.history_counts();
        assert!(d.apply_dab(&brush, dab()));
        assert_eq!(d.history_counts(), counts);
        assert!(d.cancel_relax_stroke());
        assert_eq!(d.geometry_snapshot(), original);
        assert_eq!(d.history_counts(), counts);
        assert!(!d.is_dirty());
        d.redo();
        assert_ne!(d.geometry_snapshot(), original);
    }
    #[test]
    fn relax_brush_pressure_masks_hidden_faces_creases_and_symmetry() {
        let mut d = scene();
        let brush = Brush::new(Kind::Relax);
        let original = d.mesh().clone();
        assert!(!d.apply_dab(
            &brush,
            Dab {
                pressure: 0.,
                ..dab()
            }
        ));
        d.end_stroke();
        assert_eq!(d.mesh(), &original);
        assert_eq!(d.history_counts(), (0, 0));
        d.geometry.mesh.mask[312] = 1.;
        d.geometry.mesh.creases.push([311, 312]);
        d.geometry.mesh.hidden_faces = vec![false; d.mesh().triangles.len()];
        d.geometry.mesh.hidden_faces[600] = true;
        d.symmetry.enabled = true;
        let source = d.mesh().clone();
        d.apply_dab(&brush, dab());
        let out = d.mesh();
        for i in [311, 312].into_iter().chain(source.triangles[600]) {
            assert_eq!(out.vertices[i], source.vertices[i]);
        }
        assert!(
            out.vertices
                .iter()
                .zip(&source.vertices)
                .any(|(a, b)| a != b && b.0 < -0.3)
        );
        out.validate().unwrap();
        let displacement = |pressure, falloff| {
            let mut d = scene();
            let mut b = brush.clone();
            b.falloff = falloff;
            d.apply_dab(&b, Dab { pressure, ..dab() });
            d.mesh()
                .vertices
                .iter()
                .zip(&original.vertices)
                .map(|(a, b)| (*a - *b).len())
                .sum::<f32>()
        };
        assert!(displacement(0.25, 1.5) < displacement(1., 1.5));
        assert!(displacement(1., 4.) < displacement(1., 1.));
    }
    #[test]
    fn relax_brush_repeated_inflate_dabs_retain_reference_and_improve_spacing() {
        let source = crate::sculpt::inflated_relaxation_fixture();
        let mut d = Document::default();
        d.replace_geometry(Geometry::new(source.clone())).unwrap();
        d.set_adaptive(AdaptiveSettings {
            enabled: true,
            ..Default::default()
        })
        .unwrap();
        let mut brush = Brush::new(Kind::Relax);
        brush.strength = 1.;
        brush.relax_tolerance = 0.01;
        let stamp = Dab {
            center: Vec3(0., 0., 1.1),
            radius: 1.5,
            ..dab()
        };
        let aspect = |mesh: &Mesh| {
            let ratios: Vec<_> = mesh
                .quads
                .iter()
                .filter(|q| q.iter().all(|&i| source.vertices[i].2 > 0.5))
                .map(|q| {
                    let edges = std::array::from_fn::<_, 4, _>(|i| {
                        (mesh.vertices[q[i]] - mesh.vertices[q[(i + 1) % 4]]).len()
                    });
                    edges.into_iter().fold(0_f32, f32::max)
                        / edges.into_iter().fold(f32::INFINITY, f32::min)
                })
                .collect();
            ratios.iter().sum::<f32>() / ratios.len() as f32
        };
        for _ in 0..3 {
            for _ in 0..12 {
                d.apply_dab(&brush, stamp);
            }
            d.end_stroke();
        }
        assert!(aspect(d.mesh()) < aspect(&source));
        let (_, report) = d
            .mesh()
            .relax_mesh(
                &source,
                RelaxationOptions {
                    strength: 0.,
                    iterations: 1,
                    max_deviation: 0.01,
                },
                &|| false,
            )
            .unwrap();
        assert!(report.max_deviation <= 0.01);
        d.mesh().validate_closed().unwrap();
    }
}
