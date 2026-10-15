//! Explicit topology operations. Each builds a separate mesh and validates before return.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TopologyEdit {
    DeleteFaces,
    Fill,
    Bridge,
    Connect,
    Split,
    Weld,
    Collapse,
}
impl TopologyEdit {
    pub const ALL: [Self; 7] = [
        Self::DeleteFaces,
        Self::Fill,
        Self::Bridge,
        Self::Connect,
        Self::Split,
        Self::Weld,
        Self::Collapse,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::DeleteFaces => "Delete faces",
            Self::Fill => "Fill boundary",
            Self::Bridge => "Bridge boundaries",
            Self::Connect => "Connect vertices",
            Self::Split => "Split edges",
            Self::Weld => "Weld vertices",
            Self::Collapse => "Collapse edges",
        }
    }
}
impl Mesh {
    fn push_model_polygon(
        &mut self,
        vertices: &[usize],
        group: usize,
        hidden: bool,
    ) -> Result<(), String> {
        if vertices.len() <= 4 {
            self.push_polygon(vertices, group, hidden);
            return Ok(());
        }
        let p = Polygon {
            vertices: vertices.to_vec(),
            group,
            hidden,
            visible: !hidden,
        };
        let n = normal(self, &p);
        let u = (self.vertices[vertices[1]] - self.vertices[vertices[0]]).norm();
        let v = n.cross(u);
        let coords: Vec<_> = vertices
            .iter()
            .flat_map(|&i| {
                [
                    self.vertices[i].dot(u) as f64,
                    self.vertices[i].dot(v) as f64,
                ]
            })
            .collect();
        let indices =
            earcutr::earcut(&coords, &[], 2).map_err(|_| "Could not triangulate this face")?;
        if indices.len() != (vertices.len() - 2) * 3 {
            return Err("This polygon cannot be triangulated safely".into());
        }
        for tri in indices.as_chunks::<3>().0 {
            self.push_polygon(
                &[vertices[tri[0]], vertices[tri[1]], vertices[tri[2]]],
                group,
                hidden,
            );
        }
        Ok(())
    }
    fn selected_boundaries(&self, selected: &[[usize; 2]]) -> Result<Vec<Vec<usize>>, String> {
        let selected: BTreeSet<_> = selected.iter().map(|e| edge(e[0], e[1])).collect();
        if selected.is_empty() {
            return Err("Select complete open boundaries".into());
        }
        let mut incidence = BTreeMap::<_, Vec<(usize, usize, bool)>>::new();
        for p in self.polygons() {
            for (a, b) in p.edges() {
                incidence
                    .entry(edge(a, b))
                    .or_default()
                    .push((a, b, p.visible));
            }
        }
        let mut next = BTreeMap::new();
        let mut previous = BTreeSet::new();
        for e in selected {
            let fs = incidence.get(&e).ok_or("Boundary selection is stale")?;
            if fs.len() != 1 || !fs[0].2 {
                return Err("Select only visible open boundary edges".into());
            }
            let (a, b, _) = fs[0];
            if next.insert(a, b).is_some() || !previous.insert(b) {
                return Err("Boundary branches at a vertex".into());
            }
        }
        let mut loops = Vec::new();
        while let Some((&start, _)) = next.first_key_value() {
            let mut cycle = Vec::new();
            let mut v = start;
            loop {
                cycle.push(v);
                v = next.remove(&v).ok_or("Select the complete boundary loop")?;
                if v == start {
                    break;
                }
                if cycle.len() > self.vertices.len() {
                    return Err("Invalid boundary cycle".into());
                }
            }
            if cycle.len() < 3 {
                return Err("Boundary needs at least three vertices".into());
            }
            loops.push(cycle);
        }
        Ok(loops)
    }
    pub fn edit_model_topology(
        &self,
        edit: TopologyEdit,
        faces: &[usize],
        edges: &[[usize; 2]],
        vertices: &[usize],
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        manifold(self)?;
        let polygons = self.polygons();
        let mut out = self.empty_topology();
        let mut removed = BTreeSet::new();
        let mut mapping: BTreeMap<usize, usize> = BTreeMap::new();
        let mut split = BTreeMap::new();
        let mut diagonals = BTreeMap::new();
        let pair = || -> Result<[usize; 2], String> {
            let wanted: BTreeSet<_> = vertices.iter().copied().collect();
            if wanted.len() == 2 {
                return Ok([vertices[0], vertices[1]]);
            }
            let mut candidates = Vec::new();
            for &a in vertices {
                for &b in vertices {
                    if a >= b {
                        continue;
                    }
                    let copies: BTreeSet<_> =
                        symmetry.edge_copies([a, b]).into_iter().flatten().collect();
                    if copies == wanted
                        && (edit != TopologyEdit::Connect
                            || polygons.iter().any(|p| {
                                p.vertices.len() == 4
                                    && p.vertices.contains(&a)
                                    && p.vertices.contains(&b)
                                    && !p.edges().any(|(u, v)| edge(u, v) == edge(a, b))
                            }))
                    {
                        candidates.push([a, b]);
                    }
                }
            }
            candidates
                .into_iter()
                .min_by(|a, b| {
                    (self.vertices[a[0]] - self.vertices[a[1]])
                        .len()
                        .total_cmp(&(self.vertices[b[0]] - self.vertices[b[1]]).len())
                })
                .ok_or_else(|| "Select two vertices and their symmetry copies".into())
        };
        match edit {
            TopologyEdit::DeleteFaces => {
                removed = faces
                    .iter()
                    .flat_map(|&f| symmetry.face_copies(f))
                    .collect();
                if removed.is_empty() {
                    return Err("Select faces to delete".into());
                }
                if removed
                    .iter()
                    .any(|&f| polygons.get(f).is_none_or(|p| !p.visible))
                {
                    return Err("Select visible faces".into());
                }
            }
            TopologyEdit::Split => {
                let edges = symmetry.edges(edges);
                if edges.is_empty() {
                    return Err("Select edges to split".into());
                }
                let valid: BTreeSet<_> = self.polygon_edges().into_iter().collect();
                for e in edges {
                    if !valid.contains(&e) {
                        return Err("Select current edges".into());
                    }
                    let p = (self.vertices[e[0]] + self.vertices[e[1]]) * 0.5;
                    split.insert(e, out.append_blend(self, &[(e[0], 0.5), (e[1], 0.5)], p));
                }
            }
            TopologyEdit::Connect => {
                for [a, b] in symmetry.edge_copies(pair()?) {
                    let f = polygons
                        .iter()
                        .position(|p| {
                            p.visible
                                && p.vertices.len() == 4
                                && p.vertices.contains(&a)
                                && p.vertices.contains(&b)
                                && !p.edges().any(|(u, v)| edge(u, v) == edge(a, b))
                        })
                        .ok_or("Connect requires opposite corners of a quad")?;
                    if let Some(old) = diagonals.insert(f, edge(a, b))
                        && old != edge(a, b)
                    {
                        return Err("Symmetry would create crossing diagonals in one face".into());
                    }
                }
            }
            TopologyEdit::Weld | TopologyEdit::Collapse => {
                let pairs = if edit == TopologyEdit::Weld {
                    symmetry.edges(&[pair()?])
                } else {
                    symmetry.edges(edges)
                };
                if pairs.is_empty() {
                    return Err("Select edges to collapse".into());
                }
                let mut groups: Vec<BTreeSet<usize>> = Vec::new();
                for [a, b] in pairs {
                    if a >= self.vertices.len() || b >= self.vertices.len() {
                        return Err("Stale vertex selection".into());
                    }
                    let mut group = BTreeSet::from([a, b]);
                    let mut i = 0;
                    while i < groups.len() {
                        if !groups[i].is_disjoint(&group) {
                            group.extend(groups.remove(i));
                        } else {
                            i += 1;
                        }
                    }
                    groups.push(group);
                }
                for group in groups {
                    let weight = 1. / group.len() as f32;
                    let weights: Vec<_> = group.iter().map(|&v| (v, weight)).collect();
                    let p = group
                        .iter()
                        .fold(Vec3::default(), |p, &v| p + self.vertices[v])
                        * weight;
                    let new = out.append_blend(self, &weights, p);
                    for v in group {
                        mapping.insert(v, new);
                    }
                }
            }
            TopologyEdit::Fill | TopologyEdit::Bridge => {}
        }
        for (f, p) in polygons.iter().enumerate() {
            if removed.contains(&f) {
                continue;
            }
            if let Some(&[a, b]) = diagonals.get(&f) {
                let index = p.vertices.iter().position(|&v| v == a).unwrap();
                let v = &p.vertices;
                out.push_polygon(&[a, v[(index + 1) % 4], b], p.group, p.hidden);
                out.push_polygon(&[a, b, v[(index + 3) % 4]], p.group, p.hidden);
                continue;
            }
            let mut corners = Vec::new();
            for (a, b) in p.edges() {
                corners.push(*mapping.get(&a).unwrap_or(&a));
                if let Some(&m) = split.get(&edge(a, b)) {
                    corners.push(m);
                }
            }
            corners.dedup();
            if corners.first() == corners.last() {
                corners.pop();
            }
            if corners.len() < 3 {
                continue;
            }
            if corners.iter().copied().collect::<BTreeSet<_>>().len() != corners.len() {
                return Err("Weld would pinch a polygon; choose adjacent vertices".into());
            }
            out.push_model_polygon(&corners, p.group, p.hidden)?;
        }
        if matches!(edit, TopologyEdit::Fill | TopologyEdit::Bridge) {
            let loops = self.selected_boundaries(&symmetry.edges(edges))?;
            if edit == TopologyEdit::Fill {
                for mut cycle in loops {
                    cycle.reverse();
                    let p = Polygon {
                        vertices: cycle.clone(),
                        group: 0,
                        hidden: false,
                        visible: true,
                    };
                    let n = normal(self, &p);
                    let origin = self.vertices[cycle[0]];
                    if n.len() < 0.9
                        || cycle
                            .iter()
                            .any(|&v| (self.vertices[v] - origin).dot(n).abs() > 1e-5)
                    {
                        return Err("Fill requires a planar boundary".into());
                    }
                    out.push_model_polygon(&cycle, 0, false)?;
                }
            } else {
                let keys: Vec<BTreeSet<_>> = loops
                    .iter()
                    .map(|cycle| cycle.iter().copied().collect())
                    .collect();
                let mut choices = Vec::new();
                for j in 1..loops.len() {
                    let mut pairs = BTreeSet::new();
                    let mut valid = true;
                    for map in &symmetry.vertices {
                        let locate = |cycle: &Vec<usize>| {
                            let key: BTreeSet<_> = cycle.iter().map(|&v| map[v]).collect();
                            keys.iter().position(|k| *k == key)
                        };
                        if let (Some(a), Some(b)) = (locate(&loops[0]), locate(&loops[j])) {
                            if a == b || loops[a].len() != loops[b].len() {
                                valid = false;
                                break;
                            }
                            pairs.insert(edge(a, b));
                        } else {
                            valid = false;
                            break;
                        }
                    }
                    let used: Vec<_> = pairs.iter().flatten().copied().collect();
                    if !valid
                        || used.len() != loops.len()
                        || used.iter().copied().collect::<BTreeSet<_>>().len() != loops.len()
                    {
                        continue;
                    }
                    let center = |i: usize| {
                        loops[i]
                            .iter()
                            .fold(Vec3::default(), |p, &v| p + self.vertices[v])
                            * (1. / loops[i].len() as f32)
                    };
                    let cost = pairs
                        .iter()
                        .map(|&[a, b]| (center(a) - center(b)).len())
                        .sum::<f32>();
                    choices.push((cost, pairs));
                }
                let pairs=choices.into_iter().min_by(|a,b|a.0.total_cmp(&b.0)).ok_or("Bridge requires two equal-count boundaries and their matching symmetry copies")?.1;
                for [first, second] in pairs {
                    let a = &loops[first];
                    let mut b = loops[second].clone();
                    b.reverse();
                    let count = a.len();
                    let shift = (0..count)
                        .min_by(|&i, &j| {
                            let cost = |s: usize| {
                                (0..count)
                                    .map(|k| {
                                        (self.vertices[a[k]] - self.vertices[b[(k + s) % count]])
                                            .len()
                                    })
                                    .sum::<f32>()
                            };
                            cost(i).total_cmp(&cost(j))
                        })
                        .unwrap();
                    for i in 0..count {
                        out.push_polygon(
                            &[
                                a[(i + 1) % count],
                                a[i],
                                b[(i + shift) % count],
                                b[(i + 1 + shift) % count],
                            ],
                            0,
                            false,
                        );
                    }
                }
            }
        }
        if out.triangles.is_empty() {
            return Err("This would remove the entire mesh".into());
        }
        out.creases = self
            .creases
            .iter()
            .flat_map(|&[a, b]| {
                if let Some(&m) = split.get(&edge(a, b)) {
                    vec![edge(a, m), edge(m, b)]
                } else {
                    vec![edge(
                        *mapping.get(&a).unwrap_or(&a),
                        *mapping.get(&b).unwrap_or(&b),
                    )]
                }
            })
            .collect();
        out.retain_creases();
        manifold(&out)?;
        out.compact_model_vertices();
        symmetry.validate_result(&out)?;
        Ok(out)
    }
    pub fn slide_model_edges(
        &self,
        selected: &[[usize; 2]],
        amount: f32,
        axis: Vec3,
        anchor: Vec3,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        if !amount.is_finite() || amount.abs() >= 1. {
            return Err("Slide fraction must stay between -1 and 1".into());
        }
        let selected: BTreeSet<_> = symmetry.edges(selected).into_iter().collect();
        if selected.is_empty() {
            return Err("Select an interior edge loop to slide".into());
        }
        let expanded: BTreeSet<_> = self
            .expand_edge_loop(&selected.iter().copied().collect::<Vec<_>>())?
            .into_iter()
            .collect();
        if expanded != selected {
            return Err("Select the complete loop before sliding".into());
        }
        let polygons = self.polygons();
        let vertices: BTreeSet<_> = selected.iter().flatten().copied().collect();
        let mut rails = BTreeMap::<usize, usize>::new();
        let mut pending = selected.clone();
        while let Some(&seed) = pending.first() {
            let mut queue = vec![seed];
            while let Some(e) = queue.pop() {
                if !pending.remove(&e) {
                    continue;
                }
                let incident: Vec<_> = polygons
                    .iter()
                    .filter(|p| p.edges().any(|(a, b)| edge(a, b) == e))
                    .collect();
                if incident.len() != 2
                    || incident.iter().any(|p| !p.visible || p.vertices.len() != 4)
                {
                    return Err("Slide requires an interior quad loop".into());
                }
                let neighbor = |p: &Polygon, v: usize| {
                    p.edges()
                        .find_map(|(a, b)| {
                            if a == v && !e.contains(&b) {
                                Some(b)
                            } else if b == v && !e.contains(&a) {
                                Some(a)
                            } else {
                                None
                            }
                        })
                        .unwrap()
                };
                let face = incident
                    .iter()
                    .copied()
                    .filter(|p| {
                        e.iter()
                            .all(|v| rails.get(v).is_none_or(|&w| w == neighbor(p, *v)))
                    })
                    .max_by(|a, b| {
                        let direction = |p: &Polygon| {
                            (self.vertices[neighbor(p, e[0])] - self.vertices[e[0]]).dot(axis)
                        };
                        direction(a).total_cmp(&direction(b))
                    })
                    .ok_or("The selected loop twists or branches")?;
                for &v in &e {
                    let w = neighbor(face, v);
                    if vertices.contains(&w) {
                        return Err(
                            "Slide separate loops; selected loops must not share a face".into()
                        );
                    }
                    rails.insert(v, w);
                }
                queue.extend(
                    pending
                        .iter()
                        .filter(|other| other.iter().any(|v| e.contains(v)))
                        .copied(),
                );
            }
        }
        let mut out = self.clone();
        for (&v, &positive) in &rails {
            let neighbor = if amount >= 0. {
                positive
            } else {
                let candidates: BTreeSet<_> = self
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
                    .filter(|w| !vertices.contains(w) && *w != positive)
                    .collect();
                if candidates.len() != 1 {
                    return Err("Slide needs two unambiguous rails at every vertex".into());
                }
                *candidates.first().unwrap()
            };
            out.vertices[v] =
                self.vertices[v] + (self.vertices[neighbor] - self.vertices[v]) * amount.abs();
        }
        let provisional = out.vertices.clone();
        let mut visited = BTreeSet::new();
        for &v in &vertices {
            if visited.contains(&v) {
                continue;
            }
            let copies = symmetry.vertex_copies(v);
            let seed = *copies
                .iter()
                .min_by(|&&a, &&b| {
                    (self.vertices[a] - anchor)
                        .len()
                        .total_cmp(&(self.vertices[b] - anchor).len())
                })
                .unwrap();
            visited.extend(copies);
            let delta = provisional[seed] - self.vertices[seed];
            let mut moves = BTreeMap::<usize, (Vec3, usize)>::new();
            for (map, basis) in symmetry.vertices.iter().zip(&symmetry.bases) {
                let d = basis[0] * delta.0 + basis[1] * delta.1 + basis[2] * delta.2;
                let e = moves.entry(map[seed]).or_default();
                e.0 = e.0 + d;
                e.1 += 1;
            }
            for (v, (delta, n)) in moves {
                out.vertices[v] = self.vertices[v] + delta * (1. / n as f32);
            }
        }
        manifold(&out)?;
        symmetry.validate_result(&out)?;
        Ok(out)
    }
}
