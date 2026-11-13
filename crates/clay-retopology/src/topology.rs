use crate::{Result, Surface, Vec3};
use std::collections::{BTreeMap, VecDeque};

pub(crate) fn edge(a: usize, b: usize) -> [usize; 2] {
    if a < b { [a, b] } else { [b, a] }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Topology {
    components: usize,
    euler: isize,
    boundaries: usize,
}

pub(crate) fn validate(mesh: &Surface) -> Result<Topology> {
    validate_impl(mesh, true)
}
/// Intermediate extraction still needs continuous position fitting. Only the
/// final public result may pass the full geometric validation above.
pub(crate) fn validate_connectivity(mesh: &Surface) -> Result<Topology> {
    validate_impl(mesh, false)
}
fn validate_impl(mesh: &Surface, geometry: bool) -> Result<Topology> {
    if mesh.triangles.is_empty()
        || mesh.positions.is_empty()
        || mesh.positions.iter().any(|p| !p.is_finite())
    {
        return Err("Retopology needs a finite source surface".into());
    }
    let mut edges = BTreeMap::<[usize; 2], Vec<[usize; 2]>>::new();
    let mut fans = vec![Vec::new(); mesh.positions.len()];
    for (i, &[a, b, c]) in mesh.triangles.iter().enumerate() {
        if [a, b, c].iter().any(|&v| v >= mesh.positions.len()) || a == b || b == c || c == a {
            return Err("Invalid source triangle".into());
        }
        let n =
            (mesh.positions[b] - mesh.positions[a]).cross(mesh.positions[c] - mesh.positions[a]);
        if geometry
            && (!n.is_finite() || !n.length_squared().is_finite() || n.length_squared() < 1e-24)
        {
            return Err("Degenerate source triangle".into());
        }
        for (u, v) in [(a, b), (b, c), (c, a)] {
            edges
                .entry(edge(u, v))
                .or_default()
                .push([i, usize::from(u < v)]);
            fans[u].push(i);
        }
    }
    let mut adjacent = vec![Vec::new(); mesh.triangles.len()];
    for list in edges.values() {
        if list.len() > 2 || (list.len() == 2 && list[0][1] == list[1][1]) {
            return Err("Retopology needs oriented manifold faces".into());
        }
        if list.len() == 2 {
            adjacent[list[0][0]].push(list[1][0]);
            adjacent[list[1][0]].push(list[0][0]);
        }
    }
    let mut visited = vec![usize::MAX; mesh.triangles.len()];
    for (v, fan) in fans.iter().enumerate() {
        if fan.is_empty() {
            return Err("Remove loose source vertices".into());
        }
        let mut todo = VecDeque::from([fan[0]]);
        let mut count = 0;
        while let Some(f) = todo.pop_front() {
            if visited[f] == v {
                continue;
            }
            visited[f] = v;
            count += 1;
            for &g in &adjacent[f] {
                if visited[g] != v && mesh.triangles[g].contains(&v) {
                    todo.push_back(g);
                }
            }
        }
        if count != fan.len() {
            return Err("Retopology found a disconnected vertex fan".into());
        }
    }
    let mut components = Dsu::new(mesh.positions.len());
    let mut boundary = Dsu::new(mesh.positions.len());
    let mut on_boundary = vec![false; mesh.positions.len()];
    for (&[a, b], faces) in &edges {
        components.join(a, b);
        if faces.len() == 1 {
            boundary.join(a, b);
            on_boundary[a] = true;
            on_boundary[b] = true;
        }
    }
    let component_count = (0..mesh.positions.len())
        .filter(|&i| components.find(i) == i)
        .count();
    let mut seen = vec![false; mesh.positions.len()];
    let mut boundaries = 0;
    for (i, &b) in on_boundary.iter().enumerate() {
        if b {
            let root = boundary.find(i);
            if !seen[root] {
                seen[root] = true;
                boundaries += 1;
            }
        }
    }
    Ok(Topology {
        components: component_count,
        euler: mesh.positions.len() as isize - edges.len() as isize + mesh.triangles.len() as isize,
        boundaries,
    })
}

pub(crate) fn geometry(mesh: &Surface) -> (Vec<Vec3>, Vec<f64>, Vec<Vec<usize>>, f64) {
    let n = mesh.positions.len();
    let mut normals = vec![Vec3::ZERO; n];
    let mut areas = vec![0.; n];
    let mut neighbours = vec![Vec::new(); n];
    let mut area = 0.;
    for &[a, b, c] in &mesh.triangles {
        let cross =
            (mesh.positions[b] - mesh.positions[a]).cross(mesh.positions[c] - mesh.positions[a]);
        let ar = cross.length() * 0.5;
        area += ar;
        for (u, v, w) in [(a, b, c), (b, c, a), (c, a, b)] {
            let x = (mesh.positions[v] - mesh.positions[u]).normalize();
            let y = (mesh.positions[w] - mesh.positions[u]).normalize();
            normals[u] += cross.normalize() * x.dot(y).clamp(-1., 1.).acos();
            areas[u] += ar / 3.;
            neighbours[u].extend([v, w]);
        }
    }
    for (norm, row) in normals.iter_mut().zip(&mut neighbours) {
        *norm = norm.normalize_or_zero();
        row.sort_unstable();
        row.dedup();
    }
    (normals, areas, neighbours, area)
}

pub(crate) struct Dsu(Vec<usize>);
impl Dsu {
    pub fn new(n: usize) -> Self {
        Self((0..n).collect())
    }
    pub fn find(&mut self, mut v: usize) -> usize {
        while self.0[v] != v {
            self.0[v] = self.0[self.0[v]];
            v = self.0[v];
        }
        v
    }
    pub fn join(&mut self, a: usize, b: usize) {
        let a = self.find(a);
        let b = self.find(b);
        self.0[b] = a;
    }
}
