//! Construction from explicit faces, edges and points. Drafts never mutate the source.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentBuild {
    FacePoint,
    FaceSplit,
    EdgeSplit,
    EdgeCollapse,
    EdgeExtrude,
    PointSplit,
    PointExtrude,
}
#[derive(Clone, Copy, Debug)]
pub struct ComponentBuildOptions {
    pub amount: f32,
    pub width: f32,
    pub round: bool,
    pub triangulate: bool,
    pub crease: bool,
    pub other_side: bool,
}
impl Mesh {
    pub(crate) fn edge_construction_side(
        &self,
        e: [usize; 2],
        anchor: Vec3,
        other: bool,
    ) -> Option<usize> {
        let polygons = self.polygons();
        let key = edge(e[0], e[1]);
        let mut candidates: Vec<_> = polygons
            .iter()
            .enumerate()
            .filter(|(_, p)| p.edges().any(|(a, b)| edge(a, b) == key))
            .map(|(f, _)| f)
            .collect();
        candidates.sort_by(|&a, &b| {
            let distance = |f: usize| {
                (polygons[f]
                    .vertices
                    .iter()
                    .fold(Vec3::default(), |x, &v| x + self.vertices[v])
                    * (1. / polygons[f].vertices.len() as f32)
                    - anchor)
                    .len()
            };
            distance(a).total_cmp(&distance(b))
        });
        candidates
            .get(usize::from(other && candidates.len() > 1))
            .copied()
    }
    // A center fan preserves every boundary segment, including inserted collinear
    // points. Ordinary n-gon triangulation can discard those points and make cracks.
    fn component_polygon(&self, out: &mut Mesh, corners: &[usize], source: &Polygon, fan: bool) {
        if corners.len() <= 4 && !fan {
            out.push_polygon(corners, source.group, source.hidden);
        } else {
            let position = corners
                .iter()
                .fold(Vec3::default(), |p, &v| p + out.vertices[v])
                * (1. / corners.len() as f32);
            let weights: Vec<_> = source
                .vertices
                .iter()
                .map(|&v| (v, 1. / source.vertices.len() as f32))
                .collect();
            let center = out.append_blend(self, &weights, position);
            for i in 0..corners.len() {
                out.push_polygon(
                    &[corners[i], corners[(i + 1) % corners.len()], center],
                    source.group,
                    source.hidden,
                );
            }
        }
    }
    fn finish_components(
        &self,
        mut out: Mesh,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        let valid: BTreeSet<_> = out.polygon_edges().into_iter().collect();
        out.creases.retain(|e| valid.contains(e));
        out.creases.sort_unstable();
        out.creases.dedup();
        out.compact_model_vertices();
        out.canonicalize_polygroups();
        manifold(&out)?;
        ModelingSymmetry::new(&out, &symmetry.settings)?;
        Ok(out)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn build_components(
        &self,
        kind: ComponentBuild,
        faces: &[usize],
        edges: &[[usize; 2]],
        vertices: &[usize],
        anchor: Vec3,
        options: ComponentBuildOptions,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        manifold(self)?;
        if !options.amount.is_finite() || !options.width.is_finite() {
            return Err("Use finite construction amounts".into());
        }
        match kind {
            ComponentBuild::FacePoint | ComponentBuild::FaceSplit => {
                self.build_face_components(kind, faces, symmetry)
            }
            ComponentBuild::EdgeSplit
            | ComponentBuild::EdgeCollapse
            | ComponentBuild::EdgeExtrude => {
                self.build_edge_components(kind, edges, anchor, options, symmetry)
            }
            ComponentBuild::PointSplit | ComponentBuild::PointExtrude => {
                self.build_point_components(kind, vertices, options, symmetry)
            }
        }
    }
    fn build_face_components(
        &self,
        kind: ComponentBuild,
        faces: &[usize],
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        let polygons = self.polygons();
        let selected: BTreeSet<_> = faces
            .iter()
            .flat_map(|&f| symmetry.face_copies(f))
            .collect();
        if selected.is_empty()
            || selected
                .iter()
                .any(|&f| polygons.get(f).is_none_or(|p| !p.visible))
        {
            return Err("Select visible faces".into());
        }
        let mut out = self.empty_topology();
        let mut splits = BTreeMap::new();
        if kind == ComponentBuild::FaceSplit {
            for &f in &selected {
                for (a, b) in polygons[f].edges() {
                    splits.entry(edge(a, b)).or_insert_with(|| {
                        out.append_blend(
                            self,
                            &[(a, 0.5), (b, 0.5)],
                            (self.vertices[a] + self.vertices[b]) * 0.5,
                        )
                    });
                }
            }
        }
        for (f, p) in polygons.iter().enumerate() {
            if selected.contains(&f) {
                let weights: Vec<_> = p
                    .vertices
                    .iter()
                    .map(|&v| (v, 1. / p.vertices.len() as f32))
                    .collect();
                let center = out.append_blend(
                    self,
                    &weights,
                    p.vertices
                        .iter()
                        .fold(Vec3::default(), |x, &v| x + self.vertices[v])
                        * (1. / p.vertices.len() as f32),
                );
                for i in 0..p.vertices.len() {
                    let a = p.vertices[i];
                    let b = p.vertices[(i + 1) % p.vertices.len()];
                    if kind == ComponentBuild::FacePoint {
                        out.push_polygon(&[a, b, center], p.group, p.hidden);
                    } else {
                        let previous = p.vertices[(i + p.vertices.len() - 1) % p.vertices.len()];
                        out.push_polygon(
                            &[a, splits[&edge(a, b)], center, splits[&edge(previous, a)]],
                            p.group,
                            p.hidden,
                        );
                    }
                }
            } else {
                let mut corners = Vec::new();
                for (a, b) in p.edges() {
                    corners.push(a);
                    if let Some(&v) = splits.get(&edge(a, b)) {
                        corners.push(v);
                    }
                }
                self.component_polygon(&mut out, &corners, p, false);
            }
        }
        for &[a, b] in &self.creases {
            if let Some(&m) = splits.get(&edge(a, b)) {
                out.creases.extend([edge(a, m), edge(m, b)]);
            } else {
                out.creases.push(edge(a, b));
            }
        }
        self.finish_components(out, symmetry)
    }
    fn build_edge_components(
        &self,
        kind: ComponentBuild,
        edges: &[[usize; 2]],
        anchor: Vec3,
        o: ComponentBuildOptions,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        let polygons = self.polygons();
        let mut incidence = BTreeMap::<_, Vec<usize>>::new();
        for (f, p) in polygons.iter().enumerate() {
            for (a, b) in p.edges() {
                incidence.entry(edge(a, b)).or_default().push(f);
            }
        }
        let mut pending: BTreeSet<_> = symmetry.edges(edges).into_iter().collect();
        if pending.is_empty() {
            return Err("Select edges first".into());
        }
        let mut ordered = BTreeMap::new();
        let mut sides = BTreeMap::new();
        while let Some(&seed) = pending.iter().min_by(|a, b| {
            let distance = |e: &&[usize; 2]| {
                ((self.vertices[e[0]] + self.vertices[e[1]]) * 0.5 - anchor).len()
            };
            distance(a).total_cmp(&distance(b))
        }) {
            let adjacent = incidence.get(&seed).ok_or("Select current edges")?;
            if adjacent.iter().any(|&f| !polygons[f].visible) {
                return Err("Show the adjacent faces before editing this edge".into());
            }
            let face = self
                .edge_construction_side(seed, anchor, o.other_side)
                .ok_or("Edge has no adjacent face")?;
            let [a, b] = if (self.vertices[seed[0]] - anchor).len()
                <= (self.vertices[seed[1]] - anchor).len()
            {
                seed
            } else {
                [seed[1], seed[0]]
            };
            for (i, map) in symmetry.vertices.iter().enumerate() {
                let oriented = [map[a], map[b]];
                let e = edge(oriented[0], oriented[1]);
                pending.remove(&e);
                if let Some(previous) = ordered.insert(e, oriented)
                    && previous != oriented
                    && kind != ComponentBuild::EdgeExtrude
                    && (o.amount - 0.5).abs() > 1e-6
                {
                    return Err("Symmetry reverses this edge; use its midpoint".into());
                }
                if let Some(previous) = sides.insert(e, symmetry.faces[i][face])
                    && previous != symmetry.faces[i][face]
                    && kind == ComponentBuild::EdgeExtrude
                {
                    return Err(
                        "Symmetry chooses opposite sides of this edge; choose an off-seam edge"
                            .into(),
                    );
                }
            }
        }
        if kind == ComponentBuild::EdgeExtrude {
            return self.extrude_component_edges(&ordered, &sides, &incidence, o.amount, symmetry);
        }
        if !(0. ..=1.).contains(&o.amount)
            || (kind == ComponentBuild::EdgeSplit && !(0.001..=0.999).contains(&o.amount))
        {
            return Err("Use a fraction inside the edge (collapse also permits 0 and 1)".into());
        }
        let mut out = self.empty_topology();
        let mut splits = BTreeMap::new();
        let mut mapping = BTreeMap::new();
        for (&e, &[a, b]) in &ordered {
            if kind == ComponentBuild::EdgeCollapse
                && (mapping.contains_key(&a) || mapping.contains_key(&b))
            {
                return Err(
                    "Collapse non-touching edges, or use Collapse edges for a connected selection"
                        .into(),
                );
            }
            let v = out.append_blend(
                self,
                &[(a, 1. - o.amount), (b, o.amount)],
                self.vertices[a] * (1. - o.amount) + self.vertices[b] * o.amount,
            );
            if kind == ComponentBuild::EdgeSplit {
                splits.insert(e, v);
            } else {
                mapping.insert(a, v);
                mapping.insert(b, v);
            }
        }
        for p in &polygons {
            let mut corners = Vec::new();
            for (a, b) in p.edges() {
                corners.push(*mapping.get(&a).unwrap_or(&a));
                if let Some(&v) = splits.get(&edge(a, b)) {
                    corners.push(v);
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
                return Err("Collapse would pinch a face".into());
            }
            self.component_polygon(&mut out, &corners, p, false);
        }
        for &[a, b] in &self.creases {
            if let Some(&m) = splits.get(&edge(a, b)) {
                out.creases.extend([edge(a, m), edge(m, b)]);
            } else {
                let a = *mapping.get(&a).unwrap_or(&a);
                let b = *mapping.get(&b).unwrap_or(&b);
                if a != b {
                    out.creases.push(edge(a, b));
                }
            }
        }
        self.finish_components(out, symmetry)
    }
    fn extrude_component_edges(
        &self,
        ordered: &BTreeMap<[usize; 2], [usize; 2]>,
        sides: &BTreeMap<[usize; 2], usize>,
        incidence: &BTreeMap<[usize; 2], Vec<usize>>,
        amount: f32,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        if amount.abs() < 1e-7 {
            return Ok(self.clone());
        }
        let polygons = self.polygons();
        let mut out = self.empty_topology();
        let mut modified = BTreeMap::new();
        let mut occupied = BTreeSet::new();
        let mut boundary_offsets = BTreeMap::<usize, (Vec3, usize)>::new();
        for &e in ordered.keys() {
            let f = sides[&e];
            let p = &polygons[f];
            let (a, b) = p.edges().find(|&(a, b)| edge(a, b) == e).unwrap();
            let n = normal(self, p);
            if incidence[&e].len() == 1 {
                let d = (self.vertices[b] - self.vertices[a]).cross(n).norm() * amount;
                for v in [a, b] {
                    let slot = boundary_offsets.entry(v).or_default();
                    slot.0 = slot.0 + d;
                    slot.1 += 1;
                }
            } else {
                if modified.contains_key(&f) || [a, b].iter().any(|v| !occupied.insert(*v)) {
                    return Err("Surface edge extrusions need separate edges and faces".into());
                }
                let aa = out.append_blend(self, &[(a, 1.)], self.vertices[a] + n * amount);
                let bb = out.append_blend(self, &[(b, 1.)], self.vertices[b] + n * amount);
                modified.insert(f, (a, b, aa, bb));
            }
        }
        if boundary_offsets.keys().any(|v| occupied.contains(v)) {
            return Err("Separate boundary and surface edge extrusion targets".into());
        }
        let boundary: BTreeMap<_, _> = boundary_offsets
            .into_iter()
            .map(|(v, (d, count))| {
                (
                    v,
                    out.append_blend(self, &[(v, 1.)], self.vertices[v] + d * (1. / count as f32)),
                )
            })
            .collect();
        for (f, p) in polygons.iter().enumerate() {
            if let Some(&(a, b, aa, bb)) = modified.get(&f) {
                let i = p.vertices.iter().position(|&v| v == a).unwrap();
                let previous = p.vertices[(i + p.vertices.len() - 1) % p.vertices.len()];
                let next = p.vertices[(i + 2) % p.vertices.len()];
                let corners: Vec<_> = p
                    .vertices
                    .iter()
                    .map(|&v| {
                        if v == a {
                            aa
                        } else if v == b {
                            bb
                        } else {
                            v
                        }
                    })
                    .collect();
                out.push_polygon(&corners, p.group, p.hidden);
                out.push_polygon(&[a, b, bb, aa], p.group, false);
                out.push_polygon(&[previous, a, aa], p.group, false);
                out.push_polygon(&[b, next, bb], p.group, false);
            } else {
                out.push_polygon(&p.vertices, p.group, p.hidden);
            }
        }
        for &e in ordered.keys() {
            if incidence[&e].len() == 1 {
                let p = &polygons[sides[&e]];
                let (a, b) = p.edges().find(|&(a, b)| edge(a, b) == e).unwrap();
                out.push_polygon(&[b, a, boundary[&a], boundary[&b]], p.group, false);
            }
        }
        out.creases.extend(&self.creases);
        self.finish_components(out, symmetry)
    }
    fn build_point_components(
        &self,
        kind: ComponentBuild,
        vertices: &[usize],
        o: ComponentBuildOptions,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        let selected: BTreeSet<_> = vertices
            .iter()
            .flat_map(|&v| symmetry.vertex_copies(v))
            .collect();
        if selected.is_empty() || selected.iter().any(|&v| v >= self.vertices.len()) {
            return Err("Select current vertices".into());
        }
        let width = if kind == ComponentBuild::PointSplit {
            o.amount
        } else {
            o.width
        };
        if width <= 1e-7 {
            return Err("Use a positive radius".into());
        }
        if kind == ComponentBuild::PointExtrude && o.amount.abs() < 1e-7 {
            return Ok(self.clone());
        }
        let polygons = self.polygons();
        let mut out = self.empty_topology();
        let mut cut = BTreeMap::new();
        for p in &polygons {
            if p.vertices.iter().filter(|v| selected.contains(v)).count() > 1 {
                return Err(
                    "Point construction needs vertices with separate face neighborhoods".into(),
                );
            }
        }
        for &v in &selected {
            let mut next = BTreeMap::new();
            let mut n = Vec3::default();
            for p in &polygons {
                if let Some(i) = p.vertices.iter().position(|&x| x == v) {
                    if !p.visible {
                        return Err("Show all faces around the point before constructing".into());
                    }
                    let before = p.vertices[(i + p.vertices.len() - 1) % p.vertices.len()];
                    let after = p.vertices[(i + 1) % p.vertices.len()];
                    // Cap direction is the reverse of each new corner boundary.
                    if next.insert(after, before).is_some() {
                        return Err("The point neighborhood branches".into());
                    }
                    n = n + normal(self, p);
                }
            }
            if next.len() < 3 || next.values().any(|v| !next.contains_key(v)) {
                return Err("Point construction needs a closed surface neighborhood".into());
            }
            let start = *next.keys().next().unwrap();
            let mut ring = Vec::new();
            let mut current = start;
            loop {
                ring.push(current);
                current = *next.get(&current).ok_or("Open point neighborhood")?;
                if current == start {
                    break;
                }
                if ring.len() >= next.len() {
                    return Err("Point neighborhood is disconnected".into());
                }
            }
            if ring.len() != next.len() || n.len() < 1e-6 {
                return Err("Point needs one surface fan with a stable normal".into());
            }
            n = n.norm();
            let shortest = ring
                .iter()
                .map(|&w| (self.vertices[w] - self.vertices[v]).len())
                .fold(f32::INFINITY, f32::min);
            if width >= shortest * 0.49 {
                return Err("Radius must be less than half the shortest connected edge".into());
            }
            let mut positions: Vec<_> = ring
                .iter()
                .map(|&w| self.vertices[v] + (self.vertices[w] - self.vertices[v]).norm() * width)
                .collect();
            if o.round {
                let center = positions.iter().fold(Vec3::default(), |p, &v| p + v)
                    * (1. / positions.len() as f32);
                let radius = positions.iter().map(|&p| (p - center).len()).sum::<f32>()
                    / positions.len() as f32;
                for p in &mut positions {
                    let d = *p - center;
                    let planar = d - n * d.dot(n);
                    if planar.len() < 1e-6 {
                        return Err("This ring cannot be circularized".into());
                    }
                    *p = center + planar.norm() * radius;
                }
            }
            let mut cap = Vec::new();
            for (&w, &position) in ring.iter().zip(&positions) {
                let t = width / (self.vertices[w] - self.vertices[v]).len();
                let id = out.append_blend(self, &[(v, 1. - t), (w, t)], position);
                cut.insert((v, w), id);
                cap.push(id);
            }
            let source = polygons.iter().find(|p| p.vertices.contains(&v)).unwrap();
            // A mixed-group junction gets an ungrouped cap rather than an arbitrary side's label.
            let mut cap_source = source.clone();
            if polygons
                .iter()
                .filter(|p| p.vertices.contains(&v))
                .any(|p| p.group != source.group)
            {
                cap_source.group = 0;
            }
            cap_source.vertices = vec![v];
            if kind == ComponentBuild::PointExtrude {
                let tip = out.append_blend(self, &[(v, 1.)], self.vertices[v] + n * o.amount);
                for i in 0..cap.len() {
                    out.push_polygon(
                        &[cap[i], cap[(i + 1) % cap.len()], tip],
                        cap_source.group,
                        false,
                    );
                }
            } else {
                self.component_polygon(&mut out, &cap, &cap_source, o.triangulate);
            }
            if o.crease {
                for i in 0..cap.len() {
                    out.creases.push(edge(cap[i], cap[(i + 1) % cap.len()]));
                }
            }
        }
        for p in &polygons {
            let mut corners = Vec::new();
            for i in 0..p.vertices.len() {
                let v = p.vertices[i];
                if selected.contains(&v) {
                    let previous = p.vertices[(i + p.vertices.len() - 1) % p.vertices.len()];
                    let next = p.vertices[(i + 1) % p.vertices.len()];
                    corners.extend([cut[&(v, previous)], cut[&(v, next)]]);
                } else {
                    corners.push(v);
                }
            }
            self.component_polygon(&mut out, &corners, p, false);
        }
        for &[a, b] in &self.creases {
            let aa = *cut.get(&(a, b)).unwrap_or(&a);
            let bb = *cut.get(&(b, a)).unwrap_or(&b);
            out.creases.push(edge(aa, bb));
        }
        self.finish_components(out, symmetry)
    }
}
