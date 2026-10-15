use super::*;

impl Mesh {
    pub(super) fn support_loops(
        &self,
        target: BorderTarget,
        threshold: f32,
        selected: &[Edge],
        count: u32,
        width: f32,
        cancel: &impl Fn() -> bool,
    ) -> Result<Self, String> {
        self.validate()?;
        if !(1..=8).contains(&count) || !width.is_finite() || !(0.01..=0.45).contains(&width) {
            return Err("Use 1–8 loops and a width from 1% to 45%".into());
        }
        let polygons = self.polygons();
        let mut region = regions(&polygons, self, target, threshold);
        let selected: BTreeSet<_> = selected.iter().copied().collect();
        if target == BorderTarget::Selected {
            // A closed selected edge chain separates polygon regions.
            let mut across: BTreeMap<Edge, Vec<usize>> = BTreeMap::new();
            for (i, p) in polygons.iter().enumerate() {
                if p.visible {
                    for (a, b) in p.edges() {
                        across.entry(edge(a, b)).or_default().push(i);
                    }
                }
            }
            let mut neighbors = vec![Vec::new(); polygons.len()];
            for (e, fs) in across {
                if !selected.contains(&e) && fs.len() == 2 {
                    neighbors[fs[0]].push(fs[1]);
                    neighbors[fs[1]].push(fs[0]);
                }
            }
            region.fill(None);
            for i in 0..polygons.len() {
                if polygons[i].visible && region[i].is_none() {
                    region[i] = Some(i);
                    let mut pending = vec![i];
                    while let Some(f) = pending.pop() {
                        for &next in &neighbors[f] {
                            if region[next].is_none() {
                                region[next] = Some(i);
                                pending.push(next);
                            }
                        }
                    }
                }
            }
        }
        let mut borders = boundaries(
            &polygons,
            &region,
            if target == BorderTarget::Selected {
                BorderTarget::VisibleGroups
            } else {
                target
            },
        )?;
        if target == BorderTarget::Selected {
            for edges in borders.values_mut() {
                edges.retain(|&(a, b, _)| selected.contains(&edge(a, b)));
            }
        }
        if borders.values().all(Vec::is_empty) {
            return Err(
                "No closed borders match this target; select a complete separating loop".into(),
            );
        }
        let added: usize = borders.values().map(|b| b.len() * count as usize).sum();
        if self.vertices.len() + added > MAX_VERTICES
            || self.triangles.len() + added * 2 > MAX_TRIANGLES
        {
            return Err("Support loops exceed the mesh size limit".into());
        }
        let hidden_vertices: BTreeSet<_> = polygons
            .iter()
            .filter(|p| !p.visible)
            .flat_map(|p| p.vertices.iter().copied())
            .collect();
        let mut adjacent: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (i, p) in polygons.iter().enumerate() {
            if let Some(r) = region[i] {
                for &v in &p.vertices {
                    adjacent.entry((r, v)).or_default().push(i);
                }
            }
        }
        let mut out = self.empty_topology();
        let mut inset: BTreeMap<(usize, usize), Vec<usize>> = BTreeMap::new();
        for (&r, border) in &borders {
            cancelled(cancel)?;
            let mut degree: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
            for &(a, b, _) in border {
                degree.entry(a).or_default().0 += 1;
                degree.entry(b).or_default().1 += 1;
            }
            if degree.values().any(|&d| d != (1, 1)) {
                return Err(
                    "Support loops need closed, nonbranching borders; simplify the selection first"
                        .into(),
                );
            }
            let previous: BTreeMap<_, _> = border.iter().map(|&(a, b, _)| (b, a)).collect();
            let next: BTreeMap<_, _> = border.iter().map(|&(a, b, _)| (a, b)).collect();
            for &v in degree.keys() {
                if hidden_vertices.contains(&v)
                    && matches!(target, BorderTarget::Holes | BorderTarget::Selected)
                {
                    return Err(
                        "Show faces touching this border before adding support loops".into(),
                    );
                }
                cancelled(cancel)?;
                let faces = &adjacent[&(r, v)];
                let mut normal = Vec3::default();
                for &f in faces {
                    let p = &polygons[f];
                    for j in 1..p.vertices.len() - 1 {
                        normal = normal
                            + (self.vertices[p.vertices[j]] - self.vertices[p.vertices[0]]).cross(
                                self.vertices[p.vertices[j + 1]] - self.vertices[p.vertices[0]],
                            );
                    }
                }
                let normal = normal.norm();
                let incoming = self.vertices[v] - self.vertices[previous[&v]];
                let outgoing = self.vertices[next[&v]] - self.vertices[v];
                let left = normal.cross(incoming.norm());
                let right = normal.cross(outgoing.norm());
                let denominator = 1. + left.dot(right);
                if denominator < 0.01 {
                    return Err(
                        "This border turns too sharply for support loops; relax it first".into(),
                    );
                }
                let offset =
                    (left + right) * (incoming.len().min(outgoing.len()) * width / denominator);
                let mut indices = vec![v];
                for n in 1..=count {
                    let target_point = self.vertices[v] + offset * (n as f32 / count as f32);
                    let mut closest = None;
                    for &f in faces {
                        let p = &polygons[f];
                        for j in 1..p.vertices.len() - 1 {
                            let ids = [p.vertices[0], p.vertices[j], p.vertices[j + 1]];
                            let [a, b, c] = ids.map(|i| self.vertices[i]);
                            let point = super::super::math::closest_triangle(target_point, a, b, c);
                            let ab = b - a;
                            let ac = c - a;
                            let ap = point - a;
                            let denominator = ab.dot(ab) * ac.dot(ac) - ab.dot(ac).powi(2);
                            if denominator.abs() < 1e-20 {
                                continue;
                            }
                            let bw = ((ac.dot(ac) * ap.dot(ab) - ab.dot(ac) * ap.dot(ac))
                                / denominator)
                                .clamp(0., 1.);
                            let cw = ((ab.dot(ab) * ap.dot(ac) - ab.dot(ac) * ap.dot(ab))
                                / denominator)
                                .clamp(0., 1. - bw);
                            let distance = (point - target_point).len();
                            if closest.as_ref().is_none_or(|(d, _, _)| distance < *d) {
                                closest = Some((
                                    distance,
                                    point,
                                    [(ids[0], 1. - bw - cw), (ids[1], bw), (ids[2], cw)],
                                ));
                            }
                        }
                    }
                    let (_, point, weights) =
                        closest.ok_or("This border has no room for support loops")?;
                    if (point - self.vertices[v]).len() < 1e-9 {
                        return Err("This border has no room for support loops".into());
                    }
                    indices.push(out.append_blend(self, &weights, point));
                }
                inset.insert((r, v), indices);
            }
        }
        let mut creases = BTreeSet::new();
        for (i, p) in polygons.iter().enumerate() {
            if i % 1024 == 0 {
                cancelled(cancel)?;
            }
            let map = |v| {
                region[i]
                    .and_then(|r| inset.get(&(r, v)))
                    .map_or(v, |ids| *ids.last().unwrap())
            };
            let vertices: Vec<_> = p.vertices.iter().copied().map(map).collect();
            for (a, b) in p.edges() {
                if self.creases.binary_search(&edge(a, b)).is_ok() {
                    creases.insert(edge(map(a), map(b)));
                }
            }
            for j in 1..vertices.len() - 1 {
                let normal = (self.vertices[p.vertices[j]] - self.vertices[p.vertices[0]])
                    .cross(self.vertices[p.vertices[j + 1]] - self.vertices[p.vertices[0]]);
                let new = (out.vertices[vertices[j]] - out.vertices[vertices[0]])
                    .cross(out.vertices[vertices[j + 1]] - out.vertices[vertices[0]]);
                if normal.dot(new) <= 0. {
                    return Err(
                        "Support loops would fold a face; reduce width or clean the border first"
                            .into(),
                    );
                }
            }
            out.push_polygon(&vertices, p.group, p.hidden);
        }
        for (r, border) in borders {
            for (a, b, f) in border {
                let aa = &inset[&(r, a)];
                let bb = &inset[&(r, b)];
                for n in 0..count as usize {
                    let q = [aa[n], bb[n], bb[n + 1], aa[n + 1]];
                    let p = &polygons[f].vertices;
                    let normal = (self.vertices[p[1]] - self.vertices[p[0]])
                        .cross(self.vertices[p[2]] - self.vertices[p[0]]);
                    for [a, b, c] in [[q[0], q[1], q[2]], [q[0], q[2], q[3]]] {
                        if (out.vertices[b] - out.vertices[a])
                            .cross(out.vertices[c] - out.vertices[a])
                            .dot(normal)
                            <= 0.
                        {
                            return Err("Support loops would fold a border; reduce width or relax the border first".into());
                        }
                    }
                    out.push_polygon(&q, polygons[f].group, false);
                }
                if self.creases.binary_search(&edge(a, b)).is_ok() {
                    creases.insert(edge(a, b));
                }
            }
        }
        out.creases = creases.into_iter().collect();
        out.validate()?;
        if self.validate_closed().is_ok() {
            out.validate_closed()?;
        }
        Ok(out)
    }

    pub(super) fn relax_borders(
        &self,
        target: BorderTarget,
        threshold: f32,
        selected: &[Edge],
        amount: f32,
        cancel: &impl Fn() -> bool,
    ) -> Result<Self, String> {
        if !amount.is_finite() || !(0. ..=1.).contains(&amount) {
            return Err("Relax amount must be between 0 and 1".into());
        }
        let edges = self.border_edges(target, threshold, selected)?;
        if edges.is_empty() {
            return Err("No edges match this target".into());
        }
        let mut neighbors: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for [a, b] in edges {
            neighbors.entry(a).or_default().push(b);
            neighbors.entry(b).or_default().push(a);
        }
        let editable = self.sculptable_vertices();
        let mut out = self.clone();
        // Alternating relaxation compensates for shrinkage. Junctions and sharp
        // corners stay fixed, and hidden geometry never moves.
        for weight in [0.5, -0.53, 0.5, -0.53] {
            cancelled(cancel)?;
            let old = out.vertices.clone();
            for (&v, ns) in &neighbors {
                if ns.len() == 2 && editable[v] {
                    let a = (self.vertices[ns[0]] - self.vertices[v]).norm();
                    let b = (self.vertices[ns[1]] - self.vertices[v]).norm();
                    if a.dot(b) < -0.5 {
                        out.vertices[v] =
                            old[v] + ((old[ns[0]] + old[ns[1]]) * 0.5 - old[v]) * (weight * amount);
                    }
                }
            }
        }
        for &[a, b, c] in &self.triangles {
            let before =
                (self.vertices[b] - self.vertices[a]).cross(self.vertices[c] - self.vertices[a]);
            let after =
                (out.vertices[b] - out.vertices[a]).cross(out.vertices[c] - out.vertices[a]);
            if before.dot(after) <= 0. {
                return Err("Relaxation would fold a face; reduce Relax amount".into());
            }
        }
        out.validate()?;
        Ok(out)
    }
}
