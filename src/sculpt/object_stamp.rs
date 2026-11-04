//! A captured object's top surface supplies the relief of a sculpting stamp.
use super::{
    Mesh, Vec3,
    corners::VertexCorners,
    instances::{Piece, Placement},
    projection::Surface,
};
use std::borrow::Cow;

pub(crate) struct ObjectStamp {
    source: Surface<'static>,
    min: Vec3,
    max: Vec3,
    normals: Vec<Vec3>,
    sculptable: Vec<bool>,
    corners: VertexCorners,
    visited: Vec<u32>,
    sample: u32,
    pub mesh: Mesh,
    pub changed: bool,
}
impl ObjectStamp {
    pub fn new(target: &Mesh, source: &Mesh, piece: Piece) -> Result<Self, String> {
        piece.validate()?;
        let mut local = source.clone();
        let mut min = Vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = Vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
        for point in &mut local.vertices {
            *point = piece.local(*point);
            min = Vec3(min.0.min(point.0), min.1.min(point.1), min.2.min(point.2));
            max = Vec3(max.0.max(point.0), max.1.max(point.1), max.2.max(point.2));
        }
        if max.2 <= 0. || max.0 - min.0 < 1e-6 || max.1 - min.1 < 1e-6 {
            return Err("The captured model needs a surface above its contact point to stamp. Adjust its Up axis and contact point.".into());
        }
        let source = Surface::from_mesh(Cow::Owned(local), &|| false)?;
        Ok(Self {
            source,
            min,
            max,
            normals: target.normals(),
            sculptable: target.sculptable_vertices(),
            corners: VertexCorners::new(target).ok_or("Stamp target is too large")?,
            visited: vec![0; target.vertices.len()],
            sample: 0,
            mesh: target.clone(),
            changed: false,
        })
    }
    /// Sample the original target so repeated impressions in one stroke don't build runaway spikes.
    pub fn apply(
        &mut self,
        target: &Mesh,
        placement: Placement,
        face: usize,
        invert: bool,
        offset: f32,
    ) -> Result<(), String> {
        let triangle = target.triangles.get(face).ok_or("Stamp target changed")?;
        self.sample += 1;
        let up = placement.transform.up;
        let forward = placement.transform.forward;
        let side = up.cross(forward);
        let scale = placement.transform.scale;
        let extent = (self.max.0 - self.min.0).max(self.max.1 - self.min.1);
        let depth = extent.max(1e-5) * scale;
        let mut stack = triangle.to_vec();
        let mut updates = Vec::new();
        let mut budget = 4_000_000;
        while let Some(i) = stack.pop() {
            if self.visited[i] == self.sample {
                continue;
            }
            self.visited[i] = self.sample;
            let delta = target.vertices[i] - placement.transform.origin;
            let x = delta.dot(forward) / scale;
            let y = delta.dot(side) / scale;
            if x < self.min.0
                || x > self.max.0
                || y < self.min.1
                || y > self.max.1
                || delta.dot(up).abs() > depth
                || self.normals[i].dot(up) <= 0.
            {
                continue;
            }
            // Walk only the connected, visible surface that was hit.
            for &corner in self.corners.incident(i) {
                let f = corner as usize / 3;
                if target.face_visible(f) {
                    stack.extend(
                        target.triangles[f]
                            .iter()
                            .copied()
                            .filter(|&v| self.visited[v] != self.sample),
                    );
                }
            }
            if !self.sculptable[i] || target.mask[i] >= 1. {
                continue;
            }
            let top = self.max.2 + (self.max.2 - self.min.2).max(1.) * 0.001;
            if let Some((_, hit, _)) = self.source.along_normal(
                Vec3(x, y, top),
                Vec3(0., 0., -1.),
                top - self.min.2 + 0.001,
                &mut budget,
            )? {
                let height = (hit.2 * scale + offset).max(0.) * (1. - target.mask[i]);
                let sign = if invert { -1. } else { 1. };
                let previous = (self.mesh.vertices[i] - target.vertices[i]).dot(up) * sign;
                if height > previous {
                    let next = target.vertices[i] + up * (height * sign);
                    if !super::instances::finite(next) {
                        return Err("Stamp would exceed the coordinate limit".into());
                    }
                    updates.push((i, next));
                }
            }
        }
        self.changed |= !updates.is_empty();
        for (i, point) in updates {
            self.mesh.vertices[i] = point;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{
        Primitive, PrimitiveKind,
        instances::{Source, Transform},
    };

    #[test]
    fn cone_object_stamp_raises_and_indents_only_its_surface_with_masks_and_original_topology() {
        let mut plane = Primitive::for_kind(PrimitiveKind::Plane);
        plane.divisions = [48, 48, 1];
        let target = plane.mesh().unwrap();
        let cone = Primitive::for_kind(PrimitiveKind::Cone).mesh().unwrap();
        let piece = Piece {
            source: Source::Asset(1),
            pivot: Vec3(0., -1., 0.),
            forward: 0,
            up: 1,
        };
        let placement = Placement {
            piece,
            transform: Transform {
                scale: 0.3,
                ..Default::default()
            },
        };
        let face = target
            .triangles
            .iter()
            .position(|tri| tri.iter().any(|&i| target.vertices[i].len() < 0.01))
            .unwrap();
        for invert in [false, true] {
            let mut stamp = ObjectStamp::new(&target, &cone, piece).unwrap();
            stamp.apply(&target, placement, face, invert, 0.).unwrap();
            assert!(stamp.changed);
            assert_eq!(stamp.mesh.triangles, target.triangles);
            assert_eq!(stamp.mesh.vertices.len(), target.vertices.len());
            let sign = if invert { -1. } else { 1. };
            let high = stamp
                .mesh
                .vertices
                .iter()
                .zip(&target.vertices)
                .map(|(a, b)| (a.2 - b.2) * sign)
                .fold(0_f32, f32::max);
            assert!(
                high > 0.5,
                "the cone reaches its captured height, got {high}"
            );
            assert!(
                stamp
                    .mesh
                    .vertices
                    .iter()
                    .zip(&target.vertices)
                    .all(|(a, b)| (a.2 - b.2) * sign >= -1e-6)
            );
            let once = stamp.mesh.clone();
            stamp.apply(&target, placement, face, invert, 0.).unwrap();
            assert_eq!(
                stamp.mesh, once,
                "holding or overlapping a stroke doesn't grow spikes"
            );
            stamp.mesh.validate().unwrap();
        }
        let mut masked = target.clone();
        masked.mask.fill(1.);
        let mut stamp = ObjectStamp::new(&masked, &cone, piece).unwrap();
        stamp.apply(&masked, placement, face, false, 0.).unwrap();
        assert_eq!(stamp.mesh, masked);
    }
}
