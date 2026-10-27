//! Native adaptation of Practical Quad Mesh Simplification, Tarini et al. (2010).
//! https://vcg.isti.cnr.it/publication/2010/TPCPP10/
//! Uses their local operators and edge/diagonal length objective, not their code.
//! Coarse reconstruction allows 0.75 voxel error; the caller separately checks
//! the refined result in both directions against a stricter 0.4 voxel limit.
//! Coarsen with diagonal contractions and local rotations, then subdivide and
//! reproject. All projection follows connected reference faces.
use super::super::math::{Vec3, closest_triangle};
use std::collections::{BTreeMap, BTreeSet};

type Quad = [usize; 4];
fn edge(a: usize, b: usize) -> [usize; 2] {
    [a.min(b), a.max(b)]
}
fn triangles([a, b, c, d]: Quad) -> [[usize; 3]; 2] {
    [[a, b, c], [a, c, d]]
}
fn normal([a, b, c, d]: [Vec3; 4]) -> Vec3 {
    (b - a).cross(c - a) + (c - a).cross(d - a)
}
fn shape(p: [Vec3; 4]) -> f32 {
    let n = normal(p).norm();
    (0..4)
        .map(|i| {
            let a = p[(i + 1) % 4] - p[i];
            let b = p[(i + 3) % 4] - p[i];
            a.cross(b).dot(n) / (a.len() * b.len()).max(1e-20)
        })
        .fold(f32::INFINITY, f32::min)
}
fn samples([a, b, c]: [Vec3; 3]) -> [Vec3; 7] {
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

struct Reference {
    patches: Vec<Vec<usize>>,
    budget: usize,
    cached: Vec<RefTriangle>,
}
struct RefTriangle {
    a: Vec3,
    ab: Vec3,
    ac: Vec3,
    bc: Vec3,
    aa: f32,
    cc: f32,
    abac: f32,
    bb: f32,
    det: f32,
}
impl RefTriangle {
    fn new([a, b, c]: [Vec3; 3]) -> Self {
        let ab = b - a;
        let ac = c - a;
        let bc = c - b;
        let aa = ab.dot(ab);
        let cc = ac.dot(ac);
        let abac = ab.dot(ac);
        Self {
            a,
            ab,
            ac,
            bc,
            aa,
            cc,
            abac,
            bb: bc.dot(bc),
            det: aa * cc - abac * abac,
        }
    }
    fn closest(&self, p: Vec3) -> Vec3 {
        let ap = p - self.a;
        let x = ap.dot(self.ab);
        let y = ap.dot(self.ac);
        if self.det <= 1e-20 {
            return closest_triangle(p, self.a, self.a + self.ab, self.a + self.ac);
        }
        let v = (self.cc * x - self.abac * y) / self.det;
        let w = (self.aa * y - self.abac * x) / self.det;
        if v >= 0. && w >= 0. && v + w <= 1. {
            return self.a + self.ab * v + self.ac * w;
        }
        let p0 = self.a + self.ab * (x / self.aa).clamp(0., 1.);
        let p1 = self.a + self.ac * (y / self.cc).clamp(0., 1.);
        let p2 = self.a + self.ab + self.bc * ((ap - self.ab).dot(self.bc) / self.bb).clamp(0., 1.);
        let mut best = p0;
        let mut dist = (p - p0).dot(p - p0);
        for q in [p1, p2] {
            let d = (p - q).dot(p - q);
            if d < dist {
                best = q;
                dist = d;
            }
        }
        best
    }
}
impl Reference {
    fn new(
        p: &[Vec3],
        q: &[Quad],
        cancel: &dyn Fn() -> bool,
    ) -> Result<(Self, Vec<usize>), String> {
        let t: Vec<_> = q.iter().flat_map(|&q| triangles(q)).collect();
        let mut incident = vec![Vec::new(); p.len()];
        for (f, t) in t.iter().enumerate() {
            if f % 1024 == 0 && cancel() {
                return Err("Quad optimisation cancelled".into());
            }
            for &v in t {
                incident[v].push(f);
            }
        }
        let anchors = incident.iter().map(|f| f[0]).collect();
        let patches = t
            .iter()
            .map(|t| {
                let mut fs: Vec<_> = t
                    .iter()
                    .flat_map(|&v| incident[v].iter().copied())
                    .collect();
                fs.sort_unstable();
                fs.dedup();
                fs
            })
            .collect();
        let cached = t
            .iter()
            .map(|t| RefTriangle::new(t.map(|v| p[v])))
            .collect();
        Ok((
            Self {
                patches,
                budget: 500_000_000,
                cached,
            },
            anchors,
        ))
    }
    fn project(&mut self, p: Vec3, anchor: usize) -> Result<(Vec3, usize), String> {
        let mut seed = anchor;
        let mut best = p;
        let mut d = f32::INFINITY;
        for _ in 0..64 {
            let previous = seed;
            for &f in &self.patches[previous] {
                if self.budget == 0 {
                    return Err("Quad optimisation reached its projection work limit".into());
                }
                self.budget -= 1;
                let q = self.cached[f].closest(p);
                let dist = (q - p).dot(q - p);
                if dist < d {
                    d = dist;
                    best = q;
                    seed = f;
                }
            }
            if previous == seed {
                return Ok((best, seed));
            }
        }
        Err("Quad optimisation could not follow the reference surface".into())
    }
}

struct Work {
    p: Vec<Vec3>,
    q: Vec<Option<Quad>>,
    anchor: Vec<usize>,
    incident: Vec<BTreeSet<usize>>,
    edges: BTreeMap<[usize; 2], BTreeSet<usize>>,
    reference: Reference,
    tolerance: f32,
    count: usize,
    sample_anchors: BTreeMap<Quad, [usize; 8]>,
    dirty: BTreeSet<usize>,
}
impl Work {
    fn new(p: &[Vec3], q: &[Quad], cancel: &dyn Fn() -> bool) -> Result<Self, String> {
        let (reference, anchor) = Reference::new(p, q, cancel)?;
        let mut w = Self {
            p: p.to_vec(),
            q: vec![None; q.len()],
            anchor,
            incident: vec![BTreeSet::new(); p.len()],
            edges: BTreeMap::new(),
            reference,
            tolerance: 0.75,
            count: 0,
            sample_anchors: BTreeMap::new(),
            dirty: BTreeSet::new(),
        };
        for (f, &q) in q.iter().enumerate() {
            w.insert(f, q);
        }
        Ok(w)
    }
    fn insert(&mut self, f: usize, q: Quad) {
        self.q[f] = Some(q);
        self.count += 1;
        self.dirty.extend(q);
        for i in 0..4 {
            self.incident[q[i]].insert(f);
            self.edges
                .entry(edge(q[i], q[(i + 1) % 4]))
                .or_default()
                .insert(f);
        }
    }
    fn remove(&mut self, f: usize) {
        if let Some(q) = self.q[f].take() {
            self.sample_anchors.remove(&q);
            self.dirty.extend(q);
            self.count -= 1;
            for i in 0..4 {
                self.incident[q[i]].remove(&f);
                let e = edge(q[i], q[(i + 1) % 4]);
                let faces = self.edges.get_mut(&e).unwrap();
                faces.remove(&f);
                if faces.is_empty() {
                    self.edges.remove(&e);
                }
            }
        }
    }
    fn neighbors(&self, v: usize) -> BTreeSet<usize> {
        self.incident[v]
            .iter()
            .flat_map(|&f| {
                let q = self.q[f].unwrap();
                let i = q.iter().position(|&a| a == v).unwrap();
                [q[(i + 1) % 4], q[(i + 3) % 4]]
            })
            .collect()
    }
    fn valid(
        &mut self,
        faces: &[Quad],
        old_normal: Vec3,
        limits: Option<&[f32]>,
    ) -> Result<bool, String> {
        for (f, &q) in faces.iter().enumerate() {
            let p = q.map(|v| self.p[v]);
            if shape(p) < limits.map_or(0.3, |limits| limits[f])
                || normal(p).norm().dot(old_normal) < 0.4
            {
                return Ok(false);
            }
            let points: Vec<_> = triangles(q)
                .into_iter()
                .flat_map(|t| samples(t.map(|v| self.p[v])).into_iter().skip(3))
                .collect();
            let seeds = self.sample_anchors.entry(q).or_insert_with(|| {
                std::array::from_fn(|i| {
                    let v = *q
                        .iter()
                        .min_by(|&&a, &&b| {
                            (self.p[a] - points[i])
                                .len()
                                .total_cmp(&(self.p[b] - points[i]).len())
                        })
                        .unwrap();
                    self.anchor[v]
                })
            });
            for (i, p) in points.into_iter().enumerate() {
                let (nearest, seed) = self.reference.project(p, seeds[i])?;
                seeds[i] = seed;
                if (nearest - p).len() > self.tolerance {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    fn rotate_edges(&mut self, cancel: &dyn Fn() -> bool) -> Result<usize, String> {
        let edges: Vec<_> = self.edges.keys().copied().collect();
        let mut changes = 0;
        for (index, e) in edges.into_iter().enumerate() {
            if index % 256 == 0 && cancel() {
                return Err("Quad optimisation cancelled".into());
            }
            let Some(fs) = self.edges.get(&e) else {
                continue;
            };
            if fs.len() != 2 {
                continue;
            }
            let ids: Vec<_> = fs.iter().copied().collect();
            let mut a = self.q[ids[0]].unwrap();
            let mut b = self.q[ids[1]].unwrap();
            let i = (0..4).find(|&i| edge(a[i], a[(i + 1) % 4]) == e).unwrap();
            a.rotate_left(i);
            let i = (0..4)
                .find(|&i| b[i] == a[1] && b[(i + 1) % 4] == a[0])
                .unwrap();
            b.rotate_left(i);
            let ring = [a[0], b[2], b[3], a[1], a[2], a[3]];
            if ring.iter().copied().collect::<BTreeSet<_>>().len() != 6 {
                continue;
            }
            let n = normal(a.map(|v| self.p[v])) + normal(b.map(|v| self.p[v]));
            let length = |a: usize, b: usize| (self.p[a] - self.p[b]).len();
            let before = length(a[0], a[1]);
            let mut best = None;
            for k in 1..=2 {
                let mut r = ring;
                r.rotate_left(k);
                if self.edges.contains_key(&edge(r[0], r[3])) {
                    continue;
                }
                let gain = before - length(r[0], r[3]);
                if gain <= 1e-6 {
                    continue;
                }
                let qs = [[r[0], r[1], r[2], r[3]], [r[3], r[4], r[5], r[0]]];
                let diag = |faces: &[Quad]| {
                    faces
                        .iter()
                        .map(|q| length(q[0], q[2]) + length(q[1], q[3]))
                        .sum::<f32>()
                };
                if diag(&qs) >= diag(&[a, b]) {
                    continue;
                }
                if [a[0], a[1]].iter().any(|&v| self.incident[v].len() <= 3) {
                    continue;
                }
                if best.as_ref().is_none_or(|&(g, _)| gain > g) {
                    best = Some((gain, qs));
                }
            }
            if let Some((_, qs)) = best {
                if !self.valid(&qs, n.norm(), None)? {
                    continue;
                }
                for &f in &ids {
                    self.remove(f);
                }
                for (&f, q) in ids.iter().zip(qs) {
                    self.insert(f, q);
                }
                changes += 1;
            }
        }
        Ok(changes)
    }
    fn collapse(&mut self, target: usize, cancel: &dyn Fn() -> bool) -> Result<usize, String> {
        let mut candidates = Vec::new();
        for (f, q) in self.q.iter().enumerate() {
            if let Some(q) = q {
                for r in 0..2 {
                    candidates.push((
                        (self.p[q[r]] - self.p[q[r + 2]]).len() / std::f32::consts::SQRT_2,
                        f,
                        r,
                        *q,
                    ));
                }
            }
        }
        for &[a, b] in self.edges.keys() {
            candidates.push(((self.p[a] - self.p[b]).len(), usize::MAX, 2, [a, b, 0, 0]));
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut count = 0;
        for (index, (_, f, r, mut q)) in candidates.into_iter().enumerate() {
            if self.count <= target || count >= 64 {
                break;
            }
            if index % 256 == 0 && cancel() {
                return Err("Quad optimisation cancelled".into());
            }
            if r == 2 {
                if self.collapse_edge(q[0], q[1])? || self.collapse_edge(q[1], q[0])? {
                    count += 1;
                }
                continue;
            }
            if self.q[f] != Some(q) {
                continue;
            }
            q.rotate_left(r);
            let [a, b, c, d] = q;
            if self.incident[b].len() < 4
                || self.incident[d].len() < 4
                || self.incident[a].len() + self.incident[c].len() > 8
                || self.incident[a].intersection(&self.incident[c]).count() != 1
                || self
                    .neighbors(a)
                    .intersection(&self.neighbors(c))
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != BTreeSet::from([b, d])
            {
                continue;
            }
            let affected: Vec<_> = self.incident[a].union(&self.incident[c]).copied().collect();
            let old: Vec<_> = affected.iter().map(|&f| self.q[f].unwrap()).collect();
            let n = old
                .iter()
                .fold(Vec3::default(), |n, q| n + normal(q.map(|v| self.p[v])))
                .norm();
            let before = self.p[a];
            let (position, anchor) = self
                .reference
                .project((self.p[a] + self.p[c]) * 0.5, self.anchor[a])?;
            let next: Vec<_> = affected
                .iter()
                .filter(|&&fi| fi != f)
                .map(|&fi| (fi, self.q[fi].unwrap().map(|v| if v == c { a } else { v })))
                .collect();
            self.p[a] = position;
            let safe = self.valid(&next.iter().map(|&(_, q)| q).collect::<Vec<_>>(), n, None)?;
            if !safe {
                self.p[a] = before;
                continue;
            }
            for &f in &affected {
                self.remove(f);
            }
            for (f, q) in next {
                self.insert(f, q);
            }
            self.anchor[a] = anchor;
            count += 1;
        }
        Ok(count)
    }
    fn collapse_edge(&mut self, a: usize, b: usize) -> Result<bool, String> {
        if !self.edges.contains_key(&edge(a, b)) {
            return Ok(false);
        }
        // Atomic vertex rotation + diagonal contraction (Tarini §3.2.2).
        // The rotation need not be profitable on its own.
        let mut ring = BTreeMap::new();
        for &f in &self.incident[a] {
            let mut q = self.q[f].unwrap();
            let i = q.iter().position(|&v| v == a).unwrap();
            q.rotate_left(i);
            ring.insert(q[1], (f, q[2], q[3]));
        }
        let start = *ring.keys().next().unwrap();
        let mut cursor = start;
        let mut ordered = Vec::new();
        for _ in 0..ring.len() {
            let Some(&(f, y, z)) = ring.get(&cursor) else {
                return Ok(false);
            };
            ordered.push((f, cursor, y, z));
            cursor = z;
        }
        if cursor != start
            || ordered
                .iter()
                .flat_map(|&(_, x, y, _)| [x, y])
                .collect::<BTreeSet<_>>()
                .len()
                != 2 * ring.len()
        {
            return Ok(false);
        }
        let affected: BTreeSet<_> = self.incident[a].union(&self.incident[b]).copied().collect();
        let mut replacements: BTreeMap<_, _> =
            affected.iter().map(|&f| (f, self.q[f].unwrap())).collect();
        let mut removed = None;
        for (i, &(f, _, y, z)) in ordered.iter().enumerate() {
            if z == b {
                replacements.remove(&f);
                removed = Some(f);
            } else {
                replacements.insert(f, [a, y, z, ordered[(i + 1) % ordered.len()].2]);
            }
        }
        if removed.is_none() {
            return Ok(false);
        }
        for q in replacements.values_mut() {
            for v in q {
                if *v == b {
                    *v = a;
                }
            }
        }
        if !self.valid_patch(&affected, &replacements)? {
            return Ok(false);
        }
        let n = affected
            .iter()
            .fold(Vec3::default(), |n, &f| {
                n + normal(self.q[f].unwrap().map(|v| self.p[v]))
            })
            .norm();
        let old = self.p[a];
        let (p, anchor) = self
            .reference
            .project((self.p[a] + self.p[b]) * 0.5, self.anchor[a])?;
        self.p[a] = p;
        if !self.valid(&replacements.values().copied().collect::<Vec<_>>(), n, None)? {
            self.p[a] = old;
            return Ok(false);
        }
        for f in affected {
            self.remove(f);
        }
        for (f, q) in replacements {
            self.insert(f, q);
        }
        self.anchor[a] = anchor;
        Ok(true)
    }
    fn valid_patch(
        &self,
        affected: &BTreeSet<usize>,
        replacement: &BTreeMap<usize, Quad>,
    ) -> Result<bool, String> {
        let mut delta: BTreeMap<[usize; 2], i32> = BTreeMap::new();
        let mut vertices = BTreeSet::new();
        let mut faces = BTreeSet::new();
        for &f in affected {
            let q = self.q[f].unwrap();
            vertices.extend(q);
            for i in 0..4 {
                *delta.entry(edge(q[i], q[(i + 1) % 4])).or_default() -= 1;
            }
        }
        for &q in replacement.values() {
            let mut key = q;
            key.sort_unstable();
            if key.windows(2).any(|p| p[0] == p[1]) || !faces.insert(key) {
                return Ok(false);
            }
            for &f in &self.incident[q[0]] {
                if !affected.contains(&f) {
                    let mut other = self.q[f].unwrap();
                    other.sort_unstable();
                    if other == key {
                        return Ok(false);
                    }
                }
            }
            vertices.extend(q);
            for i in 0..4 {
                *delta.entry(edge(q[i], q[(i + 1) % 4])).or_default() += 1;
            }
        }
        for (e, change) in delta {
            let count = self.edges.get(&e).map_or(0, |f| f.len()) as i32 + change;
            if count != 0 && count != 2 {
                return Ok(false);
            }
        }
        for v in vertices {
            let qs = self.incident[v]
                .iter()
                .filter(|f| !affected.contains(f))
                .map(|&f| self.q[f].unwrap())
                .chain(replacement.values().filter(|q| q.contains(&v)).copied());
            let mut next = BTreeMap::new();
            for q in qs {
                let i = q.iter().position(|&a| a == v).unwrap();
                if next.insert(q[(i + 3) % 4], q[(i + 1) % 4]).is_some() {
                    return Ok(false);
                }
            }
            if next.is_empty() {
                continue;
            }
            if !(3..=6).contains(&next.len()) {
                return Ok(false);
            }
            let start = *next.keys().next().unwrap();
            let mut cursor = start;
            for i in 0..next.len() {
                let Some(&n) = next.get(&cursor) else {
                    return Ok(false);
                };
                cursor = n;
                if cursor == start && i + 1 != next.len() {
                    return Ok(false);
                }
            }
            if cursor != start {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn rotate_vertices(&mut self, cancel: &dyn Fn() -> bool) -> Result<usize, String> {
        let mut count = 0;
        for v in 0..self.p.len() {
            if v % 256 == 0 && cancel() {
                return Err("Quad optimisation cancelled".into());
            }
            if self.incident[v].is_empty() {
                continue;
            }
            let mut ring = BTreeMap::new();
            for &f in &self.incident[v] {
                let mut q = self.q[f].unwrap();
                let i = q.iter().position(|&i| i == v).unwrap();
                q.rotate_left(i);
                ring.insert(q[1], (f, q[2], q[3]));
            }
            let start = *ring.keys().next().unwrap();
            let mut cursor = start;
            let mut ordered = Vec::new();
            let mut valid = true;
            for _ in 0..ring.len() {
                let Some(&(f, b, c)) = ring.get(&cursor) else {
                    valid = false;
                    break;
                };
                if self.incident[cursor].len() <= 3
                    || self.incident[b].len() >= 6
                    || self.edges.contains_key(&edge(v, b))
                {
                    valid = false;
                    break;
                }
                ordered.push((f, cursor, b, c));
                cursor = c;
            }
            if !valid
                || cursor != start
                || ordered
                    .iter()
                    .flat_map(|&(_, a, b, _)| [a, b])
                    .collect::<BTreeSet<_>>()
                    .len()
                    != 2 * ring.len()
            {
                continue;
            }
            let gain = ordered
                .iter()
                .map(|&(_, a, b, _)| (self.p[a] - self.p[v]).len() - (self.p[b] - self.p[v]).len())
                .sum::<f32>();
            if gain <= 1e-5 {
                continue;
            }
            let n = ordered
                .iter()
                .fold(Vec3::default(), |n, &(f, _, _, _)| {
                    n + normal(self.q[f].unwrap().map(|v| self.p[v]))
                })
                .norm();
            let qs: Vec<_> = ordered
                .iter()
                .enumerate()
                .map(|(i, &(_, _, b, c))| [v, b, c, ordered[(i + 1) % ordered.len()].2])
                .collect();
            if !self.valid(&qs, n, None)? {
                continue;
            }
            for &(f, _, _, _) in &ordered {
                self.remove(f);
            }
            for (&(f, _, _, _), q) in ordered.iter().zip(qs) {
                self.insert(f, q);
            }
            count += 1;
        }
        Ok(count)
    }
    fn smooth(&mut self, cancel: &dyn Fn() -> bool) -> Result<(), String> {
        self.smooth_region(cancel, None)
    }
    fn smooth_region(
        &mut self,
        cancel: &dyn Fn() -> bool,
        region: Option<&BTreeSet<usize>>,
    ) -> Result<(), String> {
        let mean_length = self
            .edges
            .keys()
            .map(|&[a, b]| (self.p[a] - self.p[b]).len())
            .sum::<f32>()
            / self.edges.len() as f32;
        for v in 0..self.p.len() {
            if v % 256 == 0 && cancel() {
                return Err("Quad optimisation cancelled".into());
            }
            if self.incident[v].is_empty() || region.is_some_and(|r| !r.contains(&v)) {
                continue;
            }
            let adjacent = self.neighbors(v);
            let before = self.p[v];
            let faces: Vec<_> = self.incident[v]
                .iter()
                .map(|&f| self.q[f].unwrap())
                .collect();
            let limits: Vec<_> = faces
                .iter()
                .map(|q| (shape(q.map(|v| self.p[v])) * 0.999).min(0.4))
                .collect();
            let n = faces
                .iter()
                .fold(Vec3::default(), |n, q| n + normal(q.map(|i| self.p[i])))
                .norm();
            let mut d = Vec3::default();
            for &j in &adjacent {
                let e = self.p[j] - before;
                d = d + e * (1. - mean_length / e.len().max(1e-8));
            }
            for q in &faces {
                let i = q.iter().position(|&i| i == v).unwrap();
                let e = self.p[q[(i + 2) % 4]] - before;
                d = d + e * (1. - std::f32::consts::SQRT_2 * mean_length / e.len().max(1e-8));
            }
            let d = d * (0.7 / (adjacent.len() + faces.len()) as f32);
            let delta = d - n * d.dot(n);
            if delta.len() < 1e-6 {
                continue;
            }
            let (next, anchor) = self.reference.project(before + delta, self.anchor[v])?;
            self.p[v] = next;
            if self.valid(&faces, n, Some(&limits))? {
                self.anchor[v] = anchor;
            } else {
                self.p[v] = before;
            }
        }
        Ok(())
    }
    fn subdivide(&mut self, cancel: &dyn Fn() -> bool) -> Result<Vec<Quad>, String> {
        if self.count > 125_000 || self.p.len().saturating_add(self.count * 3) > 1_500_000 {
            return Err("Optimized quads exceeds mesh limits; choose a lower resolution".into());
        }
        let mut midpoints = BTreeMap::new();
        let mut out = Vec::with_capacity(self.count * 4);
        for f in 0..self.q.len() {
            if f % 256 == 0 && cancel() {
                return Err("Quad optimisation cancelled".into());
            }
            let Some(q) = self.q[f] else {
                continue;
            };
            let mut mids = [0; 4];
            for i in 0..4 {
                let e = edge(q[i], q[(i + 1) % 4]);
                let id = if let Some(&id) = midpoints.get(&e) {
                    id
                } else {
                    let (p, anchor) = self
                        .reference
                        .project((self.p[e[0]] + self.p[e[1]]) * 0.5, self.anchor[e[0]])?;
                    let id = self.p.len();
                    self.p.push(p);
                    self.anchor.push(anchor);
                    midpoints.insert(e, id);
                    id
                };
                mids[i] = id;
            }
            let (p, anchor) = self.reference.project(
                q.iter().fold(Vec3::default(), |p, &v| p + self.p[v]) * 0.25,
                self.anchor[q[0]],
            )?;
            let center = self.p.len();
            self.p.push(p);
            self.anchor.push(anchor);
            for i in 0..4 {
                out.push([q[i], mids[i], center, mids[(i + 3) % 4]]);
            }
        }
        Ok(out)
    }
}

pub(crate) fn optimize(
    p: &[Vec3],
    q: &[Quad],
    step: f32,
    cancel: &dyn Fn() -> bool,
) -> Result<(Vec<Vec3>, Vec<Quad>), String> {
    if cancel() {
        return Err("Quad optimisation cancelled".into());
    }
    // Caller validates closed polygon fans. These checks also protect direct tests.
    if p.is_empty()
        || q.is_empty()
        || !step.is_finite()
        || step <= 0.
        || p.iter()
            .any(|p| !p.0.is_finite() || !p.1.is_finite() || !p.2.is_finite())
        || q.iter().flatten().any(|&v| v >= p.len())
    {
        return Err("Invalid quad optimisation input".into());
    }
    let mut degree = vec![0; p.len()];
    for &v in q.iter().flatten() {
        degree[v] += 1;
    }
    if degree.contains(&0) {
        return Err("Quad optimisation requires referenced vertices".into());
    }
    // Already regular closed layouts (e.g. a cube grid or a regular torus)
    // have no excess-valence junctions to remove. Keep their sharp corners.
    if q.len() < 96 || degree.iter().all(|&n| (3..=4).contains(&n)) {
        return Ok((p.to_vec(), q.to_vec()));
    }
    // Thin/high-curvature shapes may not admit a 16x coarser layout. Retry
    // at 4x instead, keeping the requested final density with one subdivision.
    for levels in [2, 1] {
        if let Some(result) = optimize_levels(p, q, step, cancel, levels)? {
            return Ok(result);
        }
    }
    Err("Optimized quads could not simplify this surface safely. Choose Surface nets or Relaxed quads; the original model is unchanged.".into())
}

type Output = (Vec<Vec3>, Vec<Quad>);
pub(super) fn optimize_levels(
    p: &[Vec3],
    q: &[Quad],
    step: f32,
    cancel: &dyn Fn() -> bool,
    levels: u32,
) -> Result<Option<Output>, String> {
    let origin = p[0];
    let normalized: Vec<_> = p.iter().map(|&p| (p - origin) * (1. / step)).collect();
    let mut w = Work::new(&normalized, q, cancel)?;
    let target = (q.len() / 4usize.pow(levels)).max(6);
    for _ in 0..6 {
        w.smooth(cancel)?;
    }
    w.dirty.clear();
    // Batches limit how far topology can change before local redistribution.
    for pass in 0..q.len() {
        let rotated = if pass % 4 == 0 {
            w.rotate_edges(cancel)? + w.rotate_vertices(cancel)?
        } else {
            0
        };
        let collapsed = w.collapse(target, cancel)?;
        let mut dirty = std::mem::take(&mut w.dirty);
        let next: Vec<_> = dirty.iter().flat_map(|&v| w.neighbors(v)).collect();
        dirty.extend(next);
        for _ in 0..2 {
            w.smooth_region(cancel, Some(&dirty))?;
        }
        if w.count <= target || (collapsed == 0 && rotated == 0) {
            break;
        }
    }
    for _ in 0..32 {
        w.rotate_edges(cancel)?;
        w.rotate_vertices(cancel)?;
        w.smooth(cancel)?;
    }
    if w.count > target {
        return Ok(None);
    }
    for _ in 0..levels {
        let refined = w.subdivide(cancel)?;
        w.q = vec![None; refined.len()];
        w.incident = vec![BTreeSet::new(); w.p.len()];
        w.edges.clear();
        w.sample_anchors.clear();
        w.count = 0;
        for (f, q) in refined.into_iter().enumerate() {
            w.insert(f, q);
        }
        for _ in 0..3 {
            w.smooth(cancel)?;
        }
    }
    for _ in 0..16 {
        w.smooth(cancel)?;
    }
    let mut ids = vec![usize::MAX; w.p.len()];
    let mut vertices = Vec::new();
    let mut quads = Vec::new();
    for q in w.q.into_iter().flatten() {
        quads.push(q.map(|v| {
            if ids[v] == usize::MAX {
                ids[v] = vertices.len();
                vertices.push(w.p[v] * step + origin);
            }
            ids[v]
        }));
    }
    Ok(Some((vertices, quads)))
}
