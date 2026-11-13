use crate::sat::Rule;
mod maxflow;
mod reference;
mod reference_rng;
use crate::symmetry::{Extrinsic, Frame, RoSy4, Sample};
use crate::{
    Result, Vec3, check,
    field::Field,
    flow::Network,
    topology::{self, Dsu},
};
use glam::IVec2;
#[cfg(feature = "audit")]
pub(crate) use reference::solve as solve_reference;
use std::collections::{BTreeMap, VecDeque};

pub(crate) const INVERSION_BUDGET_ERROR: &str = "inverted grid cells within its repair budget";

fn rot(d: IVec2, k: i32) -> IVec2 {
    match k.rem_euclid(4) {
        0 => d,
        1 => IVec2::new(-d.y, d.x),
        2 => -d,
        _ => IVec2::new(d.y, -d.x),
    }
}
fn determinant(a: IVec2, b: IVec2) -> i32 {
    a.x * b.y - a.y * b.x
}
#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy)]
pub(crate) struct Edge {
    pub(crate) ends: [usize; 2],
    #[cfg_attr(feature = "audit", serde(with = "crate::audit::offset"))]
    pub(crate) d: IVec2,
    pub(crate) transport: i32,
    /// Boundary edges must lie on one lattice axis, never a quad diagonal.
    pub(crate) fixed_axis: Option<usize>,
}
#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy)]
pub(crate) struct Face {
    pub(crate) edges: [usize; 3],
    pub(crate) rotations: [i32; 3],
}
impl Face {
    fn vectors(&self, edges: &[Edge]) -> [IVec2; 3] {
        std::array::from_fn(|i| rot(edges[self.edges[i]].d, self.rotations[i]))
    }
    fn defect(&self, edges: &[Edge]) -> IVec2 {
        self.vectors(edges).into_iter().sum()
    }
    fn area(&self, edges: &[Edge]) -> i32 {
        let v = self.vectors(edges);
        determinant(v[0], -v[2])
    }
}
#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Lattice {
    pub(crate) edges: Vec<Edge>,
    pub(crate) faces: Vec<Face>,
    vertex_edges: Vec<Vec<usize>>,
    vertex_faces: Vec<Vec<usize>>,
    edge_faces: Vec<Vec<usize>>,
}
pub(crate) struct Extraction {
    pub positions: Vec<Vec3>,
    pub quads: Vec<[usize; 4]>,
    pub samples: Vec<Vec<usize>>,
}
impl Lattice {
    /// Use the pinned mapper where its closed, unconstrained flow graph applies.
    /// Boundary and sharp-feature locks retain the constraint-aware solver.
    pub(crate) fn correct_field(
        &mut self,
        field: &Field,
        cancel: &dyn Fn() -> bool,
    ) -> Result<usize> {
        if self.edge_faces.iter().all(|faces| faces.len() == 2)
            && field.constraints.iter().all(|c| c.weight < 1.)
        {
            let (corrected, fixed) = reference::solve_with_count(field, cancel)?;
            *self = corrected;
            Ok(fixed)
        } else {
            self.correct(cancel)
        }
    }

    /// Offline loop constraint: carry one lattice axis consistently around each
    /// source-edge cycle and prohibit offsets perpendicular to that axis.
    /// This is stronger than an orientation hint. Validate all cycles before
    /// changing offsets; a rotationally inconsistent cycle cannot be a row.
    #[cfg(feature = "audit")]
    pub(crate) fn constrain_cycles(&mut self, field: &Field, cycles: &[Vec<usize>]) -> Result<()> {
        use std::collections::BTreeSet;
        let ids: BTreeMap<_, _> = self
            .edges
            .iter()
            .enumerate()
            .map(|(i, e)| (e.ends, i))
            .collect();
        let mut locks = BTreeMap::new();
        for cycle in cycles {
            if cycle.len() < 3
                || cycle.iter().any(|&v| v >= field.origins.len())
                || cycle.iter().copied().collect::<BTreeSet<_>>().len() != cycle.len()
            {
                return Err("Invalid closed guide cycle".into());
            }
            let mut rotation = 0i32;
            let mut scores = [0.; 2];
            let mut path = Vec::new();
            for k in 0..cycle.len() {
                let u = cycle[k];
                let v = cycle[(k + 1) % cycle.len()];
                let &id = ids
                    .get(&topology::edge(u, v))
                    .ok_or("Closed guide does not follow source edges")?;
                let e = self.edges[id];
                let forward = u == e.ends[0];
                let edge_rotation = rotation - if forward { 0 } else { e.transport };
                let delta = field.surface.positions[v] - field.surface.positions[u];
                let tangent =
                    (delta - field.normals[u] * delta.dot(field.normals[u])).normalize_or_zero();
                let q = field.directions[u];
                let basis = [q, field.normals[u].cross(q)];
                for (axis, score) in scores.iter_mut().enumerate() {
                    *score += basis[(axis as i32 - rotation).rem_euclid(2) as usize]
                        .dot(tangent)
                        .abs()
                        * delta.length();
                }
                path.push((id, edge_rotation));
                rotation += if forward { e.transport } else { -e.transport };
            }
            if rotation.rem_euclid(4) != 0 {
                return Err("Closed guide crosses an inconsistent orientation cycle".into());
            }
            let along = usize::from(scores[1] > scores[0]);
            for (id, rotation) in path {
                let axis = (1 - along as i32 - rotation).rem_euclid(2) as usize;
                if self.edges[id].fixed_axis.is_some_and(|a| a != axis)
                    || locks.insert(id, axis).is_some_and(|a| a != axis)
                {
                    return Err("Closed guides impose conflicting lattice axes".into());
                }
            }
        }
        for (id, axis) in locks {
            self.edges[id].fixed_axis = Some(axis);
            self.edges[id].d[axis] = 0;
        }
        Ok(())
    }

    pub fn new(f: &Field, cancel: &dyn Fn() -> bool) -> Result<Self> {
        let mut ids = BTreeMap::new();
        let mut edges = Vec::new();
        let mut faces = Vec::new();
        let mut vertex_edges = vec![Vec::new(); f.origins.len()];
        let mut vertex_faces = vertex_edges.clone();
        let frame = |i| Frame::new(f.directions[i], f.normals[i]);
        for (fi, t) in f.surface.triangles.iter().enumerate() {
            if fi % 4096 == 0 {
                check(cancel)?;
            }
            let mut face = Face {
                edges: [0; 3],
                rotations: [0; 3],
            };
            for k in 0..3 {
                let u = t[k];
                let v = t[(k + 1) % 3];
                let [a, b] = topology::edge(u, v);
                let id = *ids.entry([a, b]).or_insert_with(|| {
                    let id = edges.len();
                    let m = Extrinsic::match_orientation(frame(a), frame(b));
                    let h = (f.spacing[a] + f.spacing[b]) * 0.5;
                    let p = Extrinsic::match_position(
                        Sample::new(
                            f.surface.positions[a],
                            f.origins[a],
                            Frame::new(m.lhs.0, f.normals[a]),
                        ),
                        Sample::new(
                            f.surface.positions[b],
                            f.origins[b],
                            Frame::new(m.rhs.0, f.normals[b]),
                        ),
                        h,
                        1. / h,
                    );
                    let d = rot(p.lhs.1 - p.rhs.1, m.lhs.1);
                    edges.push(Edge {
                        ends: [a, b],
                        d,
                        transport: (m.lhs.1 - m.rhs.1).rem_euclid(4),
                        fixed_axis: None,
                    });
                    vertex_edges[a].push(id);
                    vertex_edges[b].push(id);
                    id
                });
                let m = Extrinsic::match_orientation(frame(t[0]), frame(a));
                face.edges[k] = id;
                face.rotations[k] = (m.lhs.1 - m.rhs.1 + if u > v { 2 } else { 0 }).rem_euclid(4);
            }
            faces.push(face);
            for &v in t {
                vertex_faces[v].push(fi);
            }
        }
        let mut edge_faces = vec![Vec::new(); edges.len()];
        for (i, f) in faces.iter().enumerate() {
            for &e in &f.edges {
                edge_faces[e].push(i);
            }
        }
        for (i, e) in edges.iter_mut().enumerate() {
            if edge_faces[i].len() == 1 {
                let a = e.ends[0];
                let delta = f.surface.positions[e.ends[1]] - f.surface.positions[a];
                let along = usize::from(
                    delta.dot(f.normals[a].cross(f.directions[a])).abs()
                        > delta.dot(f.directions[a]).abs(),
                );
                e.fixed_axis = Some(1 - along);
                e.d[1 - along] = 0;
            }
        }
        Ok(Self {
            edges,
            faces,
            vertex_edges,
            vertex_faces,
            edge_faces,
        })
    }
    pub fn defects(&self) -> usize {
        self.faces
            .iter()
            .filter(|f| f.defect(&self.edges) != IVec2::ZERO)
            .count()
    }
    pub fn long_edges(&self) -> usize {
        self.edges
            .iter()
            .filter(|e| e.d.abs().max_element() > 1)
            .count()
    }
    pub fn flipped(&self) -> usize {
        self.faces
            .iter()
            .filter(|f| f.area(&self.edges) < 0)
            .count()
    }
    pub fn correct(&mut self, cancel: &dyn Fn() -> bool) -> Result<usize> {
        self.correct_using(cancel, &[])
    }
    #[cfg(feature = "audit")]
    pub(crate) fn correct_pinned(
        &mut self,
        pinned: &[bool],
        cancel: &dyn Fn() -> bool,
    ) -> Result<usize> {
        if pinned.len() != self.edges.len() * 2 {
            return Err("Invalid temporary flow coordinate locks".into());
        }
        self.correct_using(cancel, pinned)
    }
    fn correct_using(&mut self, cancel: &dyn Fn() -> bool, pinned: &[bool]) -> Result<usize> {
        let initial: Vec<_> = self.edges.iter().map(|e| e.d).collect();
        let start = initial
            .iter()
            .map(|d| d.abs().max_element())
            .max()
            .unwrap_or(2)
            .max(2);
        // The paper starts at [-2,2], widening only if the bounded network is
        // infeasible. Unlimited capacity can concentrate many corrections into
        // one edge and create unnecessary inversions and subdivision work.
        for extra in [1, 2, 4, 8] {
            check(cancel)?;
            for (e, &d) in self.edges.iter_mut().zip(&initial) {
                e.d = d;
            }
            match self.correct_bounded(start * extra, cancel, pinned) {
                Ok(n) => return Ok(n),
                Err(e) if e == "The integer surface grid has no feasible correction" => continue,
                Err(e) => return Err(e),
            }
        }
        Err("The integer surface grid exceeds its correction budget".into())
    }
    fn correct_bounded(
        &mut self,
        limit: i32,
        cancel: &dyn Fn() -> bool,
        pinned: &[bool],
    ) -> Result<usize> {
        // Each coordinate of an edge offset occurs in the two incident face
        // equations. Balance their signs; inconsistent cycles run through
        // orientation singularities. Keep their initial values, changing only
        // those needed to balance each connected flow component.
        let rows = self.faces.len() * 2;
        let mut rotations = vec![i32::MIN; self.faces.len()];
        for start in 0..self.faces.len() {
            if rotations[start] != i32::MIN {
                continue;
            }
            rotations[start] = 0;
            let mut todo = VecDeque::from([start]);
            while let Some(f) = todo.pop_front() {
                for k in 0..3 {
                    let e = self.faces[f].edges[k];
                    for &g in &self.edge_faces[e] {
                        if rotations[g] != i32::MIN {
                            continue;
                        }
                        let j = self.faces[g].edges.iter().position(|&id| id == e).unwrap();
                        rotations[g] = (rotations[f] + self.faces[f].rotations[k] + 2
                            - self.faces[g].rotations[j])
                            .rem_euclid(4);
                        todo.push_back(g);
                    }
                }
            }
        }
        let mut columns = vec![Vec::<(usize, i32)>::new(); self.edges.len() * 2];
        for (i, f) in self.faces.iter().enumerate() {
            for k in 0..3 {
                for axis in 0..2 {
                    let basis = if axis == 0 { IVec2::X } else { IVec2::Y };
                    let c = rot(basis, f.rotations[k] + rotations[i]);
                    let r = i * 2 + usize::from(c.y != 0);
                    let sign = c.x + c.y;
                    let col = f.edges[k] * 2 + axis;
                    columns[col].push((r, sign));
                }
            }
        }
        let mut fixed = vec![false; columns.len()];
        let mut count = 0;
        for (i, c) in columns.iter_mut().enumerate() {
            if c.len() == 2 && c[0].1 == c[1].1 {
                fixed[i] = true;
                count += 1;
            }
        }
        if !pinned.is_empty() {
            // Audit-only reflow: temporary zero-coordinate requests can split
            // the flow graph. Balance the components that can actually carry
            // flow, preserving every real and temporary lock. Reset adjustable
            // same-sign seam values only in deficient connected components.
            // This is a feasibility experiment, not a quality objective.
            let locked: Vec<_> = (0..columns.len())
                .map(|i| pinned[i] || self.edges[i / 2].fixed_axis == Some(i % 2))
                .collect();
            let mut components = Dsu::new(rows + 1);
            for (i, c) in columns.iter().enumerate() {
                if !fixed[i] && !locked[i] {
                    components.join(c[0].0, if c.len() == 2 { c[1].0 } else { rows });
                }
            }
            let roots: Vec<_> = (0..=rows).map(|r| components.find(r)).collect();
            let mut net = vec![0; rows + 1];
            let mut seams = vec![Vec::new(); rows + 1];
            for (i, c) in columns.iter().enumerate() {
                let value = self.edges[i / 2].d[i % 2];
                for &(r, s) in c {
                    net[roots[r]] += s * value;
                    if fixed[i] && !locked[i] {
                        seams[roots[r]].push(i);
                    }
                }
                if c.len() == 1 {
                    net[roots[rows]] -= c[0].1 * value;
                }
            }
            let mut queue: VecDeque<_> = (0..=rows).filter(|&r| net[r] != 0).collect();
            while let Some(r) = queue.pop_front() {
                check(cancel)?;
                if net[r] == 0 {
                    continue;
                }
                for &i in &seams[r] {
                    let value = self.edges[i / 2].d[i % 2];
                    if value == 0 {
                        continue;
                    }
                    self.edges[i / 2].d[i % 2] = 0;
                    for &(row, sign) in &columns[i] {
                        let root = roots[row];
                        net[root] -= sign * value;
                        if net[root] != 0 {
                            queue.push_back(root);
                        }
                    }
                }
            }
            if net.iter().any(|&n| n != 0) {
                return Err("Could not balance the locked flow components".into());
            }
        } else {
            let mut components = Dsu::new(rows + 1);
            for (i, c) in columns.iter().enumerate() {
                if !fixed[i] {
                    components.join(c[0].0, if c.len() == 2 { c[1].0 } else { rows });
                }
            }
            let mut net = vec![0; rows + 1];
            for (i, c) in columns.iter().enumerate() {
                if fixed[i] {
                    let r = components.find(c[0].0);
                    net[r] += 2 * c[0].1 * self.edges[i / 2].d[i % 2];
                }
            }
            // Place the necessary seam adjustments near existing defects. Changing
            // the first eligible vertex IDs concentrates them in one part of an
            // imported sculpt and forces long correction paths across the model.
            let roots: Vec<_> = (0..=rows).map(|r| components.find(r)).collect();
            let mut residual = vec![0i32; rows + 1];
            let mut adjacent = vec![Vec::new(); rows + 1];
            let fixed_ids: Vec<_> = (0..fixed.len()).filter(|&i| fixed[i]).collect();
            for (i, c) in columns.iter().enumerate() {
                let d = self.edges[i / 2].d[i % 2];
                for &(r, s) in c {
                    residual[r] += s * d;
                }
                if c.len() == 1 {
                    residual[rows] -= c[0].1 * d;
                }
                if !fixed[i] {
                    let a = c[0].0;
                    let b = if c.len() == 2 { c[1].0 } else { rows };
                    adjacent[a].push(b);
                    adjacent[b].push(a);
                }
            }
            let mut distance = vec![usize::MAX; rows + 1];
            let mut todo = VecDeque::new();
            for r in 0..=rows {
                if residual[r].signum() * net[roots[r]].signum() > 0 {
                    distance[r] = 0;
                    todo.push_back(r);
                }
            }
            while let Some(r) = todo.pop_front() {
                for &s in &adjacent[r] {
                    if distance[s] == usize::MAX {
                        distance[s] = distance[r] + 1;
                        todo.push_back(s);
                    }
                }
            }
            let steps = net.iter().map(|n| n.unsigned_abs() as usize / 2).sum();
            for _ in 0..steps {
                check(cancel)?;
                let best = fixed_ids
                    .iter()
                    .copied()
                    .filter_map(|i| {
                        if pinned.get(i) == Some(&true) {
                            return None;
                        }
                        let c = &columns[i];
                        let a = c[0].0;
                        let b = c[1].0;
                        let component = roots[a];
                        let sign = net[component].signum();
                        if sign == 0 {
                            return None;
                        }
                        let delta = -c[0].1 * sign;
                        let value = self.edges[i / 2].d[i % 2];
                        if (value + delta).abs() > limit {
                            return None;
                        }
                        let gain = (residual[a] - sign).abs() - residual[a].abs()
                            + (residual[b] - sign).abs()
                            - residual[b].abs();
                        let distance = distance[a].saturating_add(distance[b]);
                        Some((
                            (gain, distance, (i as u64).wrapping_mul(0x9e3779b97f4a7c15)),
                            i,
                            delta,
                            sign,
                        ))
                    })
                    .min_by_key(|x| x.0);
                let Some((_, i, delta, sign)) = best else {
                    return Err("Could not balance the orientation seams".into());
                };
                let c = &columns[i];
                self.edges[i / 2].d[i % 2] += delta;
                residual[c[0].0] -= sign;
                residual[c[1].0] -= sign;
                net[roots[c[0].0]] -= 2 * sign;
            }
            if net.iter().any(|&n| n != 0) {
                return Err("Could not balance the orientation seams".into());
            }
        }
        let outside = rows;
        let source = rows + 1;
        let sink = rows + 2;
        let mut graph = Network::new(rows + 3);
        let mut balance = vec![0; rows + 1];
        let mut handles = vec![None; columns.len()];
        for (i, c) in columns.iter().enumerate() {
            let value = self.edges[i / 2].d[i % 2];
            for &(r, s) in c {
                balance[r] -= s * value;
            }
            if fixed[i] {
                continue;
            }
            let (a, sa) = c[0];
            let (b, sb) = if c.len() == 2 { c[1] } else { (outside, -sa) };
            if c.len() == 1 {
                balance[outside] -= sb * value;
            }
            let (from, to) = if sa < 0 { (a, b) } else { (b, a) };
            let locked =
                self.edges[i / 2].fixed_axis == Some(i % 2) || pinned.get(i) == Some(&true);
            let positive = graph.add(from, to, if locked { 0 } else { limit - value }, 1);
            let negative = graph.add(to, from, if locked { 0 } else { limit + value }, 1);
            handles[i] = Some((positive, negative));
        }
        if balance.iter().sum::<i32>() != 0 {
            return Err("Unbalanced retopology equations".into());
        }
        let mut amount = 0;
        for (i, &b) in balance.iter().enumerate() {
            if b < 0 {
                graph.add(source, i, -b, 0);
                amount -= b;
            } else if b > 0 {
                graph.add(i, sink, b, 0);
            }
        }
        graph.send(source, sink, amount, cancel)?;
        for (i, h) in handles.into_iter().enumerate() {
            if let Some((p, n)) = h {
                self.edges[i / 2].d[i % 2] += graph.used(p) - graph.used(n);
            }
        }
        if self.defects() != 0 {
            return Err("Integer-grid correction left a position singularity".into());
        }
        Ok(count)
    }
    /// Split long integer edges after regularization (paper, section 3.3).
    /// The new offsets add exactly to the old one; each child face inherits
    /// the parent's coordinate frame, including its orientation singularity.
    pub fn subdivide(
        &mut self,
        field: &mut Field,
        max_vertices: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<()> {
        self.subdivide_using(field, max_vertices, cancel, false)
    }
    #[cfg(feature = "audit")]
    pub(crate) fn subdivide_preserving_orientation(
        &mut self,
        field: &mut Field,
        max_vertices: usize,
        cancel: &dyn Fn() -> bool,
    ) -> Result<()> {
        self.subdivide_using(field, max_vertices, cancel, true)
    }
    fn subdivide_using(
        &mut self,
        field: &mut Field,
        max_vertices: usize,
        cancel: &dyn Fn() -> bool,
        preserve_orientation: bool,
    ) -> Result<()> {
        let mut work = 0;
        let mut pending: VecDeque<_> = self
            .edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.d.abs().max_element() > 1)
            .map(|(i, _)| i)
            .collect();
        while let Some(e) = pending.pop_front() {
            if self.edges[e].d.abs().max_element() <= 1 {
                continue;
            }
            check(cancel)?;
            work += 1;
            if work > 16384 || field.origins.len() >= max_vertices {
                return Err("Long-edge subdivision exceeds the retopology work budget".into());
            }
            let edge = self.edges[e];
            let [a, b] = edge.ends;
            let m = field.origins.len();
            let d0 = edge.d / 2;
            let d1 = rot(d0 - edge.d, -edge.transport);
            let p = (field.surface.positions[a] + field.surface.positions[b]) * 0.5;
            let n = (field.normals[a] + field.normals[b])
                .try_normalize()
                .unwrap_or(field.normals[a]);
            let q =
                crate::symmetry::rotate_vector_into_plane(field.directions[a], field.normals[a], n);
            let q = (q - n * q.dot(n)).normalize_or_zero();
            field.surface.positions.push(p);
            field.normals.push(n);
            field.directions.push(q);
            field
                .origins
                .push((field.origins[a] + field.origins[b]) * 0.5);
            field
                .spacing
                .push((field.spacing[a] + field.spacing[b]) * 0.5);
            field.areas.push(0.);
            let ca = field.constraints[a];
            let cb = field.constraints[b];
            field.constraints.push(crate::field::Constraint {
                tangent: q,
                origin: p,
                weight: ca.weight.min(cb.weight),
            });
            let incident = self.edge_faces[e].clone();
            let parent_areas: Vec<_> = if preserve_orientation {
                incident
                    .iter()
                    .map(|&f| self.faces[f].area(&self.edges))
                    .collect()
            } else {
                Vec::new()
            };
            self.edges[e] = Edge {
                ends: [a, m],
                d: d0,
                transport: 0,
                fixed_axis: edge.fixed_axis,
            };
            let second = self.edges.len();
            self.edges.push(Edge {
                ends: [b, m],
                d: d1,
                transport: -edge.transport,
                fixed_axis: edge
                    .fixed_axis
                    .map(|axis| (axis + edge.transport.rem_euclid(2) as usize) % 2),
            });
            self.edge_faces[e].clear();
            self.edge_faces.push(Vec::new());
            self.vertex_edges[b].retain(|&id| id != e);
            self.vertex_edges[b].push(second);
            self.vertex_edges.push(vec![e, second]);
            self.vertex_faces.push(Vec::new());
            for id in [e, second] {
                if self.edges[id].d.abs().max_element() > 1 {
                    pending.push_back(id);
                }
            }
            for (incident_index, fi) in incident.into_iter().enumerate() {
                let f = self.faces[fi];
                let tri = field.surface.triangles[fi];
                let k = f.edges.iter().position(|&id| id == e).unwrap();
                let [u, v, w] = [tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]];
                let ra = f.rotations[k] - if u == a { 0 } else { 2 };
                let ruv = if u == a { ra } else { ra + edge.transport };
                let rv = if v == a { ra } else { ra + edge.transport };
                let match_w = Extrinsic::match_orientation(
                    Frame::new(field.directions[a], field.normals[a]),
                    Frame::new(field.directions[w], field.normals[w]),
                );
                let transport_w = match_w.lhs.1 - match_w.rhs.1;
                let rw = ra + transport_w;
                let wm = rot(if u == a { d0 } else { d0 - edge.d }, ra)
                    + rot(self.edges[f.edges[(k + 2) % 3]].d, f.rotations[(k + 2) % 3]);
                let cross = self.edges.len();
                self.edges.push(Edge {
                    ends: [w, m],
                    d: rot(wm, -rw),
                    transport: -transport_w,
                    fixed_axis: None,
                });
                if self.edges[cross].d.abs().max_element() > 1 {
                    pending.push_back(cross);
                }
                let um = if u == a { e } else { second };
                let vm = if v == a { e } else { second };
                let child = self.faces.len();
                self.edge_faces.push(vec![fi, child]);
                self.edge_faces[um].push(fi);
                self.edge_faces[vm].push(child);
                for face in &mut self.edge_faces[f.edges[(k + 1) % 3]] {
                    if *face == fi {
                        *face = child;
                    }
                }
                self.vertex_edges[w].push(cross);
                self.vertex_edges[m].push(cross);
                self.vertex_faces[v].retain(|&f| f != fi);
                self.vertex_faces[m].extend([fi, child]);
                self.vertex_faces[v].push(child);
                self.vertex_faces[w].push(child);
                self.faces[fi] = Face {
                    edges: [um, cross, f.edges[(k + 2) % 3]],
                    rotations: [ruv, rw + 2, f.rotations[(k + 2) % 3]],
                };
                field.surface.triangles[fi] = [u, m, w];
                self.faces.push(Face {
                    edges: [vm, f.edges[(k + 1) % 3], cross],
                    rotations: [rv + 2, f.rotations[(k + 1) % 3], rw],
                });
                if preserve_orientation {
                    let parent_area = parent_areas[incident_index];
                    let areas = [
                        self.faces[fi].area(&self.edges),
                        self.faces[child].area(&self.edges),
                    ];
                    if areas.iter().sum::<i32>() != parent_area
                        || areas
                            .iter()
                            .any(|&a| (parent_area >= 0 && a < 0) || (parent_area <= 0 && a > 0))
                    {
                        return Err("Strict subdivision changed a parent face orientation".into());
                    }
                }
                field.surface.triangles.push([m, v, w]);
            }
        }
        if work > 0 {
            field.areas = topology::geometry(&field.surface).1;
        }
        if self.defects() != 0 {
            return Err("Long-edge subdivision changed a grid equation".into());
        }
        Ok(())
    }
    fn greedy(&mut self, cancel: &dyn Fn() -> bool) -> Result<()> {
        for _ in 0..12 {
            check(cancel)?;
            let mut changed = false;
            for fi in 0..self.faces.len() {
                if fi % 4096 == 0 {
                    check(cancel)?;
                }
                if self.faces[fi].area(&self.edges) >= 0 {
                    continue;
                }
                for e in self.faces[fi].edges {
                    let edge = self.edges[e];
                    for v in edge.ends {
                        let shift = if v == edge.ends[0] {
                            edge.d
                        } else {
                            -rot(edge.d, -edge.transport)
                        };
                        if shift == IVec2::ZERO {
                            continue;
                        }
                        let old: Vec<_> = self.vertex_edges[v]
                            .iter()
                            .map(|&e| (e, self.edges[e].d))
                            .collect();
                        let before: i32 = self.vertex_faces[v]
                            .iter()
                            .map(|&f| (-self.faces[f].area(&self.edges)).max(0))
                            .sum();
                        let mut valid = true;
                        for &(e, d) in &old {
                            let edge = &mut self.edges[e];
                            edge.d = if edge.ends[0] == v {
                                d - shift
                            } else {
                                d + rot(shift, edge.transport)
                            };
                            valid &= edge.d.abs().max_element() <= 1;
                            valid &= edge.fixed_axis.is_none_or(|axis| edge.d[axis] == 0);
                        }
                        let after: i32 = self.vertex_faces[v]
                            .iter()
                            .map(|&f| (-self.faces[f].area(&self.edges)).max(0))
                            .sum();
                        valid &= after < before
                            && self.vertex_faces[v]
                                .iter()
                                .all(|&f| self.faces[f].defect(&self.edges) == IVec2::ZERO);
                        if valid {
                            changed = true;
                        } else {
                            for (e, d) in old {
                                self.edges[e].d = d;
                            }
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }
        Ok(())
    }
    pub fn remove_flips(
        &mut self,
        cancel: &dyn Fn() -> bool,
        progress: &dyn Fn(crate::Stage),
    ) -> Result<()> {
        self.greedy(cancel)?;
        self.greedy_clusters(cancel)?;
        if self.flipped() > 0 {
            self.repair_patches(cancel, progress)?;
        }
        if self.flipped() > 0 {
            return Err(format!(
                "Retopology could not resolve {} {INVERSION_BUDGET_ERROR}",
                self.flipped()
            ));
        }
        Ok(())
    }
    /// Work on lattice vertices, which can contain many coincident source
    /// vertices. Moving only one source vertex cannot shorten a folded cell
    /// when a dense ring of zero offsets pins its neighbours in place.
    fn greedy_clusters(&mut self, cancel: &dyn Fn() -> bool) -> Result<()> {
        for _ in 0..24 {
            check(cancel)?;
            if self.flipped() == 0 {
                break;
            }
            let n = self.vertex_edges.len();
            let mut roots = vec![usize::MAX; n];
            let mut rotations = vec![0i32; n];
            let mut groups = Vec::new();
            let mut movable = Vec::new();
            for start in 0..n {
                if roots[start] != usize::MAX {
                    continue;
                }
                let root = groups.len();
                roots[start] = root;
                let mut vertices = vec![start];
                let mut regular = true;
                let mut i = 0;
                while i < vertices.len() {
                    let v = vertices[i];
                    i += 1;
                    for &ei in &self.vertex_edges[v] {
                        let e = self.edges[ei];
                        if e.d != IVec2::ZERO {
                            continue;
                        }
                        let (other, delta) = if e.ends[0] == v {
                            (e.ends[1], -e.transport)
                        } else {
                            (e.ends[0], e.transport)
                        };
                        let r = (rotations[v] + delta).rem_euclid(4);
                        if roots[other] == usize::MAX {
                            roots[other] = root;
                            rotations[other] = r;
                            vertices.push(other);
                        } else if rotations[other] != r {
                            regular = false;
                        }
                    }
                }
                groups.push(vertices);
                movable.push(regular);
            }
            let mut candidates = Vec::new();
            for f in &self.faces {
                if f.area(&self.edges) >= 0 {
                    continue;
                }
                for &ei in &f.edges {
                    let e = self.edges[ei];
                    for (v, shift) in [(e.ends[0], e.d), (e.ends[1], -rot(e.d, -e.transport))] {
                        let root = roots[v];
                        if movable[root] && shift != IVec2::ZERO {
                            let d = rot(shift, -rotations[v]);
                            candidates.push((root, d.x, d.y));
                        }
                    }
                }
            }
            candidates.sort_unstable();
            candidates.dedup();
            let mut changed = false;
            for (root, x, y) in candidates {
                check(cancel)?;
                let shift = IVec2::new(x, y);
                let mut edges: Vec<_> = groups[root]
                    .iter()
                    .flat_map(|&v| self.vertex_edges[v].iter().copied())
                    .collect();
                edges.sort_unstable();
                edges.dedup();
                let mut faces: Vec<_> = groups[root]
                    .iter()
                    .flat_map(|&v| self.vertex_faces[v].iter().copied())
                    .collect();
                faces.sort_unstable();
                faces.dedup();
                let before = faces
                    .iter()
                    .filter(|&&f| self.faces[f].area(&self.edges) < 0)
                    .count();
                if before == 0 {
                    continue;
                }
                let old: Vec<_> = edges.iter().map(|&e| self.edges[e].d).collect();
                let mut valid = true;
                for &ei in &edges {
                    let e = &mut self.edges[ei];
                    let [a, b] = e.ends;
                    if roots[a] == root {
                        e.d -= rot(shift, rotations[a]);
                    }
                    if roots[b] == root {
                        e.d += rot(shift, rotations[b] + e.transport);
                    }
                    valid &= e.d.abs().max_element() <= 1;
                    valid &= e.fixed_axis.is_none_or(|axis| e.d[axis] == 0);
                }
                let after = faces
                    .iter()
                    .filter(|&&f| self.faces[f].area(&self.edges) < 0)
                    .count();
                valid &= after < before
                    && faces
                        .iter()
                        .all(|&f| self.faces[f].defect(&self.edges) == IVec2::ZERO);
                if valid {
                    changed = true;
                } else {
                    for (&e, d) in edges.iter().zip(old) {
                        self.edges[e].d = d;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        Ok(())
    }
    fn repair_patches(
        &mut self,
        cancel: &dyn Fn() -> bool,
        progress: &dyn Fn(crate::Stage),
    ) -> Result<()> {
        for _ in 0..3 {
            let before = self.flipped();
            for radius in [2, 4, 8, 16, 32, 64, 96] {
                progress(crate::Stage::Repair {
                    radius,
                    flipped: self.flipped(),
                });
                let flipped: Vec<_> = (0..self.faces.len())
                    .filter(|&f| self.faces[f].area(&self.edges) < 0)
                    .collect();
                let effort = if flipped.len() <= 4 { 2000 } else { 200 };
                for fi in flipped {
                    check(cancel)?;
                    if self.faces[fi].area(&self.edges) >= 0 {
                        continue;
                    }
                    let mut distances = BTreeMap::from([(fi, 0)]);
                    let mut todo = VecDeque::from([fi]);
                    while let Some(f) = todo.pop_front() {
                        let depth = distances[&f];
                        if depth >= radius {
                            continue;
                        }
                        for e in self.faces[f].edges {
                            for &g in &self.edge_faces[e] {
                                if let std::collections::btree_map::Entry::Vacant(v) =
                                    distances.entry(g)
                                {
                                    v.insert(depth + 1);
                                    todo.push_back(g);
                                }
                            }
                        }
                    }
                    let face_ids: Vec<_> = distances.keys().copied().collect();
                    let mut edge_ids: Vec<_> =
                        face_ids.iter().flat_map(|&f| self.faces[f].edges).collect();
                    edge_ids.sort_unstable();
                    edge_ids.dedup();
                    let ids: BTreeMap<_, _> =
                        edge_ids.iter().enumerate().map(|(i, &e)| (e, i)).collect();
                    let mut domains = vec![0x1ffu16; edge_ids.len()];
                    let preferred: Vec<_> =
                        edge_ids.iter().map(|&e| encode(self.edges[e].d)).collect();
                    for (i, &e) in edge_ids.iter().enumerate() {
                        if let Some(axis) = self.edges[e].fixed_axis {
                            domains[i] = (0..9)
                                .filter(|&v| decode(v)[axis] == 0)
                                .fold(0, |d, v| d | (1 << v));
                        }
                        if self.edge_faces[e]
                            .iter()
                            .any(|f| !distances.contains_key(f))
                        {
                            domains[i] = 1 << preferred[i];
                        }
                    }
                    let rules: Vec<_> = face_ids
                        .iter()
                        .map(|&f| {
                            let f = self.faces[f];
                            let mut tuples = Vec::new();
                            for a in 0..9 {
                                for b in 0..9 {
                                    let x = rot(decode(a), f.rotations[0]);
                                    let y = rot(decode(b), f.rotations[1]);
                                    let z = -x - y;
                                    if z.abs().max_element() <= 1 && determinant(x, x + y) >= 0 {
                                        tuples.push([a, b, encode(rot(z, -f.rotations[2]))]);
                                    }
                                }
                            }
                            Rule {
                                vars: f.edges.map(|e| ids[&e]),
                                tuples,
                            }
                        })
                        .collect();
                    let mut coarse_domains = domains.clone();
                    for (i, &p) in preferred.iter().enumerate() {
                        if p == 4 {
                            coarse_domains[i] = 1 << 4;
                        }
                    }
                    let mut solution = crate::sat::solve_with_budget(
                        coarse_domains,
                        &rules,
                        &preferred,
                        cancel,
                        effort,
                    )?;
                    if solution.is_none() && radius > 8 {
                        let mut local_domains = domains.clone();
                        for (i, &e) in edge_ids.iter().enumerate() {
                            if preferred[i] == 4
                                && self.edge_faces[e]
                                    .iter()
                                    .all(|f| distances.get(f).is_none_or(|&d| d > 8))
                            {
                                local_domains[i] = 1 << 4;
                            }
                        }
                        solution = crate::sat::solve_with_budget(
                            local_domains,
                            &rules,
                            &preferred,
                            cancel,
                            effort,
                        )?;
                    }
                    if solution.is_none() {
                        solution = crate::sat::solve_with_budget(
                            domains, &rules, &preferred, cancel, effort,
                        )?;
                    }
                    if let Some(solution) = solution {
                        for (i, &e) in edge_ids.iter().enumerate() {
                            self.edges[e].d = decode(solution[i].trailing_zeros() as u8);
                        }
                    }
                }
                self.greedy(cancel)?;
                self.greedy_clusters(cancel)?;
                if self.flipped() == 0 {
                    return Ok(());
                }
            }
            if self.flipped() >= before {
                break;
            }
        }
        Ok(())
    }
    pub fn fit(&self, field: &mut Field, cancel: &dyn Fn() -> bool) -> Result<()> {
        crate::geometry::fit_fixed(field, &self.edges, cancel)
    }
    pub fn extract(&self, field: &Field, cancel: &dyn Fn() -> bool) -> Result<Extraction> {
        crate::extraction::extract(self, field, cancel)
    }
}

fn encode(v: IVec2) -> u8 {
    ((v.x + 1) * 3 + v.y + 1) as u8
}
fn decode(v: u8) -> IVec2 {
    IVec2::new(v as i32 / 3 - 1, v as i32 % 3 - 1)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "audit")]
    #[test]
    fn strict_axial_subdivision_preserves_signed_area_and_transported_locks() {
        for frame_rotation in 0..4 {
            for sign in [-1, 0, 1] {
                let positions = vec![Vec3::ZERO, Vec3::X * 2., Vec3::X + Vec3::Y];
                let rotations = [
                    frame_rotation,
                    (frame_rotation + 1) % 4,
                    (frame_rotation + 3) % 4,
                ];
                let mut field = Field {
                    origins: positions.clone(),
                    surface: crate::Surface {
                        positions: positions.clone(),
                        triangles: vec![[0, 1, 2]],
                    },
                    normals: vec![Vec3::Z; 3],
                    directions: rotations
                        .iter()
                        .map(|&r| crate::field::rotate(Vec3::X, Vec3::Z, r))
                        .collect(),
                    spacing: vec![1.; 3],
                    areas: vec![1.; 3],
                    constraints: vec![crate::field::Constraint::default(); 3],
                };
                let mut lattice = Lattice::new(&field, &|| false).unwrap();
                let uv = [IVec2::ZERO, IVec2::new(2, 0), IVec2::new(1, sign)];
                for e in &mut lattice.edges {
                    let [a, b] = e.ends;
                    e.d = rot(uv[b] - uv[a], -rotations[a]);
                    // This test locks only the long axial edge; Lattice::new
                    // otherwise locks all three edges of an open triangle.
                    e.fixed_axis = None;
                    if a == 0 && b == 1 {
                        e.fixed_axis = Some((rotations[a] as usize + 1) % 2);
                    }
                }
                assert_eq!(lattice.defects(), 0);
                let before = lattice.faces[0].area(&lattice.edges);
                assert_eq!(before, 2 * sign);
                lattice
                    .subdivide_preserving_orientation(&mut field, 10, &|| false)
                    .unwrap();
                assert_eq!(field.surface.positions[..3], positions);
                assert_eq!(field.surface.positions.len(), 4);
                assert_eq!(lattice.faces.len(), 2);
                assert_eq!(lattice.defects(), 0);
                assert_eq!(lattice.long_edges(), 0);
                assert_eq!(
                    lattice
                        .faces
                        .iter()
                        .map(|f| f.area(&lattice.edges))
                        .sum::<i32>(),
                    before
                );
                assert!(lattice.faces.iter().all(|f| f.area(&lattice.edges) == sign));
                let locked: Vec<_> = lattice
                    .edges
                    .iter()
                    .filter(|e| e.fixed_axis.is_some())
                    .collect();
                assert_eq!(locked.len(), 2);
                assert!(locked.iter().all(|e| e.d[e.fixed_axis.unwrap()] == 0));
            }
        }
    }
    #[cfg(feature = "audit")]
    #[test]
    fn closed_cycle_constraints_transport_axes_and_reject_invalid_paths_atomically() {
        let count = 16;
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut directions = Vec::new();
        for ring in 0..3 {
            for i in 0..count {
                let angle = i as f64 * std::f64::consts::TAU / count as f64;
                let n = Vec3::new(angle.cos(), angle.sin(), 0.);
                positions.push(n + Vec3::Z * ring as f64);
                normals.push(n);
                directions.push(crate::field::rotate(Vec3::Z, n, i as i32 % 4));
            }
        }
        let mut triangles = Vec::new();
        for ring in 0..2 {
            for i in 0..count {
                let a = ring * count + i;
                let b = ring * count + (i + 1) % count;
                triangles.extend([[a, b, b + count], [a, b + count, a + count]]);
            }
        }
        let field = Field {
            origins: positions.clone(),
            surface: crate::Surface {
                positions,
                triangles,
            },
            normals,
            directions,
            areas: vec![1.; count * 3],
            spacing: vec![0.3; count * 3],
            constraints: vec![crate::field::Constraint::default(); count * 3],
        };
        let mut lattice = Lattice::new(&field, &|| false).unwrap();
        let cycle: Vec<_> = (count..count * 2).collect();
        let row_edges: Vec<_> = (0..count)
            .map(|i| {
                let pair = topology::edge(cycle[i], cycle[(i + 1) % count]);
                lattice.edges.iter().position(|e| e.ends == pair).unwrap()
            })
            .collect();
        for &id in &row_edges {
            lattice.edges[id].d = IVec2::new(2, 3);
        }
        let state = |l: &Lattice| {
            l.edges
                .iter()
                .map(|e| (e.d, e.fixed_axis))
                .collect::<Vec<_>>()
        };
        let before = state(&lattice);
        assert!(
            lattice
                .constrain_cycles(&field, &[cycle.clone(), vec![0, 1, 2]])
                .is_err()
        );
        assert_eq!(before, state(&lattice));
        let bad_edge = row_edges[0];
        let transport = lattice.edges[bad_edge].transport;
        lattice.edges[bad_edge].transport = (transport + 1) % 4;
        assert!(
            lattice
                .constrain_cycles(&field, std::slice::from_ref(&cycle))
                .is_err()
        );
        assert_eq!(before, state(&lattice));
        lattice.edges[bad_edge].transport = transport;
        lattice.constrain_cycles(&field, &[cycle]).unwrap();
        for (id, e) in lattice.edges.iter().enumerate() {
            if row_edges.contains(&id) {
                let axial = e.ends[0] % 2;
                assert_eq!(e.fixed_axis, Some(axial));
                assert_eq!(e.d[axial], 0);
                assert_eq!(e.d[1 - axial], before[id].0[1 - axial]);
            } else {
                assert_eq!((e.d, e.fixed_axis), before[id]);
            }
        }
    }
    #[cfg(feature = "audit")]
    #[test]
    fn pinned_flow_handles_disconnected_zero_feasible_components() {
        let mut seed = 0x1432725u32;
        let mut random = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for _ in 0..48 {
            let mut lattice = Lattice {
                edges: [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]]
                    .into_iter()
                    .map(|ends| Edge {
                        ends,
                        d: IVec2::ZERO,
                        transport: 0,
                        fixed_axis: None,
                    })
                    .collect(),
                faces: [[1, 3, 0], [0, 4, 2], [3, 5, 4], [1, 2, 5]]
                    .into_iter()
                    .map(|edges| Face {
                        edges,
                        rotations: std::array::from_fn(|_| (random() % 4) as i32),
                    })
                    .collect(),
                vertex_edges: vec![vec![]; 4],
                vertex_faces: vec![vec![]; 4],
                edge_faces: vec![vec![]; 6],
            };
            for (f, face) in lattice.faces.iter().enumerate() {
                for &e in &face.edges {
                    lattice.edge_faces[e].push(f);
                }
            }
            let mut pinned = vec![false; 12];
            for (i, e) in lattice.edges.iter_mut().enumerate() {
                for a in 0..2 {
                    pinned[2 * i + a] = random() % 3 == 0;
                    e.d[a] = if pinned[2 * i + a] {
                        0
                    } else {
                        (random() % 3) as i32 - 1
                    };
                }
                if random() % 2 == 0 {
                    e.fixed_axis = Some(0);
                    e.d.x = 0;
                }
            }
            // The all-zero assignment proves feasibility independently of the
            // algorithm, even when locks divide the row graph into components.
            lattice.correct_pinned(&pinned, &|| false).unwrap();
            assert_eq!(lattice.defects(), 0);
            for (i, e) in lattice.edges.iter().enumerate() {
                for a in 0..2 {
                    if pinned[2 * i + a] || e.fixed_axis == Some(a) {
                        assert_eq!(e.d[a], 0);
                    }
                }
            }
        }
    }
    #[test]
    fn complete_extraction_matches_pinned_upstream_on_curved_surface() {
        fn read<T: std::str::FromStr>(w: &mut std::str::SplitWhitespace<'_>) -> T
        where
            T::Err: std::fmt::Debug,
        {
            w.next().unwrap().parse().unwrap()
        }
        fn vector(w: &mut std::str::SplitWhitespace<'_>) -> Vec3 {
            Vec3::new(read(w), read(w), read(w))
        }
        let mut w = include_str!("../tests/data/extraction-sphere.input").split_whitespace();
        let nv: usize = read(&mut w);
        let nf: usize = read(&mut w);
        let ne: usize = read(&mut w);
        let mut f = Field {
            surface: crate::Surface {
                positions: vec![],
                triangles: vec![],
            },
            origins: vec![],
            normals: vec![],
            directions: vec![],
            spacing: vec![1.; nv],
            areas: vec![1.; nv],
            constraints: vec![crate::field::Constraint::default(); nv],
        };
        for _ in 0..nv {
            f.surface.positions.push(vector(&mut w));
            f.origins.push(vector(&mut w));
            f.normals.push(vector(&mut w));
            f.directions.push(vector(&mut w));
        }
        let mut lattice = Lattice {
            faces: vec![],
            edges: vec![],
            vertex_edges: vec![vec![]; nv],
            vertex_faces: vec![vec![]; nv],
            edge_faces: vec![vec![]; ne],
        };
        for fi in 0..nf {
            let t: [usize; 3] = [read(&mut w), read(&mut w), read(&mut w)];
            f.surface.triangles.push(t);
            let edges: [usize; 3] = [read(&mut w), read(&mut w), read(&mut w)];
            let rotations: [i32; 3] = [read(&mut w), read(&mut w), read(&mut w)];
            for &v in &t {
                lattice.vertex_faces[v].push(fi);
            }
            for &e in &edges {
                lattice.edge_faces[e].push(fi);
            }
            lattice.faces.push(Face { edges, rotations });
        }
        for i in 0..ne {
            let ends: [usize; 2] = [read(&mut w), read(&mut w)];
            let d = IVec2::new(read(&mut w), read(&mut w));
            let allow: [u8; 2] = [read(&mut w), read(&mut w)];
            for &v in &ends {
                lattice.vertex_edges[v].push(i);
            }
            lattice.edges.push(Edge {
                ends,
                d,
                transport: 0,
                fixed_axis: allow.iter().position(|&v| v == 0),
            });
        }
        assert!(w.next().is_none());
        let mut mesh = lattice.extract(&f, &|| false).unwrap();
        crate::quad::fix_valence(&mut mesh, &|| false).unwrap();
        let mut expected =
            include_str!("../tests/data/extraction-sphere.expected").split_whitespace();
        assert_eq!(mesh.positions.len(), read::<usize>(&mut expected));
        assert_eq!(mesh.quads.len(), read::<usize>(&mut expected));
        for (p, samples) in mesh.positions.iter().zip(&mesh.samples) {
            assert!(p.distance(vector(&mut expected)) < 1e-12);
            let n: usize = read::<usize>(&mut expected);
            let s: Vec<usize> = (0..n).map(|_| read::<usize>(&mut expected)).collect();
            assert_eq!(samples, &s);
        }
        for q in mesh.quads {
            assert_eq!(
                q,
                [
                    read::<usize>(&mut expected),
                    read::<usize>(&mut expected),
                    read::<usize>(&mut expected),
                    read::<usize>(&mut expected)
                ]
            );
        }
        assert!(expected.next().is_none());
    }
    #[test]
    fn subdividing_long_integer_edges_preserves_regular_faces_and_geometry() {
        let surface = crate::Surface {
            positions: vec![
                Vec3::ZERO,
                Vec3::X * 2.,
                (Vec3::X + Vec3::Y) * 2.,
                Vec3::Y * 2.,
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let (normals, areas, _, area) = topology::geometry(&surface);
        let mut f = Field {
            origins: surface.positions.clone(),
            surface,
            normals,
            areas,
            directions: vec![Vec3::X; 4],
            spacing: vec![1.; 4],
            constraints: vec![crate::field::Constraint::default(); 4],
        };
        let mut l = Lattice::new(&f, &|| false).unwrap();
        assert_eq!(l.defects(), 0);
        assert!(l.edges.iter().any(|e| e.d.abs().max_element() > 1));
        l.subdivide(&mut f, 100, &|| false).unwrap();
        assert_eq!(l.defects(), 0);
        assert_eq!(l.flipped(), 0);
        assert!(l.edges.iter().all(|e| e.d.abs().max_element() <= 1));
        assert!((topology::geometry(&f.surface).3 - area).abs() < 1e-10);
        topology::validate(&f.surface).unwrap();
        for (e, incident) in l.edge_faces.iter().enumerate() {
            let mut expected: Vec<_> = l
                .faces
                .iter()
                .enumerate()
                .filter(|(_, f)| f.edges.contains(&e))
                .map(|(i, _)| i)
                .collect();
            let mut actual = incident.clone();
            expected.sort_unstable();
            actual.sort_unstable();
            assert_eq!(actual, expected);
        }
        for v in 0..f.origins.len() {
            let mut expected: Vec<_> = f
                .surface
                .triangles
                .iter()
                .enumerate()
                .filter(|(_, t)| t.contains(&v))
                .map(|(i, _)| i)
                .collect();
            let mut actual = l.vertex_faces[v].clone();
            expected.sort_unstable();
            actual.sort_unstable();
            assert_eq!(actual, expected);
        }
    }
}
