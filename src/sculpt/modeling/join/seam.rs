//! Reconnect only the rims identified by complete, cancelled side patches.
use super::*;

impl Mesh {
    pub(in crate::sculpt::modeling) fn stitch_join_seam(
        &mut self,
        seam: &BTreeMap<usize, usize>,
        source: &[Polygon],
    ) -> Result<(), String> {
        if seam.values().any(|v| seam.contains_key(v)) {
            return Err("The tunnel has conflicting seam correspondences".into());
        }
        let canonical = |v: usize| seam.get(&v).copied().unwrap_or(v);
        let polygons = self.polygons();
        let mut incidence = BTreeMap::<_, Vec<usize>>::new();
        for (f, polygon) in polygons.iter().enumerate() {
            for (a, b) in polygon.edges() {
                incidence
                    .entry(edge(canonical(a), canonical(b)))
                    .or_default()
                    .push(f);
            }
        }

        // Earlier tunnels can touch along an edge. Keep each sheet's fan
        // separate wherever four faces would otherwise acquire the same edge.
        let endpoints: BTreeSet<_> = seam.values().copied().collect();
        let mut affected = endpoints.clone();
        for (&e, faces) in &incidence {
            if faces.len() > 2 {
                if e.iter().all(|v| !endpoints.contains(v)) {
                    return Err("The tunnel would overlap an edge outside the joined seam".into());
                }
                affected.extend(e);
            }
        }
        let mut stars = BTreeMap::<usize, BTreeSet<usize>>::new();
        let mut corners = Vec::with_capacity(polygons.len());
        for (f, p) in polygons.iter().enumerate() {
            let mapped: Vec<_> = p.vertices.iter().map(|&v| canonical(v)).collect();
            for &v in &mapped {
                if affected.contains(&v) {
                    stars.entry(v).or_default().insert(f);
                }
            }
            corners.push(mapped);
        }
        let mut out = self.empty_topology();
        for (v, mut remaining) in stars {
            let mut used = BTreeSet::new();
            while let Some(first) = remaining.pop_first() {
                let mut fan = vec![first];
                let mut i = 0;
                while i < fan.len() {
                    let p = &polygons[fan[i]];
                    for (a, b) in p.edges() {
                        let e = edge(canonical(a), canonical(b));
                        if e.contains(&v) && incidence[&e].len() == 2 {
                            for &other in &incidence[&e] {
                                if remaining.remove(&other) {
                                    fan.push(other);
                                }
                            }
                        }
                    }
                    i += 1;
                }
                let originals: BTreeSet<_> = fan
                    .iter()
                    .flat_map(|&f| &polygons[f].vertices)
                    .copied()
                    .filter(|&w| canonical(w) == v)
                    .collect();
                let first = *originals.first().unwrap();
                let id = if originals.len() == 1 && used.insert(first) {
                    first
                } else {
                    if out.vertices.len() >= max_vertices() {
                        return Err("Tunnel seam would exceed the vertex limit".into());
                    }
                    let weight = 1. / originals.len() as f32;
                    let weights: Vec<_> = originals.iter().map(|&w| (w, weight)).collect();
                    out.append_blend(self, &weights, self.vertices[v])
                };
                for f in fan {
                    for (k, &w) in polygons[f].vertices.iter().enumerate() {
                        if canonical(w) == v {
                            corners[f][k] = id;
                        }
                    }
                }
            }
        }

        let creases: BTreeSet<_> = self.creases.iter().copied().collect();
        let mut borders = BTreeSet::new();
        for p in source {
            for (a, b) in p.edges() {
                if affected.contains(&canonical(a)) || affected.contains(&canonical(b)) {
                    let e = edge(a, b);
                    if !borders.insert(e) {
                        borders.remove(&e);
                    }
                }
            }
        }
        let original_borders: BTreeSet<_> = borders
            .into_iter()
            .map(|[a, b]| edge(canonical(a), canonical(b)))
            .collect();
        let mut open = BTreeMap::new();
        for (p, cycle) in polygons.iter().zip(&corners) {
            out.push_polygon(cycle, p.group, p.hidden);
            for (i, (a, b)) in p.edges().enumerate() {
                let e = edge(cycle[i], cycle[(i + 1) % cycle.len()]);
                if creases.contains(&edge(a, b)) {
                    out.creases.push(e);
                }
                let parent = edge(canonical(a), canonical(b));
                if (affected.contains(&parent[0]) || affected.contains(&parent[1]))
                    && open.remove(&e).is_none()
                {
                    open.insert(e, parent);
                }
            }
        }
        if open
            .values()
            .any(|parent| !original_borders.contains(parent))
        {
            return Err("The tunnel seam would leave an open edge".into());
        }
        *self = out;
        Ok(())
    }
}
