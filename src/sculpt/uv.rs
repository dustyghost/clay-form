//! Seam-aware face-corner UVs and least-squares conformal parameterization.
//! Lév​y et al., SIGGRAPH 2002, https://brunolevy.github.io/papers/LSCM_SIGGRAPH_2002.pdf
use super::{Mesh, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

mod editing;

pub type Point = [f32; 2];
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UvMap {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub textures: Vec<super::texture::Texture>,
    pub corners: Vec<[Point; 3]>,
    /// Retained topology detects stale UVs after edits, even at equal face counts.
    pub topology: Vec<[usize; 3]>,
    pub seams: Vec<[usize; 2]>,
    /// Empty arrays preserve compatibility with projects saved before UV editing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub masks: Vec<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<bool>,
}
pub fn edge(a: usize, b: usize) -> [usize; 2] {
    if a < b { [a, b] } else { [b, a] }
}
pub fn area(t: [Point; 3]) -> f32 {
    (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0])
}
pub fn polygon_triangles(mesh: &Mesh) -> Vec<Vec<usize>> {
    let key = |mut t: [usize; 3]| {
        t.sort_unstable();
        t
    };
    let lookup: HashMap<_, _> = mesh
        .triangles
        .iter()
        .enumerate()
        .map(|(i, &t)| (key(t), i))
        .collect();
    mesh.polygons()
        .into_iter()
        .map(|p| {
            let v = p.vertices;
            let candidates = if v.len() == 3 {
                vec![[v[0], v[1], v[2]]]
            } else {
                vec![
                    [v[0], v[1], v[2]],
                    [v[0], v[2], v[3]],
                    [v[0], v[1], v[3]],
                    [v[1], v[2], v[3]],
                ]
            };
            candidates
                .into_iter()
                .filter_map(|t| lookup.get(&key(t)).copied())
                .collect()
        })
        .collect()
}
#[derive(Default)]
pub struct Report {
    pub degenerate: usize,
    pub overlaps: usize,
    pub outside: usize,
    pub complete: bool,
    pub stretch: Vec<f32>,
}
impl UvMap {
    pub fn inspect(&self, mesh: &Mesh) -> Report {
        let mut report = Report {
            complete: true,
            ..Default::default()
        };
        if !self.current(mesh) {
            return report;
        }
        let mut bins = HashMap::<(i32, i32), Vec<usize>>::new();
        let mut pairs = std::collections::HashSet::new();
        for (f, t) in self.corners.iter().enumerate() {
            if !self.mapped(f) {
                report.stretch.push(1.);
                continue;
            }
            if area(*t).abs() < 1e-12 {
                report.degenerate += 1;
            }
            if t.iter().flatten().any(|v| !(0. ..=1.).contains(v)) {
                report.outside += 1;
            }
            let [a, b, c] = mesh.triangles[f].map(|i| mesh.vertices[i]);
            let e = b - a;
            let d = c - a;
            let l = e.len();
            let x = d.dot(e) / l;
            let y = e.cross(d).len() / l;
            let u = [t[1][0] - t[0][0], t[1][1] - t[0][1]];
            let v = [t[2][0] - t[0][0], t[2][1] - t[0][1]];
            let j = [
                u[0] / l,
                (v[0] - u[0] * x / l) / y,
                u[1] / l,
                (v[1] - u[1] * x / l) / y,
            ];
            let sum = j.iter().map(|v| v * v).sum::<f32>();
            let det = j[0] * j[3] - j[1] * j[2];
            let disc = (sum * sum - 4. * det * det).max(0.).sqrt();
            report
                .stretch
                .push(((sum + disc) / (sum - disc).max(1e-20)).sqrt().max(1.));
            if !report.complete {
                continue;
            }
            let lo: [i32; 2] = std::array::from_fn(|k| {
                (t.iter().map(|p| p[k]).fold(f32::INFINITY, f32::min) * 64.)
                    .floor()
                    .clamp(-1., 64.) as i32
            });
            let hi: [i32; 2] = std::array::from_fn(|k| {
                (t.iter().map(|p| p[k]).fold(f32::NEG_INFINITY, f32::max) * 64.)
                    .floor()
                    .clamp(-1., 64.) as i32
            });
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let bin = bins.entry((x, y)).or_default();
                    for &g in bin.iter() {
                        if pairs.len() > 1_000_000 {
                            report.complete = false;
                            break;
                        }
                        if pairs.insert((g, f)) && triangles_overlap(*t, self.corners[g]) {
                            report.overlaps += 1;
                        }
                    }
                    bin.push(f);
                }
            }
        }
        report
    }
}
fn triangles_overlap(a: [Point; 3], b: [Point; 3]) -> bool {
    // Strict separating-axis test: shared edges/points are not overlapping interiors.
    for t in [a, b] {
        for c in 0..3 {
            let d = (c + 1) % 3;
            let axis = [t[d][1] - t[c][1], t[c][0] - t[d][0]];
            let project = |p: Point| p[0] * axis[0] + p[1] * axis[1];
            let interval = |q: [Point; 3]| {
                let values = q.map(project);
                [
                    values.into_iter().fold(f32::INFINITY, f32::min),
                    values.into_iter().fold(f32::NEG_INFINITY, f32::max),
                ]
            };
            let p = interval(a);
            let q = interval(b);
            let epsilon = 1e-7 * (axis[0] * axis[0] + axis[1] * axis[1]).sqrt();
            if p[1].min(q[1]) - p[0].max(q[0]) <= epsilon {
                return false;
            }
        }
    }
    true
}
impl UvMap {
    pub fn mapped(&self, face: usize) -> bool {
        face < self.corners.len() && !self.removed.get(face).copied().unwrap_or(false)
    }
    pub fn mask(&self, face: usize, corner: usize) -> f32 {
        self.masks.get(face).map_or(0., |m| m[corner])
    }
    pub fn has_mask(&self) -> bool {
        self.masks
            .iter()
            .enumerate()
            .any(|(f, m)| self.mapped(f) && m.iter().any(|&v| v > 0.))
    }
    pub fn remove_islands(&mut self, faces: &[usize]) -> Result<(), String> {
        if faces.is_empty() || faces.iter().any(|&f| !self.mapped(f)) {
            return Err("Select mapped UV islands to remove".into());
        }
        if faces.iter().any(|&f| (0..3).any(|c| self.mask(f, c) > 0.)) {
            return Err("Clear the UV mask on these islands before removing their mapping".into());
        }
        self.removed.resize(self.corners.len(), false);
        for &f in faces {
            self.removed[f] = true;
        }
        Ok(())
    }
    /// Apply a soft brush to UV corners. A hard center pins points completely.
    pub fn paint_mask(&mut self, point: Point, radius: f32, erase: bool) {
        if !radius.is_finite() || radius <= 0. || point.iter().any(|v| !v.is_finite()) {
            return;
        }
        self.masks.resize(self.corners.len(), [0.; 3]);
        for f in 0..self.corners.len() {
            if !self.mapped(f) {
                continue;
            }
            for c in 0..3 {
                let q = self.corners[f][c];
                let distance = ((q[0] - point[0]).powi(2) + (q[1] - point[1]).powi(2)).sqrt();
                let weight = ((1. - distance / radius) * 2.).clamp(0., 1.);
                self.masks[f][c] = if erase {
                    (self.masks[f][c] - weight).max(0.)
                } else {
                    self.masks[f][c].max(weight)
                };
            }
        }
        if self.masks.iter().flatten().all(|&v| v == 0.) {
            self.masks.clear();
        }
    }
    pub fn is_empty(&self) -> bool {
        self.corners.is_empty() && self.seams.is_empty() && self.textures.is_empty()
    }
    pub fn current(&self, mesh: &Mesh) -> bool {
        !self.corners.is_empty()
            && self.corners.len() == mesh.triangles.len()
            && self.topology == mesh.triangles
    }
    pub fn validate(&self) -> Result<(), String> {
        if (!self.masks.is_empty() && self.masks.len() != self.corners.len())
            || (!self.removed.is_empty() && self.removed.len() != self.corners.len())
            || self
                .masks
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || self.corners.len() != self.topology.len()
            || self.corners.len() > super::MAX_TRIANGLES
            || self
                .corners
                .iter()
                .flatten()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e6)
            || self.seams.len() > super::MAX_TRIANGLES * 3
        {
            return Err("Invalid UV data".into());
        }
        if self.textures.len() > 3 {
            return Err("A model supports one texture per channel".into());
        }
        let mut channels = std::collections::BTreeSet::new();
        for texture in &self.textures {
            texture.validate()?;
            if !channels.insert(texture.channel.index()) {
                return Err("Duplicate texture channel".into());
            }
        }
        Ok(())
    }
    pub fn retained_bytes(&self) -> usize {
        self.masks.len() * 12
            + self.removed.len()
            + self.corners.len() * 24
            + self.topology.len() * 24
            + self.seams.len() * 16
            + self
                .textures
                .iter()
                .map(|t| {
                    t.name.len() + t.profile.len() + t.exchange.as_ref().map_or(0, |e| e.path.len())
                })
                .sum::<usize>()
    }
    pub fn islands(&self) -> Vec<Vec<usize>> {
        let mut parent: Vec<_> = (0..self.topology.len()).collect();
        let mut edges = HashMap::new();
        for (f, t) in self.topology.iter().enumerate() {
            if !self.mapped(f) {
                continue;
            }
            for c in 0..3 {
                let d = (c + 1) % 3;
                let key = edge(t[c], t[d]);
                let q = if t[c] < t[d] {
                    [self.corners[f][c], self.corners[f][d]]
                } else {
                    [self.corners[f][d], self.corners[f][c]]
                };
                if let Some(&(g, r)) = edges.get(&key) {
                    if q == r {
                        join(&mut parent, f, g);
                    }
                } else {
                    edges.insert(key, (f, q));
                }
            }
        }
        let mut groups = BTreeMap::<usize, Vec<usize>>::new();
        for f in 0..parent.len() {
            if !self.mapped(f) {
                continue;
            }
            groups.entry(root(&mut parent, f)).or_default().push(f);
        }
        groups.into_values().collect()
    }
    pub fn transform(
        &mut self,
        faces: &[usize],
        offset: Point,
        scale: f32,
        radians: f32,
    ) -> Result<(), String> {
        if faces.is_empty()
            || faces.iter().any(|&f| !self.mapped(f))
            || !scale.is_finite()
            || scale <= 0.
            || !radians.is_finite()
            || offset.iter().any(|v| !v.is_finite())
        {
            return Err("Select UV islands and use a positive, finite scale".into());
        }
        if offset == [0.; 2] && scale == 1. && radians == 0. {
            return Ok(());
        }
        let mut center = [0.; 2];
        for &f in faces {
            for q in self.corners[f] {
                for k in 0..2 {
                    center[k] += q[k] / (faces.len() * 3) as f32;
                }
            }
        }
        let (s, c) = radians.sin_cos();
        let mut next = self.clone();
        for &f in faces {
            for (i, q) in next.corners[f].iter_mut().enumerate() {
                let x = (q[0] - center[0]) * scale;
                let y = (q[1] - center[1]) * scale;
                let target = [
                    center[0] + c * x - s * y + offset[0],
                    center[1] + s * x + c * y + offset[1],
                ];
                let influence = 1. - self.mask(f, i);
                for k in 0..2 {
                    q[k] += (target[k] - q[k]) * influence;
                }
            }
        }
        next.validate()?;
        *self = next;
        Ok(())
    }
    /// Uniform scale across islands preserves their relative texel density.
    pub fn pack(&mut self, padding: f32) -> Result<(), String> {
        if !padding.is_finite() || !(0. ..=0.1).contains(&padding) {
            return Err("UV padding must be 0–10%".into());
        }
        let islands = self.islands();
        if islands.is_empty() {
            return Err("Generate UVs before packing".into());
        }
        let mut rects: Vec<_> = islands
            .iter()
            .enumerate()
            .map(|(id, fs)| {
                let mut lo = [f32::INFINITY; 2];
                let mut hi = [f32::NEG_INFINITY; 2];
                for &f in fs {
                    for q in self.corners[f] {
                        for k in 0..2 {
                            lo[k] = lo[k].min(q[k]);
                            hi[k] = hi[k].max(q[k]);
                        }
                    }
                }
                (
                    id,
                    lo,
                    [(hi[0] - lo[0]).max(1e-8), (hi[1] - lo[1]).max(1e-8)],
                )
            })
            .collect();
        rects.sort_by(|a, b| b.2[1].total_cmp(&a.2[1]).then(a.0.cmp(&b.0)));
        let fixed: BTreeSet<_> = islands
            .iter()
            .enumerate()
            .filter(|(_, fs)| fs.iter().any(|&f| (0..3).any(|c| self.mask(f, c) > 0.)))
            .map(|(id, _)| id)
            .collect();
        if !fixed.is_empty() {
            return self.pack_around_masks(padding, &islands, &rects, &fixed);
        }
        let layout = |scale: f32| -> Option<Vec<Point>> {
            let (mut x, mut y, mut row) = (padding, padding, 0_f32);
            let mut origins = vec![[0.; 2]; islands.len()];
            for &(id, _, size) in &rects {
                let w = size[0] * scale;
                let h = size[1] * scale;
                if x + w + padding > 1. {
                    x = padding;
                    y += row + 2. * padding;
                    row = 0.;
                }
                if x + w + padding > 1. || y + h + padding > 1. {
                    return None;
                }
                origins[id] = [x, y];
                x += w + 2. * padding;
                row = row.max(h);
            }
            Some(origins)
        };
        if layout(0.).is_none() {
            return Err("Too many islands for this padding; reduce padding".into());
        }
        let mut low = 0.;
        let mut high = 1.
            / rects
                .iter()
                .map(|r| r.2[0].max(r.2[1]))
                .fold(1e-8, f32::max);
        for _ in 0..40 {
            let mid = (low + high) * 0.5;
            if layout(mid).is_some() {
                low = mid;
            } else {
                high = mid;
            }
        }
        if low < 1e-12 {
            return Err("UV packing collapsed; reduce padding".into());
        }
        let origins = layout(low).unwrap();
        for (id, lo, _) in rects {
            for &f in &islands[id] {
                for q in &mut self.corners[f] {
                    for k in 0..2 {
                        q[k] = (q[k] - lo[k]) * low + origins[id][k];
                    }
                }
            }
        }
        Ok(())
    }
}
fn root(p: &mut [usize], mut i: usize) -> usize {
    while p[i] != i {
        p[i] = p[p[i]];
        i = p[i];
    }
    i
}
fn join(p: &mut [usize], a: usize, b: usize) {
    let a = root(p, a);
    let b = root(p, b);
    p[a] = b;
}
fn check(cancel: &impl Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("UV unwrap cancelled".into())
    } else {
        Ok(())
    }
}

impl Mesh {
    /// Automatic cuts split by dominant normal; marked seams are always respected.
    pub fn unwrap_uv(
        &self,
        automatic: bool,
        padding: f32,
        cancel: &impl Fn() -> bool,
    ) -> Result<UvMap, String> {
        self.validate()?;
        check(cancel)?;
        if self.triangles.is_empty() {
            return Err("The model has no faces to unwrap".into());
        }
        if self.uv.current(self) && self.uv.has_mask() {
            return Err(
                "Clear the UV mask before unwrapping; masked points must stay in place".into(),
            );
        }
        let seams: BTreeSet<_> = if self.uv.corners.is_empty() || self.uv.current(self) {
            self.uv.seams.iter().copied().collect()
        } else {
            BTreeSet::new()
        };
        let n = self.triangles.len();
        let mut parent: Vec<_> = (0..n * 3).collect();
        let mut face_parent: Vec<_> = (0..n).collect();
        let mut edges = BTreeMap::<[usize; 2], Vec<(usize, usize)>>::new();
        let mut regions = Vec::with_capacity(n);
        for (f, t) in self.triangles.iter().enumerate() {
            if f % 1024 == 0 {
                check(cancel)?;
            }
            let [a, b, c] = t.map(|i| self.vertices[i]);
            let normal = (b - a).cross(c - a);
            if normal.len() < 1e-12 {
                return Err(format!(
                    "Face {} is degenerate; repair it before unwrapping",
                    f + 1
                ));
            }
            let v = [normal.0, normal.1, normal.2];
            let axis = (0..3)
                .max_by(|&a, &b| v[a].abs().total_cmp(&v[b].abs()))
                .unwrap();
            regions.push(axis * 2 + usize::from(v[axis] < 0.));
            for c in 0..3 {
                edges
                    .entry(edge(t[c], t[(c + 1) % 3]))
                    .or_default()
                    .push((f, c));
            }
        }
        let mut cuts = seams.clone();
        for (e, adj) in &edges {
            if let [(a, c), (b, d)] = adj.as_slice() {
                if seams.contains(e) || (automatic && regions[*a] != regions[*b]) {
                    cuts.insert(*e);
                    continue;
                }
                let ta = self.triangles[*a];
                let tb = self.triangles[*b];
                if ta[*c] == tb[*d] {
                    return Err(
                        "Inconsistent face winding; repair normals before unwrapping".into(),
                    );
                }
                join(&mut parent, a * 3 + c, b * 3 + (d + 1) % 3);
                join(&mut parent, a * 3 + (c + 1) % 3, b * 3 + d);
                join(&mut face_parent, *a, *b);
            } else if adj.len() > 2 {
                return Err("Non-manifold edge; split the surface before unwrapping".into());
            }
        }
        let mut charts = BTreeMap::<usize, Vec<usize>>::new();
        for f in 0..n {
            charts.entry(root(&mut face_parent, f)).or_default().push(f);
        }
        let mut output = UvMap {
            textures: self.uv.textures.clone(),
            corners: vec![[[0.; 2]; 3]; n],
            topology: self.triangles.clone(),
            seams: cuts.into_iter().collect(),
            ..Default::default()
        };
        for faces in charts.values() {
            check(cancel)?;
            let mut ids = HashMap::new();
            let mut points = Vec::new();
            let mut triangles = Vec::new();
            for &f in faces {
                let t = std::array::from_fn(|c| {
                    let r = root(&mut parent, f * 3 + c);
                    *ids.entry(r).or_insert_with(|| {
                        let i = points.len();
                        points.push(self.vertices[self.triangles[f][c]]);
                        i
                    })
                });
                triangles.push(t);
            }
            let coords = lscm(&points, &triangles, cancel)?;
            for (&f, t) in faces.iter().zip(triangles) {
                output.corners[f] = t.map(|i| coords[i]);
            }
        }
        output.pack(padding)?;
        output.validate()?;
        Ok(output)
    }
}

/// Matrix-free normal equations with Jacobi-preconditioned conjugate gradients.
/// Each triangle contributes the two area-weighted Cauchy–Riemann rows.
fn lscm(
    points: &[Vec3],
    triangles: &[[usize; 3]],
    cancel: &impl Fn() -> bool,
) -> Result<Vec<Point>, String> {
    let mut counts = BTreeMap::<[usize; 2], usize>::new();
    for t in triangles {
        for c in 0..3 {
            *counts.entry(edge(t[c], t[(c + 1) % 3])).or_default() += 1;
        }
    }
    let boundary: BTreeSet<_> = counts
        .iter()
        .filter(|(_, n)| **n == 1)
        .flat_map(|(e, _)| *e)
        .collect();
    if boundary.len() < 3 {
        return Err("This UV chart is closed. Mark a seam or use Automatic unwrap".into());
    }
    let farthest = |a: usize| {
        *boundary
            .iter()
            .max_by(|&&b, &&c| {
                (points[b] - points[a])
                    .len()
                    .total_cmp(&(points[c] - points[a]).len())
            })
            .unwrap()
    };
    let a = farthest(*boundary.first().unwrap());
    let b = farthest(a);
    let length = (points[b] - points[a]).len() as f64;
    if length < 1e-10 {
        return Err("UV chart has no usable boundary".into());
    }
    let len = points.len() * 2;
    let pinned = |i: usize| i / 2 == a || i / 2 == b;
    let mut rows = Vec::<[(usize, f64); 6]>::with_capacity(triangles.len() * 2);
    for &[a, b, c] in triangles {
        let e = points[b] - points[a];
        let d = points[c] - points[a];
        let l = e.len() as f64;
        let x = (d.dot(e) / e.len()) as f64;
        let y = e.cross(d).len() as f64 / l;
        if y < 1e-12 || l < 1e-12 {
            return Err("Degenerate chart triangle".into());
        }
        let weight = (l * y).sqrt();
        let gx = [-y, y, 0.];
        let gy = [x - l, -x, l];
        let t = [a, b, c];
        rows.push(std::array::from_fn(|j| {
            let k = j / 2;
            (
                2 * t[k] + j % 2,
                if j % 2 == 0 {
                    gx[k] / weight
                } else {
                    -gy[k] / weight
                },
            )
        }));
        rows.push(std::array::from_fn(|j| {
            let k = j / 2;
            (
                2 * t[k] + j % 2,
                if j % 2 == 0 {
                    gy[k] / weight
                } else {
                    gx[k] / weight
                },
            )
        }));
    }
    let mut fixed = vec![0.; len];
    fixed[2 * b] = length;
    let mut rhs = vec![0.; len];
    let mut diag = vec![0.; len];
    for row in &rows {
        let value: f64 = row.iter().map(|&(i, w)| w * fixed[i]).sum();
        for &(i, w) in row {
            if !pinned(i) {
                rhs[i] -= w * value;
                diag[i] += w * w;
            }
        }
    }
    let multiply = |v: &[f64], out: &mut [f64]| {
        out.fill(0.);
        for row in &rows {
            let sum: f64 = row
                .iter()
                .filter(|&&(i, _)| !pinned(i))
                .map(|&(i, w)| w * v[i])
                .sum();
            for &(i, w) in row {
                if !pinned(i) {
                    out[i] += w * sum;
                }
            }
        }
    };
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let mut solution = vec![0.; len];
    let mut r = rhs.clone();
    let mut z: Vec<_> = r.iter().zip(&diag).map(|(r, d)| r / d.max(1e-30)).collect();
    let mut p = z.clone();
    let mut rz = dot(&r, &z);
    let initial = dot(&rhs, &rhs).max(1e-30);
    let mut ap = vec![0.; len];
    let mut converged = dot(&r, &r) <= initial * 1e-16;
    for iteration in 0..4000 {
        if converged {
            break;
        }
        if iteration % 16 == 0 {
            check(cancel)?;
        }
        multiply(&p, &mut ap);
        let denominator = dot(&p, &ap);
        if denominator <= 1e-30 {
            break;
        }
        let alpha = rz / denominator;
        for i in 0..len {
            solution[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
            z[i] = r[i] / diag[i].max(1e-30);
        }
        converged = dot(&r, &r) <= initial * 1e-16;
        let next = dot(&r, &z);
        let beta = next / rz.max(1e-300);
        for i in 0..len {
            p[i] = z[i] + beta * p[i];
        }
        rz = next;
    }
    if !converged {
        return Err("UV solver did not converge. Add seams to make smaller charts".into());
    }
    let result: Vec<_> = (0..points.len())
        .map(|i| {
            [
                (solution[2 * i] + fixed[2 * i]) as f32,
                (solution[2 * i + 1] + fixed[2 * i + 1]) as f32,
            ]
        })
        .collect();
    if triangles
        .iter()
        .any(|t| area(t.map(|i| result[i])) <= 1e-12)
    {
        return Err(
            "UV unwrap folded or collapsed a chart. Add seams or use Automatic unwrap".into(),
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cube_atlas_keeps_welded_geometry_and_seams_roundtrip() {
        let mesh = super::super::Primitive::for_kind(super::super::PrimitiveKind::Cube)
            .mesh()
            .unwrap();
        let uv = mesh.unwrap_uv(true, 0.01, &|| false).unwrap();
        assert_eq!(uv.islands().len(), 6);
        assert!(uv.corners.iter().all(|&t| area(t) > 0.));
        assert!(
            uv.corners
                .iter()
                .flatten()
                .flatten()
                .all(|&v| (0.009..=0.991).contains(&v))
        );
        assert!(uv.current(&mesh));
        let json = serde_json::to_string(&uv).unwrap();
        assert_eq!(serde_json::from_str::<UvMap>(&json).unwrap(), uv);
        let mut changed = mesh.clone();
        changed.triangles[0].swap(0, 1);
        assert!(!uv.current(&changed));
        assert!(mesh.unwrap_uv(false, 0.01, &|| false).is_err());
        assert!(mesh.unwrap_uv(true, 0.01, &|| true).is_err());
    }
    #[test]
    fn marked_cylinder_seam_and_caps_make_three_continuous_charts() {
        let n = 24;
        let mut mesh = Mesh::default();
        for z in [0., 2.] {
            for i in 0..n {
                let a = i as f32 * std::f32::consts::TAU / n as f32;
                mesh.vertices.push(Vec3(a.cos(), a.sin(), z));
            }
        }
        mesh.vertices.extend([Vec3(0., 0., 0.), Vec3(0., 0., 2.)]);
        mesh.mask = vec![0.; mesh.vertices.len()];
        for a in 0..n {
            let b = (a + 1) % n;
            mesh.triangles.extend([
                [a, b, b + n],
                [a, b + n, a + n],
                [n * 2, b, a],
                [n * 2 + 1, a + n, b + n],
            ]);
            mesh.uv.seams.extend([edge(a, b), edge(a + n, b + n)]);
        }
        mesh.uv.seams.push(edge(0, n));
        let uv = mesh.unwrap_uv(false, 0.01, &|| false).unwrap();
        assert_eq!(uv.islands().len(), 3);
        assert!(uv.corners.iter().all(|t| area(*t) > 1e-10));
    }
    #[test]
    fn sphere_unwrap_has_finite_positive_charts() {
        let mesh = Mesh::sphere();
        let uv = mesh.unwrap_uv(true, 0.01, &|| false).unwrap();
        assert!(uv.corners.iter().all(|t| area(*t) > 0.));
    }
}
