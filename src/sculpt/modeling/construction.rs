//! Region construction with explicit individual/connected behavior.
use super::*;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Construction {
    Extrude,
    Inset,
    Bevel,
}

impl Mesh {
    pub fn extrude_model(
        &self,
        selected: &[usize],
        distance: f32,
        join: bool,
        individual: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<Extrusion, String> {
        if !individual {
            match self.extrude_symmetric(selected, distance, join, symmetry) {
                Ok(result) => return Ok(result),
                Err(e) if e.contains("flat region") || e.contains("meet at a corner") => {}
                Err(e) => return Err(e),
            }
        }
        let mesh = self.construct_symmetric(
            selected,
            distance,
            Construction::Extrude,
            individual,
            symmetry,
        )?;
        Ok(Extrusion {
            mesh,
            distance,
            joined: false,
            collapsed: false,
            partial_contact: false,
            varying_distance: false,
            rim: Vec::new(),
        })
    }

    /// Inset a planar region; bevel additionally trims its exterior corner edges.
    pub fn inset_faces(&self, selected: &[usize], width: f32, bevel: bool) -> Result<Mesh, String> {
        manifold(self)?;
        if !width.is_finite() || width < 0. {
            return Err("Width must be nonnegative".into());
        }
        if width < 1e-7 {
            return Ok(self.clone());
        }
        let polygons = self.polygons();
        let selected: BTreeSet<_> = selected.iter().copied().collect();
        let first = selected
            .first()
            .and_then(|&f| polygons.get(f))
            .ok_or("Select faces first")?;
        let n = normal(self, first);
        let origin = self.vertices[first.vertices[0]];
        if selected.iter().any(|&f| {
            polygons.get(f).is_none_or(|p| {
                !p.visible
                    || normal(self, p).dot(n) < 0.9999
                    || p.vertices
                        .iter()
                        .any(|&v| (self.vertices[v] - origin).dot(n).abs() > 1e-5)
            })
        }) {
            return Err("Inset a flat connected region, or choose Individual faces".into());
        }
        let mut boundary = BTreeMap::new();
        let mut vertices = BTreeSet::new();
        for &f in &selected {
            let p = &polygons[f];
            vertices.extend(p.vertices.iter().copied());
            for (a, b) in p.edges() {
                if boundary.remove(&edge(a, b)).is_none() {
                    boundary.insert(edge(a, b), (a, b, p.group));
                }
            }
        }
        if boundary.is_empty() {
            return Err("Choose a region with a boundary".into());
        }
        let mut outgoing = BTreeMap::new();
        let mut incoming = BTreeMap::new();
        for &(a, b, _) in boundary.values() {
            if outgoing.insert(a, b).is_some() || incoming.insert(b, a).is_some() {
                return Err("Region boundaries must not touch or branch".into());
            }
        }
        let mut offset = BTreeMap::new();
        for (&v, &next) in &outgoing {
            let previous = *incoming.get(&v).ok_or("Region boundary is open")?;
            let a = (self.vertices[v] - self.vertices[previous]).norm();
            let b = (self.vertices[next] - self.vertices[v]).norm();
            let inward_a = n.cross(a);
            let inward_b = n.cross(b);
            let bisector = (inward_a + inward_b).norm();
            let denominator = bisector.dot(inward_a);
            if denominator < 1e-4 {
                return Err("Inset cannot cross a sharp or folded corner".into());
            }
            let delta = bisector * (width / denominator);
            if delta.len()
                >= (self.vertices[v] - self.vertices[previous])
                    .len()
                    .min((self.vertices[next] - self.vertices[v]).len())
                    * 0.49
            {
                return Err("Inset is too wide for this region; reduce Width".into());
            }
            offset.insert(v, delta);
        }
        let mut out = self.empty_topology();
        let mapping: BTreeMap<_, _> = vertices
            .iter()
            .map(|&v| {
                (
                    v,
                    out.append_blend(
                        self,
                        &[(v, 1.)],
                        self.vertices[v] + offset.get(&v).copied().unwrap_or_default(),
                    ),
                )
            })
            .collect();
        if bevel {
            for &v in outgoing.keys() {
                let neighbors: Vec<_> = self
                    .polygon_edges()
                    .into_iter()
                    .filter_map(|[a, b]| {
                        if a == v {
                            Some(b)
                        } else if b == v {
                            Some(a)
                        } else {
                            None
                        }
                    })
                    .filter(|w| !vertices.contains(w))
                    .collect();
                if neighbors.len() != 1 {
                    return Err("Bevel needs one exterior edge at each region corner; inset or split the region first".into());
                }
                let d = self.vertices[neighbors[0]] - self.vertices[v];
                if d.dot(n) > -1e-6 || width >= d.len() * 0.49 {
                    return Err(
                        "Bevel needs an exterior corner with room below the selected face".into(),
                    );
                }
                out.vertices[v] = self.vertices[v] + d.norm() * width;
            }
        }
        for (f, p) in polygons.iter().enumerate() {
            let corners = if selected.contains(&f) {
                p.vertices.iter().map(|v| mapping[v]).collect()
            } else {
                p.vertices.clone()
            };
            out.push_polygon(&corners, p.group, p.hidden);
        }
        for &(a, b, group) in boundary.values() {
            out.push_polygon(&[a, b, mapping[&b], mapping[&a]], group, false);
        }
        out.creases = self.creases.clone();
        out.creases.extend(
            self.creases
                .iter()
                .filter_map(|[a, b]| Some(edge(*mapping.get(a)?, *mapping.get(b)?))),
        );
        out.retain_creases();
        manifold(&out)?;
        // Inset must not turn or fold any cap polygon.
        for (&f, p) in selected.iter().zip(selected.iter().map(|&f| &polygons[f])) {
            let cap = Polygon {
                vertices: p.vertices.iter().map(|v| mapping[v]).collect(),
                ..p.clone()
            };
            if normal(&out, &cap).dot(normal(self, &polygons[f])) < 0.999 {
                return Err("Inset would fold the region; reduce Width".into());
            }
        }
        out.compact_model_vertices();
        Ok(out)
    }
    /// Extrude connected faces using averaged vertex normals, retaining cap quads.
    pub fn extrude_region(&self, selected: &[usize], distance: f32) -> Result<Mesh, String> {
        manifold(self)?;
        if !distance.is_finite() {
            return Err("Distance must be finite".into());
        }
        if distance.abs() < 1e-7 {
            return Ok(self.clone());
        }
        let polygons = self.polygons();
        let selected: BTreeSet<_> = selected.iter().copied().collect();
        if selected.is_empty()
            || selected
                .iter()
                .any(|&f| polygons.get(f).is_none_or(|p| !p.visible))
        {
            return Err("Select visible faces".into());
        }
        let mut boundary = BTreeMap::new();
        let mut normals = BTreeMap::<usize, Vec3>::new();
        for &f in &selected {
            let p = &polygons[f];
            let n = normal(self, p);
            for &v in &p.vertices {
                let value = normals.entry(v).or_default();
                *value = *value + n;
            }
            for (a, b) in p.edges() {
                if boundary.remove(&edge(a, b)).is_none() {
                    boundary.insert(edge(a, b), (a, b, p.group));
                }
            }
        }
        if boundary.is_empty() {
            return Err("Extrusion needs a region boundary".into());
        }
        if normals.values().any(|n| n.len() < 1e-6) {
            return Err("Selected normals cancel; choose a smaller region".into());
        }
        let mut out = self.empty_topology();
        let mapping: BTreeMap<_, _> = normals
            .iter()
            .map(|(&v, &n)| {
                (
                    v,
                    out.append_blend(self, &[(v, 1.)], self.vertices[v] + n.norm() * distance),
                )
            })
            .collect();
        for (f, p) in polygons.iter().enumerate() {
            let vertices = if selected.contains(&f) {
                p.vertices.iter().map(|v| mapping[v]).collect()
            } else {
                p.vertices.clone()
            };
            out.push_polygon(&vertices, p.group, p.hidden);
        }
        for &(a, b, g) in boundary.values() {
            out.push_polygon(&[a, b, mapping[&b], mapping[&a]], g, false);
        }
        out.creases = self.creases.clone();
        out.creases.extend(
            self.creases
                .iter()
                .filter_map(|[a, b]| Some(edge(*mapping.get(a)?, *mapping.get(b)?))),
        );
        out.retain_creases();
        manifold(&out)?;
        out.compact_model_vertices();
        Ok(out)
    }
    pub fn construct_symmetric(
        &self,
        selected: &[usize],
        amount: f32,
        kind: Construction,
        individual: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        let polygons = self.polygons();
        let all: BTreeSet<_> = selected
            .iter()
            .flat_map(|&f| symmetry.face_copies(f))
            .collect();
        if all.is_empty() {
            return Err("Select faces first".into());
        }
        if kind == Construction::Extrude && !individual {
            let out = self.extrude_region(&all.into_iter().collect::<Vec<_>>(), amount)?;
            symmetry.validate_result(&out)?;
            return Ok(out);
        }
        let mut pending = all;
        let mut groups = Vec::new();
        while let Some(f) = pending.pop_first() {
            let mut group = vec![f];
            let mut queue = vec![f];
            if !individual {
                while let Some(a) = queue.pop() {
                    let adjacent: Vec<_> = pending
                        .iter()
                        .copied()
                        .filter(|&b| {
                            normal(self, &polygons[f]).dot(normal(self, &polygons[b])) > 0.9999
                                && polygons[a].edges().any(|(u, v)| {
                                    polygons[b].edges().any(|(x, y)| edge(u, v) == edge(x, y))
                                })
                        })
                        .collect();
                    for b in adjacent {
                        pending.remove(&b);
                        group.push(b);
                        queue.push(b);
                    }
                }
            }
            groups.push(group);
        }
        let mut out = self.clone();
        for group in groups {
            let current = out.polygons();
            let indices: Vec<_> = group
                .iter()
                .filter_map(|&f| {
                    current.iter().position(|p| {
                        p.vertices.len() == polygons[f].vertices.len()
                            && polygons[f].vertices.iter().all(|&v| {
                                p.vertices
                                    .iter()
                                    .any(|&w| self.vertices[v] == out.vertices[w])
                            })
                    })
                })
                .collect();
            if indices.len() != group.len() {
                return Err("These construction regions overlap; choose separate regions".into());
            }
            out = if kind == Construction::Extrude {
                out.extrude_region(&indices, amount)?
            } else {
                out.inset_faces(&indices, amount, kind == Construction::Bevel)?
            };
        }
        symmetry.validate_result(&out)?;
        Ok(out)
    }
}
