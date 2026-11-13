//! Native surface-field reconstruction; attributes are sampled after validation.
use super::{
    Axis, Mesh, Vec3,
    paths::{PathSample, SurfacePath},
};
mod bilateral;
mod density;
pub(crate) use bilateral::SymmetrySource;
pub(crate) use density::DensityPaint;
pub(crate) type GuideCurves = std::sync::Arc<[Vec<PathSample>]>;
pub(crate) struct Layout {
    pub mesh: Mesh,
    pub polygons: usize,
    pub guides: GuideCurves,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Options {
    pub input_triangle_limit: usize,
    pub target: usize,
    pub flow: f32,
    pub density: f32,
    pub adaptive: f32,
    pub mirror_path_x: bool,
    pub preserve_sharp: bool,
    pub sharp_angle_degrees: f64,
    pub symmetry_axis: Option<Axis>,
    pub symmetry_center: Vec3,
    pub symmetry_source: SymmetrySource,
    pub symmetry_centreline: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            input_triangle_limit: 200_000,
            target: 500,
            flow: 1.,
            density: 1.,
            adaptive: 0.,
            mirror_path_x: false,
            preserve_sharp: false,
            sharp_angle_degrees: 45.,
            symmetry_axis: None,
            symmetry_center: Vec3::default(),
            symmetry_source: SymmetrySource::Original,
            symmetry_centreline: false,
        }
    }
}
impl Options {
    pub(crate) fn require_input_budget(&self, mesh: &Mesh) -> Result<(), String> {
        if !(1..=super::max_triangles()).contains(&self.input_triangle_limit) {
            return Err(format!(
                "Input triangle limit must be between 1 and {}",
                super::max_triangles()
            ));
        }
        if mesh.triangles.len() > self.input_triangle_limit {
            return Err(format!(
                "Source has {} triangles; set Input triangle limit to at least {}",
                mesh.triangles.len(),
                mesh.triangles.len()
            ));
        }
        Ok(())
    }
}
fn check(cancel: &dyn Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Retopology cancelled".into())
    } else {
        Ok(())
    }
}
fn native(v: Vec3) -> clay_retopology::Vec3 {
    clay_retopology::Vec3::new(v.0.into(), v.1.into(), v.2.into())
}
fn solver_positions(vertices: &[Vec3]) -> Result<Vec<clay_retopology::Vec3>, String> {
    // Use the same shortest-roundtrip decimal coordinates as .clay exports and
    // the standalone solver. Exact binary widening changes near-tied preparation
    // decisions: this boundary makes live and exported inputs reproducible.
    // Every decimal maps back to the identical f32. Published output is still
    // checked against the exact in-memory source below.
    let encoded = serde_json::to_vec(vertices).map_err(|e| e.to_string())?;
    let coordinates: Vec<[f64; 3]> = serde_json::from_slice(&encoded).map_err(|e| e.to_string())?;
    Ok(coordinates
        .into_iter()
        .map(clay_retopology::Vec3::from_array)
        .collect())
}
impl Mesh {
    pub(crate) fn require_retopology_input(&self) -> Result<(), String> {
        if self.triangles.is_empty() || self.vertices.len() > self.triangles.len().saturating_mul(3)
        {
            return Err("Retopology needs source triangles with no loose vertices".into());
        }
        if !self.uv.is_empty() {
            return Err("Use a source without UVs or textures for guided retopology".into());
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn guided_retopology(
        &self,
        options: Options,
        path: Option<&SurfacePath>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Layout, String> {
        self.guided_retopology_with_progress(
            options,
            &path.into_iter().cloned().collect::<Vec<_>>(),
            cancel,
            &|_| {},
        )
    }
    pub(crate) fn guided_retopology_with_progress(
        &self,
        options: Options,
        paths: &[SurfacePath],
        cancel: &dyn Fn() -> bool,
        progress: &dyn Fn(clay_retopology::Stage),
    ) -> Result<Layout, String> {
        check(cancel)?;
        options.require_input_budget(self)?;
        self.require_retopology_input()?;
        self.validate()?;
        if self.has_hidden_faces() {
            return Err("Show all faces before guided retopology".into());
        }
        if !(4..=50_000).contains(&options.target)
            || ![options.flow, options.density, options.adaptive]
                .iter()
                .all(|x| x.is_finite() && (0. ..=1.).contains(x))
        {
            return Err(
                "Use a target of 4–50,000 quads and constraint strengths between 0 and 1".into(),
            );
        }
        let (mut lo, mut hi) = (self.vertices[0], self.vertices[0]);
        for &Vec3(x, y, z) in &self.vertices {
            lo = Vec3(lo.0.min(x), lo.1.min(y), lo.2.min(z));
            hi = Vec3(hi.0.max(x), hi.1.max(y), hi.2.max(z));
        }
        // Limit each guide to a local strip so nearby paths and thin limbs do
        // not compete across broad sections of the model.
        let radius = (hi - lo).len() * 0.025;
        let mut guides = Vec::new();
        for path in paths.iter().filter(|_| options.flow > 0.) {
            check(cancel)?;
            if path.mode != super::paths::PathMode::Surface {
                return Err("Select saved surface paths".into());
            }
            let mut samples = path
                .samples_on_mesh_cancelled(self, radius / 4., &cancel)
                .map_err(|error| format!("Guide ‘{}’: {error}", path.name))?;
            if path.closed {
                samples.push(*samples.first().ok_or("Guide has no samples")?);
            }
            if options.mirror_path_x && options.symmetry_axis.is_none() {
                let mirror = |v: Vec3| Vec3(-v.0, v.1, v.2);
                guides.push(
                    samples
                        .iter()
                        .map(|s| PathSample {
                            position: mirror(s.position),
                            normal: mirror(s.normal),
                            tangent: mirror(s.tangent),
                            binormal: mirror(s.binormal) * (-1.),
                            ..*s
                        })
                        .collect::<Vec<_>>(),
                );
            }
            if let Some(axis) = options.symmetry_axis
                && options.symmetry_source != SymmetrySource::Original
            {
                let axis = axis.vector();
                let positive = options.symmetry_source == SymmetrySource::Positive;
                guides.extend(
                    samples
                        .split(|s| {
                            if positive {
                                (s.position - options.symmetry_center).dot(axis) < 0.
                            } else {
                                (s.position - options.symmetry_center).dot(axis) > 0.
                            }
                        })
                        .filter(|segment| !segment.is_empty())
                        .map(<[PathSample]>::to_vec),
                );
            } else {
                guides.push(samples);
            }
        }
        let settings = clay_retopology::Options {
            target_quads: options.target,
            detail: self
                .retopology_detail()
                .into_iter()
                .map(|w| f64::from(w * options.density))
                .collect(),
            guides: guides
                .iter()
                .flat_map(|curve| {
                    if curve.len() > 1 && curve.first() == curve.last() {
                        &curve[..curve.len() - 1]
                    } else {
                        curve
                    }
                })
                .filter(|_| options.flow > 0.)
                .map(|s| clay_retopology::Guide {
                    position: native(s.position),
                    normal: native(s.normal),
                    tangent: native(s.tangent),
                    radius: radius.into(),
                    strength: options.flow.into(),
                })
                .collect(),
            adaptive: options.adaptive.into(),
            preserve_sharp: options.preserve_sharp,
            sharp_angle_degrees: options.sharp_angle_degrees,
            max_vertices: 150_000,
        };
        let mirrored_reference;
        let (mut result, reference) = if let Some(symmetry_axis) = options.symmetry_axis {
            let axis = symmetry_axis.vector();
            let offset = options.symmetry_center.dot(axis);
            mirrored_reference = match options.symmetry_source {
                SymmetrySource::Original => None,
                side => Some(bilateral::mirrored_reference(
                    self,
                    super::MirrorOptions {
                        axis: symmetry_axis,
                        source_positive: side == SymmetrySource::Positive,
                        offset,
                    },
                    cancel,
                )?),
            };
            let reference = mirrored_reference.as_ref().unwrap_or(self);
            let mut result = if options.symmetry_source == SymmetrySource::Original {
                bilateral::reconstruct(reference, &settings, axis, options, cancel, progress)?
            } else {
                bilateral::mirror_layout(
                    self, reference, &settings, axis, options, cancel, progress,
                )?
            };
            if options.symmetry_centreline && options.symmetry_source == SymmetrySource::Original {
                bilateral::split_centreline(&mut result, axis, offset, cancel)?;
            }
            let reflected: Vec<_> = guides
                .iter()
                .map(|curve| {
                    curve
                        .iter()
                        .map(|s| PathSample {
                            position: bilateral::reflect_point(s.position, axis, offset),
                            normal: bilateral::reflect(s.normal, axis),
                            tangent: bilateral::reflect(s.tangent, axis),
                            binormal: bilateral::reflect(s.binormal, axis) * -1.,
                            ..*s
                        })
                        .collect()
                })
                .collect();
            guides.extend(reflected);
            (result, reference)
        } else {
            let source = clay_retopology::Surface {
                positions: solver_positions(&self.vertices)?,
                triangles: self.triangles.clone(),
            };
            (
                clay_retopology::reconstruct_regularized_with_progress(
                    &source, &settings, cancel, progress,
                )?,
                self,
            )
        };
        check(cancel)?;
        for point in &mut result.positions {
            *point = native(Vec3(point.x as f32, point.y as f32, point.z as f32));
        }
        let exact_source = clay_retopology::Surface {
            positions: reference.vertices.iter().copied().map(native).collect(),
            triangles: reference.triangles.clone(),
        };
        let validation_settings = if options.symmetry_axis.is_some()
            && options.symmetry_source != SymmetrySource::Original
        {
            let mut settings = settings;
            settings.detail = reference
                .retopology_detail()
                .into_iter()
                .map(|w| f64::from(w * options.density))
                .collect();
            settings
        } else {
            settings
        };
        clay_retopology::validate_layout(&exact_source, &validation_settings, &mut result, cancel)?;
        let mut mesh = Mesh {
            vertices: result
                .positions
                .iter()
                .map(|p| Vec3(p.x as f32, p.y as f32, p.z as f32))
                .collect(),
            // Keep the solver's validated diagonal, including concave quads.
            triangles: result.triangles,
            quads: result.quads,
            ..Default::default()
        };
        reference.transfer_layout_attributes(&mut mesh, cancel)?;
        mesh.validate()?;
        check(cancel)?;
        Ok(Layout {
            polygons: mesh.quads.len(),
            mesh,
            guides: guides.into(),
        })
    }
    pub(crate) fn quad_mirror_available(&self) -> bool {
        !self.quads.is_empty()
            && self.triangles.len() == 2 * self.quads.len()
            && self.uv.is_empty()
            && self.creases.is_empty()
            && self.shading_seams.is_none()
    }

    pub(crate) fn mirror_with_quad_bridge(
        &self,
        options: super::MirrorOptions,
        centreline: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        if !self.quad_mirror_available() {
            return Err("Bridge with quads needs an all-quad mesh without UVs or creases".into());
        }
        self.validate_closed()?;
        let reference = bilateral::mirrored_reference(self, options, cancel)?;
        let sign = if options.source_positive { 1. } else { -1. };
        let input = clay_retopology::Layout {
            positions: self.vertices.iter().copied().map(native).collect(),
            quads: self.quads.clone(),
            triangles: Vec::new(),
            diagnostics: Default::default(),
        };
        let mut layout = bilateral::bridge_layout(
            input,
            &reference,
            options.axis.vector() * sign,
            options.offset * sign,
            self.quads.len(),
            centreline,
            cancel,
        )?;
        let vertices: Vec<_> = layout
            .positions
            .iter()
            .map(|p| Vec3(p.x as f32, p.y as f32, p.z as f32))
            .collect();
        layout.positions = vertices.iter().copied().map(native).collect();
        clay_retopology::validate_layout(
            &clay_retopology::Surface {
                positions: reference.vertices.iter().copied().map(native).collect(),
                triangles: reference.triangles.clone(),
            },
            &clay_retopology::Options {
                target_quads: self.quads.len(),
                max_vertices: super::max_vertices(),
                ..Default::default()
            },
            &mut layout,
            cancel,
        )?;
        let mut mesh = Self {
            vertices,
            triangles: layout.triangles,
            quads: layout.quads,
            ..Default::default()
        };
        reference.transfer_layout_attributes(&mut mesh, cancel)?;
        mesh.validate_closed()?;
        Ok(mesh)
    }

    fn transfer_layout_attributes(
        &self,
        dest: &mut Mesh,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        let surface = super::projection::Surface::new(self, &cancel)?;
        let mut budget = 50_000_000;
        dest.vertex_groups = self
            .vertex_groups
            .iter()
            .map(|g| super::VertexGroup {
                weights: Vec::with_capacity(dest.vertices.len()),
                ..g.clone()
            })
            .collect();
        for (i, &p) in dest.vertices.iter().enumerate() {
            if i % 256 == 0 {
                check(cancel)?;
            }
            let (face, q, _) = surface
                .nearest(p, f32::MAX.sqrt(), None, &mut budget)?
                .ok_or("Could not transfer source attributes")?;
            let ids = self.triangles[face];
            let [a, b, c] = ids.map(|v| self.vertices[v]);
            let (u, v, w) = (b - a, c - a, q - a);
            let (uu, uv, vv, wu, wv) = (u.dot(u), u.dot(v), v.dot(v), w.dot(u), w.dot(v));
            let denominator = uu * vv - uv * uv;
            let weights = if denominator > 1e-20 {
                let y = ((vv * wu - uv * wv) / denominator).clamp(0., 1.);
                let z = ((uu * wv - uv * wu) / denominator).clamp(0., 1.);
                let x = (1. - y - z).max(0.);
                let sum = x + y + z;
                [x / sum, y / sum, z / sum]
            } else {
                [1., 0., 0.]
            };
            let sample = |values: &[f32]| {
                ids.into_iter()
                    .zip(weights)
                    .map(|(v, w)| values[v] * w)
                    .sum::<f32>()
                    .clamp(0., 1.)
            };
            dest.mask.push(sample(&self.mask));
            for (old, new) in self.vertex_groups.iter().zip(&mut dest.vertex_groups) {
                new.weights.push(sample(&old.weights));
            }
            if !self.materials.is_empty() {
                dest.materials.push(super::Material::blend(
                    ids.into_iter()
                        .zip(weights)
                        .map(|(v, w)| (self.material(v), w)),
                ));
            }
        }
        if !self.polygroups.is_empty() {
            for (i, quad) in dest.quads.iter().enumerate() {
                if i % 256 == 0 {
                    check(cancel)?;
                }
                let center = quad
                    .iter()
                    .map(|&v| dest.vertices[v])
                    .fold(Vec3::default(), |a, b| a + b)
                    * 0.25;
                let (face, _, _) = surface
                    .nearest(center, f32::MAX.sqrt(), None, &mut budget)?
                    .ok_or("Could not transfer face groups")?;
                dest.polygroups.extend([self.polygroups[face]; 2]);
            }
            dest.canonicalize_polygroups();
        }
        Ok(())
    }
}
pub(crate) fn progress_label(stage: clay_retopology::Stage) -> String {
    use clay_retopology::Stage::*;
    match stage {
        Preparation { iteration, .. } => format!("Preparing surface · pass {iteration}"),
        RetryPreparation => {
            "Grid repair failed. Retrying with different surface preparation…".into()
        }
        Sampling | Hierarchy { .. } => "Building surface samples…".into(),
        Orientation => "Solving edge directions…".into(),
        Position => "Placing the quad grid…".into(),
        Regularity { .. } => "Connecting the quad grid…".into(),
        Subdivision { .. } => "Resolving grid spacing…".into(),
        Inversions { .. } | Repair { .. } | CoarseRepair { .. } => "Repairing the grid…".into(),
        Fitting | SurfaceFit => "Fitting quads to the surface…".into(),
        Extraction => "Building quads…".into(),
        QuadOptimization => "Improving quad connections…".into(),
        OriginalSurfaceValidation => "Checking against the original sculpt…".into(),
    }
}
#[cfg(test)]
pub(crate) mod tests;
