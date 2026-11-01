//! Bounded triangle queries for local reconstruction and connector validation.
use super::*;

#[derive(Clone, Copy)]
struct Bounds {
    low: [f32; 3],
    high: [f32; 3],
}
fn xyz(v: Vec3) -> [f32; 3] {
    [v.0, v.1, v.2]
}
impl Bounds {
    fn empty() -> Self {
        Self {
            low: [f32::INFINITY; 3],
            high: [f32::NEG_INFINITY; 3],
        }
    }
    fn add(&mut self, p: Vec3) {
        for (k, v) in xyz(p).into_iter().enumerate() {
            self.low[k] = self.low[k].min(v);
            self.high[k] = self.high[k].max(v);
        }
    }
    fn triangle(p: [Vec3; 3]) -> Self {
        let mut b = Self::empty();
        for p in p {
            b.add(p);
        }
        b
    }
    fn overlaps(self, other: Self) -> bool {
        (0..3).all(|i| self.low[i] <= other.high[i] && other.low[i] <= self.high[i])
    }
    fn contains(self, other: Self) -> bool {
        (0..3).all(|i| self.low[i] <= other.low[i] && self.high[i] >= other.high[i])
    }

    fn ray(self, p: Vec3, d: Vec3) -> bool {
        let mut lo = 0_f64;
        let mut hi = f64::INFINITY;
        for i in 0..3 {
            let p = xyz(p)[i] as f64;
            let d = xyz(d)[i] as f64;
            if d.abs() < 1e-15 {
                if p < self.low[i] as f64 || p > self.high[i] as f64 {
                    return false;
                }
            } else {
                let a = (self.low[i] as f64 - p) / d;
                let b = (self.high[i] as f64 - p) / d;
                lo = lo.max(a.min(b));
                hi = hi.min(a.max(b));
            }
        }
        hi >= lo
    }
}
struct Node {
    bounds: Bounds,
    range: std::ops::Range<usize>,
    children: Option<[usize; 2]>,
}
pub(super) struct Index<'a> {
    mesh: &'a Mesh,
    faces: Vec<usize>,
    nodes: Vec<Node>,
}
impl<'a> Index<'a> {
    pub fn new(mesh: &'a Mesh, cancel: &dyn Fn() -> bool) -> Result<Self, String> {
        Self::from_faces(mesh, (0..mesh.triangles.len()).collect(), cancel)
    }

    pub fn from_faces(
        mesh: &'a Mesh,
        faces: Vec<usize>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        let mut s = Self {
            mesh,
            faces,
            nodes: Vec::new(),
        };
        if !s.faces.is_empty() {
            s.build(0..s.faces.len(), cancel)?;
        }
        Ok(s)
    }
    fn build(
        &mut self,
        range: std::ops::Range<usize>,
        cancel: &dyn Fn() -> bool,
    ) -> Result<usize, String> {
        cancelled(cancel)?;
        let mut bounds = Bounds::empty();
        for &f in &self.faces[range.clone()] {
            for v in self.mesh.triangles[f] {
                bounds.add(self.mesh.vertices[v]);
            }
        }
        let id = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            range: range.clone(),
            children: None,
        });
        if range.len() > 12 {
            let axis = (0..3)
                .max_by(|&a, &b| {
                    (bounds.high[a] - bounds.low[a]).total_cmp(&(bounds.high[b] - bounds.low[b]))
                })
                .unwrap();
            let mid = range.len() / 2;
            let mesh = self.mesh;
            let center = |f: usize| {
                mesh.triangles[f]
                    .iter()
                    .map(|&v| xyz(mesh.vertices[v])[axis])
                    .sum::<f32>()
            };
            self.faces[range.clone()].select_nth_unstable_by(mid, |&a, &b| {
                center(a).total_cmp(&center(b)).then(a.cmp(&b))
            });
            let split = range.start + mid;
            self.nodes[id].children = Some([
                self.build(range.start..split, cancel)?,
                self.build(split..range.end, cancel)?,
            ]);
        }
        Ok(id)
    }
    pub fn contacts(&self, t: [Vec3; 3], mut visit: impl FnMut(usize)) {
        if self.nodes.is_empty() {
            return;
        }
        let bounds = Bounds::triangle(t);
        let mut todo = vec![0];
        while let Some(id) = todo.pop() {
            let n = &self.nodes[id];
            if !n.bounds.overlaps(bounds) {
                continue;
            }
            if let Some(children) = n.children {
                todo.extend(children);
            } else {
                for &f in &self.faces[n.range.clone()] {
                    let q = self.mesh.triangles[f].map(|v| self.mesh.vertices[v]);
                    if Bounds::triangle(q).overlaps(bounds) && intersects(t, q) {
                        visit(f);
                    }
                }
            }
        }
    }
    pub fn inside(&self, p: Vec3) -> Result<bool, String> {
        let hits = self.ray_crossings(p, |_| true, |hits: &mut usize, _| *hits += 1)?;
        Ok(hits % 2 == 1)
    }

    fn ray_crossings<T: Default>(
        &self,
        p: Vec3,
        include: impl Fn(usize) -> bool,
        record: impl Fn(&mut T, usize),
    ) -> Result<T, String> {
        if self.nodes.is_empty() {
            return Ok(T::default());
        }
        for direction in [
            Vec3(1., 0.371, 0.193),
            Vec3(0.217, 1., 0.419),
            Vec3(0.513, 0.231, 1.),
        ] {
            let mut crossings = T::default();
            let mut ambiguous = false;
            let mut todo = vec![0];
            while let Some(id) = todo.pop() {
                let n = &self.nodes[id];
                if !n.bounds.ray(p, direction) {
                    continue;
                }
                if let Some(children) = n.children {
                    todo.extend(children);
                } else {
                    for &f in &self.faces[n.range.clone()] {
                        if !include(f) {
                            continue;
                        }
                        let q = self.mesh.triangles[f].map(|v| self.mesh.vertices[v]);
                        if let Some([t, u, v]) = ray_hit(p, direction, q)
                            && t >= -1e-8
                            && u >= -1e-8
                            && v >= -1e-8
                            && u + v <= 1. + 1e-8
                        {
                            if t < 1e-8 || u < 1e-8 || v < 1e-8 || 1. - u - v < 1e-8 {
                                ambiguous = true;
                                break;
                            }
                            record(&mut crossings, f);
                        }
                    }
                }
                if ambiguous {
                    break;
                }
            }
            if !ambiguous {
                return Ok(crossings);
            }
        }
        Err("The surfaces touch or coincide ambiguously; adjust their overlap before fusing".into())
    }
}

type V = [f64; 3];
fn v(p: Vec3) -> V {
    [p.0 as f64, p.1 as f64, p.2 as f64]
}
fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn ray_hit(p: Vec3, d: Vec3, t: [Vec3; 3]) -> Option<[f64; 3]> {
    let [a, b, c] = t.map(v);
    let e = sub(b, a);
    let f = sub(c, a);
    let h = cross(v(d), f);
    let det = dot(e, h);
    if det.abs() < 1e-12 * (dot(e, e) * dot(h, h)).sqrt() {
        return None;
    }
    let s = sub(v(p), a);
    let q = cross(s, e);
    Some([dot(f, q) / det, dot(s, h) / det, dot(v(d), q) / det])
}
fn intersects(a: [Vec3; 3], b: [Vec3; 3]) -> bool {
    let scale = a
        .iter()
        .chain(&b)
        .flat_map(|p| a.iter().chain(&b).map(move |q| (*p - *q).len()))
        .fold(0_f32, f32::max);
    let epsilon = scale * 1e-6;
    let common = a
        .iter()
        .copied()
        .filter(|p| b.iter().any(|q| (*p - *q).len() < epsilon))
        .collect::<Vec<_>>();
    for (p, q) in [(a, b), (b, a)] {
        for i in 0..3 {
            let d = p[(i + 1) % 3] - p[i];
            if let Some([t, u, w]) = ray_hit(p[i], d, q)
                && (-1e-8..=1. + 1e-8).contains(&t)
                && u >= -1e-8
                && w >= -1e-8
                && u + w <= 1. + 1e-8
            {
                let start = v(p[i]);
                let direction = v(d);
                let point = [
                    start[0] + direction[0] * t,
                    start[1] + direction[1] * t,
                    start[2] + direction[2] * t,
                ];
                if common.iter().any(|&c| {
                    let d = sub(point, v(c));
                    dot(d, d) < (epsilon as f64).powi(2)
                }) {
                    continue;
                }
                if common.len() == 2 {
                    let edge = sub(v(common[1]), v(common[0]));
                    let cross = cross(sub(point, v(common[0])), edge);
                    if dot(cross, cross) < (epsilon as f64).powi(2) * dot(edge, edge) {
                        continue;
                    }
                }
                return true;
            }
        }
    }
    // Coplanar triangles overlap only if every projected separating axis has
    // positive interior overlap. Shared vertices/edges alone are allowed.
    let n = (a[1] - a[0]).cross(a[2] - a[0]).norm();
    if b.iter().any(|p| (*p - a[0]).dot(n).abs() > epsilon) {
        return false;
    }
    for p in [a, b] {
        for i in 0..3 {
            let axis = (p[(i + 1) % 3] - p[i]).cross(n).norm();
            let span = |t: [Vec3; 3]| {
                let x = t.map(|p| dot(sub(v(p), v(a[0])), v(axis)));
                (
                    x.into_iter().fold(f64::INFINITY, f64::min),
                    x.into_iter().fold(f64::NEG_INFINITY, f64::max),
                )
            };
            let x = span(a);
            let y = span(b);
            if x.1.min(y.1) - x.0.max(y.0) <= epsilon as f64 {
                return false;
            }
        }
    }
    true
}

pub(super) fn validate_connector(
    mesh: &Mesh,
    first_new: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let index = Index::new(mesh, cancel)?;
    for i in first_new..mesh.triangles.len() {
        if i % 128 == 0 {
            cancelled(cancel)?;
        }
        let mut hit = false;
        index.contacts(mesh.triangles[i].map(|v| mesh.vertices[v]), |j| {
            if j < i {
                hit = true;
            }
        });
        if hit {
            return Err(
                "The connector intersects a surface; adjust alignment, curvature or the openings"
                    .into(),
            );
        }
    }
    Ok(())
}

struct Shell {
    bounds: Bounds,
    sample: Vec3,
    outward: bool,
}

/// On disjoint, closed manifold shells, a point on one shell is inside
/// another iff the entire shell is enclosed. Count ray hits separately for
/// each potential container so double nesting cannot cancel to "outside".
/// Exclude the sampled shell itself from the ray test.
pub(super) fn enclosed_triangles(
    mesh: &Mesh,
    outward_exterior: bool,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<bool>, String> {
    let mut incident = vec![Vec::new(); mesh.vertices.len()];
    for (f, t) in mesh.triangles.iter().enumerate() {
        for &v in t {
            incident[v].push(f);
        }
    }
    let mut shell_of = vec![usize::MAX; mesh.triangles.len()];
    let mut shells = Vec::new();
    for first in 0..mesh.triangles.len() {
        if shell_of[first] != usize::MAX {
            continue;
        }
        let origin = v(mesh.vertices[mesh.triangles[first][0]]);
        let mut volume = 0.;
        let mut pending = vec![first];
        let mut bounds = Bounds::empty();
        shell_of[first] = shells.len();
        while let Some(f) = pending.pop() {
            if f % 256 == 0 {
                cancelled(cancel)?;
            }
            let points = mesh.triangles[f].map(|i| mesh.vertices[i]);
            for p in points {
                bounds.add(p);
            }
            let [a, b, c] = points.map(|p| sub(v(p), origin));
            volume += dot(a, cross(b, c));
            for i in mesh.triangles[f] {
                for &other in &incident[i] {
                    if shell_of[other] == usize::MAX {
                        shell_of[other] = shells.len();
                        pending.push(other);
                    }
                }
            }
        }
        let extent = (0..3)
            .map(|i| (bounds.high[i] - bounds.low[i]) as f64)
            .fold(0., f64::max);
        shells.push(Shell {
            bounds,
            sample: mesh.vertices[mesh.triangles[first][0]],
            outward: volume > extent.powi(3) * 1e-12,
        });
    }
    let mut enclosed = vec![false; shells.len()];
    if shells.len() > 1 {
        let index = Index::new(mesh, cancel)?;
        for (id, shell) in shells.iter().enumerate() {
            cancelled(cancel)?;
            let containers = index.ray_crossings(
                shell.sample,
                |f| shell_of[f] != id && shells[shell_of[f]].bounds.contains(shell.bounds),
                |odd: &mut BTreeSet<usize>, f| {
                    let other = shell_of[f];
                    if !odd.insert(other) {
                        odd.remove(&other);
                    }
                },
            )?;
            enclosed[id] = !containers.is_empty();
        }
    }
    if outward_exterior
        && shells
            .iter()
            .zip(&enclosed)
            .any(|(shell, inside)| !inside && !shell.outward)
    {
        return Err("Intersection fusion requires outward-facing exterior solid shells".into());
    }
    Ok(shell_of.into_iter().map(|shell| enclosed[shell]).collect())
}
