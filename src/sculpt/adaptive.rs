//! Local, conforming edge splits and conservative manifold edge collapses.
use super::{
    Alpha, Brush, Dab, Kind, Mesh, Vec3,
    math::closest_triangle,
    subdivision::{MAX_TRIANGLES, MAX_VERTICES},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

// Face index followed by its directed edge and opposite vertex.
type OrientedFaceEdge = (usize, usize, usize, usize);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdaptiveSettings {
    pub enabled: bool,
    pub detail: f32,
    pub relative: bool,
    pub refine: bool,
    pub simplify: bool,
    pub collapse_ratio: f32,
}
impl Default for AdaptiveSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            detail: 8.,
            relative: false,
            refine: true,
            simplify: true,
            collapse_ratio: 0.4,
        }
    }
}
impl AdaptiveSettings {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.detail.is_finite()
            || !(1. ..=32.).contains(&self.detail)
            || !self.collapse_ratio.is_finite()
            || !(0.1..=0.8).contains(&self.collapse_ratio)
        {
            return Err("Invalid adaptive sculpt settings".into());
        }
        Ok(())
    }
    pub fn edge_length(&self, brush: &Brush, dab: Dab) -> f32 {
        if self.relative {
            dab.radius * self.detail / 100.
        } else {
            let pixels = brush.radius
                * if brush.pressure_radius {
                    dab.pressure.powf(brush.pressure_curve).max(0.05)
                } else {
                    1.
                };
            dab.radius * self.detail / pixels
        }
    }
}
/// Bound topology work per rendered input frame, shared by all interpolated dabs.
pub(crate) struct Budget {
    pub splits: usize,
    pub collapses: usize,
    passes: usize,
    flips: usize,
    flip_passes: usize,
    relax: bool,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            splits: 256,
            collapses: 32,
            passes: 8,
            flips: 128,
            flip_passes: 2,
            relax: true,
        }
    }
}

impl Mesh {
    /// A placed stamp has no later dabs to finish refinement. Resolve its footprint
    /// on the undeformed surface so new vertices sample the alpha, rather than
    /// merely subdividing an already jagged displacement afterwards.
    pub(crate) fn prepare_drag_stamp(
        &mut self,
        brush: &Brush,
        dabs: &[Dab],
        settings: &AdaptiveSettings,
    ) -> bool {
        if !settings.enabled
            || !settings.refine
            || !matches!(
                brush.effective_stroke_mode(),
                super::StrokeMode::DragRect | super::StrokeMode::DragDot
            )
            || matches!(brush.kind, Kind::Mask | Kind::Smooth)
            || brush.strength <= 0.
            || dabs.is_empty()
        {
            return false;
        }
        let target = settings.edge_length(brush, dabs[0]);
        if !target.is_finite() || target <= 1e-7 {
            return false;
        }
        let mut remaining = 8192;
        let mut changed = false;
        for _ in 0..32 {
            let count = self.split_local(brush, dabs, target, remaining.min(512));
            changed |= count > 0;
            remaining -= count;
            if count == 0 || remaining == 0 {
                break;
            }
        }
        changed
    }
    pub(crate) fn adapt(
        &mut self,
        brush: &Brush,
        dabs: &[Dab],
        settings: &AdaptiveSettings,
        budget: &mut Budget,
        collapse: bool,
    ) -> bool {
        if !settings.enabled
            || brush.kind == Kind::Mask
            || brush.strength <= 0.
            || dabs.is_empty()
            || matches!(&brush.alpha,Alpha::Image { pixels,.. } if pixels.iter().all(|&p|p==0.))
        {
            return false;
        }
        let target = settings.edge_length(brush, dabs[0]);
        if !target.is_finite() || target <= 1e-7 {
            return false;
        }
        let cleanup = brush.kind == Kind::Smooth && settings.simplify;
        let mut changed = false;
        if collapse && settings.simplify && budget.collapses > 0 {
            let count = self.collapse_local(
                brush,
                dabs,
                target
                    * if cleanup {
                        (settings.collapse_ratio * 2.).min(0.95)
                    } else {
                        settings.collapse_ratio
                    },
                budget.collapses.min(32),
            );
            budget.collapses = 0;
            changed |= count > 0;
        }
        if settings.refine && !cleanup && budget.splits > 0 {
            for _ in 0..4 {
                if budget.passes == 0 {
                    break;
                }
                budget.passes -= 1;
                let count = self.split_local(brush, dabs, target, budget.splits.min(64));
                budget.splits -= count;
                changed |= count > 0;
                if count == 0 || budget.splits == 0 {
                    break;
                }
            }
        }
        if settings.refine && budget.flips > 0 && budget.flip_passes > 0 {
            budget.flip_passes -= 1;
            let count = self.improve_edges(brush, dabs, budget.flips.min(64));
            budget.flips -= count;
            changed |= count > 0;
        }
        if brush.kind == Kind::Snake && !collapse && settings.refine && budget.relax {
            budget.relax = false;
            changed |= self.relax_tip(brush, dabs, target);
        }
        changed
    }
    // Redistribute vertices along the surface without Laplacian shrinkage.
    // Splits and flips alone cannot undo the compression behind a moving tip.
    fn relax_tip(&mut self, brush: &Brush, dabs: &[Dab], target: f32) -> bool {
        let mut changed = false;
        let sculptable = self.sculptable_vertices();
        for _ in 0..2 {
            let normals = self.normals();
            let mut sums = vec![Vec3::default(); self.vertices.len()];
            let mut counts = vec![0.; self.vertices.len()];
            let mut active = vec![false; self.vertices.len()];
            for (fi, &t @ [a, b, c]) in self.triangles.iter().enumerate() {
                if self.in_patch(fi, t, brush, dabs) {
                    for i in t {
                        active[i] = true;
                    }
                }
                for (i, j, k) in [(a, b, c), (b, c, a), (c, a, b)] {
                    sums[i] = sums[i] + self.vertices[j] + self.vertices[k];
                    counts[i] += 2.;
                }
            }
            let mut moves = vec![Vec3::default(); self.vertices.len()];
            for i in 0..moves.len() {
                if !sculptable[i]
                    || !active[i]
                    || self.mask[i] >= 0.999
                    || counts[i] == 0.
                    || dabs
                        .iter()
                        .any(|dab| (self.vertices[i] - dab.center).len() < dab.radius * 0.35)
                {
                    continue;
                }
                let offset = sums[i] * (1. / counts[i]) - self.vertices[i];
                let tangent = offset - normals[i] * offset.dot(normals[i]);
                let influence = dabs
                    .iter()
                    .map(|dab| brush.snake_weight(*dab, self.vertices[i] - dab.center))
                    .fold(0f32, f32::max);
                moves[i] = tangent * (0.5 * influence * (1. - self.mask[i]));
                let length = moves[i].len();
                if length > target * 0.5 {
                    moves[i] = moves[i] * (target * 0.5 / length);
                }
            }
            // Simultaneous proposals must not invert or flatten any incident face.
            for pass in 0..5 {
                let mut unsafe_vertices = HashSet::new();
                for &t in &self.triangles {
                    if t.iter().all(|&i| moves[i].len() == 0.) {
                        continue;
                    }
                    let [a, b, c] = t.map(|i| self.vertices[i]);
                    let [x, y, z] = t.map(|i| self.vertices[i] + moves[i]);
                    let old = (b - a).cross(c - a);
                    let new = (y - x).cross(z - x);
                    if new.len() < old.len() * 0.3 || new.norm().dot(old.norm()) < 0.8 {
                        unsafe_vertices.extend(t);
                    }
                }
                if unsafe_vertices.is_empty() {
                    break;
                }
                if pass == 4 {
                    moves.fill(Vec3::default());
                    break;
                }
                for i in unsafe_vertices {
                    moves[i] = Vec3::default();
                }
            }
            for (position, delta) in self.vertices.iter_mut().zip(moves) {
                changed |= delta.len() > 0.;
                *position = *position + delta;
            }
        }
        changed
    }
    // Reconnect diagonals instead of repeatedly subdividing skinny triangles.
    // No vertex positions or mask values change, and distant faces stay intact.
    fn improve_edges(&mut self, brush: &Brush, dabs: &[Dab], limit: usize) -> usize {
        let mut edges: BTreeMap<(usize, usize), Vec<OrientedFaceEdge>> = BTreeMap::new();
        let mut existing = HashSet::new();
        for (i, &[a, b, c]) in self.triangles.iter().enumerate() {
            let local = self.in_patch(i, [a, b, c], brush, dabs);
            for (a, b, c) in [(a, b, c), (b, c, a), (c, a, b)] {
                existing.insert(edge(a, b));
                if local {
                    edges.entry(edge(a, b)).or_default().push((i, a, b, c));
                }
            }
        }
        let quality = |t: [usize; 3]| {
            let [a, b, c] = t.map(|i| self.vertices[i]);
            let sum = (b - a).dot(b - a) + (c - b).dot(c - b) + (a - c).dot(a - c);
            2. * 3f32.sqrt() * (b - a).cross(c - a).len() / sum.max(1e-20)
        };
        let normal = |t: [usize; 3]| {
            let [a, b, c] = t.map(|i| self.vertices[i]);
            (b - a).cross(c - a).norm()
        };
        let mut used = HashSet::new();
        let mut replacements = Vec::new();
        for pair in edges.values() {
            let [(i, a, b, c), (j, x, y, d)] = pair.as_slice() else {
                continue;
            };
            if !self.polygroups.is_empty() && self.polygroups[*i] != self.polygroups[*j] {
                continue;
            }
            if replacements.len() >= limit {
                break;
            }
            if a != y
                || b != x
                || c == d
                || used.contains(i)
                || used.contains(j)
                || existing.contains(&edge(*c, *d))
                || [*a, *b, *c, *d]
                    .iter()
                    .any(|&v| self.mask[v] > 0.95 || (self.mask[v] - self.mask[*a]).abs() > 0.05)
            {
                continue;
            }
            let old = [self.triangles[*i], self.triangles[*j]];
            let new = [[*c, *d, *b], [*d, *c, *a]];
            let n = normal(old[0]);
            if n.dot(normal(old[1])) < 0.85
                || new.iter().any(|&t| normal(t).dot(n) < 0.85)
                || quality(new[0]).min(quality(new[1]))
                    <= quality(old[0]).min(quality(old[1])) + 0.03
            {
                continue;
            }
            // Block the whole quadrilateral so simultaneous flips cannot
            // introduce a duplicate diagonal or conflicting connectivity.
            if [*a, *b, *c, *d]
                .iter()
                .any(|v| used.contains(&(self.triangles.len() + v)))
            {
                continue;
            }
            used.extend([*i, *j]);
            used.extend([*a, *b, *c, *d].map(|v| self.triangles.len() + v));
            replacements.push((*i, *j, new));
        }
        let count = replacements.len();
        if count > 0 {
            self.quads.clear();
        }
        for (i, j, [a, b]) in replacements {
            self.triangles[i] = a;
            self.triangles[j] = b;
        }
        count
    }
    fn split_local(&mut self, brush: &Brush, dabs: &[Dab], target: f32, limit: usize) -> usize {
        if limit == 0 {
            return 0;
        }
        let mut candidates = BTreeMap::new();
        for (fi, &t) in self.triangles.iter().enumerate() {
            if !self.in_patch(fi, t, brush, dabs) {
                continue;
            }
            let [a, b, c] = t;
            for (x, y) in [(a, b), (b, c), (c, a)] {
                let length =
                    (self.vertices[x] - self.vertices[y]).dot(self.vertices[x] - self.vertices[y]);
                // Spend extra resolution where a placed alpha changes sharply.
                // The midpoint also catches a narrow feature between two endpoints.
                let stamp = matches!(
                    brush.effective_stroke_mode(),
                    super::StrokeMode::DragRect | super::StrokeMode::DragDot
                );
                let boundary = stamp
                    && dabs.iter().any(|dab| {
                        let weights = [
                            self.vertices[x],
                            self.vertices[y],
                            (self.vertices[x] + self.vertices[y]) * 0.5,
                        ]
                        .map(|p| {
                            let d = p - dab.center;
                            brush.weight(d.dot(dab.right) / dab.radius, d.dot(dab.up) / dab.radius)
                        });
                        let low = weights.into_iter().fold(f32::INFINITY, f32::min);
                        let high = weights.into_iter().fold(0_f32, f32::max);
                        high - low > 0.15
                    });
                let edge_target = if boundary { target * 0.5 } else { target };
                if length > edge_target * edge_target {
                    candidates.insert(edge(x, y), length / (edge_target * edge_target));
                }
            }
        }
        let mut candidates: Vec<_> = candidates.into_iter().collect();
        candidates.sort_by(|(a, x), (b, y)| y.total_cmp(x).then(a.cmp(b)));
        // Each manifold edge adds one vertex and two faces. Leave headroom for
        // malformed/nonmanifold inputs as well; adaptive mode validates closure.
        let capacity = limit
            .min(MAX_VERTICES.saturating_sub(self.vertices.len()))
            .min(MAX_TRIANGLES.saturating_sub(self.triangles.len()) / 2);
        candidates.truncate(capacity);
        if candidates.is_empty() {
            return 0;
        }
        candidates.sort_by_key(|(edge, _)| *edge);
        let mut mids = HashMap::new();
        for &((a, b), _) in &candidates {
            let index = self.vertices.len();
            self.vertices
                .push((self.vertices[a] + self.vertices[b]) * 0.5);
            self.mask.push((self.mask[a] + self.mask[b]) * 0.5);
            for g in &mut self.vertex_groups {
                g.weights.push((g.weights[a] + g.weights[b]) * 0.5);
            }
            mids.insert((a, b), index);
        }
        let mut faces = Vec::with_capacity(self.triangles.len() + candidates.len() * 2);
        let mut new_groups = Vec::new();
        let mut new_hidden = Vec::new();
        for (fi, &[a, b, c]) in self.triangles.iter().enumerate() {
            let start = faces.len();
            let m = [
                mids.get(&edge(a, b)).copied(),
                mids.get(&edge(b, c)).copied(),
                mids.get(&edge(c, a)).copied(),
            ];
            match m.iter().flatten().count() {
                0 => faces.push([a, b, c]),
                1 => {
                    for i in 0..3 {
                        if let Some(mid) = m[i] {
                            let v = [a, b, c];
                            let (x, y, z) = (v[i], v[(i + 1) % 3], v[(i + 2) % 3]);
                            faces.extend([[x, mid, z], [mid, y, z]]);
                        }
                    }
                }
                2 => {
                    let i = (0..3)
                        .find(|&i| m[i].is_some() && m[(i + 1) % 3].is_some())
                        .unwrap();
                    let v = [a, b, c];
                    let (x, y, z) = (v[i], v[(i + 1) % 3], v[(i + 2) % 3]);
                    let p = m[i].unwrap();
                    let q = m[(i + 1) % 3].unwrap();
                    faces.push([y, q, p]);
                    if (self.vertices[x] - self.vertices[q]).len()
                        < (self.vertices[p] - self.vertices[z]).len()
                    {
                        faces.extend([[x, p, q], [x, q, z]]);
                    } else {
                        faces.extend([[x, p, z], [p, q, z]]);
                    }
                }
                _ => {
                    let [p, q, r] = m.map(Option::unwrap);
                    faces.extend([[a, p, r], [p, b, q], [r, q, c], [p, q, r]]);
                }
            }
            if !self.hidden_faces.is_empty() {
                new_hidden.resize(faces.len(), self.hidden_faces[fi]);
            }
            if !self.polygroups.is_empty() {
                new_groups.resize(faces.len(), self.polygroups[fi]);
                debug_assert!(faces.len() > start);
            }
        }
        self.polygroups = new_groups;
        self.hidden_faces = new_hidden;
        self.quads.clear();
        self.triangles = faces;
        candidates.len()
    }
    fn collapse_local(
        &mut self,
        brush: &Brush,
        dabs: &[Dab],
        threshold: f32,
        limit: usize,
    ) -> usize {
        let mut candidates = BTreeMap::new();
        for (fi, &t) in self.triangles.iter().enumerate() {
            let [a, b, c] = t;
            if self.in_patch(fi, t, brush, dabs) {
                for (a, b) in [(a, b), (b, c), (c, a)] {
                    let length = (self.vertices[a] - self.vertices[b]).len();
                    if length < threshold {
                        candidates.insert(edge(a, b), length);
                    }
                }
            }
        }
        if candidates.is_empty() {
            return 0;
        }
        let mut neighbors = vec![Vec::new(); self.vertices.len()];
        let mut incident = vec![Vec::new(); self.vertices.len()];
        for (index, &[a, b, c]) in self.triangles.iter().enumerate() {
            for (i, j, k) in [(a, b, c), (b, c, a), (c, a, b)] {
                neighbors[i].extend([j, k]);
                incident[i].push(index);
            }
        }
        for ns in &mut neighbors {
            ns.sort_unstable();
            ns.dedup();
        }
        let mut candidates: Vec<_> = candidates.into_iter().collect();
        candidates.sort_by(|(a, x), (b, y)| x.total_cmp(y).then(a.cmp(b)));
        let mut blocked = HashSet::new();
        let mut merged = HashMap::new();
        for ((a, b), _) in candidates {
            if merged.len() >= limit {
                break;
            }
            if brush.kind == Kind::Snake
                && dabs.iter().any(|dab| {
                    (self.vertices[a] - dab.center).len() < dab.radius * 0.35
                        || (self.vertices[b] - dab.center).len() < dab.radius * 0.35
                })
            {
                continue;
            }
            if blocked.contains(&a)
                || blocked.contains(&b)
                || self.mask[a] > 0.95
                || self.mask[b] > 0.95
                || (self.mask[a] - self.mask[b]).abs() > 0.05
            {
                continue;
            }
            if incident[a]
                .iter()
                .chain(&incident[b])
                .any(|&i| !self.face_visible(i))
            {
                continue;
            }
            let shared: Vec<_> = incident[a]
                .iter()
                .copied()
                .filter(|&i| self.triangles[i].contains(&b))
                .collect();
            if !self.polygroups.is_empty() {
                let a_groups: std::collections::HashSet<_> =
                    incident[a].iter().map(|&i| self.polygroups[i]).collect();
                let b_groups: std::collections::HashSet<_> =
                    incident[b].iter().map(|&i| self.polygroups[i]).collect();
                if a_groups != b_groups {
                    continue;
                }
            }
            if shared.len() != 2 {
                continue;
            }
            let opposite: Vec<_> = shared
                .iter()
                .filter_map(|&i| {
                    self.triangles[i]
                        .iter()
                        .copied()
                        .find(|&v| v != a && v != b)
                })
                .collect();
            let common: Vec<_> = neighbors[a]
                .iter()
                .copied()
                .filter(|n| neighbors[b].binary_search(n).is_ok())
                .collect();
            if common.len() != 2
                || opposite.len() != 2
                || opposite[0] == opposite[1]
                || !common.iter().all(|v| opposite.contains(v))
            {
                continue;
            }
            let normal = |t: [usize; 3]| {
                let [x, y, z] = t.map(|i| self.vertices[i]);
                (y - x).cross(z - x).norm()
            };
            if normal(self.triangles[shared[0]]).dot(normal(self.triangles[shared[1]])) < 0.85 {
                continue;
            }
            let position = (self.vertices[a] + self.vertices[b]) * 0.5;
            let mut local_keys = HashSet::new();
            let mut safe = true;
            let mut affected = incident[a].clone();
            affected.extend(&incident[b]);
            affected.sort_unstable();
            affected.dedup();
            for index in affected {
                let old = self.triangles[index];
                if old.contains(&a) && old.contains(&b) {
                    continue;
                }
                let new = old.map(|i| if i == b { a } else { i });
                let mut key = new;
                key.sort_unstable();
                if !local_keys.insert(key) {
                    safe = false;
                    break;
                }
                let [x, y, z] = new.map(|i| if i == a { position } else { self.vertices[i] });
                let cross = (y - x).cross(z - x);
                if brush.kind == Kind::Smooth {
                    let quality = |x: Vec3, y: Vec3, z: Vec3| {
                        let sum = (y - x).dot(y - x) + (z - y).dot(z - y) + (x - z).dot(x - z);
                        2. * 3f32.sqrt() * (y - x).cross(z - x).len() / sum.max(1e-20)
                    };
                    let [u, v, w] = old.map(|i| self.vertices[i]);
                    let minimum = (quality(u, v, w) * 0.8).clamp(0.2, 0.35);
                    if quality(x, y, z) < minimum {
                        safe = false;
                        break;
                    }
                }
                if cross.len() < threshold * threshold * 1e-5 || cross.norm().dot(normal(old)) < 0.8
                {
                    safe = false;
                    break;
                }
            }
            if !safe {
                continue;
            }
            blocked.extend(neighbors[a].iter().copied());
            blocked.extend(neighbors[b].iter().copied());
            blocked.extend([a, b]);
            merged.insert(b, a);
        }
        if merged.is_empty() {
            return 0;
        }
        for (&b, &a) in &merged {
            self.vertices[a] = (self.vertices[a] + self.vertices[b]) * 0.5;
            self.mask[a] = (self.mask[a] + self.mask[b]) * 0.5;
            for g in &mut self.vertex_groups {
                g.weights[a] = (g.weights[a] + g.weights[b]) * 0.5;
            }
        }
        let mut indices = vec![0; self.vertices.len()];
        let mut vertices = Vec::with_capacity(self.vertices.len() - merged.len());
        let mut mask = Vec::with_capacity(vertices.capacity());
        for (i, &v) in self.vertices.iter().enumerate() {
            if !merged.contains_key(&i) {
                indices[i] = vertices.len();
                vertices.push(v);
                mask.push(self.mask[i]);
            }
        }
        for (&b, &a) in &merged {
            indices[b] = indices[a];
        }
        let old_faces: Vec<_> = self
            .triangles
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                let [a, b, c] = t.map(|i| indices[i]);
                a != b && b != c && a != c
            })
            .map(|(i, _)| i)
            .collect();
        let triangles = old_faces
            .iter()
            .map(|&i| self.triangles[i].map(|v| indices[v]))
            .collect();
        if !self.polygroups.is_empty() {
            self.polygroups = old_faces.iter().map(|&i| self.polygroups[i]).collect();
        }
        if !self.hidden_faces.is_empty() {
            self.hidden_faces = old_faces.iter().map(|&i| self.hidden_faces[i]).collect();
        }
        for g in &mut self.vertex_groups {
            g.weights = g
                .weights
                .iter()
                .enumerate()
                .filter(|(i, _)| !merged.contains_key(i))
                .map(|(_, w)| *w)
                .collect();
        }
        self.vertices = vertices;
        self.mask = mask;
        self.quads.clear();
        self.triangles = triangles;
        merged.len()
    }
    fn in_patch(&self, face: usize, t: [usize; 3], brush: &Brush, dabs: &[Dab]) -> bool {
        if !self.face_visible(face) || t.iter().all(|&i| self.mask[i] >= 0.999) {
            return false;
        }
        let [a, b, c] = t.map(|i| self.vertices[i]);
        let normal = (b - a).cross(c - a).norm();
        dabs.iter().any(|dab| {
            if brush.pressure && dab.pressure <= 0. {
                return false;
            }
            if brush.kind != Kind::Snake && normal.dot(dab.normal) < 0.1 {
                return false;
            }
            let p = closest_triangle(dab.center, a, b, c);
            let offset = p - dab.center;
            // Drag stamps use the same depth reach as the displacement sampler;
            // on curved surfaces the projected alpha edge can lie beyond radius.
            let reach = if matches!(
                brush.effective_stroke_mode(),
                super::StrokeMode::DragRect | super::StrokeMode::DragDot
            ) {
                1.5
            } else {
                1.
            };
            if offset.len() > dab.radius * reach {
                return false;
            }
            [p, a, b, c, (a + b + c) * (1. / 3.)].into_iter().any(|p| {
                let d = p - dab.center;
                brush.weight(d.dot(dab.right) / dab.radius, d.dot(dab.up) / dab.radius) > 0.001
            })
        })
    }
}
fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dab() -> Dab {
        Dab {
            center: Vec3(0., 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 0.35,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        }
    }
    #[test]
    fn drag_stamp_refines_alpha_boundary_before_displacement_and_preserves_protection() {
        for alpha in [
            Alpha::Hard,
            Alpha::Square,
            Alpha::Image {
                width: 2,
                height: 2,
                pixels: vec![0., 1., 0., 1.].into(),
            },
        ] {
            let mut mesh = Mesh::sphere();
            let original = mesh.clone();
            let mut brush = Brush::new(Kind::Standard);
            brush.stroke_mode = super::super::StrokeMode::DragRect;
            brush.radius = 180.;
            brush.alpha = alpha;
            let settings = AdaptiveSettings {
                enabled: true,
                detail: 8.,
                simplify: false,
                ..Default::default()
            };
            let d = Dab {
                radius: 0.7,
                ..dab()
            };
            assert!(mesh.prepare_drag_stamp(&brush, &[d], &settings));
            let added = mesh.vertices.len() - original.vertices.len();
            assert!(
                added > 256,
                "a large stamp needs more than one freehand frame: {added}"
            );
            assert!(added <= 8192);
            assert_eq!(
                &mesh.vertices[..original.vertices.len()],
                &original.vertices
            );
            mesh.validate_closed().unwrap();
            assert!(
                !mesh.prepare_drag_stamp(&brush, &[d], &settings),
                "this footprint should converge in one stamp budget"
            );
            let target = settings.edge_length(&brush, d);
            let mut boundary_edges = 0;
            for (fi, &t) in mesh.triangles.iter().enumerate() {
                if !mesh.in_patch(fi, t, &brush, &[d]) {
                    continue;
                }
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    let weights = [
                        mesh.vertices[a],
                        mesh.vertices[b],
                        (mesh.vertices[a] + mesh.vertices[b]) * 0.5,
                    ]
                    .map(|p| {
                        let v = p - d.center;
                        brush.weight(v.dot(d.right) / d.radius, v.dot(d.up) / d.radius)
                    });
                    if weights.into_iter().fold(0_f32, f32::max)
                        - weights.into_iter().fold(f32::INFINITY, f32::min)
                        > 0.15
                    {
                        boundary_edges += 1;
                        assert!(
                            (mesh.vertices[a] - mesh.vertices[b]).len() <= target * 0.501,
                            "alpha boundary must receive finer edges before deformation"
                        );
                    }
                }
            }
            if !matches!(brush.alpha, Alpha::Image { .. }) {
                assert!(boundary_edges > 0);
            }
            let mut protected = original.clone();
            protected.mask.fill(1.);
            let before = protected.clone();
            assert!(!protected.prepare_drag_stamp(&brush, &[d], &settings));
            assert_eq!(protected, before);
            brush.alpha = Alpha::Image {
                width: 1,
                height: 1,
                pixels: vec![0.].into(),
            };
            let mut black = original.clone();
            assert!(!black.prepare_drag_stamp(&brush, &[d], &settings));
            assert_eq!(black, original);
        }
    }

    #[test]
    fn local_refinement_is_conforming_preserves_masks_and_obeys_work_budget() {
        let mut mesh = Mesh::sphere();
        mesh.mask.iter_mut().for_each(|m| *m = 0.4);
        let original = mesh.clone();
        let settings = AdaptiveSettings {
            enabled: true,
            detail: 2.,
            simplify: false,
            ..Default::default()
        };
        let mut budget = Budget {
            splits: 23,
            collapses: 0,
            // Isolate the split budget; quality flips have their own budget.
            flips: 0,
            ..Default::default()
        };
        assert!(mesh.adapt(
            &Brush::new(Kind::Standard),
            &[dab()],
            &settings,
            &mut budget,
            false
        ));
        assert_eq!(mesh.vertices.len(), original.vertices.len() + 23);
        assert_eq!(mesh.triangles.len(), original.triangles.len() + 46);
        assert_eq!(
            &mesh.vertices[..original.vertices.len()],
            &original.vertices
        );
        assert!(
            mesh.vertices[original.vertices.len()..]
                .iter()
                .all(|v| v.2 > 0.7)
        );
        assert!(mesh.mask.iter().all(|m| (*m - 0.4).abs() < 1e-6));
        mesh.validate_closed().unwrap();
        assert!(!mesh.adapt(
            &Brush::new(Kind::Standard),
            &[dab()],
            &settings,
            &mut budget,
            false
        ));
    }
    #[test]
    fn dense_patch_simplification_preserves_closed_manifold_and_distant_surface() {
        let mut mesh = Mesh::sphere();
        let before = mesh.clone();
        let settings = AdaptiveSettings {
            enabled: true,
            detail: 32.,
            refine: false,
            collapse_ratio: 0.8,
            ..Default::default()
        };
        let mut budget = Budget::default();
        assert!(mesh.adapt(
            &Brush::new(Kind::Smooth),
            &[dab()],
            &settings,
            &mut budget,
            true
        ));
        assert!(mesh.vertices.len() < before.vertices.len());
        mesh.validate_closed().unwrap();
        for v in before.vertices.iter().filter(|v| v.2 < 0.5) {
            assert!(mesh.vertices.contains(v));
        }
        assert!(
            mesh.vertices
                .iter()
                .all(|v| v.len() > 0.98 && v.len() < 1.001)
        );
    }
    #[test]
    fn smooth_cleanup_never_refines_and_simplify_off_restores_refinement() {
        let original = Mesh::sphere();
        let mut mesh = original.clone();
        let mut settings = AdaptiveSettings {
            enabled: true,
            detail: 2.,
            ..Default::default()
        };
        let brush = Brush::new(Kind::Smooth);
        mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true);
        assert!(mesh.vertices.len() <= original.vertices.len());
        mesh.validate_closed().unwrap();
        settings.simplify = false;
        mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true);
        assert!(mesh.vertices.len() > original.vertices.len());
        mesh.validate_closed().unwrap();
    }
    #[test]
    fn protected_zero_alpha_and_mask_brush_leave_topology_unchanged() {
        let settings = AdaptiveSettings {
            enabled: true,
            detail: 1.,
            ..Default::default()
        };
        let mut mesh = Mesh::sphere();
        mesh.mask.fill(1.);
        let before = mesh.clone();
        assert!(!mesh.adapt(
            &Brush::new(Kind::Snake),
            &[dab()],
            &settings,
            &mut Budget::default(),
            true
        ));
        assert_eq!(mesh, before);
        mesh.mask.fill(0.);
        let before = mesh.clone();
        assert!(!mesh.adapt(
            &Brush::new(Kind::Mask),
            &[dab()],
            &settings,
            &mut Budget::default(),
            true
        ));
        let mut brush = Brush::new(Kind::Snake);
        brush.alpha = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![0.].into(),
        };
        assert!(!mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true));
        assert_eq!(mesh, before);
    }
    #[test]
    fn density_scales_with_view_and_relative_brush_size() {
        let mut settings = AdaptiveSettings::default();
        let brush = Brush::new(Kind::Standard);
        let d = dab();
        let mut closer = d;
        closer.radius *= 0.5;
        assert!(
            (settings.edge_length(&brush, closer) * 2. - settings.edge_length(&brush, d)).abs()
                < 1e-6
        );
        settings.relative = true;
        assert!((settings.edge_length(&brush, d) - d.radius * settings.detail / 100.).abs() < 1e-6);
        settings.detail = f32::NAN;
        assert!(settings.validate().is_err());
    }
}

#[cfg(test)]
mod group_tests {
    use super::*;
    #[test]
    fn adaptive_topology_keeps_face_visibility_and_hidden_positions() {
        let mut m = crate::sculpt::Mesh::starting_sphere();
        m.hidden_faces = m
            .triangles
            .iter()
            .map(|t| t.iter().any(|&v| m.vertices[v].0 > 0.))
            .collect();
        let hidden_points: Vec<_> = m
            .triangles
            .iter()
            .enumerate()
            .filter(|(i, _)| !m.face_visible(*i))
            .flat_map(|(_, t)| t.iter().map(|&v| m.vertices[v]))
            .collect();
        let brush = Brush::new(Kind::Standard);
        let dab = Dab {
            center: Vec3(-1., 0., 0.),
            normal: Vec3(-1., 0., 0.),
            right: Vec3(0., 0., 1.),
            up: Vec3(0., 1., 0.),
            radius: 2.,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        assert!(m.split_local(&brush, &[dab], 0.02, 16) > 0);
        m.validate_closed().unwrap();
        assert!(m.hidden_face_count() > 0);
        m.collapse_local(&brush, &[dab], 0.3, 16);
        m.validate_closed().unwrap();
        assert!(m.hidden_face_count() > 0);
        for p in hidden_points {
            assert!(m.vertices.contains(&p));
        }
    }
    #[test]
    fn adaptive_splits_and_collapses_preserve_groups() {
        let mut m = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Cube)
            .mesh()
            .unwrap();
        m.create_vertex_group("Whole".into(), 0.5, true).unwrap();
        let brush = Brush::new(Kind::Standard);
        let dab = Dab {
            center: Vec3(0., 0., 1.),
            normal: Vec3(0., 0., 1.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 1., 0.),
            radius: 10.,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        };
        assert!(m.split_local(&brush, &[dab], 0.1, 16) > 0);
        m.validate_closed().unwrap();
        assert_eq!(m.polygroups, vec![1; m.triangles.len()]);
        assert!(m.vertex_groups[0].weights.iter().all(|&w| w == 1.));
        let count = m.collapse_local(&brush, &[dab], 3., 16);
        assert!(count > 0);
        m.validate_closed().unwrap();
        assert_eq!(m.polygroups, vec![1; m.triangles.len()]);
        assert!(m.vertex_groups[0].weights.iter().all(|&w| w == 1.));
    }
}
