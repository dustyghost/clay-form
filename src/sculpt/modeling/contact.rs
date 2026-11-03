//! Contact uses the complete region boundary; internal selected edges never form walls.
use super::*;

pub(super) struct ContactTarget {
    pub caps: BTreeSet<usize>,
    pub depth: f32,
    pub mapping: BTreeMap<usize, usize>,
}
pub(super) struct ContactRegion<'a> {
    pub polygons: &'a [Polygon],
    pub selected: &'a BTreeSet<usize>,
    pub vertices: &'a BTreeSet<usize>,
    pub boundary: &'a BTreeMap<[usize; 2], (usize, usize, usize)>,
    pub direction: Vec3,
    pub distance: f32,
    pub snap: f32,
}

/// Find vertices at a translated corner, including distinct sheets sharing a position.
struct Positions<'a> {
    mesh: &'a Mesh,
    tolerance: f32,
    bins: BTreeMap<(i64, i64, i64), Vec<usize>>,
}
impl<'a> Positions<'a> {
    fn new(mesh: &'a Mesh, tolerance: f32) -> Self {
        let mut out = Self {
            mesh,
            tolerance,
            bins: BTreeMap::new(),
        };
        for (i, &p) in mesh.vertices.iter().enumerate() {
            out.bins.entry(out.bucket(p)).or_default().push(i);
        }
        out
    }
    fn bucket(&self, p: Vec3) -> (i64, i64, i64) {
        (
            (p.0 / self.tolerance).floor() as i64,
            (p.1 / self.tolerance).floor() as i64,
            (p.2 / self.tolerance).floor() as i64,
        )
    }
    fn candidates(&self, v: usize, region: &ContactRegion<'_>, depth: f32) -> Vec<usize> {
        let target = self.mesh.vertices[v] + region.direction * depth;
        let (x, y, z) = self.bucket(target);
        let mut found = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(indices) = self.bins.get(&(
                        x.saturating_add(dx),
                        y.saturating_add(dy),
                        z.saturating_add(dz),
                    )) {
                        found.extend(indices.iter().copied().filter(|w| {
                            !region.vertices.contains(w)
                                && (self.mesh.vertices[*w] - target).len() <= self.tolerance
                        }));
                    }
                }
            }
        }
        found
    }
    fn translated(
        &self,
        vertices: &BTreeSet<usize>,
        region: &ContactRegion<'_>,
        depth: f32,
    ) -> Option<BTreeMap<usize, usize>> {
        let mut mapping = BTreeMap::new();
        let mut used = BTreeSet::new();
        for &v in vertices {
            let candidates = self.candidates(v, region, depth);
            let &[w] = candidates.as_slice() else {
                return None;
            };
            if !used.insert(w) {
                return None;
            }
            mapping.insert(v, w);
        }
        Some(mapping)
    }

    // A cap identifies its own corners by their reversed polygon cycle. A lone
    // coincident vertex on another sheet is not a second matching polygon.
    fn matching_face(
        &self,
        source: &Polygon,
        region: &ContactRegion<'_>,
        depth: f32,
        incident: &[Vec<(usize, usize)>],
    ) -> Option<(usize, Vec<usize>)> {
        let offset = region.direction * depth;
        let mut found = None;
        for v in self.candidates(source.vertices[0], region, depth) {
            for &(f, start) in &incident[v] {
                let face = &region.polygons[f];
                let n = face.vertices.len();
                if n != source.vertices.len() {
                    continue;
                }
                let cycle = (0..n).map(|i| face.vertices[(start + n - i) % n]);
                if source.vertices.iter().zip(cycle.clone()).all(|(&a, b)| {
                    !region.vertices.contains(&b)
                        && (self.mesh.vertices[a] + offset - self.mesh.vertices[b]).len()
                            <= self.tolerance
                }) {
                    if found.is_some() {
                        return None;
                    }
                    found = Some((f, cycle.collect()));
                }
            }
        }
        found
    }
}
impl ContactRegion<'_> {
    fn depths(&self, mesh: &Mesh, size: f32) -> Vec<f32> {
        let origin = mesh.vertices[self.boundary.values().next().unwrap().0];
        let tolerance = (size * 1e-4).max(1e-7);
        let mut depths: Vec<_> = mesh
            .vertices
            .iter()
            .filter_map(|&p| {
                let depth = (p - origin).dot(self.direction);
                (depth * self.distance > 0.
                    && depth.abs() > tolerance
                    && depth.abs() <= self.distance.abs() + size * self.snap
                    && (p - origin - self.direction * depth).len() <= tolerance)
                    .then_some(depth)
            })
            .collect();
        depths.sort_by(|a, b| a.abs().total_cmp(&b.abs()));
        depths.dedup_by(|a, b| (*a - *b).abs() < tolerance);
        depths
    }
}

pub(super) fn connected(polygons: &[Polygon], selected: &BTreeSet<usize>) -> bool {
    let Some(&first) = selected.first() else {
        return false;
    };
    let mut incidence = BTreeMap::<_, Vec<_>>::new();
    for &f in selected {
        for (a, b) in polygons[f].edges() {
            incidence.entry(edge(a, b)).or_default().push(f);
        }
    }
    let mut seen = BTreeSet::new();
    let mut pending = vec![first];
    while let Some(f) = pending.pop() {
        if seen.insert(f) {
            for (a, b) in polygons[f].edges() {
                pending.extend(&incidence[&edge(a, b)]);
            }
        }
    }
    seen.len() == selected.len()
}

impl Mesh {
    pub(super) fn cap_contact(
        &self,
        region: &ContactRegion<'_>,
        size: f32,
    ) -> Option<ContactTarget> {
        let positions = Positions::new(self, (size * 1e-4).max(1e-7));
        let mut incident = vec![Vec::new(); self.vertices.len()];
        for (f, p) in region.polygons.iter().enumerate() {
            if normal(self, p).dot(region.direction) <= -0.9999 && !region.selected.contains(&f) {
                for (corner, &v) in p.vertices.iter().enumerate() {
                    incident[v].push((f, corner));
                }
            }
        }
        'depths: for depth in region.depths(self, size) {
            let mut caps = BTreeSet::new();
            let mut mapping = BTreeMap::new();
            for &f in region.selected {
                let source = &region.polygons[f];
                let Some((cap, cycle)) = positions.matching_face(source, region, depth, &incident)
                else {
                    continue 'depths;
                };
                if !caps.insert(cap) {
                    continue 'depths;
                }
                for (&v, w) in source.vertices.iter().zip(cycle) {
                    if mapping.insert(v, w).is_some_and(|old| old != w) {
                        continue 'depths;
                    }
                }
            }
            if mapping.values().collect::<BTreeSet<_>>().len() == mapping.len() {
                return Some(ContactTarget {
                    caps,
                    depth,
                    mapping,
                });
            }
        }
        None
    }
    pub(super) fn loop_contact(
        &self,
        region: &ContactRegion<'_>,
        cap_depth: Option<f32>,
    ) -> Result<Option<ContactTarget>, String> {
        let size = region
            .boundary
            .values()
            .map(|&(a, b, _)| (self.vertices[a] - self.vertices[b]).len())
            .fold(f32::INFINITY, f32::min);
        let tolerance = (size * 1e-4).max(1e-7);
        let positions = Positions::new(self, tolerance);
        let boundary_vertices = region
            .boundary
            .values()
            .flat_map(|&(a, b, _)| [a, b])
            .collect();
        for depth in region.depths(self, size) {
            if cap_depth.is_some_and(|cap| depth.abs() >= cap.abs()) {
                break;
            }
            let Some(mapping) = positions.translated(&boundary_vertices, region, depth) else {
                continue;
            };
            let target_edges: BTreeSet<_> = region
                .boundary
                .values()
                .map(|&(a, b, _)| edge(mapping[&a], mapping[&b]))
                .collect();
            // A cross-sectional cap is a different operation, controlled by Join.
            // Never paste a moved cap onto an existing cap, including a tiled one.
            if region.polygons.iter().any(|p| {
                normal(self, p).dot(region.direction).abs() > 0.9999
                    && p.edges().any(|(a, b)| target_edges.contains(&edge(a, b)))
            }) {
                continue;
            }
            let sides: Vec<_> = region
                .boundary
                .values()
                .map(|&(a, b, g)| ([a, b, mapping[&b], mapping[&a]], g))
                .collect();
            let mut out = self.empty_topology();
            let walls =
                self.joined_walls(region.polygons, &sides, region.selected, false, &mut out)?;
            // A corner from a nearby notch can line up with the moved rim without
            // belonging to any swept wall. Welding it would pinch the two surfaces.
            let attached: BTreeSet<_> = walls
                .remove
                .iter()
                .flat_map(|&f| region.polygons[f].vertices.iter().copied())
                .collect();
            if walls.seam.is_empty() && mapping.values().all(|v| attached.contains(v)) {
                return Ok(Some(ContactTarget {
                    caps: BTreeSet::new(),
                    depth,
                    mapping,
                }));
            }
        }
        Ok(None)
    }
}
