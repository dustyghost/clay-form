use super::{Brush, Kind, Vec3};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy)]
pub struct Dab {
    pub center: Vec3,
    pub normal: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    pub radius: f32,
    pub pressure: f32,
    pub delta: Vec3,
    pub invert: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Mesh {
    /// Persistent hard polygon edges, sorted and unique.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub creases: Vec<[usize; 2]>,
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[usize; 3]>,
    /// Polygon topology; triangles remain the surface used for picking and rendering.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quads: Vec<[usize; 4]>,
    pub mask: Vec<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vertex_groups: Vec<super::VertexGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub polygroups: Vec<usize>,
    /// Independent face visibility; empty means no faces hidden by selection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden_faces: Vec<bool>,
}
impl Mesh {
    /// Copy fully masked faces and compact their vertices; the boundary stays open.
    pub fn extract_masked(&self) -> Result<Self, String> {
        self.extract_masked_threshold(0.5)
    }
    pub fn extract_masked_threshold(&self, threshold: f32) -> Result<Self, String> {
        let mut result = Self {
            creases: Vec::new(),
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            quads: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
            mask: Vec::new(),
        };
        let mut indices = vec![usize::MAX; self.vertices.len()];
        let mut old_vertices = Vec::new();
        let mut old_faces = Vec::new();
        for (fi, face) in self.triangles.iter().enumerate() {
            if !face
                .iter()
                .all(|&i| super::mask_selected(self.mask[i], threshold))
            {
                continue;
            }
            let mut triangle = [0; 3];
            for (corner, &i) in face.iter().enumerate() {
                if indices[i] == usize::MAX {
                    indices[i] = result.vertices.len();
                    result.vertices.push(self.vertices[i]);
                    old_vertices.push(i);
                    result.mask.push(0.);
                }
                triangle[corner] = indices[i];
            }
            result.triangles.push(triangle);
            old_faces.push(fi);
        }
        for q in &self.quads {
            if q.iter()
                .all(|&i| super::mask_selected(self.mask[i], threshold))
            {
                result.quads.push(q.map(|i| indices[i]));
            }
        }
        if result.triangles.is_empty() {
            return Err("Mask faces above the threshold at every vertex before extraction".into());
        }
        self.reindex_groups(&mut result, &old_vertices, &old_faces);
        result.validate()?;
        Ok(result)
    }
    /// Remove fully masked surface faces and compact surviving vertex indices.
    /// None means the mask did not cover any complete face.
    pub fn delete_masked(&self) -> Result<Option<Self>, String> {
        self.delete_masked_threshold(0.5)
    }
    pub fn delete_masked_threshold(&self, threshold: f32) -> Result<Option<Self>, String> {
        let old_faces: Vec<_> = self
            .triangles
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                !t.iter()
                    .all(|&i| super::mask_selected(self.mask[i], threshold))
            })
            .map(|(i, _)| i)
            .collect();
        let kept: Vec<_> = self
            .triangles
            .iter()
            .copied()
            .filter(|t| {
                !t.iter()
                    .all(|&i| super::mask_selected(self.mask[i], threshold))
            })
            .collect();
        if kept.len() == self.triangles.len() {
            return Ok(None);
        }
        let key = |mut t: [usize; 3]| {
            t.sort_unstable();
            t
        };
        let faces: std::collections::HashSet<_> = kept.iter().copied().map(key).collect();
        let quads: Vec<_> = self
            .quads
            .iter()
            .copied()
            .filter(|&[a, b, c, d]| {
                (faces.contains(&key([a, b, c])) && faces.contains(&key([a, c, d])))
                    || (faces.contains(&key([a, b, d])) && faces.contains(&key([b, c, d])))
            })
            .collect();
        let mut result = Self {
            creases: Vec::new(),
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
            quads: Vec::new(),
            mask: Vec::new(),
        };
        let mut indices = vec![usize::MAX; self.vertices.len()];
        for &i in kept.iter().flatten() {
            indices[i] = 0;
        }
        let mut old_vertices = Vec::new();
        for (i, index) in indices.iter_mut().enumerate() {
            if *index != usize::MAX {
                *index = result.vertices.len();
                result.vertices.push(self.vertices[i]);
                old_vertices.push(i);
                result.mask.push(self.mask[i]);
            }
        }
        result.triangles = kept.into_iter().map(|t| t.map(|i| indices[i])).collect();
        result.quads = quads.into_iter().map(|q| q.map(|i| indices[i])).collect();
        self.reindex_groups(&mut result, &old_vertices, &old_faces);
        result.validate()?;
        Ok(Some(result))
    }

    /// A welded cube sphere has only quad polygons, with no triangle pole fans.
    pub fn sphere() -> Self {
        let mut cube = super::Primitive::for_kind(super::PrimitiveKind::Cube);
        cube.divisions = [28; 3];
        let mut mesh = cube.mesh().expect("fixed cube sphere parameters are valid");
        for p in &mut mesh.vertices {
            let Vec3(x, y, z) = *p;
            let (xx, yy, zz) = (x * x, y * y, z * z);
            // Spherified-cube mapping avoids the crowding at cube corners
            // produced by simply normalizing each grid point.
            *p = Vec3(
                x * (1. - yy * 0.5 - zz * 0.5 + yy * zz / 3.).sqrt(),
                y * (1. - zz * 0.5 - xx * 0.5 + zz * xx / 3.).sqrt(),
                z * (1. - xx * 0.5 - yy * 0.5 + xx * yy / 3.).sqrt(),
            );
        }
        // Preserve the established north-tip seed at vertex zero for strokes.
        let north = mesh
            .vertices
            .iter()
            .position(|p| *p == Vec3(0., 1., 0.))
            .unwrap();
        mesh.vertices.swap(0, north);
        let remap = |i: usize| {
            if i == 0 {
                north
            } else if i == north {
                0
            } else {
                i
            }
        };
        for face in &mut mesh.triangles {
            *face = face.map(remap);
        }
        for face in &mut mesh.quads {
            *face = face.map(remap);
        }
        mesh
    }
    pub fn normals(&self) -> Vec<Vec3> {
        let mut n = vec![Vec3::default(); self.vertices.len()];
        for &[a, b, c] in &self.triangles {
            let normal =
                (self.vertices[b] - self.vertices[a]).cross(self.vertices[c] - self.vertices[a]);
            for i in [a, b, c] {
                n[i] = n[i] + normal;
            }
        }
        n.into_iter().map(Vec3::norm).collect()
    }
    pub fn validate(&self) -> Result<(), String> {
        self.validate_groups()?;
        self.validate_creases()?;
        if self.vertices.len() > super::subdivision::MAX_VERTICES
            || self.triangles.len() > super::subdivision::MAX_TRIANGLES
            || self
                .quads
                .iter()
                .flatten()
                .any(|&i| i >= self.vertices.len())
            || self.mask.len() != self.vertices.len()
            || self
                .vertices
                .iter()
                .any(|v| !v.0.is_finite() || !v.1.is_finite() || !v.2.is_finite())
            || self
                .mask
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || self
                .triangles
                .iter()
                .flatten()
                .any(|&i| i >= self.vertices.len())
        {
            return Err("Invalid mesh data".into());
        }
        if !self.quads.is_empty() {
            let key = |mut t: [usize; 3]| {
                t.sort_unstable();
                t
            };
            let faces: std::collections::HashSet<_> =
                self.triangles.iter().copied().map(key).collect();
            let group_ids: std::collections::HashMap<_, _> = if self.polygroups.is_empty() {
                std::collections::HashMap::new()
            } else {
                self.triangles
                    .iter()
                    .enumerate()
                    .map(|(i, &t)| (key(t), self.polygroups[i]))
                    .collect()
            };
            let mut used = std::collections::HashSet::new();
            for &[a, b, c, d] in &self.quads {
                if [a, b, c, d]
                    .iter()
                    .copied()
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    != 4
                {
                    return Err("Invalid quad face".into());
                }
                let pair = if faces.contains(&key([a, b, c])) && faces.contains(&key([a, c, d])) {
                    [key([a, b, c]), key([a, c, d])]
                } else if faces.contains(&key([a, b, d])) && faces.contains(&key([b, c, d])) {
                    [key([a, b, d]), key([b, c, d])]
                } else {
                    return Err("Quad does not match surface triangulation".into());
                };
                if !self.polygroups.is_empty() && group_ids[&pair[0]] != group_ids[&pair[1]] {
                    return Err("Quad has inconsistent vertex group face assignment".into());
                }
                if !used.insert(pair[0]) || !used.insert(pair[1]) {
                    return Err("Overlapping quad faces".into());
                }
            }
        }
        Ok(())
    }
    pub fn stamp(&mut self, b: &Brush, dab: Dab) {
        self.stamp_many(b, &[dab]);
    }
    /// Blend symmetry copies against the same geometry without doubling strength.
    /// Soft Standard footprints join near the seam, bounded by the normal brush peak.
    pub fn stamp_many(&mut self, b: &Brush, dabs: &[Dab]) -> bool {
        let stable_dabs = (b.kind == Kind::Detail).then(|| self.detail_dabs(dabs));
        let dabs = stable_dabs.as_deref().unwrap_or(dabs);
        let mut deformed = false;
        let old = self.vertices.clone();
        let normals = self.normals();
        let mut avg = vec![Vec3::default(); old.len()];
        let mut count = vec![0f32; old.len()];
        if b.kind == Kind::Smooth {
            for &[a, c, d] in &self.triangles {
                for (i, j, k) in [(a, c, d), (c, a, d), (d, a, c)] {
                    avg[i] = avg[i] + old[j] + old[k];
                    count[i] += 2.;
                }
            }
        }
        let mut sculptable = self.sculptable_vertices();
        if b.kind == Kind::Smooth {
            for edge in &self.creases {
                for &v in edge {
                    sculptable[v] = false;
                }
            }
        }
        let polish = (b.kind == Kind::Polish)
            .then(|| self.polish_planes(b, dabs, &normals, &mut sculptable));
        let clay = (b.kind == Kind::Clay).then(|| self.clay_planes(dabs));
        let join_soft =
            b.kind == Kind::Standard && matches!(b.alpha, super::Alpha::Soft) && dabs.len() > 1;
        let origin = dabs.first().map_or(Vec3::default(), |dab| dab.center);
        for i in 0..old.len() {
            if !sculptable[i] {
                continue;
            }
            let mut overlap = super::overlap::SoftOverlap::new(origin);
            let mut strongest = 0.;
            let mut influence_sum = 0.;
            let mut blended = Vec3::default();
            let mut strongest_motion_squared = 0_f32;
            let mut contributions = 0;
            let mut mask_sign = 1.;
            for (dab_index, dab) in dabs.iter().enumerate() {
                let offset = old[i] - dab.center;
                if offset.len() > dab.radius * 1.5
                    || (b.kind != Kind::Snake && normals[i].dot(dab.normal) < 0.1)
                {
                    continue;
                }
                let plane = polish.as_ref().and_then(|planes| planes[dab_index]);
                if b.kind == Kind::Polish
                    && plane.is_none_or(|(_, normal)| normals[i].dot(normal) < 0.5)
                {
                    continue;
                }
                let weight = if b.kind == Kind::Snake {
                    b.snake_weight(*dab, offset)
                } else {
                    b.weight(
                        offset.dot(dab.right) / dab.radius,
                        offset.dot(dab.up) / dab.radius,
                    )
                };
                let strength = b.strength
                    * if b.pressure {
                        dab.pressure.powf(b.pressure_curve)
                    } else {
                        1.
                    };
                let influence = weight * strength;
                if influence <= 0. {
                    continue;
                }
                let sign = if b.subtract ^ dab.invert { -1. } else { 1. };
                if influence > strongest {
                    strongest = influence;
                    mask_sign = sign;
                }
                if join_soft {
                    overlap.add(dab, influence);
                }
                influence_sum += influence;
                contributions += 1;
                let Dab {
                    center,
                    normal,
                    radius,
                    delta,
                    ..
                } = *dab;
                let w = influence * (1. - self.mask[i]);
                let displacement = match b.kind {
                    Kind::Standard => normal * (radius * 0.15 * w * sign),
                    Kind::Clay => {
                        let (plane_center, plane_normal) = clay.as_ref().unwrap()[dab_index];
                        // Fill toward an offset surface plane, with the same
                        // bounded response for removal. The old subtract rule
                        // increased its displacement as a depression deepened.
                        let height = radius * 0.2;
                        let distance = (old[i] - plane_center).dot(plane_normal) * sign;
                        let depth = (height - distance).clamp(0., height * 2.);
                        let gain = if matches!(
                            b.stroke_mode,
                            super::StrokeMode::DragRect | super::StrokeMode::DragDot
                        ) {
                            4.
                        } else {
                            1.
                        };
                        plane_normal * (depth * (w * gain).min(1.) * sign)
                    }
                    Kind::Inflate => normals[i] * (radius * 0.15 * w * sign),
                    Kind::Smooth => (avg[i] * (1. / count[i].max(1.)) - old[i]) * (w * sign),
                    Kind::Flatten => normal * (-offset.dot(normal) * w * sign),
                    Kind::Polish => {
                        let (center, normal) = plane.expect("Polish requires a sampled plane");
                        normal * (-(old[i] - center).dot(normal) * w.min(1.) * sign)
                    }
                    Kind::Pinch => {
                        (center - old[i] - normal * (-offset.dot(normal))) * (w * 0.2 * sign)
                    }
                    Kind::Crease => {
                        (center - old[i]) * (w * 0.12 * sign) - normal * (radius * w * 0.08 * sign)
                    }
                    Kind::Detail => {
                        // A narrow incision with raised shoulders, plus tangent-only
                        // pinching. Pinching stays inward when the depth is inverted.
                        let tangent = offset - normal * offset.dot(normal);
                        let r = tangent.len() / radius;
                        let core = (-(r / 0.20).powi(2)).exp();
                        let shoulder = (-((r - 0.38) / 0.13).powi(2)).exp();
                        let depth = radius * (-0.18 * core + 0.045 * shoulder);
                        normal * (depth * w * sign)
                            - tangent * (0.12 * (-(r / 0.45).powi(2)).exp() * w)
                    }
                    Kind::Grab => delta * (w * sign),
                    Kind::Snake => (delta + normal * (delta.len() * 0.12)) * (w * sign),
                    Kind::Mask => Vec3::default(),
                };
                blended = blended + displacement;
                if b.kind == Kind::Standard {
                    strongest_motion_squared =
                        strongest_motion_squared.max(displacement.dot(displacement));
                }
            }
            if influence_sum <= 0. {
                continue;
            }
            if b.kind == Kind::Mask {
                self.mask[i] = (self.mask[i] + strongest * mask_sign).clamp(0., 1.);
                continue;
            }
            // Average directions smoothly across overlaps, but retain the strongest
            // falloff rather than accumulating the strength of the mirrored copies.
            let displacement = if b.kind == Kind::Standard && contributions > 1 {
                // Standard lays down a depth along a brush normal. Blend the
                // direction separately from depth: averaging diverging normals
                // otherwise weakens the seam and leaves a trough between copies.
                // Keep the strongest masked/pressure-weighted depth, never the sum.
                let mut depth = strongest_motion_squared.sqrt();
                if join_soft {
                    depth = overlap.depth(b, dabs, old[i], normals[i], 1. - self.mask[i], depth);
                }
                blended.norm() * depth
            } else {
                blended * (strongest / influence_sum)
            };
            // Freehand builds depth from many overlapping dabs. A drag stamp gets
            // one application, so give relief brushes a useful single-stamp depth.
            // Keep mask opacity and smoothing/flattening/pinching weights unchanged.
            let stamp_gain = if matches!(
                b.stroke_mode,
                super::StrokeMode::DragRect | super::StrokeMode::DragDot
            ) && matches!(
                b.kind,
                Kind::Standard | Kind::Inflate | Kind::Crease | Kind::Detail
            ) {
                4.
            } else {
                1.
            };
            self.vertices[i] = old[i] + displacement * stamp_gain;
            deformed |= self.vertices[i] != old[i];
        }
        if deformed {
            self.limit_sculpt_deformation(&old)
        } else {
            false
        }
    }
}
