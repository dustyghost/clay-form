//! Persistent world-space paths. A guide follows its own stroke through known
//! surface correspondence; unrelated geometry edits require explicit reprojection.
use super::{Mesh, Vec3};
use serde::{Deserialize, Serialize};
pub mod closed;
pub mod ring;

pub const MAX_PATHS: usize = 128;
pub const MAX_POINTS: usize = 2048;
pub const MAX_SAMPLES: usize = 16384;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathMode {
    Surface,
    Free,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PathPoint {
    pub position: Vec3,
    pub normal: Vec3,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathSample {
    pub position: Vec3,
    pub normal: Vec3,
    pub tangent: Vec3,
    pub binormal: Vec3,
    pub distance: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfacePath {
    pub id: u64,
    pub name: String,
    pub target: u64,
    pub mode: PathMode,
    pub points: Vec<PathPoint>,
    pub smoothing: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub corner_rounding: Vec<f32>,
    pub closed: bool,
    pub seam: usize,
    pub surface_signature: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_route: Option<AttachedRoute>,
}
/// Surface correspondence retained separately from the editable control polygon.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttachedRoute {
    pub points: Vec<PathPoint>,
    pub shape_signature: u64,
}
fn finite(v: Vec3) -> bool {
    v.0.is_finite() && v.1.is_finite() && v.2.is_finite()
}
impl SurfacePath {
    /// Absolute trim distance on both incident edges; endpoints stay sharp.
    pub fn corner_limit(&self, index: usize) -> f32 {
        let n = self.points.len();
        if n < 3 || index >= n || (!self.closed && (index == 0 || index + 1 == n)) {
            return 0.;
        }
        super::path_geometry::corner_limit(
            self.points[(index + n - 1) % n].position,
            self.points[index].position,
            self.points[(index + 1) % n].position,
        )
    }
    pub fn corner_rounding(&self, index: usize) -> f32 {
        self.corner_rounding
            .get(index)
            .copied()
            .unwrap_or(0.)
            .min(self.corner_limit(index))
    }
    pub fn set_corner_rounding(&mut self, index: usize, amount: f32) -> Result<(), String> {
        if index >= self.points.len() || !amount.is_finite() || amount < 0. {
            return Err(
                "Choose an existing corner and a finite, nonnegative rounding amount".into(),
            );
        }
        if (!self.corner_rounding.is_empty() && self.corner_rounding.len() != self.points.len())
            || self
                .corner_rounding
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1e6).contains(v))
        {
            return Err("Corner rounding must match the path's control points".into());
        }
        let amount = amount.min(self.corner_limit(index));
        // A handle press can report the same effective trim every frame. Keep
        // both the deformed route and the configured amount, even when a shorter
        // incident edge currently clamps that amount below its stored value.
        if self.corner_rounding(index) == amount {
            return Ok(());
        }
        self.corner_rounding.resize(self.points.len(), 0.);
        self.corner_rounding[index] = amount;
        if self.corner_rounding.iter().all(|&v| v == 0.) {
            self.corner_rounding.clear();
        }
        self.attached_route = None;
        Ok(())
    }
    pub fn insert_point(&mut self, index: usize, point: PathPoint) -> Result<(), String> {
        if index > self.points.len()
            || self.points.len() >= MAX_POINTS
            || (!self.corner_rounding.is_empty() && self.corner_rounding.len() != self.points.len())
            || !finite(point.position)
            || !finite(point.normal)
            || point.position.len() > 1e6
            || !(0.5..=1.5).contains(&point.normal.len())
        {
            return Err("Choose a valid insertion point within the path's point limit".into());
        }
        if self.closed && index <= self.seam {
            self.seam += 1;
        }
        self.points.insert(index, point);
        if !self.corner_rounding.is_empty() {
            self.corner_rounding.insert(index, 0.);
        }
        self.attached_route = None;
        Ok(())
    }
    pub fn remove_point(&mut self, index: usize) -> Result<(), String> {
        if index >= self.points.len()
            || (!self.corner_rounding.is_empty() && self.corner_rounding.len() != self.points.len())
        {
            return Err("Choose an existing path point to remove".into());
        }
        self.points.remove(index);
        if !self.corner_rounding.is_empty() {
            self.corner_rounding.remove(index);
        }
        if self.points.len() < 3 {
            self.closed = false;
            self.seam = 0;
        } else if self.closed {
            self.seam = if index < self.seam {
                self.seam - 1
            } else {
                self.seam % self.points.len()
            };
        }
        self.attached_route = None;
        Ok(())
    }
    fn shape_signature(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325_u64;
        let mut add = |v: u64| {
            for byte in v.to_le_bytes() {
                hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
            }
        };
        add(self.mode as u64);
        add(self.smoothing.to_bits() as u64);
        add(self.closed as u64);
        add(self.seam as u64);
        for amount in &self.corner_rounding {
            add(amount.to_bits() as u64);
        }
        for point in &self.points {
            for v in [
                point.position.0,
                point.position.1,
                point.position.2,
                point.normal.0,
                point.normal.1,
                point.normal.2,
            ] {
                add(v.to_bits() as u64);
            }
        }
        hash
    }
    fn attached_points(&self) -> Option<&[PathPoint]> {
        self.attached_route
            .as_ref()
            .filter(|r| r.shape_signature == self.shape_signature())
            .map(|r| r.points.as_slice())
    }
    pub(crate) fn has_attached_route(&self) -> bool {
        self.attached_points().is_some()
    }
    pub fn validate(&self) -> Result<(), String> {
        if (!self.corner_rounding.is_empty() && self.corner_rounding.len() != self.points.len())
            || self
                .corner_rounding
                .iter()
                .any(|v| !v.is_finite() || !(0. ..=1e6).contains(v))
        {
            return Err(
                "Corner rounding must match the points and contain finite, bounded distances"
                    .into(),
            );
        }
        if self.id == 0 || self.target == 0 || self.name.trim().is_empty() || self.name.len() > 128
        {
            return Err("Path requires an identity, target, and name of 1–128 bytes".into());
        }
        if self.points.len() < if self.closed { 3 } else { 2 }
            || self.points.len() > MAX_POINTS
            || self.seam >= self.points.len()
            || (!self.closed && self.seam != 0)
            || !self.smoothing.is_finite()
            || !(0. ..=1.).contains(&self.smoothing)
        {
            return Err("Invalid path point count, seam, or smoothing".into());
        }
        for point in &self.points {
            if !finite(point.position)
                || !finite(point.normal)
                || point.position.len() > 1e6
                || !(0.5..=1.5).contains(&point.normal.len())
            {
                return Err("Path points require finite positions and unit normals".into());
            }
        }
        if let Some(route) = &self.attached_route
            && (!(2..=MAX_SAMPLES).contains(&route.points.len())
                || route.points.iter().any(|point| {
                    !finite(point.position)
                        || !finite(point.normal)
                        || point.position.len() > 1e6
                        || !(0.5..=1.5).contains(&point.normal.len())
                })
                || (self.attached_points().is_some()
                    && self.closed
                    && (route.points.len() < 4 || route.points.first() != route.points.last())))
        {
            return Err("Invalid attached surface route".into());
        }
        for i in 0..self.points.len() - usize::from(!self.closed) {
            if (self.points[i].position - self.points[(i + 1) % self.points.len()].position).len()
                < 1e-6
            {
                return Err("Consecutive path points must be distinct".into());
            }
        }
        Ok(())
    }
    pub fn is_stale(&self, mesh: &Mesh) -> bool {
        self.mode == PathMode::Surface && self.surface_signature != mesh_signature(mesh)
    }
    pub fn dense_curve(&self) -> Vec<PathPoint> {
        if let Some(points) = self.attached_points() {
            return points.to_vec();
        }
        if (0..self.points.len()).any(|i| self.corner_rounding(i) > 0.) {
            return self.rounded_curve();
        }
        if self.closed {
            return closed::dense_curve(self);
        }
        if self.points.len() < 2 {
            return self.points.clone();
        }
        let mut result = Vec::new();
        for i in 0..self.points.len() - 1 {
            let a = self.points[i];
            let b = self.points[i + 1];
            let before = self.points[i.saturating_sub(1)].position;
            let after = self.points[(i + 2).min(self.points.len() - 1)].position;
            for j in 0..16 {
                let t = j as f32 / 16.;
                let t2 = t * t;
                let t3 = t2 * t;
                let curve = a.position * (2. * t3 - 3. * t2 + 1.)
                    + (b.position - before) * ((t3 - 2. * t2 + t) * 0.5)
                    + b.position * (-2. * t3 + 3. * t2)
                    + (after - a.position) * ((t3 - t2) * 0.5);
                let linear = a.position * (1. - t) + b.position * t;
                result.push(PathPoint {
                    position: linear * (1. - self.smoothing) + curve * self.smoothing,
                    normal: (a.normal * (1. - t) + b.normal * t).norm(),
                });
            }
        }
        result.push(*self.points.last().unwrap());
        result
    }
    fn corner_curve(&self, index: usize) -> Vec<PathPoint> {
        let n = self.points.len();
        let amount = self.corner_rounding(index);
        if amount == 0. {
            return vec![self.points[index]];
        }
        let previous = self.points[(index + n - 1) % n];
        let corner = self.points[index];
        let next = self.points[(index + 1) % n];
        let Some(samples) = super::path_geometry::rounded_corner(
            previous.position,
            corner.position,
            next.position,
            amount,
            if self.closed { 24 } else { 16 },
        ) else {
            return vec![corner];
        };
        samples
            .into_iter()
            .map(|sample| PathPoint {
                position: sample.position,
                normal: (previous.normal * sample.weights[0]
                    + corner.normal * sample.weights[1]
                    + next.normal * sample.weights[2])
                    .norm(),
            })
            .collect()
    }
    /// Rounded corners replace global smoothing, leaving every other corner sharp.
    fn rounded_curve(&self) -> Vec<PathPoint> {
        if !self.closed {
            return (0..self.points.len())
                .flat_map(|i| self.corner_curve(i))
                .collect();
        }
        let seam = self.seam.min(self.points.len() - 1);
        let first = self.corner_curve(seam);
        let middle = first.len() / 2;
        let mut result = first[middle..].to_vec();
        for offset in 1..self.points.len() {
            result.extend(self.corner_curve((seam + offset) % self.points.len()));
        }
        result.extend_from_slice(&first[..=middle]);
        result
    }
    pub fn sample(&self, spacing: f32) -> Result<Vec<PathSample>, String> {
        self.validate()?;
        if self.closed {
            return closed::sample(self, spacing);
        }
        sample_open(&self.dense_curve(), spacing)
    }
    pub fn samples_on_mesh(&self, mesh: &Mesh, spacing: f32) -> Result<Vec<PathSample>, String> {
        self.samples_on_mesh_cancelled(mesh, spacing, &|| false)
    }
    pub fn samples_on_mesh_cancelled(
        &self,
        mesh: &Mesh,
        spacing: f32,
        cancel: &impl Fn() -> bool,
    ) -> Result<Vec<PathSample>, String> {
        if cancel() {
            return Err("Path sampling cancelled".into());
        }
        if self.is_stale(mesh) {
            return Err("Path surface changed; reproject the path before using it".into());
        }
        if self.mode == PathMode::Free {
            return self.sample(spacing);
        }
        self.validate()?;
        // Project the dense curve before measuring arc length. This prevents
        // uneven brush spacing caused by projecting already-spaced chord samples.
        let curve = self.dense_curve();
        let mut dense = refine_curve(&curve, spacing, cancel)?;
        let attached = self.attached_points().is_some();
        let max_distance = if attached {
            curve
                .windows(2)
                .map(|p| (p[1].position - p[0].position).len())
                .fold(0_f32, f32::max)
                .max(1e-4)
        } else {
            self.points
                .windows(2)
                .map(|p| (p[1].position - p[0].position).len())
                .chain(self.closed.then(|| {
                    (self.points[0].position - self.points.last().unwrap().position).len()
                }))
                .fold(0_f32, f32::max)
                .max(1e-4)
        };
        let surface = super::projection::Surface::new(mesh, cancel)?;
        let normals = mesh.normals();
        let mut budget = 30_000_000;
        project_on_surface(
            mesh,
            &surface,
            &normals,
            &mut dense,
            max_distance,
            &mut budget,
            cancel,
            attached,
        )?;
        let mut samples = if self.closed {
            closed::sample_curve(&dense, spacing)?
        } else {
            sample_open(&dense, spacing)?
        };
        // Arc interpolation can cross a triangle edge on a sharpened groove.
        // Keep every returned point on the surface so viewport picking and the
        // next pass agree, rather than treating an off-surface chord as hidden.
        // Frame construction may rotate a normal away from a sharp chord, or
        // flip it for continuity. Surface correspondence must instead use the
        // original interpolated mesh normal, including at an incision's rim.
        let mut lengths = vec![0.];
        for pair in dense.windows(2) {
            lengths.push(lengths.last().unwrap() + (pair[1].position - pair[0].position).len());
        }
        let mut edge = 1;
        let mut points = Vec::with_capacity(samples.len());
        for sample in &samples {
            while edge + 1 < lengths.len() && lengths[edge] < sample.distance {
                edge += 1;
            }
            let t = ((sample.distance - lengths[edge - 1])
                / (lengths[edge] - lengths[edge - 1]).max(1e-10))
            .clamp(0., 1.);
            points.push(PathPoint {
                position: sample.position,
                normal: (dense[edge - 1].normal * (1. - t) + dense[edge].normal * t).norm(),
            });
        }
        project_on_surface(
            mesh,
            &surface,
            &normals,
            &mut points,
            max_distance,
            &mut budget,
            cancel,
            attached,
        )?;
        for (sample, point) in samples.iter_mut().zip(points) {
            sample.position = point.position;
            sample.normal = point.normal;
            let tangent = sample.tangent - point.normal * sample.tangent.dot(point.normal);
            sample.tangent = if tangent.len() > 1e-6 {
                tangent.norm()
            } else {
                sample.binormal.cross(point.normal).norm()
            };
            sample.binormal = sample.normal.cross(sample.tangent).norm();
        }
        Ok(samples)
    }

    /// Follow the selected guide's own completed stroke as part of that stroke's
    /// geometry commit. Unchanged topology supplies exact barycentric anchors;
    /// adaptive topology uses only the bounded, ambiguity-checked fallback.
    /// Failure or cancellation leaves this path untouched, hence visibly stale.
    pub fn reattach_after_stroke(
        &mut self,
        before: &Mesh,
        after: &Mesh,
        max_distance: f32,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        self.validate()?;
        if cancel() {
            return Err("Path attachment cancelled".into());
        }
        if !max_distance.is_finite() || max_distance <= 0. {
            return Err("Path attachment distance must be positive".into());
        }
        if self.mode != PathMode::Surface || self.is_stale(before) {
            return Err("The guide was not attached to the stroke's original surface; reproject it explicitly".into());
        }
        if !self.is_stale(after) {
            return Ok(());
        }
        if before.vertices.len() != after.vertices.len() || before.triangles != after.triangles {
            return self.reproject_cancelled(after, max_distance, cancel);
        }
        let source = super::projection::Surface::new(before, cancel)?;
        let normals = after.normals();
        let mut budget = 30_000_000;
        let mut next = self.clone();
        let mut route = if let Some(points) = self.attached_points() {
            points.to_vec()
        } else {
            let length: f32 = self
                .dense_curve()
                .windows(2)
                .map(|p| (p[1].position - p[0].position).len())
                .sum();
            let samples =
                self.samples_on_mesh_cancelled(before, (length / 2047.).max(1e-6), cancel)?;
            let mut points: Vec<_> = samples
                .into_iter()
                .map(|s| PathPoint {
                    position: s.position,
                    normal: s.normal,
                })
                .collect();
            if self.closed {
                points.push(points[0]);
            }
            points
        };
        for point in next.points.iter_mut().chain(route.iter_mut()) {
            if cancel() {
                return Err("Path attachment cancelled".into());
            }
            // Saved controls lie on the source mesh. A tiny tolerance permits
            // serialization/raycast rounding, never a new nearest-sheet choice.
            let Some((face, position, _)) =
                source.nearest(point.position, 0.001, None, &mut budget)?
            else {
                return Err(
                    "The guide lost its original surface anchor; reproject it explicitly".into(),
                );
            };
            if !after.face_visible(face) {
                return Err(
                    "The guide's surface anchor became hidden; reproject it explicitly".into(),
                );
            }
            let [ia, ib, ic] = before.triangles[face];
            let a = before.vertices[ia];
            let b = before.vertices[ib];
            let c = before.vertices[ic];
            let ab = b - a;
            let ac = c - a;
            let ap = position - a;
            let determinant = ab.dot(ab) * ac.dot(ac) - ab.dot(ac) * ab.dot(ac);
            if determinant <= 1e-20 {
                return Err("The guide's original surface anchor collapsed".into());
            }
            let v = (ac.dot(ac) * ap.dot(ab) - ab.dot(ac) * ap.dot(ac)) / determinant;
            let w = (ab.dot(ab) * ap.dot(ac) - ab.dot(ac) * ap.dot(ab)) / determinant;
            let u = 1. - v - w;
            let a = after.vertices[ia];
            let b = after.vertices[ib];
            let c = after.vertices[ic];
            let face_normal = (b - a).cross(c - a).norm();
            if face_normal.len() < 0.5 {
                return Err("The guide's deformed surface anchor collapsed".into());
            }
            let smooth = (normals[ia] * u + normals[ib] * v + normals[ic] * w).norm();
            *point = PathPoint {
                position: a * u + b * v + c * w,
                normal: if smooth.dot(face_normal) > 0.2 || u.min(v).min(w) <= 1e-5 {
                    smooth
                } else {
                    face_normal
                },
            };
        }
        // Every route anchor follows a known original triangle. Retain that
        // complete correspondence instead of rebuilding a sparse polygon over
        // the newly carved groove and guessing its attachment again.
        if self.closed {
            *route.last_mut().unwrap() = route[0];
        }
        next.surface_signature = mesh_signature(after);
        next.attached_route = Some(AttachedRoute {
            points: route,
            shape_signature: next.shape_signature(),
        });
        next.validate()?;
        if cancel() {
            return Err("Path attachment cancelled".into());
        }
        *self = next;
        Ok(())
    }

    pub fn reproject(&mut self, mesh: &Mesh, max_distance: f32) -> Result<(), String> {
        self.reproject_cancelled(mesh, max_distance, &|| false)
    }
    pub fn reproject_cancelled(
        &mut self,
        mesh: &Mesh,
        max_distance: f32,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        self.validate()?;
        let mut points = self.points.clone();
        project_points_cancelled(mesh, &mut points, max_distance, cancel)?;
        let mut next = self.clone();
        next.attached_route = None;
        next.points = points;
        next.mode = PathMode::Surface;
        next.surface_signature = mesh_signature(mesh);
        next.validate()?;
        // A valid attachment includes the route between its control points.
        // Checking only the knots would allow a curve to jump across a gap.
        let length: f32 = next
            .points
            .windows(2)
            .map(|p| (p[1].position - p[0].position).len())
            .sum();
        let samples = next.samples_on_mesh_cancelled(mesh, (length / 2047.).max(1e-6), cancel)?;
        let mut points: Vec<_> = samples
            .into_iter()
            .map(|s| PathPoint {
                position: s.position,
                normal: s.normal,
            })
            .collect();
        if next.closed {
            points.push(points[0]);
        }
        next.attached_route = Some(AttachedRoute {
            points,
            shape_signature: next.shape_signature(),
        });
        next.validate()?;
        if cancel() {
            return Err("Path reprojection cancelled".into());
        }
        *self = next;
        Ok(())
    }
}
/// Resolve the surface at the requested sampling scale before measuring arc
/// length. Sixteen knots per control span alone can bridge a narrow incision.
fn refine_curve(
    points: &[PathPoint],
    spacing: f32,
    cancel: &impl Fn() -> bool,
) -> Result<Vec<PathPoint>, String> {
    if !spacing.is_finite() || spacing < 1e-6 {
        return Err("Path spacing must be positive".into());
    }
    let mut refined = Vec::new();
    for pair in points.windows(2) {
        if cancel() {
            return Err("Path sampling cancelled".into());
        }
        let count = ((pair[1].position - pair[0].position).len() / (spacing * 0.5))
            .ceil()
            .max(1.) as usize;
        if count > MAX_SAMPLES * 4 || refined.len().saturating_add(count) > MAX_SAMPLES * 4 {
            return Err("Path exceeds sample limit; increase spacing".into());
        }
        for index in 0..count {
            let t = index as f32 / count as f32;
            refined.push(PathPoint {
                position: pair[0].position * (1. - t) + pair[1].position * t,
                normal: (pair[0].normal * (1. - t) + pair[1].normal * t).norm(),
            });
        }
    }
    if let Some(&last) = points.last() {
        refined.push(if points.first() == points.last() {
            refined[0]
        } else {
            last
        });
    }
    Ok(refined)
}
fn sample_open(points: &[PathPoint], spacing: f32) -> Result<Vec<PathSample>, String> {
    if !spacing.is_finite() || spacing < 1e-6 {
        return Err("Path spacing must be positive".into());
    }
    let mut lengths = vec![0.];
    for pair in points.windows(2) {
        lengths.push(lengths.last().unwrap() + (pair[1].position - pair[0].position).len());
    }
    let total = *lengths.last().unwrap_or(&0.);
    if !total.is_finite() || total < 1e-6 {
        return Err("Path has no usable length".into());
    }
    let intervals = (total / spacing).ceil().max(1.) as usize;
    if intervals >= MAX_SAMPLES {
        return Err("Path exceeds sample limit; increase spacing".into());
    }
    let mut samples = Vec::with_capacity(intervals + 1);
    let mut edge = 1;
    for i in 0..=intervals {
        let distance = total * i as f32 / intervals as f32;
        while edge + 1 < lengths.len() && lengths[edge] < distance {
            edge += 1;
        }
        let a = points[edge - 1];
        let b = points[edge];
        let t = ((distance - lengths[edge - 1]) / (lengths[edge] - lengths[edge - 1]).max(1e-10))
            .clamp(0., 1.);
        let tangent = (b.position - a.position).norm();
        let desired = (a.normal * (1. - t) + b.normal * t).norm();
        let mut normal = (desired - tangent * desired.dot(tangent)).norm();
        if normal.len() < 0.5 {
            let axis = if tangent.0.abs() < 0.8 {
                Vec3(1., 0., 0.)
            } else {
                Vec3(0., 1., 0.)
            };
            normal = (axis - tangent * axis.dot(tangent)).norm();
        }
        if let Some(previous) = samples.last() {
            let previous: &PathSample = previous;
            if normal.dot(previous.normal) < 0. {
                normal = normal * -1.;
            }
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
/// Deterministic geometry identity; mask-only edits preserve attachments.
pub fn mesh_signature(mesh: &Mesh) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    let mut add = |v: u64| {
        for byte in v.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    add(mesh.vertices.len() as u64);
    add(mesh.triangles.len() as u64);
    for p in &mesh.vertices {
        for v in [p.0, p.1, p.2] {
            add(v.to_bits() as u64);
        }
    }
    for (i, face) in mesh.triangles.iter().enumerate() {
        for &v in face {
            add(v as u64);
        }
        add(mesh.face_visible(i) as u64);
    }
    hash
}
pub fn validate_library(paths: &[SurfacePath]) -> Result<(), String> {
    if paths.len() > MAX_PATHS {
        return Err("Paths are limited to 128 per project".into());
    }
    let mut ids = std::collections::HashSet::new();
    for path in paths {
        path.validate()?;
        if !ids.insert(path.id) {
            return Err("Duplicate path identity".into());
        }
    }
    Ok(())
}
/// Reuses the projection BVH with a shared query budget. Rejects hidden faces,
/// opposing sheets and similarly close disconnected surfaces atomically.
pub fn project_points(mesh: &Mesh, points: &mut [PathPoint], distance: f32) -> Result<(), String> {
    project_points_cancelled(mesh, points, distance, &|| false)
}
pub fn project_points_cancelled(
    mesh: &Mesh,
    points: &mut [PathPoint],
    distance: f32,
    cancel: &impl Fn() -> bool,
) -> Result<(), String> {
    if !distance.is_finite() || distance <= 0. {
        return Err("Reprojection distance must be positive".into());
    }
    let surface = super::projection::Surface::new(mesh, cancel)?;
    let normals = mesh.normals();
    let mut budget = 30_000_000;
    project_on_surface(
        mesh,
        &surface,
        &normals,
        points,
        distance,
        &mut budget,
        cancel,
        false,
    )
}
#[allow(clippy::too_many_arguments)]
fn project_on_surface(
    mesh: &Mesh,
    surface: &super::projection::Surface<'_>,
    normals: &[Vec3],
    points: &mut [PathPoint],
    distance: f32,
    budget: &mut usize,
    cancel: &impl Fn() -> bool,
    attached: bool,
) -> Result<(), String> {
    let mut projected = Vec::with_capacity(points.len());
    let mut component = None;
    for point in points.iter() {
        if cancel() {
            return Err("Path projection cancelled".into());
        }
        // A nearest-point query above an existing incision can prefer its wall
        // over its floor. Project along the guide's surface normal first, keeping
        // repeated passes centered; nearest is only a bounded fallback at edges.
        let hit = if attached {
            surface.nearest(point.position, distance, None, budget)?
        } else {
            surface.along_normal(point.position, point.normal.norm(), distance, budget)?
        };
        let hit = match hit {
            Some(hit) => Some(hit),
            None => surface.nearest(point.position, distance, None, budget)?,
        };
        let Some((face, position, delta)) = hit else {
            return Err(
                "Path could not reach the visible target surface; increase reprojection distance"
                    .into(),
            );
        };
        if component.is_some_and(|previous| previous != surface.components[face]) {
            return Err(
                "A surface path cannot jump between disconnected pieces; use separate paths".into(),
            );
        }
        component = Some(surface.components[face]);
        let [a, b, c] = mesh.triangles[face].map(|v| mesh.vertices[v]);
        let face_normal = (b - a).cross(c - a).norm();
        if surface
            .nearest(
                point.position,
                (delta * 1.25 + 1e-6).min(distance),
                Some(surface.components[face]),
                budget,
            )?
            .is_some()
        {
            return Err("Path projection is ambiguous between disconnected surfaces".into());
        }
        // Interpolate vertex normals so frames on a curved surface do not
        // rotate abruptly whenever a sample crosses a triangle boundary.
        let ab = b - a;
        let ac = c - a;
        let ap = position - a;
        let determinant = ab.dot(ab) * ac.dot(ac) - ab.dot(ac) * ab.dot(ac);
        let (normal, on_edge) = if determinant > 1e-20 {
            let v = (ac.dot(ac) * ap.dot(ab) - ab.dot(ac) * ap.dot(ac)) / determinant;
            let w = (ab.dot(ab) * ap.dot(ac) - ab.dot(ac) * ap.dot(ab)) / determinant;
            let [ia, ib, ic] = mesh.triangles[face];
            let smooth = (normals[ia] * (1. - v - w) + normals[ib] * v + normals[ic] * w).norm();
            let on_edge = v.min(w).min(1. - v - w) <= 1e-5;
            let exact_edge = on_edge && delta <= 1e-6;
            (
                if smooth.dot(face_normal) > 0.2 || (exact_edge && smooth.dot(point.normal) >= 0.15)
                {
                    smooth
                } else {
                    face_normal
                },
                exact_edge,
            )
        } else {
            (face_normal, false)
        };
        // At a sharp shared edge, a zero-distance query can select either
        // incident face. Their interpolated vertex normal is the same, so use
        // that stable orientation for an existing anchor instead of rejecting
        // it because the BVH happened to return the other side of the crease.
        if !attached
            && face_normal.dot(point.normal) < 0.15
            && !(on_edge && normal.dot(point.normal) >= 0.15)
        {
            return Err(
                "Path projection reaches an opposing surface; adjust its control points".into(),
            );
        }
        projected.push(PathPoint { position, normal });
    }
    points.copy_from_slice(&projected);
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_surface_error(mesh: &Mesh, samples: &[PathSample]) -> f32 {
    let surface = super::projection::Surface::new(mesh, &|| false).unwrap();
    let mut budget = 30_000_000;
    samples
        .iter()
        .map(|sample| {
            surface
                .nearest(sample.position, 0.1, None, &mut budget)
                .unwrap()
                .unwrap()
                .2
        })
        .fold(0_f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> SurfacePath {
        SurfacePath {
            id: 1,
            name: "Line".into(),
            target: 1,
            mode: PathMode::Free,
            points: vec![
                PathPoint {
                    position: Vec3(0., 0., 0.),
                    normal: Vec3(0., 0., 1.),
                },
                PathPoint {
                    position: Vec3(1., 0., 0.),
                    normal: Vec3(0., 0., 1.),
                },
            ],
            smoothing: 0.,
            closed: false,
            seam: 0,
            surface_signature: 0,
            attached_route: None,
            corner_rounding: Vec::new(),
        }
    }
    #[test]
    fn sampling_is_uniform_and_bounded() {
        let p = path();
        let samples = p.sample(0.3).unwrap();
        assert_eq!(samples.len(), 5);
        for sample in &samples {
            assert!(sample.tangent.cross(sample.binormal).dot(sample.normal) > 0.999);
        }
        for pair in samples.windows(2) {
            assert!(((pair[1].position - pair[0].position).len() - 0.25).abs() < 1e-5);
        }
        assert_eq!(samples.last().unwrap().position, Vec3(1., 0., 0.));
        assert!(p.sample(1e-8).is_err());
        assert!(p.sample(f32::NAN).is_err());
    }
    #[test]
    fn geometry_signature_ignores_masks_but_detects_deformation_and_topology() {
        let mut mesh = Mesh::sphere();
        let signature = mesh_signature(&mesh);
        mesh.mask[0] = 0.9;
        assert_eq!(signature, mesh_signature(&mesh));
        mesh.vertices[0].0 += 0.01;
        assert_ne!(signature, mesh_signature(&mesh));
    }
    #[test]
    fn invalid_paths_and_duplicate_ids_are_rejected() {
        let p = path();
        assert!(validate_library(&[p.clone(), p.clone()]).is_err());
        let mut bad = p;
        bad.points[1].position = bad.points[0].position;
        assert!(bad.validate().is_err());
    }
    fn plane() -> Mesh {
        Mesh {
            vertices: vec![
                Vec3(-1., -1., 0.),
                Vec3(1., -1., 0.),
                Vec3(1., 1., 0.),
                Vec3(-1., 1., 0.),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            mask: vec![0.; 4],
            ..Mesh::default()
        }
    }
    fn corner_path(closed: bool) -> SurfacePath {
        let mut p = path();
        p.points = [
            Vec3(-0.8, -0.5, 0.),
            Vec3(0., -0.5, 0.),
            Vec3(0., 0.5, 0.),
            Vec3(0.8, 0.5, 0.),
        ]
        .into_iter()
        .map(|position| PathPoint {
            position,
            normal: Vec3(0., 0., 1.),
        })
        .collect();
        p.closed = closed;
        p
    }
    #[test]
    fn individual_rounding_keeps_other_corners_sharp_and_overrides_global_smoothing() {
        let mut p = corner_path(false);
        p.smoothing = 0.8;
        p.set_corner_rounding(1, 0.2).unwrap();
        assert_eq!(p.corner_limit(0), 0.);
        assert_eq!(p.corner_limit(3), 0.);
        let curve = p.dense_curve();
        assert_eq!(curve.first(), p.points.first());
        assert_eq!(curve.last(), p.points.last());
        assert!(curve.contains(&p.points[2]));
        assert!(!curve.contains(&p.points[1]));
        assert!((curve[9].position - Vec3(-0.05, -0.45, 0.)).len() < 1e-6);
        p.smoothing = 0.;
        assert_eq!(p.dense_curve(), curve);
        p.reproject(&plane(), 0.01).unwrap();
        let samples = p.samples_on_mesh(&plane(), 0.01).unwrap();
        assert!(sample_surface_error(&plane(), &samples) < 1e-5);
        assert!(
            samples
                .iter()
                .all(|s| (s.position - p.points[1].position).len() > 0.06)
        );
        assert!(p.has_attached_route());
        p.set_corner_rounding(1, 0.3).unwrap();
        assert!(!p.has_attached_route());
    }
    #[test]
    fn unchanged_rounding_retains_exact_attachment_after_deformation_clamps_its_limit() {
        let before = plane();
        let mut p = corner_path(false);
        p.set_corner_rounding(1, 0.2).unwrap();
        p.reproject(&before, 0.01).unwrap();
        let original = p.clone();
        p.set_corner_rounding(1, 0.2).unwrap();
        assert_eq!(p, original);
        let mut after = before.clone();
        for point in &mut after.vertices {
            *point = *point * 0.2;
        }
        p.reattach_after_stroke(&before, &after, 0.1, &|| false)
            .unwrap();
        assert!(p.has_attached_route());
        let amount = p.corner_rounding(1);
        assert!(amount < p.corner_rounding[1]);
        let deformed = p.clone();
        p.set_corner_rounding(1, amount).unwrap();
        assert_eq!(p, deformed);
        p.set_corner_rounding(1, 100.).unwrap();
        assert_eq!(
            p, deformed,
            "clamping an unchanged trim must retain its configured amount"
        );
        p.set_corner_rounding(1, amount * 0.5).unwrap();
        assert!(!p.has_attached_route());
        assert_eq!(p.corner_rounding(1), amount * 0.5);
    }
    #[test]
    fn rounded_closed_seam_reverse_open_and_point_edits_preserve_corner_metadata() {
        let mut p = corner_path(true);
        p.set_corner_rounding(0, 0.1).unwrap();
        p.set_corner_rounding(1, 0.2).unwrap();
        p.set_corner_rounding(2, 0.15).unwrap();
        p.reproject(&plane(), 0.01).unwrap();
        p.set_seam(1).unwrap();
        assert!(p.has_attached_route());
        let route = p.dense_curve();
        assert_eq!(route.first(), route.last());
        assert!((route[0].position - p.points[1].position).len() > 0.06);
        let original = p.clone();
        p.reverse();
        assert_eq!(p.corner_rounding, vec![0., 0.15, 0.2, 0.1]);
        assert_eq!(p.dense_curve()[0], route[0]);
        p.reverse();
        assert_eq!(p, original);
        p.set_closed(false).unwrap();
        assert_eq!(p.corner_rounding, vec![0.2, 0.15, 0., 0.1]);
        assert_eq!(p.corner_rounding(0), 0.);
        p.insert_point(
            1,
            PathPoint {
                position: Vec3(0.1, 0., 0.),
                normal: Vec3(0., 0., 1.),
            },
        )
        .unwrap();
        assert_eq!(p.corner_rounding, vec![0.2, 0., 0.15, 0., 0.1]);
        p.remove_point(1).unwrap();
        assert_eq!(p.corner_rounding, vec![0.2, 0.15, 0., 0.1]);
        assert!(p.set_corner_rounding(1, f32::NAN).is_err());
        p.corner_rounding.pop();
        assert!(p.validate().is_err());
    }

    #[test]
    fn attached_route_roundtrips_and_invalidates_every_shape_edit() {
        let mesh = plane();
        let mut p = path();
        p.points = vec![
            PathPoint {
                position: Vec3(-0.5, -0.5, 0.),
                normal: Vec3(0., 0., 1.),
            },
            PathPoint {
                position: Vec3(0.5, -0.5, 0.),
                normal: Vec3(0., 0., 1.),
            },
            PathPoint {
                position: Vec3(0., 0.5, 0.),
                normal: Vec3(0., 0., 1.),
            },
        ];
        p.closed = true;
        p.reproject(&mesh, 0.01).unwrap();
        assert!(p.attached_points().is_some());
        let mut json = serde_json::to_value(&p).unwrap();
        assert_eq!(
            serde_json::from_value::<SurfacePath>(json.clone()).unwrap(),
            p
        );
        json.as_object_mut().unwrap().remove("attached_route");
        let older: SurfacePath = serde_json::from_value(json).unwrap();
        assert!(older.attached_route.is_none());
        assert!(older.samples_on_mesh(&mesh, 0.1).is_ok());
        let mut renamed = p.clone();
        renamed.name = "Renamed".into();
        assert!(renamed.attached_points().is_some());
        let mut reversed = p.clone();
        reversed.reverse();
        let expected: Vec<_> = p.attached_points().unwrap().iter().copied().rev().collect();
        assert_eq!(reversed.attached_points().unwrap(), expected);
        reversed.reverse();
        assert_eq!(reversed, p);
        let mut reseamed = p.clone();
        reseamed.set_seam(1).unwrap();
        let route = reseamed.attached_points().unwrap();
        assert_eq!(route.len(), p.attached_points().unwrap().len());
        assert_eq!(route.first(), Some(&p.points[1]));
        assert_eq!(route.first(), route.last());
        assert!(reseamed.samples_on_mesh(&mesh, 0.1).is_ok());
        for change in 0..5 {
            let mut edited = p.clone();
            match change {
                0 => edited.points[0].position.0 += 0.1,
                1 => edited.smoothing = 0.5,
                2 => edited.closed = false,
                3 => edited.seam = 1,
                _ => edited.points.reverse(),
            }
            assert!(
                edited.attached_points().is_none(),
                "edit {change} reused stale correspondence"
            );
        }
        let mut invalid = p.clone();
        invalid.attached_route.as_mut().unwrap().points[0]
            .position
            .0 = f32::NAN;
        assert!(invalid.validate().is_err());
        invalid = p;
        let point = invalid.points[0];
        invalid.attached_route.as_mut().unwrap().points = vec![point; MAX_SAMPLES + 1];
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn attached_route_fine_sampling_keeps_the_original_chord_correction_bound() {
        let before = plane();
        let mut p = path();
        p.points[0].position = Vec3(-0.7, -0.2, 0.);
        p.points[1].position = Vec3(0.8, 0.3, 0.);
        p.reproject(&before, 0.01).unwrap();
        let mut after = before.clone();
        after.vertices[1].2 = 1.5;
        p.reattach_after_stroke(&before, &after, 0.01, &|| false)
            .unwrap();
        let samples = p.samples_on_mesh(&after, 0.0003).unwrap();
        assert!(sample_surface_error(&after, &samples) < 1e-5);
    }

    #[test]
    fn projection_is_atomic_bounded_and_rejects_disconnected_or_opposing_surfaces() {
        let mesh = plane();
        let mut points = vec![PathPoint {
            position: Vec3(0., 0., 0.1),
            normal: Vec3(0., 0., 1.),
        }];
        let before = points.clone();
        assert!(project_points(&mesh, &mut points, 0.01).is_err());
        assert_eq!(points, before);
        project_points(&mesh, &mut points, 0.2).unwrap();
        assert_eq!(points[0].position.2, 0.);
        points[0].normal = Vec3(0., 0., -1.);
        assert!(project_points(&mesh, &mut points, 0.2).is_err());
        let mut disjoint = mesh.clone();
        disjoint
            .vertices
            .extend(mesh.vertices.iter().map(|p| *p + Vec3(4., 0., 0.)));
        disjoint.mask.extend([0.; 4]);
        disjoint.triangles.extend([[4, 5, 6], [4, 6, 7]]);
        let mut separated = vec![
            PathPoint {
                position: Vec3(0., 0., 0.),
                normal: Vec3(0., 0., 1.),
            },
            PathPoint {
                position: Vec3(4., 0., 0.),
                normal: Vec3(0., 0., 1.),
            },
        ];
        let before = separated.clone();
        assert!(project_points(&disjoint, &mut separated, 0.1).is_err());
        assert_eq!(separated, before);
        let mut cancelled = before.clone();
        assert!(project_points_cancelled(&disjoint, &mut cancelled, 0.1, &|| true).is_err());
        assert_eq!(cancelled, before);
    }
    #[test]
    fn exact_edge_projection_rejects_opposing_sheet_normals() {
        let mut mesh = plane();
        for triangle in &mut mesh.triangles {
            triangle.swap(1, 2);
        }
        for position in [Vec3(0., 0., 0.), mesh.vertices[0]] {
            let mut points = [PathPoint {
                position,
                normal: Vec3(0., 0., 1.),
            }];
            let before = points;
            let error = project_points(&mesh, &mut points, 0.01).unwrap_err();
            assert!(error.contains("opposing surface"));
            assert_eq!(points, before);
        }
    }

    #[test]
    fn surface_geometry_changes_require_explicit_reprojection() {
        let mut mesh = plane();
        let mut p = path();
        p.reproject(&mesh, 0.01).unwrap();
        assert_eq!(p.mode, PathMode::Surface);
        let before = p.clone();
        for v in &mut mesh.vertices {
            v.2 += 0.1;
        }
        assert!(p.is_stale(&mesh));
        assert!(p.samples_on_mesh(&mesh, 0.1).is_err());
        assert!(p.reproject(&mesh, 0.01).is_err());
        assert_eq!(p, before);
        p.reproject(&mesh, 0.2).unwrap();
        assert!(!p.is_stale(&mesh));
        for sample in p.samples_on_mesh(&mesh, 0.1).unwrap() {
            assert!((sample.position.2 - 0.1).abs() < 1e-6);
        }
    }
    #[test]
    fn own_stroke_follows_barycentric_anchors_without_nearest_surface_drift() {
        let before = plane();
        let mut guide = path();
        guide.reproject(&before, 0.01).unwrap();
        let original = guide.clone();
        let mut after = before.clone();
        for p in &mut after.vertices {
            p.2 = 0.4 + p.0 * 0.2;
            p.0 += 2.;
        }
        // Known correspondences remain safe even when movement exceeds the
        // adaptive-topology fallback distance or the old world-space route.
        guide
            .reattach_after_stroke(&before, &after, 0.01, &|| false)
            .unwrap();
        assert!((guide.points[0].position - Vec3(2., 0., 0.4)).len() < 1e-5);
        assert!((guide.points[1].position - Vec3(3., 0., 0.6)).len() < 1e-5);
        assert!(!guide.is_stale(&after));
        assert!(guide.samples_on_mesh(&after, 0.1).is_ok());
        assert!(
            original.is_stale(&after),
            "unrelated edits still require explicit reprojection"
        );
        let once = guide.clone();
        let mut second = after.clone();
        for p in &mut second.vertices {
            p.2 += 0.1;
        }
        guide
            .reattach_after_stroke(&after, &second, 0.01, &|| false)
            .unwrap();
        assert!((guide.points[0].position.2 - once.points[0].position.2 - 0.1).abs() < 1e-5);
        assert!(!guide.is_stale(&second));
    }
    #[test]
    fn own_stroke_adaptive_fallback_is_bounded_and_failure_is_atomic() {
        let before = plane();
        let mut guide = path();
        guide.reproject(&before, 0.01).unwrap();
        let original = guide.clone();
        let mut after = before.subdivide().unwrap();
        for p in &mut after.vertices {
            p.2 += 0.1;
        }
        assert!(
            guide
                .reattach_after_stroke(&before, &after, 0.01, &|| false)
                .is_err()
        );
        assert_eq!(guide, original);
        assert!(guide.is_stale(&after));
        assert!(
            guide
                .reattach_after_stroke(&before, &after, 0.2, &|| true)
                .is_err()
        );
        assert_eq!(guide, original);
        guide
            .reattach_after_stroke(&before, &after, 0.2, &|| false)
            .unwrap();
        assert!(!guide.is_stale(&after));
        assert!(
            guide
                .points
                .iter()
                .all(|p| (p.position.2 - 0.1).abs() < 1e-5)
        );
    }

    #[test]
    fn spaced_samples_stay_on_the_surface_between_sparse_controls() {
        let mut mesh = Mesh::default();
        for y in [-0.2, 0., 0.2] {
            for i in 0..=40 {
                let x = i as f32 / 40.;
                mesh.vertices
                    .push(Vec3(x, y, -0.12 * (-((x - 0.43) / 0.06).powi(2)).exp()));
                mesh.mask.push(0.);
            }
        }
        for y in 0..2 {
            for x in 0..40 {
                let a = y * 41 + x;
                mesh.triangles
                    .extend([[a, a + 1, a + 42], [a, a + 42, a + 41]]);
            }
        }
        let mut guide = path();
        guide.reproject(&mesh, 0.2).unwrap();
        let surface = super::super::projection::Surface::new(&mesh, &|| false).unwrap();
        let mut budget = 30_000_000;
        for sample in guide.samples_on_mesh(&mesh, 0.002).unwrap() {
            let (_, _, distance) = surface
                .nearest(sample.position, 0.2, None, &mut budget)
                .unwrap()
                .unwrap();
            assert!(
                distance < 1e-5,
                "Interpolated guide floats off its surface by {distance}"
            );
        }
    }
}
