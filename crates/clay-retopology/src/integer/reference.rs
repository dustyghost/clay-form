//! Original QuadriFlow integer seeding, seam balancing and default max flow.
//! Adapted from parametrizer-sing.cpp, parametrizer-int.cpp and optimizer.cpp
//! at 810b7a0967c35b0dc85b4464e3835e26a756c967; see LICENSE-QuadriFlow.
use super::{Lattice, maxflow::Network, reference_rng::Rng, rot};
use crate::{
    Result, Vec3, check,
    field::Field,
    symmetry::{Extrinsic, Frame, RoSy4},
};
use glam::IVec2;
use std::collections::VecDeque;
const NONE: usize = usize::MAX;

fn indices(q0: Vec3, n0: Vec3, q1: Vec3, n1: Vec3) -> (i32, i32) {
    let m = Extrinsic::match_orientation(Frame::new(q0, n0), Frame::new(q1, n1));
    (m.lhs.1, m.rhs.1)
}
fn position_indices(f: &Field, a: usize, b: usize, q0: Vec3, q1: Vec3) -> IVec2 {
    let n0 = f.normals[a];
    let n1 = f.normals[b];
    let o0 = f.origins[a];
    let o1 = f.origins[b];
    let middle =
        crate::field::reference_midpoint(f.surface.positions[a], n0, f.surface.positions[b], n1);
    let t0 = n0.cross(q0);
    let t1 = n1.cross(q1);
    let base = |o: Vec3, q: Vec3, t: Vec3, h: f64| {
        IVec2::new(
            (q.dot(middle - o) * (1. / h)).floor() as i32,
            (t.dot(middle - o) * (1. / h)).floor() as i32,
        )
    };
    let x = base(o0, q0, t0, f.spacing[a]);
    let y = base(o1, q1, t1, f.spacing[b]);
    let mut best = f64::INFINITY;
    let mut answer = IVec2::ZERO;
    for i in 0..4 {
        let u = x + IVec2::new(i & 1, (i & 2) >> 1);
        let p = o0 + (q0 * u.x as f64 * f.spacing[a] + t0 * u.y as f64 * f.spacing[a]);
        for j in 0..4 {
            let v = y + IVec2::new(j & 1, (j & 2) >> 1);
            let r = o1 + (q1 * v.x as f64 * f.spacing[b] + t1 * v.y as f64 * f.spacing[b]);
            let cost = (p - r).length_squared();
            if cost < best {
                best = cost;
                answer = u - v;
            }
        }
    }
    answer
}

struct Tree {
    parent: Vec<usize>,
    size: Vec<usize>,
    turn: Vec<i32>,
}
impl Tree {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            size: vec![1; n],
            turn: vec![0; n],
        }
    }
    fn find(&mut self, v: usize) -> (usize, i32) {
        let mut r = v;
        let mut t = 0;
        while self.parent[r] != r {
            t = (t + self.turn[r]) % 4;
            r = self.parent[r];
        }
        let mut u = v;
        let mut left = t;
        while self.parent[u] != u {
            let next = self.parent[u];
            let old = self.turn[u];
            self.parent[u] = r;
            self.turn[u] = left;
            left = (left - old).rem_euclid(4);
            u = next;
        }
        (r, t)
    }
    fn orient_merge(&mut self, a: usize, b: usize, oa: i32, ob: i32) {
        let (x, tx) = self.find(a);
        let (y, ty) = self.find(b);
        if x == y {
            return;
        }
        if self.size[y] < self.size[x] {
            self.size[x] += self.size[y];
            self.parent[y] = x;
            self.turn[y] = (ob - oa + tx - ty).rem_euclid(4);
        } else {
            self.size[y] += self.size[x];
            self.parent[x] = y;
            self.turn[x] = (oa - ob + ty - tx).rem_euclid(4);
        }
    }
    fn merge(&mut self, a: usize, b: usize) {
        let (x, _) = self.find(a);
        let (y, _) = self.find(b);
        if x == y {
            return;
        }
        if self.size[x] < self.size[y] {
            self.size[y] += self.size[x];
            self.parent[x] = y;
        } else {
            self.size[x] += self.size[y];
            self.parent[y] = x;
        }
    }
    fn compact(&mut self) -> (Vec<usize>, usize) {
        let mut labels = vec![NONE; self.parent.len()];
        let mut count = 0;
        for (i, v) in labels.iter_mut().enumerate() {
            if self.parent[i] == i {
                *v = count;
                count += 1;
            }
        }
        (
            (0..labels.len()).map(|v| labels[self.find(v).0]).collect(),
            count,
        )
    }
}

type Columns = Vec<Vec<(usize, i32)>>;
fn columns(l: &Lattice) -> Columns {
    let mut c = vec![vec![]; l.edges.len() * 2];
    for (f, face) in l.faces.iter().enumerate() {
        for k in 0..3 {
            for axis in 0..2 {
                let d = rot(
                    if axis == 0 { IVec2::X } else { IVec2::Y },
                    face.rotations[k],
                );
                c[face.edges[k] * 2 + axis].push((f * 2 + usize::from(d.y != 0), d.x + d.y));
            }
        }
    }
    c
}
fn balance(
    l: &mut Lattice,
    c: &Columns,
    labels: &[usize],
    count: usize,
    rng: &mut Rng,
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    let mut total = vec![0i32; count];
    for (f, face) in l.faces.iter().enumerate() {
        let d = face.defect(&l.edges);
        for k in 0..2 {
            total[labels[2 * f + k]] += d[k];
        }
    }
    let mut modifications = vec![vec![vec![]; count]; 2];
    for (i, col) in c.iter().enumerate() {
        let sum = col.iter().map(|x| x.1).sum::<i32>();
        if col.len() != 2 || sum != 0 {
            let group = labels[col[0].0];
            let parity = (sum.abs() % 2) as usize;
            let value = l.edges[i / 2].d[i % 2];
            let direction = -(total[group].signum() * sum.signum());
            if (direction == -1 && value > -1) || (direction == 1 && value < 1) {
                modifications[parity][group].push((i, direction));
            }
        }
    }
    for groups in &mut modifications {
        for a in groups {
            check(cancel)?;
            rng.shuffle(a);
        }
    }
    for (group, net) in total.iter_mut().enumerate() {
        for (kind, groups) in modifications.iter().enumerate() {
            let step = if kind == 0 { 2 } else { 1 };
            let dir = -net.signum();
            let amount = ((*net).unsigned_abs() as usize / step).min(groups[group].len());
            for &(i, d) in &groups[group][..amount] {
                l.edges[i / 2].d[i % 2] += d;
                *net += dir * step as i32;
            }
        }
    }
    // Like upstream, first (face) balancing can leave a nonzero component;
    // coordinate balancing and the checked max-flow pass decide feasibility.
    Ok(())
}

#[cfg(any(feature = "audit", test))]
pub(crate) fn solve(f: &Field, cancel: &dyn Fn() -> bool) -> Result<Lattice> {
    solve_with_count(f, cancel).map(|(lattice, _)| lattice)
}
pub(super) fn solve_with_count(f: &Field, cancel: &dyn Fn() -> bool) -> Result<(Lattice, usize)> {
    solve_observed(f, cancel, &mut |_, _| Ok(()))
}
pub(super) fn solve_observed(
    f: &Field,
    cancel: &dyn Fn() -> bool,
    observe: &mut dyn FnMut(&str, &Lattice) -> Result<()>,
) -> Result<(Lattice, usize)> {
    check(cancel)?;
    let mut lattice = Lattice::new(f, cancel)?;
    if lattice.edge_faces.iter().any(|v| v.len() != 2)
        || f.constraints.iter().any(|c| c.weight >= 1.)
    {
        return Err("Reference integer mapping currently requires a closed surface without hard constraints".into());
    }
    let mut q = f.directions.clone();
    let mut singular = vec![false; lattice.faces.len()];
    for (i, t) in f.surface.triangles.iter().enumerate() {
        if i.is_multiple_of(4096) {
            check(cancel)?;
        }
        let mut index = 0;
        for k in 0..3 {
            let a = t[k];
            let b = t[(k + 1) % 3];
            let (x, y) = indices(q[a], f.normals[a], q[b], f.normals[b]);
            index += y - x;
        }
        if matches!(index.rem_euclid(4), 1 | 3) {
            singular[i] = true;
            if !(0..4).contains(&index) {
                q[t[0]] = -q[t[0]];
            }
        }
    }
    let mut seen = vec![false; lattice.edges.len()];
    for (i, &t) in f.surface.triangles.iter().enumerate() {
        if i.is_multiple_of(1024) {
            check(cancel)?;
        }
        let frames = t.map(|v| q[v] / q[v].length());
        let normals = t.map(|v| f.normals[v]);
        let turns: [[_; 4]; 3] = std::array::from_fn(|k| {
            std::array::from_fn(|r| crate::field::rotate(frames[k], normals[k], r as i32))
        });
        let mut best = f64::NEG_INFINITY;
        let mut ranks = [0; 3];
        for a in 0..4 {
            for b in 0..4 {
                for c in 0..4 {
                    let score = turns[0][a]
                        .dot(turns[1][b])
                        .min(turns[1][b].dot(turns[2][c]))
                        .min(turns[2][c].dot(turns[0][a]));
                    if score > best {
                        best = score;
                        ranks = [a, b, c];
                    }
                }
            }
        }
        for k in 0..3 {
            let j = (k + 1) % 3;
            let e = lattice.faces[i].edges[k];
            if !seen[e] || !singular[i] {
                let d = position_indices(f, t[k], t[j], turns[k][ranks[k]], turns[j][ranks[j]]);
                lattice.edges[e].d = if t[k] > t[j] {
                    rot(-d, ranks[j] as i32)
                } else {
                    rot(d, ranks[k] as i32)
                };
            }
            seen[e] = true;
        }
        let (a, b) = indices(q[t[0]], normals[0], q[t[1]], normals[1]);
        let r1 = (a - b).rem_euclid(4);
        let (a, b) = indices(q[t[0]], normals[0], q[t[2]], normals[2]);
        let r2 = (a - b).rem_euclid(4);
        lattice.faces[i].rotations = [
            if t[1] < t[0] { (r1 + 2) % 4 } else { 0 },
            if t[2] < t[1] { (r2 + 2) % 4 } else { r1 },
            if t[2] < t[0] { r2 } else { 2 },
        ];
    }
    observe("seed", &lattice)?;
    let mut tree = Tree::new(lattice.faces.len());
    let merge = |tree: &mut Tree, e: usize| {
        let a = lattice.edge_faces[e][0];
        let b = lattice.edge_faces[e][1];
        let k = lattice.faces[a].edges.iter().position(|&v| v == e).unwrap();
        let j = lattice.faces[b].edges.iter().position(|&v| v == e).unwrap();
        tree.orient_merge(
            a,
            b,
            (lattice.faces[b].rotations[j] + 2) % 4,
            lattice.faces[a].rotations[k],
        );
    };
    for e in 0..lattice.edges.len() {
        if e.is_multiple_of(4096) {
            check(cancel)?;
        }
        let fs = &lattice.edge_faces[e];
        if !singular[fs[0]] && !singular[fs[1]] {
            merge(&mut tree, e);
        }
    }
    for (i, &s) in singular.iter().enumerate() {
        if s {
            for &e in &lattice.faces[i].edges {
                merge(&mut tree, e);
            }
        }
    }
    for (i, face) in lattice.faces.iter_mut().enumerate() {
        let turn = tree.find(i).1;
        for r in &mut face.rotations {
            *r = (*r + turn) % 4;
        }
    }
    let mut labels = vec![NONE; lattice.faces.len()];
    let mut count = 0;
    for start in 0..labels.len() {
        if labels[start] != NONE {
            continue;
        }
        labels[start] = count;
        let mut todo = VecDeque::from([start]);
        while let Some(v) = todo.pop_front() {
            if v.is_multiple_of(4096) {
                check(cancel)?;
            }
            for &e in &lattice.faces[v].edges {
                let a = lattice.edge_faces[e][0];
                let b = lattice.edge_faces[e][1];
                let k = lattice.faces[a].edges.iter().position(|&v| v == e).unwrap();
                let j = lattice.faces[b].edges.iter().position(|&v| v == e).unwrap();
                if (lattice.faces[a].rotations[k] - lattice.faces[b].rotations[j]).rem_euclid(4)
                    != 2
                {
                    continue;
                }
                for u in [a, b] {
                    if labels[u] == NONE {
                        labels[u] = count;
                        todo.push_back(u);
                    }
                }
            }
        }
        count += 1;
    }
    let cols = columns(&lattice);
    let fixed = cols
        .iter()
        .filter(|c| c.iter().map(|x| x.1).sum::<i32>() != 0)
        .count();
    let mut rng = Rng::new();
    let face_labels = labels.iter().flat_map(|&g| [g, g]).collect::<Vec<_>>();
    balance(&mut lattice, &cols, &face_labels, count, &mut rng, cancel)?;
    let mut tree = Tree::new(lattice.faces.len() * 2);
    for col in &cols {
        if col.len() == 2 && col[0].1 == -col[1].1 {
            tree.merge(col[0].0, col[1].0);
        }
    }
    let (labels, count) = tree.compact();
    balance(&mut lattice, &cols, &labels, count, &mut rng, cancel)?;
    observe("balanced", &lattice)?;
    let rows = lattice.faces.len() * 2;
    let mut complete = false;
    for capacity in 2..12 {
        check(cancel)?;
        let mut initial = vec![0i32; rows];
        let mut net = Network::new(rows + 2);
        let mut variables = vec![];
        for (i, col) in cols.iter().enumerate() {
            let value = lattice.edges[i / 2].d[i % 2];
            for &(r, s) in col {
                initial[r] += s * value;
            }
            if col.len() == 2 && col[0].1 == -col[1].1 {
                let (a, b) = if col[0].1 > 0 {
                    (col[0].0, col[1].0)
                } else {
                    (col[1].0, col[0].0)
                };
                let e = net.add(
                    a + 1,
                    b + 1,
                    (value + capacity).max(0),
                    (-value + capacity).max(0),
                );
                variables.push((i, e));
            }
        }
        let mut supply = 0;
        let mut demand = 0;
        for (i, &d) in initial.iter().enumerate() {
            if d > 0 {
                net.add(0, i + 1, d, 0);
                supply += d;
            } else if d < 0 {
                net.add(i + 1, rows + 1, -d, 0);
                demand -= d;
            }
        }
        if supply != demand {
            return Err("Reference seam balancing left unequal flow supply and demand".into());
        }
        let sent = net.solve(supply < 20, cancel)?;
        for (i, e) in variables {
            lattice.edges[i / 2].d[i % 2] -= net.used(e);
        }
        if sent == supply {
            complete = true;
            break;
        }
    }
    if !complete || lattice.defects() != 0 {
        return Err("Reference integer maximum flow did not close every cell".into());
    }
    observe("corrected", &lattice)?;
    // Restore the caller's unchanged fourfold field frames.
    let flipped = q
        .iter()
        .zip(&f.directions)
        .map(|(a, b)| a.dot(*b) < 0.)
        .collect::<Vec<_>>();
    for e in &mut lattice.edges {
        if flipped[e.ends[0]] {
            e.d = -e.d;
        }
    }
    for face in &mut lattice.faces {
        for k in 0..3 {
            if flipped[lattice.edges[face.edges[k]].ends[0]] {
                face.rotations[k] = (face.rotations[k] + 2) % 4;
            }
        }
    }
    if lattice.defects() != 0 {
        return Err("Reference integer frame restoration broke cell closure".into());
    }
    Ok((lattice, fixed))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn field(input: &str) -> Field {
        let mut words = input.split_whitespace();
        let n: usize = words.next().unwrap().parse().unwrap();
        let nf: usize = words.next().unwrap().parse().unwrap();
        let ne: usize = words.next().unwrap().parse().unwrap();
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut directions = Vec::new();
        let mut origins = Vec::new();
        let mut spacing = Vec::new();
        for _ in 0..n {
            let mut vector = || {
                Vec3::new(
                    words.next().unwrap().parse().unwrap(),
                    words.next().unwrap().parse().unwrap(),
                    words.next().unwrap().parse().unwrap(),
                )
            };
            positions.push(vector());
            normals.push(vector());
            directions.push(vector());
            origins.push(vector());
            spacing.push(words.next().unwrap().parse().unwrap());
        }
        let mut triangles = Vec::new();
        for _ in 0..nf {
            triangles.push(std::array::from_fn(|_| {
                words.next().unwrap().parse().unwrap()
            }));
            for _ in 0..6 {
                words.next().unwrap();
            }
        }
        for _ in 0..ne * 4 {
            words.next().unwrap();
        }
        assert!(words.next().is_none());
        Field {
            surface: crate::Surface {
                positions,
                triangles,
            },
            normals,
            directions,
            origins,
            spacing,
            areas: vec![1.; n],
            constraints: vec![crate::field::Constraint::default(); n],
        }
    }
    #[test]
    fn integer_grids_match_independent_pinned_cpp_fixtures() {
        for (input, expected) in [
            (
                include_str!("../../tests/data/integer-asymmetric-19.input"),
                include_str!("../../tests/data/integer-asymmetric-19.expected.json"),
            ),
            (
                include_str!("../../tests/data/integer-asymmetric-83.input"),
                include_str!("../../tests/data/integer-asymmetric-83.expected.json"),
            ),
        ] {
            let f = field(input);
            let lattice = solve(&f, &|| false).unwrap();
            let expected: serde_json::Value = serde_json::from_str(expected).unwrap();
            let edges = expected["edges"].as_array().unwrap();
            assert_eq!(edges.len(), lattice.edges.len());
            for (i, (e, r)) in lattice.edges.iter().zip(edges).enumerate() {
                let actual = [
                    e.ends[0] as i64,
                    e.ends[1] as i64,
                    e.d.x.into(),
                    e.d.y.into(),
                ];
                let wanted: [i64; 4] = std::array::from_fn(|k| r[k].as_i64().unwrap());
                assert_eq!(actual, wanted, "edge {i}");
            }
            let faces = expected["faces"].as_array().unwrap();
            assert_eq!(faces.len(), lattice.faces.len());
            for (i, (f, r)) in lattice.faces.iter().zip(faces).enumerate() {
                let actual = [
                    f.edges[0] as i64,
                    f.edges[1] as i64,
                    f.edges[2] as i64,
                    f.rotations[0].into(),
                    f.rotations[1].into(),
                    f.rotations[2].into(),
                ];
                let wanted: [i64; 6] = std::array::from_fn(|k| r[k].as_i64().unwrap());
                assert_eq!(actual, wanted, "face {i}");
            }
            assert_eq!(lattice.defects(), 0);
        }
    }
    #[test]
    fn reference_mapping_cancels_and_excludes_unported_hard_constraints() {
        let mut f = field(include_str!("../../tests/data/integer-asymmetric-19.input"));
        let calls = std::cell::Cell::new(0);
        assert!(
            solve(&f, &|| {
                calls.set(calls.get() + 1);
                calls.get() > 3
            })
            .is_err()
        );
        f.constraints[0].weight = 1.;
        assert!(matches!(solve(&f,&||false), Err(e) if e.contains("without hard constraints")));
    }
}
