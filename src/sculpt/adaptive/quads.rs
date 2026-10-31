//! Local all-quad operations. Splitting the spokes of a vertex star gives each
//! incident face two adjacent split edges; a three-quad template closes each
//! face without propagating splits outside the star. Coarsening contracts a
//! quad diagonal, subject to link, protection, orientation and sampled-error checks.
use super::*;
use crate::sculpt::Topology;

fn key(mut t: [usize; 3]) -> [usize; 3] {
    t.sort_unstable();
    t
}

struct Faces {
    pairs: Vec<[usize; 2]>,
    incident: Vec<Vec<usize>>,
}

impl Faces {
    fn new(mesh: &Mesh) -> Option<Self> {
        if mesh.quads.is_empty() || mesh.quads.len() * 2 != mesh.triangles.len() {
            return None;
        }
        let triangles: HashMap<_, _> = mesh
            .triangles
            .iter()
            .enumerate()
            .map(|(i, &t)| (key(t), i))
            .collect();
        let mut pairs = Vec::with_capacity(mesh.quads.len());
        let mut incident = vec![Vec::new(); mesh.vertices.len()];
        for (fi, &[a, b, c, d]) in mesh.quads.iter().enumerate() {
            let pair = [[[a, b, c], [a, c, d]], [[a, b, d], [b, c, d]]]
                .into_iter()
                .find_map(|ts| {
                    Some([*triangles.get(&key(ts[0]))?, *triangles.get(&key(ts[1]))?])
                })?;
            pairs.push(pair);
            for v in [a, b, c, d] {
                incident[v].push(fi);
            }
        }
        Some(Self { pairs, incident })
    }
    fn visible(&self, mesh: &Mesh, fi: usize) -> bool {
        self.pairs[fi].iter().all(|&t| mesh.face_visible(t))
    }
    fn active(&self, mesh: &Mesh, fi: usize, brush: &Brush, region: Region<'_>) -> bool {
        self.pairs[fi]
            .iter()
            .any(|&t| mesh.in_patch(t, mesh.triangles[t], brush, region.dabs, region.stencils))
    }
}

fn triangles([a, b, c, d]: [usize; 4]) -> [[usize; 3]; 2] {
    [[a, b, c], [a, c, d]]
}
fn normal([a, b, c]: [Vec3; 3]) -> Vec3 {
    (b - a).cross(c - a)
}
fn quality([a, b, c]: [Vec3; 3]) -> f32 {
    2. * 3_f32.sqrt() * (b - a).cross(c - a).len()
        / ((b - a).dot(b - a) + (c - b).dot(c - b) + (a - c).dot(a - c)).max(1e-20)
}
fn distance(p: Vec3, surface: &[[Vec3; 3]]) -> f32 {
    surface
        .iter()
        .map(|&[a, b, c]| (p - closest_triangle(p, a, b, c)).len())
        .fold(f32::INFINITY, f32::min)
}
fn samples(t: [Vec3; 3]) -> [Vec3; 7] {
    let [a, b, c] = t;
    [
        a,
        b,
        c,
        (a + b) * 0.5,
        (b + c) * 0.5,
        (c + a) * 0.5,
        (a + b + c) * (1. / 3.),
    ]
}
fn within_error(a: &[[Vec3; 3]], b: &[[Vec3; 3]], tolerance: f32) -> bool {
    a.iter()
        .all(|&t| samples(t).into_iter().all(|p| distance(p, b) <= tolerance))
        && b.iter()
            .all(|&t| samples(t).into_iter().all(|p| distance(p, a) <= tolerance))
}

impl Mesh {
    pub(crate) fn require_adaptive_quads(&self) -> Result<(), String> {
        self.validate_closed()?;
        let Some(faces) = Faces::new(self) else {
            return Err("Quad adaptation requires an all-quad mesh. Use Dynamic mesh → Quads → Remesh now first, or choose Triangles.".into());
        };
        // Edge incidence alone allows two closed sheets to touch at one vertex.
        // Each vertex link must be one cycle before applying local quad edits.
        for (v, incident) in faces.incident.iter().enumerate() {
            if incident.is_empty() {
                continue;
            }
            let mut next = HashMap::new();
            for &fi in incident {
                let q = self.quads[fi];
                let i = q.iter().position(|&i| i == v).unwrap();
                if next.insert(q[(i + 3) % 4], q[(i + 1) % 4]).is_some() {
                    return Err(
                        "Quad adaptation requires a single connected face fan at each vertex"
                            .into(),
                    );
                }
            }
            let start = *next.keys().next().unwrap();
            let mut cursor = start;
            for step in 0..incident.len() {
                let Some(&n) = next.get(&cursor) else {
                    return Err(
                        "Quad adaptation requires consistently oriented polygon faces".into(),
                    );
                };
                cursor = n;
                if cursor == start && step + 1 != incident.len() {
                    return Err(
                        "Quad adaptation cannot edit sheets that touch at a single vertex".into(),
                    );
                }
            }
            if cursor != start {
                return Err("Quad adaptation requires closed polygon face fans".into());
            }
        }
        Ok(())
    }
    pub(super) fn adapt_quads(
        &mut self,
        brush: &Brush,
        region: Region<'_>,
        settings: &AdaptiveSettings,
        budget: &mut Budget,
        collapse: bool,
    ) -> bool {
        debug_assert_eq!(settings.topology, Topology::Quads);
        let target = settings.edge_length(brush, region.dabs[0]);
        let cleanup = brush.kind == Kind::Decimate;
        let mut changed = false;
        if collapse && (settings.simplify || cleanup) && budget.collapses > 0 {
            let count = self.collapse_quads(
                brush,
                region,
                target * settings.collapse_fraction(brush.kind),
                budget.collapses.min(if cleanup { 64 } else { 16 }),
                settings.unmasked_only,
            );
            budget.collapses = 0;
            changed |= count > 0;
        }
        if settings.refine && !cleanup {
            for _ in 0..4 {
                if budget.splits == 0 || budget.passes == 0 {
                    break;
                }
                budget.passes -= 1;
                let count = self.split_quad_stars(
                    brush,
                    region,
                    target,
                    budget.splits,
                    SplitOptions {
                        relief: false,
                        unmasked_only: settings.unmasked_only,
                    },
                );
                budget.splits -= count;
                changed |= count > 0;
                if count == 0 {
                    break;
                }
            }
        }
        changed
    }
    pub(super) fn prepare_quad_stamp(
        &mut self,
        brush: &Brush,
        region: Region<'_>,
        settings: &AdaptiveSettings,
        cancelled: &impl Fn() -> bool,
    ) -> bool {
        let mut remaining = 16384;
        let mut changed = false;
        for _ in 0..32 {
            if cancelled() {
                break;
            }
            let count = self.split_quad_stars(
                brush,
                region,
                settings.edge_length(brush, region.dabs[0]),
                remaining.min(1024),
                SplitOptions {
                    relief: true,
                    unmasked_only: settings.unmasked_only,
                },
            );
            remaining -= count;
            changed |= count > 0;
            if count == 0 || remaining == 0 {
                break;
            }
        }
        changed
    }

    fn append_quad_vertex(&mut self, weights: &[(usize, f32)], position: Vec3) -> usize {
        let index = self.vertices.len();
        self.vertices.push(position);
        self.mask
            .push(weights.iter().map(|&(v, w)| self.mask[v] * w).sum());
        if !self.materials.is_empty() {
            self.materials.push(crate::sculpt::Material::blend(
                weights.iter().map(|&(v, w)| (self.material(v), w)),
            ));
        }
        for group in &mut self.vertex_groups {
            group
                .weights
                .push(weights.iter().map(|&(v, w)| group.weights[v] * w).sum());
        }
        index
    }
    fn replace_quad_faces(
        &mut self,
        faces: &Faces,
        replacements: &HashMap<usize, Vec<[usize; 4]>>,
    ) {
        let mut quads = Vec::new();
        let mut tris = Vec::new();
        let mut groups = Vec::new();
        let mut hidden = Vec::new();
        for (fi, &q) in self.quads.iter().enumerate() {
            let pair = faces.pairs[fi];
            if let Some(new) = replacements.get(&fi) {
                for &q in new {
                    quads.push(q);
                    tris.extend(triangles(q));
                    if !self.polygroups.is_empty() {
                        groups.extend([self.polygroups[pair[0]]; 2]);
                    }
                    if !self.hidden_faces.is_empty() {
                        hidden.extend([self.hidden_faces[pair[0]]; 2]);
                    }
                }
            } else {
                quads.push(q);
                for i in pair {
                    tris.push(self.triangles[i]);
                    if !self.polygroups.is_empty() {
                        groups.push(self.polygroups[i]);
                    }
                    if !self.hidden_faces.is_empty() {
                        hidden.push(self.hidden_faces[i]);
                    }
                }
            }
        }
        self.quads = quads;
        self.triangles = tris;
        self.polygroups = groups;
        self.hidden_faces = hidden;
    }

    fn split_quad_stars(
        &mut self,
        brush: &Brush,
        region: Region<'_>,
        target: f32,
        limit: usize,
        options: SplitOptions,
    ) -> usize {
        let Some(faces) = Faces::new(self) else {
            return 0;
        };
        let mut priorities = BTreeMap::<usize, f32>::new();
        let mut sampling = options
            .relief
            .then(|| relief::Sampling::new(brush, region, target));
        for (fi, &q) in self.quads.iter().enumerate() {
            if !faces.active(self, fi, brush, region) {
                continue;
            }
            for i in 0..4 {
                let (a, b) = (q[i], q[(i + 1) % 4]);
                let mut edge_target = target;
                if brush.kind == Kind::Detail
                    && region.dabs.iter().any(|d| {
                        ((self.vertices[a] + self.vertices[b]) * 0.5 - d.center).len()
                            < d.radius * crate::sculpt::detail::FEATURE_RADIUS_FRACTION
                    })
                {
                    edge_target = target
                        .min(region.dabs[0].radius * crate::sculpt::detail::CORE_EDGE_FRACTION);
                }
                let ratio = (self.vertices[a] - self.vertices[b]).len() / edge_target;
                if ratio > 1. {
                    for v in [a, b] {
                        priorities
                            .entry(v)
                            .and_modify(|r| *r = r.max(ratio))
                            .or_insert(ratio);
                    }
                }
            }
            if let Some(sampling) = &mut sampling {
                for &t in &faces.pairs[fi] {
                    if let Some(((a, b), ratio)) = sampling.candidate(self, self.triangles[t]) {
                        for v in [a, b] {
                            priorities
                                .entry(v)
                                .and_modify(|r| *r = r.max(ratio))
                                .or_insert(ratio);
                        }
                    }
                }
            }
        }
        let mut candidates: Vec<_> = priorities.into_iter().collect();
        candidates.sort_by(|(a, x), (b, y)| y.total_cmp(x).then(a.cmp(b)));
        let mut replacements = HashMap::new();
        let original_vertices = self.vertices.len();
        let mut remaining = limit
            .min(max_vertices().saturating_sub(original_vertices))
            .min(max_triangles().saturating_sub(self.triangles.len()) / 2);
        for (v, _) in candidates {
            let star = &faces.incident[v];
            let cost = star.len() * 2;
            if star.len() < 3
                || cost > remaining
                || star.iter().any(|&f| {
                    replacements.contains_key(&f)
                        || !faces.visible(self, f)
                        || (!region.stencils.is_empty() && !faces.active(self, f, brush, region))
                        || self.quads[f].iter().any(|&i| {
                            (options.unmasked_only && self.mask[i] > 0.) || self.mask[i] >= 0.999
                        })
                })
            {
                continue;
            }
            // Plan and validate the complete star before appending any vertex.
            let mut plans = Vec::new();
            let mut safe = true;
            for &fi in star {
                let mut q = self.quads[fi];
                let offset = q.iter().position(|&i| i == v).unwrap();
                q.rotate_left(offset);
                let [a, b, c, d] = q;
                let p = (self.vertices[a] + self.vertices[b]) * 0.5;
                let r = (self.vertices[d] + self.vertices[a]) * 0.5;
                // The existing render diagonal lies on the retained surface.
                let pair = faces.pairs[fi].map(|t| self.triangles[t]);
                let diagonal: Vec<_> = pair[0]
                    .into_iter()
                    .filter(|i| pair[1].contains(i))
                    .collect();
                let center = (self.vertices[diagonal[0]] + self.vertices[diagonal[1]]) * 0.5;
                let shapes = [
                    [center, p, self.vertices[b], self.vertices[c]],
                    [center, self.vertices[c], self.vertices[d], r],
                    [center, r, self.vertices[a], p],
                ];
                let old = pair.map(|t| t.map(|i| self.vertices[i]));
                let n = (normal(old[0]) + normal(old[1])).norm();
                let new: Vec<_> = shapes
                    .into_iter()
                    .flat_map(|[a, b, c, d]| [[a, b, c], [a, c, d]])
                    .collect();
                if new
                    .iter()
                    .any(|&t| quality(t) < 0.1 || normal(t).norm().dot(n) < 0.5)
                    || !within_error(&old, &new, target * 0.15)
                {
                    safe = false;
                    break;
                }
                plans.push((fi, q, diagonal, center));
            }
            if !safe {
                continue;
            }
            let mut mids = HashMap::new();
            for (fi, [a, b, c, d], diagonal, center) in plans {
                let mut midpoint = |x, y| {
                    *mids.entry(edge(x, y)).or_insert_with(|| {
                        self.append_quad_vertex(
                            &[(x, 0.5), (y, 0.5)],
                            (self.vertices[x] + self.vertices[y]) * 0.5,
                        )
                    })
                };
                let ab = midpoint(a, b);
                let da = midpoint(d, a);
                let center =
                    self.append_quad_vertex(&[(diagonal[0], 0.5), (diagonal[1], 0.5)], center);
                replacements.insert(
                    fi,
                    vec![[center, ab, b, c], [center, c, d, da], [center, da, a, ab]],
                );
            }
            remaining -= cost;
        }
        let added = self.vertices.len() - original_vertices;
        if added > 0 {
            self.replace_quad_faces(&faces, &replacements);
        }
        added
    }

    fn collapse_quads(
        &mut self,
        brush: &Brush,
        region: Region<'_>,
        threshold: f32,
        limit: usize,
        unmasked_only: bool,
    ) -> usize {
        let Some(faces) = Faces::new(self) else {
            return 0;
        };
        let mut neighbors = vec![HashSet::new(); self.vertices.len()];
        for q in &self.quads {
            for i in 0..4 {
                neighbors[q[i]].extend([q[(i + 1) % 4], q[(i + 3) % 4]]);
            }
        }
        let mut candidates = Vec::new();
        for (fi, &q) in self.quads.iter().enumerate() {
            if !faces.active(self, fi, brush, region) {
                continue;
            }
            for rotation in 0..2 {
                let mut q = q;
                q.rotate_left(rotation);
                let length = (self.vertices[q[0]] - self.vertices[q[2]]).len();
                if length < threshold {
                    candidates.push((length, fi, q));
                }
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut merged = BTreeMap::new();
        let mut removed = HashSet::new();
        let mut blocked = HashSet::new();
        for (_, fi, [a, b, c, d]) in candidates {
            if merged.len() >= limit {
                break;
            }
            let mut incident = faces.incident[a].clone();
            incident.extend(&faces.incident[c]);
            incident.sort_unstable();
            incident.dedup();
            let vertices: HashSet<_> = incident.iter().flat_map(|&f| self.quads[f]).collect();
            if vertices.iter().any(|&v| {
                blocked.contains(&v) || self.mask[v] > 0.95 || (unmasked_only && self.mask[v] > 0.)
            }) || incident.iter().any(|&f| !faces.visible(self, f))
                || (self.mask[a] - self.mask[c]).abs() > 0.05
                || faces.incident[b].len() < 4
                || faces.incident[d].len() < 4
                || faces.incident[a]
                    .iter()
                    .filter(|f| faces.incident[c].contains(f))
                    .count()
                    != 1
                || neighbors[a]
                    .intersection(&neighbors[c])
                    .copied()
                    .collect::<HashSet<_>>()
                    != HashSet::from([b, d])
            {
                continue;
            }
            if !self.polygroups.is_empty()
                && incident.iter().any(|&f| {
                    self.polygroups[faces.pairs[f][0]] != self.polygroups[faces.pairs[fi][0]]
                })
            {
                continue;
            }
            let points: Vec<_> = vertices.iter().map(|&v| self.vertices[v]).collect();
            if !region.permits_reconnection(brush, &points) {
                continue;
            }
            let position = (self.vertices[a] + self.vertices[c]) * 0.5;
            // Coarsen only inside one brush copy; do not consume masked falloff
            // or a Snake tip just because a nearby face intersects the brush.
            if !region.dabs.iter().any(|dab| {
                [a, c].iter().all(|&v| {
                    let delta = self.vertices[v] - dab.center;
                    let weight = brush.weight(
                        delta.dot(dab.right) / dab.radius,
                        delta.dot(dab.up) / dab.radius,
                    );
                    let pressure = if brush.pressure {
                        dab.pressure.powf(brush.pressure_curve)
                    } else {
                        1.
                    };
                    delta.len() < dab.radius
                        && (self.vertices[a] - self.vertices[c]).len()
                            < threshold
                                * (weight * pressure * brush.strength * 4. * (1. - self.mask[v]))
                                    .clamp(0., 1.)
                                    .sqrt()
                        && (brush.kind != Kind::Snake || delta.len() > dab.radius * 0.35)
                })
            }) {
                continue;
            }
            let old: Vec<_> = incident
                .iter()
                .flat_map(|&f| faces.pairs[f].map(|t| self.triangles[t].map(|i| self.vertices[i])))
                .collect();
            let mut new = Vec::new();
            let mut safe = true;
            for &f in &incident {
                if f == fi {
                    continue;
                }
                for ti in faces.pairs[f] {
                    let before = self.triangles[ti].map(|v| self.vertices[v]);
                    let after = self.triangles[ti].map(|v| {
                        if v == a || v == c {
                            position
                        } else {
                            self.vertices[v]
                        }
                    });
                    if quality(after) < (quality(before) * 0.8).clamp(0.15, 0.35)
                        || normal(after).norm().dot(normal(before).norm()) < 0.8
                    {
                        safe = false;
                        break;
                    }
                    new.push(after);
                }
            }
            if !safe || !within_error(&old, &new, threshold * 0.08) {
                continue;
            }
            blocked.extend(vertices);
            merged.insert(c, a);
            removed.insert(fi);
        }
        if merged.is_empty() {
            return 0;
        }
        for (&c, &a) in &merged {
            self.vertices[a] = (self.vertices[a] + self.vertices[c]) * 0.5;
            self.mask[a] = (self.mask[a] + self.mask[c]) * 0.5;
            if !self.materials.is_empty() {
                self.materials[a] = crate::sculpt::Material::blend([
                    (self.material(a), 0.5),
                    (self.material(c), 0.5),
                ]);
            }
            for g in &mut self.vertex_groups {
                g.weights[a] = (g.weights[a] + g.weights[c]) * 0.5;
            }
        }
        let kept: Vec<_> = (0..self.vertices.len())
            .filter(|v| !merged.contains_key(v))
            .collect();
        let mut indices = vec![0; self.vertices.len()];
        for (new, &old) in kept.iter().enumerate() {
            indices[old] = new;
        }
        for (&c, &a) in &merged {
            indices[c] = indices[a];
        }
        let replacements = removed.into_iter().map(|fi| (fi, Vec::new())).collect();
        self.replace_quad_faces(&faces, &replacements);
        for t in &mut self.triangles {
            *t = t.map(|i| indices[i]);
        }
        for q in &mut self.quads {
            *q = q.map(|i| indices[i]);
        }
        self.materials = self.reindex_materials(kept.iter().copied());
        self.vertices = kept.iter().map(|&i| self.vertices[i]).collect();
        self.mask = kept.iter().map(|&i| self.mask[i]).collect();
        for group in &mut self.vertex_groups {
            group.weights = kept.iter().map(|&i| group.weights[i]).collect();
        }
        merged.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dab() -> Dab {
        Dab {
            center: Vec3(0., 1., 0.),
            normal: Vec3(0., 1., 0.),
            right: Vec3(1., 0., 0.),
            up: Vec3(0., 0., 1.),
            radius: 0.7,
            pressure: 1.,
            delta: Vec3::default(),
            invert: false,
        }
    }
    fn check(mesh: &Mesh) {
        mesh.validate_closed().unwrap();
        assert_eq!(mesh.quads.len() * 2, mesh.triangles.len());
        let keys: HashSet<_> = mesh.triangles.iter().copied().map(key).collect();
        assert_eq!(keys.len(), mesh.triangles.len());
        for t in &mesh.triangles {
            assert!(quality(t.map(|v| mesh.vertices[v])) > 0.001);
        }
        // Every polygon vertex link must be one cycle, not two sheets touching
        // at a single vertex (an edge-incidence check would miss that).
        let mut links = vec![HashMap::<usize, Vec<usize>>::new(); mesh.vertices.len()];
        for q in &mesh.quads {
            for i in 0..4 {
                let (a, b) = (q[(i + 1) % 4], q[(i + 3) % 4]);
                links[q[i]].entry(a).or_default().push(b);
                links[q[i]].entry(b).or_default().push(a);
            }
        }
        for link in links {
            assert!(!link.is_empty());
            assert!(link.values().all(|ns| ns.len() == 2));
            let mut todo = vec![*link.keys().next().unwrap()];
            let mut seen = HashSet::new();
            while let Some(v) = todo.pop() {
                if seen.insert(v) {
                    todo.extend(&link[&v]);
                }
            }
            assert_eq!(seen.len(), link.len());
        }
    }
    fn face_positions(
        mesh: &Mesh,
        faces: impl Iterator<Item = [usize; 3]>,
    ) -> HashSet<[[u32; 3]; 3]> {
        faces
            .map(|t| {
                let mut p = t.map(|v| {
                    let p = mesh.vertices[v];
                    [p.0.to_bits(), p.1.to_bits(), p.2.to_bits()]
                });
                p.sort_unstable();
                p
            })
            .collect()
    }

    #[test]
    fn adaptive_quads_refine_coarsen_and_repeat_without_cracks() {
        let mut mesh = Mesh::sphere();
        let initial = mesh.vertices.len();
        let mut brush = Brush::new(Kind::Standard);
        brush.radius = 100.;
        let mut settings = AdaptiveSettings {
            enabled: true,
            topology: Topology::Quads,
            detail: 3.,
            simplify: false,
            ..Default::default()
        };
        for _ in 0..6 {
            mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true);
            check(&mesh);
        }
        assert!(mesh.vertices.len() > initial);
        let refined = mesh.vertices.len();
        brush.kind = Kind::Decimate;
        brush.strength = 1.;
        settings.detail = 32.;
        settings.collapse_ratio = 0.8;
        for _ in 0..6 {
            mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true);
            check(&mesh);
        }
        assert!(
            mesh.vertices.len() < refined,
            "coarsening must actually remove quads"
        );
        brush.kind = Kind::Standard;
        settings.detail = 3.;
        assert!(mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true));
        check(&mesh);
        let saved = serde_json::to_string(&mesh).unwrap();
        let loaded: Mesh = serde_json::from_str(&saved).unwrap();
        assert_eq!(mesh, loaded);
    }

    #[test]
    fn adaptive_unmasked_only_preserves_partial_mask_faces_in_both_modes() {
        for topology in [Topology::Triangles, Topology::Quads] {
            let mut mesh = Mesh::sphere();
            for (p, w) in mesh.vertices.iter().zip(&mut mesh.mask) {
                if p.0 < 0. {
                    *w = 0.25;
                }
            }
            // A single masked corner protects its whole editable quad, even
            // when only one of the two render triangles contains that corner.
            let q = mesh
                .quads
                .iter()
                .find(|q| {
                    q.iter()
                        .all(|&v| mesh.vertices[v].0 > 0. && mesh.vertices[v].1 > 0.9)
                })
                .copied()
                .unwrap();
            mesh.mask[q[1]] = 0.25;
            let original = mesh.clone();
            let protected_quads: Vec<_> = original
                .quads
                .iter()
                .filter(|q| q.iter().any(|&v| original.mask[v] > 0.))
                .map(|q| q.map(|i| original.vertices[i]))
                .collect();
            let protected = face_positions(
                &mesh,
                mesh.triangles
                    .iter()
                    .copied()
                    .filter(|t| t.iter().any(|&v| mesh.mask[v] > 0.)),
            );
            let mut brush = Brush::new(Kind::Standard);
            brush.radius = 100.;
            let mut settings = AdaptiveSettings {
                enabled: true,
                topology,
                unmasked_only: true,
                detail: 3.,
                simplify: false,
                ..Default::default()
            };
            assert!(mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true));
            assert!(mesh.vertices.len() > original.vertices.len());
            brush.kind = Kind::Decimate;
            brush.strength = 1.;
            settings.detail = 32.;
            settings.collapse_ratio = 0.8;
            for _ in 0..3 {
                mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true);
            }
            mesh.validate_closed().unwrap();
            let after = face_positions(&mesh, mesh.triangles.iter().copied());
            assert!(
                protected.is_subset(&after),
                "{topology:?}: masked boundary faces changed"
            );
            let quad_positions: Vec<_> = mesh
                .quads
                .iter()
                .map(|q| q.map(|i| mesh.vertices[i]))
                .collect();
            assert!(
                protected_quads.iter().all(|q| quad_positions.contains(q)),
                "{topology:?}: protected quad polygon changed"
            );
            for (&p, &w) in original.vertices.iter().zip(&original.mask) {
                if w > 0. {
                    let i = mesh
                        .vertices
                        .iter()
                        .position(|&v| v == p)
                        .expect("protected vertex removed");
                    assert_eq!(mesh.mask[i], w);
                }
            }
        }
    }

    #[test]
    fn adaptive_quad_stamps_masks_and_hidden_regions_keep_topology() {
        let mut mesh = Mesh::sphere();
        let mut brush = Brush::new(Kind::Standard);
        brush.radius = 100.;
        brush.stroke_mode = crate::sculpt::StrokeMode::DragRect;
        let settings = AdaptiveSettings {
            enabled: true,
            topology: Topology::Quads,
            detail: 5.,
            ..Default::default()
        };
        assert!(mesh.prepare_drag_stamp(&brush, &[dab()], &settings));
        check(&mesh);
        mesh.mask.fill(1.);
        let before = mesh.clone();
        assert!(!mesh.prepare_drag_stamp(&brush, &[dab()], &settings));
        assert_eq!(mesh, before);
        mesh.mask.fill(0.);
        mesh.hidden_faces = vec![true; mesh.triangles.len()];
        let before = mesh.clone();
        assert!(!mesh.adapt(&brush, &[dab()], &settings, &mut Budget::default(), true));
        assert_eq!(mesh, before);
    }

    #[test]
    fn adaptive_quads_follow_symmetry_and_stencil_protection_with_attributes() {
        let mut mesh = Mesh::sphere();
        mesh.materials = vec![
            crate::sculpt::Material::from_srgb([0.8, 0.2, 0.1], 0.25, 0.6);
            mesh.vertices.len()
        ];
        mesh.create_vertex_group("Surface".into(), 0.5, true)
            .unwrap();
        let brush = Brush::new(Kind::Standard);
        let settings = AdaptiveSettings {
            enabled: true,
            topology: Topology::Quads,
            detail: 3.,
            simplify: false,
            ..Default::default()
        };
        let symmetry = crate::sculpt::Symmetry {
            enabled: true,
            ..Default::default()
        };
        let center = Vec3(0.5, 0.8660254, 0.);
        let dabs = symmetry.dabs(Dab {
            center,
            normal: center.norm(),
            radius: 0.25,
            ..dab()
        });
        let mut stencil = Projection {
            image: Alpha::Image {
                width: 1,
                height: 1,
                pixels: vec![0.].into(),
            },
            frame: crate::sculpt::alpha::Frame {
                origin: Vec3::default(),
                right: Vec3(1., 0., 0.),
                up: Vec3(0., 0., 1.),
            },
            view: Vec3(0., 1., 0.),
            distance: 0.,
            position: [0.; 2],
            size: [3.; 2],
            rotation: 0.,
        };
        let before = mesh.clone();
        let copies = stencil.copies(&symmetry);
        assert!(!mesh.adapt_stenciled(
            &brush,
            Region {
                dabs: &dabs,
                stencils: &copies
            },
            &settings,
            &mut Budget::default(),
            true
        ));
        assert_eq!(mesh, before);
        stencil.image = Alpha::Image {
            width: 1,
            height: 1,
            pixels: vec![1.].into(),
        };
        let copies = stencil.copies(&symmetry);
        for _ in 0..4 {
            mesh.adapt_stenciled(
                &brush,
                Region {
                    dabs: &dabs,
                    stencils: &copies,
                },
                &settings,
                &mut Budget::default(),
                true,
            );
        }
        let added = &mesh.vertices[before.vertices.len()..];
        assert!(added.iter().any(|v| v.0 > 0.3));
        assert!(added.iter().any(|v| v.0 < -0.3));
        assert_eq!(mesh.vertex_groups[0].name, "Surface");
        assert!(
            mesh.vertex_groups[0]
                .weights
                .iter()
                .all(|&w| (w - 1.).abs() < 1e-6)
        );
        assert!(
            mesh.materials
                .iter()
                .all(|m| (m.metallic - 0.6).abs() < 1e-6)
        );
        check(&mesh);
    }
}
