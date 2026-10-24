//! A simple open path extruded through the model along the viewing direction.
use super::{Mesh, SliceMode, SliceOptions, SlicePlane, Vec3};

#[derive(Clone, Debug)]
pub struct SlicePath {
    pub points: Vec<Vec3>,
    pub depth: Vec3,
    /// Perspective paths converge at the eye instead of extruding in parallel.
    pub eye: Option<Vec3>,
}
struct Path {
    points: Vec<Vec3>,
    lengths: Vec<f32>,
    depth: Vec3,
    eye: Option<Vec3>,
}
impl Path {
    fn new(source: &SlicePath, mesh: &Mesh) -> Result<Self, String> {
        let finite = |p: Vec3| p.0.is_finite() && p.1.is_finite() && p.2.is_finite();
        if !(2..=256).contains(&source.points.len())
            || !finite(source.depth)
            || !source.depth.len().is_finite()
            || source.depth.len() < 1e-6
            || source.points.iter().any(|&p| !finite(p))
        {
            return Err("A slice path needs 2–256 finite points".into());
        }
        let depth = source.depth.norm();
        let mut points = source.points.clone();
        if let Some(eye) = source.eye {
            let distance = (eye - points[0]).dot(depth);
            if !finite(eye)
                || distance <= 1e-5
                || mesh
                    .vertices
                    .iter()
                    .any(|&p| (eye - p).dot(depth) <= distance * 0.001)
            {
                return Err(
                    "Move the model in front of the camera before cutting a perspective path"
                        .into(),
                );
            }
        }
        if points.windows(2).any(|p| (p[1] - p[0]).len() < 1e-6)
            || points
                .iter()
                .any(|&p| (p - points[0]).dot(depth).abs() > 1e-5)
        {
            return Err(
                "Slice points must form a planar path with distinct adjacent points".into(),
            );
        }
        // Extend the first and last segments beyond every source vertex.
        let reach = mesh
            .vertices
            .iter()
            .chain(&points)
            .map(|&p| (p - points[0]).len())
            .fold(1_f32, f32::max)
            * 4.;
        let n = points.len();
        points[0] = points[0] - (points[1] - points[0]).norm() * reach;
        points[n - 1] = points[n - 1] + (points[n - 1] - points[n - 2]).norm() * reach;
        let cross = |a: Vec3, b: Vec3| a.cross(b).dot(depth) as f64;
        for (i, a) in points.windows(2).enumerate() {
            let da = a[1] - a[0];
            if i + 2 < n && da.norm().dot((points[i + 2] - a[1]).norm()) < -0.9999 {
                return Err("A slice path cannot double back on itself".into());
            }
            for b in points.windows(2).skip(i + 2) {
                let db = b[1] - b[0];
                let offset = b[0] - a[0];
                let denominator = cross(da, db);
                let intersects = if denominator.abs() < 1e-10 {
                    cross(offset, da).abs() < 1e-7 && {
                        let lo = offset.dot(da) / da.dot(da);
                        let hi = (b[1] - a[0]).dot(da) / da.dot(da);
                        lo.min(hi) <= 1. && lo.max(hi) >= 0.
                    }
                } else {
                    let t = cross(offset, db) / denominator;
                    let u = cross(offset, da) / denominator;
                    (-1e-7..=1. + 1e-7).contains(&t) && (-1e-7..=1. + 1e-7).contains(&u)
                };
                if intersects {
                    return Err(
                        "The slice path or its end extensions cross; move the points apart".into(),
                    );
                }
            }
        }
        // Keep the original endpoints for numerically stable parameterization;
        // only the intersection check above needs the distant ray endpoints.
        let points = source.points.clone();
        let mut lengths = vec![0.];
        for pair in points.windows(2) {
            lengths.push(lengths.last().unwrap() + (pair[1] - pair[0]).len());
        }
        Ok(Self {
            points,
            lengths,
            depth,
            eye: source.eye,
        })
    }
    // Nearest point on a simple oriented path determines its side. All supporting
    // planes have already split the surface, so a face cannot cross that path.
    fn coordinates(&self, p: Vec3) -> (f32, f32, f32) {
        let z = (p - self.points[0]).dot(self.depth);
        let flat = if let Some(eye) = self.eye {
            let distance = (eye - self.points[0]).dot(self.depth);
            eye + (p - eye) * (distance / (distance - z))
        } else {
            p - self.depth * z
        };
        let mut best = (f32::INFINITY, 0., 0.);
        for (i, pair) in self.points.windows(2).enumerate() {
            let delta = pair[1] - pair[0];
            let t = ((flat - pair[0]).dot(delta) / delta.dot(delta)).clamp(
                if i == 0 { f32::NEG_INFINITY } else { 0. },
                if i + 2 == self.points.len() {
                    f32::INFINITY
                } else {
                    1.
                },
            );
            let offset = flat - (pair[0] + delta * t);
            let distance = offset.dot(offset);
            if distance < best.0 {
                // In the corner's endpoint region, the two segment distances tie.
                // Its bisector gives the side consistently even for tight turns;
                // choosing either segment's normal can create a spurious wedge.
                let mut normal = delta.norm().cross(self.depth);
                if t == 0. && i > 0 {
                    normal =
                        (normal + (pair[0] - self.points[i - 1]).norm().cross(self.depth)).norm();
                } else if t == 1. && i + 2 < self.points.len() {
                    normal =
                        (normal + (self.points[i + 2] - pair[1]).norm().cross(self.depth)).norm();
                }
                best = (
                    distance,
                    self.lengths[i] + t * (self.lengths[i + 1] - self.lengths[i]),
                    offset.dot(normal),
                );
            }
        }
        (best.1, z, best.2)
    }
    fn position(&self, s: f32, z: f32) -> Vec3 {
        let i = self
            .lengths
            .partition_point(|&v| v < s)
            .saturating_sub(1)
            .min(self.points.len() - 2);
        let t = (s - self.lengths[i]) / (self.lengths[i + 1] - self.lengths[i]);
        let flat = self.points[i] + (self.points[i + 1] - self.points[i]) * t;
        if let Some(eye) = self.eye {
            let distance = (eye - self.points[0]).dot(self.depth);
            eye + (flat - eye) * (1. - z / distance)
        } else {
            flat + self.depth * z
        }
    }
    fn cap(
        &self,
        mesh: &mut Mesh,
        positive: bool,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        let sign = if positive { -1. } else { 1. };
        let offset = self.coordinates(mesh.vertices[0]).0;
        let tolerance = mesh
            .vertices
            .iter()
            .map(|&p| (p - mesh.vertices[0]).len())
            .fold(0_f32, f32::max)
            * 1e-5;
        let coordinates: Vec<_> = mesh
            .vertices
            .iter()
            .map(|&p| {
                let (mut s, z, _) = self.coordinates(p);
                // Plane intersections at shallow bends have amplified roundoff.
                // Align their unfolded coordinate with the bend before splitting
                // caps, so a near-corner boundary is never split a second time.
                if let Some(&corner) = self.lengths[1..self.lengths.len() - 1]
                    .iter()
                    .min_by(|a, b| (*a - s).abs().total_cmp(&(*b - s).abs()))
                    && (corner - s).abs() < tolerance
                {
                    s = corner;
                }
                [((s - offset) * sign) as f64, z as f64]
            })
            .collect();
        let first = mesh.triangles.len();
        mesh.cap_slice_projected(&coordinates, cancel)?;
        // Triangulate in unfolded path coordinates, then split at every bend so
        // no cap triangle shortcuts across a corner when folded back into 3D.
        let mut cap = mesh.clone();
        cap.vertices = coordinates
            .iter()
            .map(|p| Vec3(p[0] as f32, p[1] as f32, 0.))
            .collect();
        cap.triangles = cap.triangles.split_off(first);
        cap.polygroups = cap.polygroups.split_off(first);
        cap.quads.clear();
        for &s in &self.lengths[1..self.lengths.len() - 1] {
            if cancel() {
                return Err("Slice cancelled".into());
            }
            cap = cap
                .split_plane(
                    SlicePlane {
                        origin: Vec3((s - offset) * sign, 0., 0.),
                        normal: Vec3(1., 0., 0.),
                    },
                    cancel,
                )?
                .0;
        }
        let original_count = mesh.vertices.len();
        mesh.vertices.extend(
            cap.vertices[original_count..]
                .iter()
                .map(|p| self.position(p.0 * sign + offset, p.1)),
        );
        mesh.mask = cap.mask;
        mesh.materials = cap.materials;
        mesh.vertex_groups = cap.vertex_groups;
        mesh.triangles.truncate(first);
        mesh.polygroups.truncate(first);
        mesh.triangles.extend(cap.triangles);
        mesh.polygroups.extend(cap.polygroups);
        mesh.validate_closed()
            .map_err(|e| format!("Could not close the path cut cleanly: {e}"))
    }
}
impl Mesh {
    pub fn slice_path(
        &self,
        path: &SlicePath,
        options: SliceOptions,
        cancel: &impl Fn() -> bool,
    ) -> Result<(Self, Option<Self>), String> {
        self.validate()?;
        let path = Path::new(path, self)?;
        let capped = options.cap && options.mode != SliceMode::Polygroups;
        if capped {
            self.validate_closed().map_err(|_| {
                "Capped cuts require a closed surface; turn off Cap cuts".to_string()
            })?;
        }
        let mut work = self.clone();
        for pair in path.points.windows(2) {
            if cancel() {
                return Err("Slice cancelled".into());
            }
            work = work
                .split_plane(
                    SlicePlane {
                        origin: pair[0],
                        normal: (pair[1] - pair[0])
                            .norm()
                            .cross(path.eye.map_or(path.depth, |eye| (eye - pair[0]).norm())),
                    },
                    cancel,
                )?
                .0;
        }
        let sides: Vec<_> = work
            .triangles
            .iter()
            .map(|face| {
                let center =
                    (work.vertices[face[0]] + work.vertices[face[1]] + work.vertices[face[2]])
                        * (1. / 3.);
                let mut d = path.coordinates(center).2;
                if d.abs() < 1e-6 {
                    // Thin triangles can have centroids rounded onto the cut.
                    // Use an off-boundary vertex before treating a face as coplanar.
                    if let Some(distance) = face
                        .iter()
                        .map(|&v| path.coordinates(work.vertices[v]).2)
                        .max_by(|a, b| a.abs().total_cmp(&b.abs()))
                        && distance.abs() > 1e-6
                    {
                        d = distance;
                    }
                }
                let positive = if d.abs() < 1e-7 {
                    // A face already on the cut belongs to the solid on its inward side.
                    let n = (work.vertices[face[1]] - work.vertices[face[0]])
                        .cross(work.vertices[face[2]] - work.vertices[face[0]])
                        .norm();
                    path.coordinates(center - n * 1e-5).2 >= 0.
                } else {
                    d > 0.
                };
                positive != (options.flip && options.mode == SliceMode::Trim)
            })
            .collect();
        work.finish_slice(&sides, options, |mesh, positive| {
            if capped {
                path.cap(
                    mesh,
                    positive != (options.flip && options.mode == SliceMode::Trim),
                    cancel,
                )?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    fn sphere() -> Mesh {
        Primitive::for_kind(PrimitiveKind::UvSphere).mesh().unwrap()
    }
    fn path(points: &[(f32, f32)]) -> SlicePath {
        SlicePath {
            points: points.iter().map(|&(x, y)| Vec3(x, y, 0.)).collect(),
            depth: Vec3(0., 0., 1.),
            eye: None,
        }
    }
    fn volume(mesh: &Mesh) -> f32 {
        mesh.triangles
            .iter()
            .map(|&[a, b, c]| mesh.vertices[a].dot(mesh.vertices[b].cross(mesh.vertices[c])) / 6.)
            .sum()
    }
    #[test]
    fn bent_path_cuts_all_modes_and_caps_follow_each_segment() {
        let source = sphere();
        for points in [
            vec![(-2., 0.13), (0., 0.13), (2., 0.6)],
            vec![(-2., -0.2), (-0.2, 0.3), (0.25, -0.3), (2., 0.2)],
            vec![(-2., 0.2), (0.1, 0.2), (0.1, 2.)],
            vec![(-2., 0.2), (0.3, -0.2), (-2., -0.2)],
        ] {
            let path = path(&points);
            let prepared = Path::new(&path, &source).unwrap();
            for mode in SliceMode::ALL {
                for flip in [false, true] {
                    let (a, b) = source
                        .slice_path(
                            &path,
                            SliceOptions {
                                mode,
                                cap: true,
                                flip,
                            },
                            &|| false,
                        )
                        .unwrap_or_else(|e| panic!("{points:?} {mode:?} flip={flip}: {e}"));
                    a.validate_closed().unwrap();
                    if let Some(b) = &b {
                        b.validate_closed().unwrap();
                        assert!((volume(&a) + volume(b) - volume(&source)).abs() < 1e-4);
                    }
                    for mesh in std::iter::once(&a).chain(b.as_ref()) {
                        for (i, face) in mesh.triangles.iter().enumerate() {
                            if mesh.polygroups[i] > 0
                                && mesh.vertex_groups[mesh.polygroups[i] - 1]
                                    .name
                                    .starts_with("Cut")
                            {
                                let center = (mesh.vertices[face[0]]
                                    + mesh.vertices[face[1]]
                                    + mesh.vertices[face[2]])
                                    * (1. / 3.);
                                assert!(
                                    prepared.coordinates(center).2.abs() < 2e-5,
                                    "cap bridges corner"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn rounded_path_cap_preserves_tiny_boundary_edges_and_the_hole() {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Torus);
        primitive.segments = 32;
        primitive.rings = 16;
        let mut source = primitive.mesh().unwrap();
        for p in &mut source.vertices {
            *p = p
                .rotated(Vec3(0., 1., 0.), 0.21)
                .rotated(Vec3(1., 0., 0.), -0.13);
        }
        source.validate_closed().unwrap();
        // This shallow curve creates a cap edge about 1e-6 long with an
        // intermediate point one f32 ULP beyond its endpoint after projection.
        let a = (-2., 0.20829564);
        let b = (-0.43961632, -0.042946994);
        let c = (2., -0.022040188);
        let points: Vec<_> = (0..=16)
            .map(|i| {
                let t = i as f32 / 16.;
                let u = 1. - t;
                (
                    u * u * a.0 + 2. * u * t * b.0 + t * t * c.0,
                    u * u * a.1 + 2. * u * t * b.1 + t * t * c.1,
                )
            })
            .collect();
        let path = path(&points);
        let (positive, negative) = source
            .slice_path(
                &path,
                SliceOptions {
                    mode: SliceMode::Separate,
                    cap: true,
                    flip: false,
                },
                &|| false,
            )
            .unwrap();
        let negative = negative.unwrap();
        for mesh in [&positive, &negative] {
            mesh.validate_closed().unwrap();
            assert!(volume(mesh) > 0.);
        }
        assert!((volume(&positive) + volume(&negative) - volume(&source)).abs() < 1e-5);
        for (flip, expected) in [(false, &positive), (true, &negative)] {
            let (trimmed, other) = source
                .slice_path(
                    &path,
                    SliceOptions {
                        mode: SliceMode::Trim,
                        cap: true,
                        flip,
                    },
                    &|| false,
                )
                .unwrap();
            assert!(other.is_none());
            trimmed.validate_closed().unwrap();
            assert!((volume(&trimmed) - volume(expected)).abs() < 1e-6);
        }
    }
    #[test]
    fn crossing_and_cancelled_paths_reject_without_changing_source() {
        let source = sphere();
        let before = source.clone();
        assert!(
            source
                .slice_path(
                    &path(&[(-2., -0.5), (1., 0.5), (-1., 0.5), (2., -0.5)]),
                    SliceOptions::default(),
                    &|| false
                )
                .unwrap_err()
                .contains("cross")
        );
        assert!(
            source
                .slice_path(
                    &path(&[(-2., 0.2), (0., 0.1), (2., 0.3)]),
                    SliceOptions::default(),
                    &|| true
                )
                .unwrap_err()
                .contains("cancelled")
        );
        assert_eq!(source, before);
    }
}

#[cfg(test)]
mod perspective_tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    #[test]
    fn perspective_path_caps_follow_converging_planes_and_preserve_volume() {
        let source = Primitive::for_kind(PrimitiveKind::UvSphere).mesh().unwrap();
        let path = SlicePath {
            points: vec![Vec3(-2., 0.2, 0.), Vec3(0., -0.2, 0.), Vec3(2., 0.3, 0.)],
            depth: Vec3(0., 0., 1.),
            eye: Some(Vec3(0., 0., 3.)),
        };
        let prepared = Path::new(&path, &source).unwrap();
        for &p in &source.vertices {
            let (s, z, _) = prepared.coordinates(p);
            let on_cut = prepared.position(s, z);
            assert!(prepared.coordinates(on_cut).2.abs() < 1e-5);
        }
        let (a, b) = source
            .slice_path(
                &path,
                SliceOptions {
                    mode: SliceMode::Separate,
                    cap: true,
                    flip: false,
                },
                &|| false,
            )
            .unwrap();
        let b = b.unwrap();
        a.validate_closed().unwrap();
        b.validate_closed().unwrap();
        let volume = |m: &Mesh| {
            m.triangles
                .iter()
                .map(|&[a, b, c]| m.vertices[a].dot(m.vertices[b].cross(m.vertices[c])) / 6.)
                .sum::<f32>()
        };
        assert!((volume(&a) + volume(&b) - volume(&source)).abs() < 1e-4);
    }
}
