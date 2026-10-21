//! Cyclic authoring and sampling. The seam belongs to a control point, while
//! sampling divides the entire perimeter evenly and never repeats that point.
use super::{MAX_SAMPLES, PathPoint, PathSample, SurfacePath};
use crate::sculpt::Vec3;

const EPSILON: f32 = 1e-6;
const SUBDIVISIONS: usize = 24;

impl SurfacePath {
    /// Reverse traversal without moving the seam in model space.
    pub fn reverse(&mut self) {
        let preserve = self.has_attached_route();
        if !self.points.is_empty() {
            self.seam = if self.closed {
                self.points.len() - 1 - self.seam.min(self.points.len() - 1)
            } else {
                0
            };
            self.points.reverse();
            self.corner_rounding.reverse();
        }
        if preserve {
            let signature = self.shape_signature();
            let route = self.attached_route.as_mut().unwrap();
            route.points.reverse();
            route.shape_signature = signature;
        } else {
            self.attached_route = None;
        }
    }

    pub fn set_seam(&mut self, index: usize) -> Result<(), String> {
        if !self.closed {
            return Err("Close the path before choosing its seam".into());
        }
        if index >= self.points.len() {
            return Err("Choose an existing path point for the seam".into());
        }
        let preserve = self.has_attached_route();
        self.seam = index;
        if preserve {
            let signature = self.shape_signature();
            let rounded = self.corner_rounding(index) > 0.;
            let curve = self.corner_curve(index);
            let seam = curve[curve.len() / 2];
            let route = self.attached_route.as_mut().unwrap();
            route.points.pop();
            let nearest = route
                .points
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (a.position - seam.position)
                        .len()
                        .total_cmp(&(b.position - seam.position).len())
                })
                .map(|(i, _)| i)
                .unwrap();
            route.points.rotate_left(nearest);
            let seam = if rounded { route.points[0] } else { seam };
            route.points[0] = seam;
            route.points.push(seam);
            route.shape_signature = signature;
        } else {
            self.attached_route = None;
        }
        Ok(())
    }

    pub fn set_closed(&mut self, closed: bool) -> Result<(), String> {
        if closed {
            validate_points(self)?;
        } else if self.closed && !self.points.is_empty() {
            // Opening the loop starts at the visible seam.
            let seam = self.seam.min(self.points.len() - 1);
            self.points.rotate_left(seam);
            if !self.corner_rounding.is_empty() {
                self.corner_rounding.rotate_left(seam);
            }
        }
        self.closed = closed;
        self.seam = if closed {
            self.seam.min(self.points.len().saturating_sub(1))
        } else {
            0
        };
        Ok(())
    }
}

fn finite(p: Vec3) -> bool {
    p.0.is_finite() && p.1.is_finite() && p.2.is_finite()
}

fn validate_points(path: &SurfacePath) -> Result<(), String> {
    if path.points.len() < 3 {
        return Err("A closed path needs at least three distinct points".into());
    }
    for index in 0..path.points.len() {
        let a = &path.points[index];
        let b = &path.points[(index + 1) % path.points.len()];
        if !finite(a.position) || !finite(a.normal) || a.normal.len() < EPSILON {
            return Err("Closed path points need finite positions and surface normals".into());
        }
        if (a.position - b.position).len() < EPSILON {
            return Err("Separate neighboring points before closing the path".into());
        }
        if a.normal.norm().dot(b.normal.norm()) < -0.98 {
            return Err(
                "The route between opposite surface normals is ambiguous; add a point between them"
                    .into(),
            );
        }
    }
    Ok(())
}

/// A cyclic Catmull–Rom curve blended with the control polygon. Its final point
/// repeats the seam only to describe the closing segment, never as a brush dab.
pub(super) fn dense_curve(path: &SurfacePath) -> Vec<PathPoint> {
    let count = path.points.len();
    if count < 3 {
        return path.points.clone();
    }
    let smoothing = path.smoothing.clamp(0., 1.);
    let mut result = Vec::with_capacity(count * SUBDIVISIONS + 1);
    for index in 0..count {
        let i = (index + path.seam.min(count - 1)) % count;
        let a = &path.points[(i + count - 1) % count];
        let b = &path.points[i];
        let c = &path.points[(i + 1) % count];
        let d = &path.points[(i + 2) % count];
        for step in 0..SUBDIVISIONS {
            let t = step as f32 / SUBDIVISIONS as f32;
            let linear = b.position * (1. - t) + c.position * t;
            let curve = (b.position * 2.
                + (c.position - a.position) * t
                + (a.position * 2. - b.position * 5. + c.position * 4. - d.position) * (t * t)
                + (b.position * 3. - a.position - c.position * 3. + d.position) * (t * t * t))
                * 0.5;
            result.push(PathPoint {
                position: linear * (1. - smoothing) + curve * smoothing,
                normal: (b.normal.norm() * (1. - t) + c.normal.norm() * t).norm(),
            });
        }
    }
    result.push(result[0]);
    result
}

pub(super) fn sample(path: &SurfacePath, spacing: f32) -> Result<Vec<PathSample>, String> {
    validate_points(path)?;
    if !spacing.is_finite() || spacing <= 0. || !path.smoothing.is_finite() {
        return Err("Path spacing must be positive and smoothing must be finite".into());
    }
    let curve = path.dense_curve();
    sample_curve(&curve, spacing)
}

/// Resample an already surface-projected cyclic polyline. The final point must
/// repeat the first, so the perimeter includes its closing edge exactly once.
pub(super) fn sample_curve(curve: &[PathPoint], spacing: f32) -> Result<Vec<PathSample>, String> {
    if curve.len() < 4 || curve.first() != curve.last() {
        return Err("A closed curve needs three points and a matching seam endpoint".into());
    }
    if !spacing.is_finite() || spacing <= 0. {
        return Err("Path spacing must be positive".into());
    }
    if curve
        .iter()
        .any(|p| !finite(p.position) || !finite(p.normal))
    {
        return Err("Closed curve points must be finite".into());
    }
    let mut lengths = Vec::with_capacity(curve.len());
    lengths.push(0.);
    for pair in curve.windows(2) {
        lengths.push(lengths.last().unwrap() + (pair[1].position - pair[0].position).len());
    }
    let perimeter = *lengths.last().unwrap();
    if !perimeter.is_finite() || perimeter < EPSILON {
        return Err("The closed path has no usable length".into());
    }
    let count = (perimeter / spacing).ceil().max(3.) as usize;
    if count > MAX_SAMPLES {
        return Err("The closed path needs too many samples; increase brush spacing".into());
    }
    let unique = curve.len() - 1;
    let tangent_at = |i: usize| {
        let i = i % unique;
        (curve[(i + 1) % unique].position - curve[(i + unique - 1) % unique].position).norm()
    };
    let mut samples = Vec::with_capacity(count);
    let mut segment = 0;
    for index in 0..count {
        let distance = perimeter * index as f32 / count as f32;
        while segment + 2 < lengths.len() && lengths[segment + 1] <= distance {
            segment += 1;
        }
        let length = lengths[segment + 1] - lengths[segment];
        let t = ((distance - lengths[segment]) / length.max(f32::MIN_POSITIVE)).clamp(0., 1.);
        let a = &curve[segment];
        let b = &curve[segment + 1];
        let normal = (a.normal * (1. - t) + b.normal * t).norm();
        let direction = tangent_at(segment) * (1. - t) + tangent_at(segment + 1) * t;
        let tangent = (direction - normal * direction.dot(normal)).norm();
        if tangent.len() < 0.5 || normal.len() < 0.5 {
            return Err(
                "The closed path doubles back or leaves the surface; move points or add a guide point"
                    .into(),
            );
        }
        samples.push(PathSample {
            position: a.position * (1. - t) + b.position * t,
            normal,
            tangent,
            binormal: normal.cross(tangent).norm(),
            distance,
        });
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::paths::PathMode;

    fn ring(radius: f32, tilt: f32, taper: f32) -> SurfacePath {
        let axis = Vec3(tilt.sin(), 0., tilt.cos());
        let u = Vec3(tilt.cos(), 0., -tilt.sin());
        let v = Vec3(0., 1., 0.);
        SurfacePath {
            id: 1,
            name: "Ring".into(),
            target: 1,
            mode: PathMode::Surface,
            points: (0..32)
                .map(|i| {
                    let angle = std::f32::consts::TAU * i as f32 / 32.;
                    let radial = u * angle.cos() + v * angle.sin();
                    PathPoint {
                        position: axis * 0.4 + radial * radius,
                        normal: (radial + axis * taper).norm(),
                    }
                })
                .collect(),
            smoothing: 1.,
            closed: true,
            seam: 0,
            surface_signature: 0,
            attached_route: None,
            corner_rounding: Vec::new(),
        }
    }

    #[test]
    fn full_ring_has_even_spacing_and_no_duplicate_seam() {
        let path = ring(1., 0., 0.);
        let samples = sample(&path, 0.06).unwrap();
        let interval = samples[1].distance;
        assert!(interval <= 0.06);
        assert_eq!(samples[0].position, path.points[0].position);
        for i in 0..samples.len() {
            let a = &samples[i];
            let b = &samples[(i + 1) % samples.len()];
            let separation = (a.position - b.position).len();
            assert!(separation > interval * 0.995);
            assert!(separation < interval * 1.005);
            assert!(a.tangent.dot(b.tangent) > 0.99);
            assert!(a.binormal.dot(b.binormal) > 0.99);
            assert!(a.position.0.hypot(a.position.1) > 0.9998);
        }
    }

    #[test]
    fn moving_seam_and_reversal_keep_the_same_closed_geometry() {
        let mut path = ring(1., 0., 0.);
        path.set_seam(11).unwrap();
        let before = sample(&path, 0.07).unwrap();
        assert_eq!(before[0].position, path.points[11].position);
        path.reverse();
        let after = sample(&path, 0.07).unwrap();
        assert!(path.closed);
        assert_eq!(before.len(), after.len());
        assert!((before[0].position - after[0].position).len() < 1e-6);
        for i in 0..before.len() {
            let a = &before[i];
            let b = &after[(after.len() - i) % after.len()];
            assert!((a.position - b.position).len() < 2e-5);
            assert!(a.tangent.dot(b.tangent) < -0.999);
            assert!(a.normal.dot(b.normal) > 0.999);
        }
        path.set_closed(false).unwrap();
        assert!(!path.closed);
        assert_eq!(path.seam, 0);
        assert_eq!(path.points[0].position, before[0].position);
        path.reverse();
        assert_eq!(path.seam, 0);
        assert!(path.validate().is_ok());
        path.set_closed(true).unwrap();
        assert!(path.closed);
    }

    #[test]
    fn tapered_and_bent_surface_frames_remain_continuous_at_the_seam() {
        for (radius, tilt, taper) in [(0.8, 0., 0.45), (0.6, 0.8, 0.), (0.5, 1.2, -0.3)] {
            let mut path = ring(radius, tilt, taper);
            path.set_seam(17).unwrap();
            let samples = sample(&path, 0.025).unwrap();
            for i in 0..samples.len() {
                let a = &samples[i];
                let b = &samples[(i + 1) % samples.len()];
                assert!(a.normal.dot(b.normal) > 0.99);
                assert!(a.tangent.dot(b.tangent) > 0.99);
                assert!(a.binormal.dot(b.binormal) > 0.99);
                assert!(a.tangent.dot(a.normal).abs() < 1e-5);
                assert!((a.binormal.len() - 1.).abs() < 1e-5);
            }
        }
    }

    #[test]
    fn closed_path_edits_preserve_closure_and_reject_ambiguous_guides() {
        let mut path = ring(1., 0., 0.);
        path.points[5].position = path.points[5].position * 1.1;
        assert!(sample(&path, 0.05).is_ok());
        assert!(path.closed);
        path.points[1].normal = path.points[0].normal * -1.;
        assert!(sample(&path, 0.05).unwrap_err().contains("ambiguous"));
        path.points.truncate(2);
        assert!(path.set_closed(true).unwrap_err().contains("three"));
        assert!(path.set_seam(20).is_err());
    }

    #[test]
    fn reject_duplicate_endpoint_and_unbounded_sampling() {
        let mut path = ring(1., 0., 0.);
        assert!(sample(&path, 0.).is_err());
        assert!(sample(&path, f32::NAN).is_err());
        assert!(sample(&path, 1e-12).unwrap_err().contains("too many"));
        path.points.push(path.points[0]);
        assert!(path.set_closed(true).unwrap_err().contains("neighboring"));
    }

    #[test]
    fn projected_loops_remain_continuous_on_tapered_and_bent_meshes() {
        use crate::sculpt::{Primitive, PrimitiveKind, paths::mesh_signature};
        for (taper, bend) in [(0., 0.), (0.25, 0.), (0.15, 0.4)] {
            let mut primitive = Primitive::for_kind(PrimitiveKind::Cylinder);
            primitive.segments = 96;
            let mut mesh = primitive.mesh().unwrap();
            for position in &mut mesh.vertices {
                let radius = 1. - taper * position.1;
                position.0 = position.0 * radius + bend * position.1 * position.1;
                position.2 *= radius;
            }
            let normals = mesh.normals();
            let mut path = ring(1., 0., 0.);
            path.points = mesh
                .vertices
                .iter()
                .enumerate()
                .filter(|(_, p)| (p.1 - 0.5).abs() < 1e-6)
                .step_by(3)
                .map(|(i, &position)| PathPoint {
                    position,
                    normal: normals[i],
                })
                .collect();
            assert_eq!(path.points.len(), 32);
            path.surface_signature = mesh_signature(&mesh);
            path.set_seam(13).unwrap();
            let samples = path.samples_on_mesh(&mesh, 0.03).unwrap();
            let interval = samples[1].distance;
            for i in 0..samples.len() {
                let a = samples[i];
                let b = samples[(i + 1) % samples.len()];
                assert!(a.normal.dot(b.normal) > 0.98);
                assert!(a.tangent.dot(b.tangent) > 0.98);
                assert!(a.binormal.dot(b.binormal) > 0.98);
                assert!((a.position - b.position).len() > interval * 0.95);
                assert!((a.position - b.position).len() < interval * 1.05);
            }
        }
    }

    #[test]
    fn collapsed_projected_tangent_reports_unusable_route() {
        let curve: Vec<_> = [
            Vec3(0., 0., 0.),
            Vec3(1., 0., 0.),
            Vec3(0., 0., 0.),
            Vec3(0., 1., 0.),
            Vec3(0., 0., 0.),
        ]
        .into_iter()
        .map(|position| PathPoint {
            position,
            normal: Vec3(0., 0., 1.),
        })
        .collect();
        assert!(
            sample_curve(&curve, 0.25)
                .unwrap_err()
                .contains("doubles back")
        );
    }
}
