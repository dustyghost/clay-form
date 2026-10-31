//! Replace one half with a reflected source half, sharing the center seam.
use super::{Axis, Geometry, Mesh, MeshSettings, SlicePlane, Vec3, max_triangles, max_vertices};

#[derive(Clone, Copy, Debug)]
pub struct MirrorOptions {
    pub axis: Axis,
    /// Copy the positive half to the negative half when true.
    pub source_positive: bool,
    /// Object-space coordinate of the mirror plane along the chosen axis.
    pub offset: f32,
}
impl MirrorOptions {
    fn coordinate(self, p: Vec3) -> f32 {
        match self.axis {
            Axis::X => p.0,
            Axis::Y => p.1,
            Axis::Z => p.2,
        }
    }
    fn set_coordinate(self, p: &mut Vec3, value: f32) {
        match self.axis {
            Axis::X => p.0 = value,
            Axis::Y => p.1 = value,
            Axis::Z => p.2 = value,
        }
    }
}
fn check_cancel(cancel: &impl Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Mirror cancelled".into())
    } else {
        Ok(())
    }
}
impl Geometry {
    pub fn mirror(
        &mut self,
        options: MirrorOptions,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        if self.settings.primitive.is_some() {
            return Err("Convert the primitive to a sculpting mesh before mirroring".into());
        }
        if !self.scene.active_metadata.visible || self.mesh.has_hidden_faces() {
            return Err("Show the active model and all of its faces before mirroring".into());
        }
        if self.scene.active_metadata.temporary_thickness > 0. {
            return Err("Apply or remove thickness modifier before mirroring".into());
        }
        let mesh = self.mesh.mirror_half(options, cancel)?;
        check_cancel(cancel)?;
        self.mesh = mesh;
        self.settings = MeshSettings {
            resolution: self.settings.resolution,
            ..Default::default()
        };
        Ok(())
    }
}
impl Mesh {
    pub fn mirror_half(
        &self,
        options: MirrorOptions,
        cancel: &impl Fn() -> bool,
    ) -> Result<Self, String> {
        check_cancel(cancel)?;
        self.validate()?;
        if !options.offset.is_finite() || options.offset.abs() > 1_000_000. {
            return Err("Mirror center must be finite and within model coordinate limits".into());
        }
        if self.has_hidden_faces() {
            return Err("Show all faces before mirroring".into());
        }
        let was_closed = self.validate_closed().is_ok();
        let distance = |p| options.coordinate(p) as f64 - options.offset as f64;
        let extent = self
            .vertices
            .iter()
            .map(|&p| distance(p).abs())
            .fold(0_f64, f64::max);
        let epsilon = (extent * 1e-7).max(1e-10);
        let mut input = self.clone();
        // Snap the same tolerance used by split_plane, so seam vertices are
        // exactly on the plane and are reused rather than welded by proximity.
        for (i, p) in input.vertices.iter_mut().enumerate() {
            if i % 1024 == 0 {
                check_cancel(cancel)?;
            }
            if distance(*p).abs() <= epsilon {
                options.set_coordinate(p, options.offset);
            }
        }
        input.hidden_faces.clear();
        let normal = options.axis.vector() * if options.source_positive { 1. } else { -1. };
        let (work, mut sides) = input.split_plane(
            SlicePlane {
                origin: options.axis.vector() * options.offset,
                normal,
            },
            cancel,
        )?;
        // Faces lying in the mirror plane would become duplicate/internal
        // walls. Removing them also allows a capped half-model to be joined.
        for (face, source) in work.triangles.iter().zip(&mut sides) {
            if face
                .iter()
                .all(|&i| distance(work.vertices[i]).abs() <= epsilon)
            {
                *source = false;
            }
        }
        let half = work.slice_subset(&sides, true);
        if half.triangles.is_empty() {
            return Err(
                "The selected source side has no surface away from the mirror plane".into(),
            );
        }
        let seam: Vec<_> = half
            .vertices
            .iter()
            .map(|&p| distance(p).abs() <= epsilon)
            .collect();
        let count = half.vertices.len() + seam.iter().filter(|&&on_plane| !on_plane).count();
        if count > max_vertices() || half.triangles.len() > max_triangles() / 2 {
            return Err(
                "Mirrored mesh exceeds the mesh size limit; reduce resolution first".into(),
            );
        }
        let mut result = half.clone();
        let mut reflected = Vec::with_capacity(half.vertices.len());
        for (i, &p) in half.vertices.iter().enumerate() {
            if i % 1024 == 0 {
                check_cancel(cancel)?;
            }
            if seam[i] {
                options.set_coordinate(&mut result.vertices[i], options.offset);
                reflected.push(i);
            } else {
                reflected.push(result.vertices.len());
                let mut mirror = p;
                options.set_coordinate(
                    &mut mirror,
                    (2. * options.offset as f64 - options.coordinate(p) as f64) as f32,
                );
                result.vertices.push(mirror);
                result.mask.push(half.mask[i]);
                result.append_material(&half, &[(i, 1.)]);
                for (out, original) in result.vertex_groups.iter_mut().zip(&half.vertex_groups) {
                    out.weights.push(original.weights[i]);
                }
            }
        }
        for (i, &[a, b, c]) in half.triangles.iter().enumerate() {
            if i % 1024 == 0 {
                check_cancel(cancel)?;
            }
            result
                .triangles
                .push([reflected[a], reflected[c], reflected[b]]);
        }
        result.polygroups.extend_from_slice(&half.polygroups);
        result.quads.extend(
            half.quads
                .iter()
                .map(|&[a, b, c, d]| [reflected[a], reflected[d], reflected[c], reflected[b]]),
        );
        result.creases.extend(
            half.creases
                .iter()
                .map(|&[a, b]| super::edges::edge(reflected[a], reflected[b])),
        );
        result.creases.sort_unstable();
        result.creases.dedup();
        result.validate()?;
        if was_closed {
            result
                .validate_closed()
                .map_err(|e| format!("Mirror could not create a closed center seam: {e}"))?;
        }
        check_cancel(cancel)?;
        Ok(result)
    }
}
