//! Local surface joining. Source polygons and vertices are never resampled.
use super::uv::{UvMap, edge};
use super::{Mesh, Vec3, max_triangles, max_vertices};
use std::collections::{BTreeMap, BTreeSet};
mod intersection;
mod spatial;
pub(crate) use intersection::{FusionReport, fuse, join_open_ends};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct JoinOptions {
    pub segments: usize,
    pub twist: i32,
    pub curvature: f32,
}
impl Default for JoinOptions {
    fn default() -> Self {
        Self {
            segments: 4,
            twist: 0,
            curvature: 0.5,
        }
    }
}
impl JoinOptions {
    fn validate(self) -> Result<(), String> {
        if !(1..=64).contains(&self.segments)
            || !self.curvature.is_finite()
            || !(0. ..=1.).contains(&self.curvature)
            || !(-4096..=4096).contains(&self.twist)
        {
            return Err("Choose 1–64 connector segments and curvature between 0 and 1".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Boundary {
    /// Directed as the existing surface: a connector must use the reverse edge.
    pub vertices: Vec<usize>,
    pub center: Vec3,
    pub perimeter: f32,
}

pub(crate) fn cancelled(cancel: &dyn Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Mesh joining cancelled".into())
    } else {
        Ok(())
    }
}

/// Includes hidden boundaries so a hidden adjacent face cannot masquerade as a hole.
pub(crate) fn boundaries(mesh: &Mesh) -> Result<Vec<Boundary>, String> {
    mesh.validate()?;
    let mut incidence = BTreeMap::<[usize; 2], Vec<[usize; 2]>>::new();
    for t in &mesh.triangles {
        for i in 0..3 {
            incidence
                .entry(edge(t[i], t[(i + 1) % 3]))
                .or_default()
                .push([t[i], t[(i + 1) % 3]]);
        }
    }
    let mut next = BTreeMap::new();
    let mut incoming = BTreeSet::new();
    for faces in incidence.values() {
        if faces.len() > 2 || faces.len() == 2 && faces[0][0] == faces[1][0] {
            return Err("Joining requires consistently oriented manifold surfaces".into());
        }
        if faces.len() == 1 {
            let [a, b] = faces[0];
            if next.insert(a, b).is_some() || !incoming.insert(b) {
                return Err("An open boundary branches at a vertex".into());
            }
        }
    }
    let mut result = Vec::new();
    while let Some((&first, _)) = next.first_key_value() {
        let mut vertices = Vec::new();
        let mut v = first;
        loop {
            vertices.push(v);
            v = next.remove(&v).ok_or("An open boundary is incomplete")?;
            if v == first {
                break;
            }
        }
        if vertices.len() < 3 {
            return Err("An opening needs at least three vertices".into());
        }
        let center = vertices
            .iter()
            .fold(Vec3::default(), |s, &v| s + mesh.vertices[v])
            * (1. / vertices.len() as f32);
        let perimeter = (0..vertices.len())
            .map(|i| {
                (mesh.vertices[vertices[i]] - mesh.vertices[vertices[(i + 1) % vertices.len()]])
                    .len()
            })
            .sum();
        result.push(Boundary {
            vertices,
            center,
            perimeter,
        });
    }
    Ok(result)
}

pub(crate) fn validate_surface(mesh: &Mesh) -> Result<(), String> {
    boundaries(mesh)?;
    let mut unique = BTreeSet::new();
    let mut fans = vec![Vec::new(); mesh.vertices.len()];
    for t in &mesh.triangles {
        let mut key = *t;
        key.sort_unstable();
        let [a, b, c] = t.map(|v| mesh.vertices[v]);
        if !unique.insert(key)
            || (b - a).cross(c - a).len() <= ((b - a).len() * (c - a).len() * 1e-6).max(1e-12)
        {
            return Err("The connection would create duplicate or collapsed faces; adjust alignment or curvature".into());
        }
        for i in 0..3 {
            fans[t[i]].push([t[(i + 1) % 3], t[(i + 2) % 3]]);
        }
    }
    for fan in fans.iter().filter(|fan| !fan.is_empty()) {
        let mut neighbors = BTreeMap::<usize, Vec<usize>>::new();
        for &[a, b] in fan {
            neighbors.entry(a).or_default().push(b);
            neighbors.entry(b).or_default().push(a);
        }
        let mut seen = BTreeSet::new();
        let mut pending = vec![fan[0][0]];
        while let Some(v) = pending.pop() {
            if seen.insert(v) {
                pending.extend(&neighbors[&v]);
            }
        }
        if seen.len() != neighbors.len() {
            return Err("Joining requires a single connected face fan at each vertex".into());
        }
    }
    Ok(())
}

/// Drop buried, unreferenced source vertices only at the document boundary.
/// Local algorithms keep source indices stable for exact preservation checks.
pub(crate) fn compact(mut mesh: Mesh) -> Mesh {
    let used: BTreeSet<_> = mesh.triangles.iter().flatten().copied().collect();
    let mut mapping = vec![usize::MAX; mesh.vertices.len()];
    for (next, &old) in used.iter().enumerate() {
        mapping[old] = next;
    }
    let uv = mesh.uv.current(&mesh);
    mesh.vertices = used.iter().map(|&v| mesh.vertices[v]).collect();
    mesh.mask = used.iter().map(|&v| mesh.mask[v]).collect();
    if !mesh.materials.is_empty() {
        mesh.materials = used.iter().map(|&v| mesh.materials[v].clone()).collect();
    }
    for group in &mut mesh.vertex_groups {
        group.weights = used.iter().map(|&v| group.weights[v]).collect();
    }
    for t in &mut mesh.triangles {
        *t = t.map(|v| mapping[v]);
    }
    for q in &mut mesh.quads {
        *q = q.map(|v| mapping[v]);
    }
    mesh.creases = mesh
        .creases
        .iter()
        .filter(|e| e.iter().all(|&v| mapping[v] != usize::MAX))
        .map(|e| e.map(|v| mapping[v]))
        .collect();
    mesh.uv.seams = mesh
        .uv
        .seams
        .iter()
        .filter(|e| e.iter().all(|&v| mapping[v] != usize::MAX))
        .map(|e| e.map(|v| mapping[v]))
        .collect();
    if uv {
        mesh.uv.topology = mesh.triangles.clone();
    }
    mesh
}

fn check_uv(mesh: &Mesh) -> Result<(), String> {
    if !mesh.uv.is_empty() && !mesh.uv.current(mesh) {
        return Err("Restore or clear the inactive UV mapping before joining".into());
    }
    Ok(())
}

/// Concatenate attributes as well as geometry. Unmapped faces remain unmapped;
/// distinct texture atlases require an explicit bake rather than silent replacement.
pub(crate) fn merge(a: &Mesh, b: &Mesh) -> Result<Mesh, String> {
    check_uv(a)?;
    check_uv(b)?;
    if !a.uv.textures.is_empty() && !b.uv.textures.is_empty() && a.uv.textures != b.uv.textures {
        return Err("The two meshes use different texture images; bake them to a shared atlas before joining".into());
    }
    let mut out = a.merged_with(b)?;
    if a.uv.current(a) || b.uv.current(b) {
        out.uv = UvMap {
            textures: if a.uv.textures.is_empty() {
                b.uv.textures.clone()
            } else {
                a.uv.textures.clone()
            },
            ..Default::default()
        };
        for mesh in [a, b] {
            for i in 0..mesh.triangles.len() {
                out.uv
                    .corners
                    .push(mesh.uv.corners.get(i).copied().unwrap_or([[0.; 2]; 3]));
                out.uv
                    .masks
                    .push(mesh.uv.masks.get(i).copied().unwrap_or([0.; 3]));
                out.uv
                    .removed
                    .push(!mesh.uv.current(mesh) || !mesh.uv.mapped(i));
            }
        }
        out.uv.topology = out.triangles.clone();
        out.uv.seams =
            a.uv.seams
                .iter()
                .copied()
                .chain(b.uv.seams.iter().map(|e| e.map(|v| v + a.vertices.len())))
                .collect();
        out.uv.seams.sort_unstable();
        out.uv.seams.dedup();
    }
    out.validate()?;
    Ok(out)
}

fn parameters(mesh: &Mesh, ring: &[usize]) -> Result<Vec<f32>, String> {
    let mut result = vec![0.];
    for i in 0..ring.len() {
        let length = (mesh.vertices[ring[i]] - mesh.vertices[ring[(i + 1) % ring.len()]]).len();
        if length < 1e-8 {
            return Err("The selected opening contains a collapsed edge".into());
        }
        result.push(result.last().unwrap() + length);
    }
    let total = *result.last().unwrap();
    for t in &mut result {
        *t /= total;
    }
    Ok(result)
}
fn sample(ring: &[usize], params: &[f32], t: f32) -> [(usize, f32); 2] {
    let i = params
        .partition_point(|&x| x <= t)
        .saturating_sub(1)
        .min(ring.len() - 1);
    let f = ((t - params[i]) / (params[i + 1] - params[i])).clamp(0., 1.);
    [(ring[i], 1. - f), (ring[(i + 1) % ring.len()], f)]
}
fn position(mesh: &Mesh, weights: &[(usize, f32)]) -> Vec3 {
    weights
        .iter()
        .fold(Vec3::default(), |s, &(v, w)| s + mesh.vertices[v] * w)
}

fn outward_tangents(mesh: &Mesh, ring: &[usize]) -> Vec<Vec3> {
    let chosen: BTreeSet<_> = ring.iter().copied().collect();
    let mut sums = vec![Vec3::default(); mesh.vertices.len()];
    for p in mesh.polygons() {
        for &v in &p.vertices {
            if chosen.contains(&v) {
                for &other in &p.vertices {
                    if !chosen.contains(&other) {
                        sums[v] = sums[v] + (mesh.vertices[v] - mesh.vertices[other]);
                    }
                }
            }
        }
    }
    sums.into_iter().map(Vec3::norm).collect()
}

/// Add just the connector; existing vertices, triangles, quads and attributes stay exact.
pub(crate) fn bridge(
    mesh: &Mesh,
    first: &[usize],
    second: &[usize],
    options: JoinOptions,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    options.validate()?;
    check_uv(mesh)?;
    cancelled(cancel)?;
    validate_surface(mesh)?;
    let available = boundaries(mesh)?;
    for ring in [first, second] {
        if !available.iter().any(|b| b.vertices == ring) {
            return Err("Choose two complete current boundary loops".into());
        }
    }
    let selected: BTreeSet<_> = first.iter().chain(second).copied().collect();
    if selected.len() != first.len() + second.len() {
        return Err("Choose two different openings that do not share vertices".into());
    }
    for (i, t) in mesh.triangles.iter().enumerate() {
        if t.iter().any(|v| selected.contains(v)) && !mesh.face_visible(i) {
            return Err("Show the faces adjoining both openings before joining".into());
        }
    }
    if selected.iter().any(|&v| mesh.mask[v] > 0.) {
        return Err("Clear the mask on both joining boundaries first".into());
    }
    if first.len().max(second.len()) > 4096 {
        return Err("Joining supports up to 4096 vertices per opening".into());
    }
    let a = first.to_vec();
    let mut b = second.iter().rev().copied().collect::<Vec<_>>();
    let pa = parameters(mesh, &a)?;
    // Search offsets in bounded work. The user can correct the correspondence.
    let trials = b.len().min(256);
    let mut best = (f32::INFINITY, 0);
    for i in 0..trials {
        cancelled(cancel)?;
        let shift = i * b.len() / trials;
        let rotated = (0..b.len())
            .map(|j| b[(j + shift) % b.len()])
            .collect::<Vec<_>>();
        let pb = parameters(mesh, &rotated)?;
        let cost = (0..32)
            .map(|s| {
                let t = s as f32 / 32.;
                let d =
                    position(mesh, &sample(&a, &pa, t)) - position(mesh, &sample(&rotated, &pb, t));
                d.dot(d)
            })
            .sum::<f32>();
        if cost < best.0 {
            best = (cost, shift);
        }
    }
    let shift = (best.1 as i32 + options.twist).rem_euclid(b.len() as i32) as usize;
    b.rotate_left(shift);
    let pb = parameters(mesh, &b)?;
    // Carry every endpoint corner through the connector. Uniform rings can
    // shortcut a concave/jagged cut rim and fold back through its source faces.
    let mut knots = pa[..a.len()]
        .iter()
        .chain(&pb[..b.len()])
        .copied()
        .collect::<Vec<_>>();
    knots.sort_by(f32::total_cmp);
    knots.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
    let n = knots.len();
    let added_vertices = (options.segments - 1) * n;
    if mesh.vertices.len() + added_vertices > max_vertices()
        || mesh.triangles.len() + 2 * n * options.segments > max_triangles()
    {
        return Err("The connector exceeds the mesh budget; reduce segments".into());
    }
    let ta = outward_tangents(mesh, &a);
    let tb = outward_tangents(mesh, &b);
    let mut out = mesh.clone();
    out.polygroups.resize(out.triangles.len(), 0);
    out.hidden_faces.resize(out.triangles.len(), false);
    let mut rings = vec![(a.clone(), pa.clone())];
    for step in 1..options.segments {
        cancelled(cancel)?;
        let t = step as f32 / options.segments as f32;
        let mut ring = Vec::new();
        for &f in &knots {
            let wa = sample(&a, &pa, f);
            let wb = sample(&b, &pb, f);
            let p = position(mesh, &wa);
            let q = position(mesh, &wb);
            let delta = q - p;
            let da = wa
                .iter()
                .fold(Vec3::default(), |s, &(v, w)| s + ta[v] * w)
                .norm();
            let db = wb
                .iter()
                .fold(Vec3::default(), |s, &(v, w)| s + tb[v] * w)
                .norm();
            let length = delta.len();
            // Hermite endpoint tangents blend toward the straight connector.
            let ma = delta * (1. - options.curvature) + da * (length * options.curvature);
            let mb = delta * (1. - options.curvature) - db * (length * options.curvature);
            let point = p * (2. * t * t * t - 3. * t * t + 1.)
                + q * (-2. * t * t * t + 3. * t * t)
                + ma * (t * t * t - 2. * t * t + t)
                + mb * (t * t * t - t * t);
            let weights = [
                (wa[0].0, wa[0].1 * (1. - t)),
                (wa[1].0, wa[1].1 * (1. - t)),
                (wb[0].0, wb[0].1 * t),
                (wb[1].0, wb[1].1 * t),
            ];
            ring.push(out.append_blend(mesh, &weights, point));
        }
        let mut params = knots.clone();
        params.push(1.);
        rings.push((ring, params));
    }
    rings.push((b, pb));
    for pair in rings.windows(2) {
        let (a, pa) = &pair[0];
        let (b, pb) = &pair[1];
        let (mut i, mut j) = (0, 0);
        while i < a.len() || j < b.len() {
            let x = if i < a.len() {
                pa[i + 1]
            } else {
                f32::INFINITY
            };
            let y = if j < b.len() {
                pb[j + 1]
            } else {
                f32::INFINITY
            };
            let ai = a[i % a.len()];
            let bj = b[j % b.len()];
            if (x - y).abs() < 1e-5 {
                out.push_polygon(
                    &[a[(i + 1) % a.len()], ai, bj, b[(j + 1) % b.len()]],
                    0,
                    false,
                );
                i += 1;
                j += 1;
            } else if x < y {
                out.push_polygon(&[a[(i + 1) % a.len()], ai, bj], 0, false);
                i += 1;
            } else {
                out.push_polygon(&[ai, bj, b[(j + 1) % b.len()]], 0, false);
                j += 1;
            }
        }
    }
    if mesh.uv.current(mesh) {
        out.uv.removed.resize(mesh.triangles.len(), false);
        out.uv.removed.resize(out.triangles.len(), true);
        out.uv.corners.resize(out.triangles.len(), [[0.; 2]; 3]);
        if !out.uv.masks.is_empty() {
            out.uv.masks.resize(out.triangles.len(), [0.; 3]);
        }
        out.uv.topology = out.triangles.clone();
    }
    cancelled(cancel)?;
    validate_surface(&out)?;
    spatial::validate_connector(&out, mesh.triangles.len(), cancel)?;
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests;
