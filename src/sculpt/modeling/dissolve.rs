//! Remove complete, nonbranching quad loops without leaving holes or T junctions.
use super::*;
impl Mesh {
    pub fn dissolve_loops(&self, selected: &[[usize; 2]]) -> Result<Mesh, String> {
        manifold(self)?;
        let selected: BTreeSet<_> = selected.iter().map(|e| edge(e[0], e[1])).collect();
        if selected.is_empty() {
            return Err("Select complete edge loops to dissolve".into());
        }
        let polygons = self.polygons();
        let mut incidence = BTreeMap::<_, Vec<_>>::new();
        let mut at_vertex = BTreeMap::<_, BTreeSet<_>>::new();
        for (f, p) in polygons.iter().enumerate() {
            for (a, b) in p.edges() {
                let e = edge(a, b);
                incidence.entry(e).or_default().push(f);
                for v in e {
                    at_vertex.entry(v).or_default().insert(e);
                }
            }
        }
        let mut degree = BTreeMap::<usize, usize>::new();
        let mut neighbors = vec![Vec::new(); polygons.len()];
        for e in &selected {
            let fs = incidence
                .get(e)
                .ok_or("The selected edge no longer exists")?;
            if fs.len() != 2 {
                return Err("Dissolve interior loops; border edges would open the surface".into());
            }
            if fs
                .iter()
                .any(|&f| !polygons[f].visible || polygons[f].vertices.len() != 4)
            {
                return Err("Dissolve loops needs visible quads on both sides".into());
            }
            for &v in e {
                *degree.entry(v).or_default() += 1;
            }
            neighbors[fs[0]].push(fs[1]);
            neighbors[fs[1]].push(fs[0]);
        }
        for (&v, &n) in &degree {
            let edges = &at_vertex[&v];
            let borders = edges.iter().filter(|e| incidence[*e].len() == 1).count();
            if !((n == 2 && edges.len() == 4 && borders == 0)
                || (n == 1 && edges.len() == 3 && borders == 2))
            {
                return Err(
                    "Select complete, noncrossing loops; open loops must end at the mesh border"
                        .into(),
                );
            }
            if edges
                .iter()
                .flat_map(|e| &incidence[e])
                .any(|&f| !polygons[f].visible || polygons[f].vertices.len() != 4)
            {
                return Err("Show all adjoining faces and use quad loops before dissolving".into());
            }
        }
        let mut out = self.empty_topology();
        let mut visited = BTreeSet::new();
        for first in 0..polygons.len() {
            if !visited.insert(first) {
                continue;
            }
            let mut patch = vec![first];
            let mut pending = vec![first];
            while let Some(f) = pending.pop() {
                for &next in &neighbors[f] {
                    if visited.insert(next) {
                        patch.push(next);
                        pending.push(next);
                    }
                }
            }
            if patch.len() == 1 {
                let p = &polygons[first];
                out.push_polygon(&p.vertices, p.group, p.hidden);
                continue;
            }
            let group = polygons[first].group;
            if patch.iter().any(|&f| polygons[f].group != group) {
                return Err("Dissolving this loop would merge different face-group labels".into());
            }
            let mut next = BTreeMap::new();
            for &f in &patch {
                for (a, b) in polygons[f].edges() {
                    if !selected.contains(&edge(a, b)) && next.insert(a, b).is_some() {
                        return Err(
                            "The selected loops cross or surround an ambiguous region".into()
                        );
                    }
                }
            }
            let &start = next
                .keys()
                .next()
                .ok_or("These loops would remove an entire surface")?;
            let mut boundary = vec![start];
            let mut v = next[&start];
            while v != start {
                if boundary.contains(&v) {
                    return Err("The dissolved boundary branches".into());
                }
                boundary.push(v);
                v = *next.get(&v).ok_or("The dissolved boundary is incomplete")?;
            }
            if boundary.len() != next.len() {
                return Err(
                    "The selected loops surround a hole; dissolve fewer loops at once".into(),
                );
            }
            let corners: Vec<_> = boundary
                .iter()
                .copied()
                .filter(|v| !degree.contains_key(v))
                .collect();
            if corners.len() != 4 {
                return Err("These loops cannot be dissolved into regular quads".into());
            }
            // Follow each old boundary path to carry its crease onto the new edge.
            for i in 0..corners.len() {
                let a = corners[i];
                let b = corners[(i + 1) % corners.len()];
                let mut v = a;
                let mut creased = false;
                while v != b {
                    let w = next[&v];
                    creased |= self.creases.binary_search(&edge(v, w)).is_ok();
                    v = w;
                }
                if creased {
                    out.creases.push(edge(a, b));
                }
            }
            out.push_polygon(&corners, group, false);
        }
        out.creases.extend(
            self.creases
                .iter()
                .copied()
                .filter(|e| !degree.contains_key(&e[0]) && !degree.contains_key(&e[1])),
        );
        out.retain_creases();
        out.compact_model_vertices();
        manifold(&out)?;
        // Every old boundary stays a boundary; closed input must remain closed.
        if incidence.values().all(|fs| fs.len() == 2) {
            out.validate_closed()?;
        }
        Ok(out)
    }
}
