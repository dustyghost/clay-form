//! Adaptation of QuadriFlow AdvancedExtractQuad, BuildTriangleManifold and
//! FixHoles, commit 810b7a0967c35b0dc85b4464e3835e26a756c967.
//! See LICENSE-QuadriFlow. Repair triangle fans before pairing into quads;
//! keep split vertices coincident until the position optimizer separates them.
use crate::{
    Result, Vec3, check,
    coarse::Graph,
    field::Field,
    integer::{Extraction, Lattice},
    topology::{self, Dsu},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
const NONE: usize = usize::MAX;
fn next(h: usize) -> usize {
    h / 3 * 3 + (h + 1) % 3
}
fn prev(h: usize) -> usize {
    h / 3 * 3 + (h + 2) % 3
}
fn degenerate(t: [usize; 3]) -> bool {
    t[0] == t[1] || t[1] == t[2] || t[2] == t[0]
}

pub(crate) fn extract(
    lattice: &Lattice,
    field: &Field,
    cancel: &dyn Fn() -> bool,
) -> Result<Extraction> {
    let graph = Graph::from_lattice(lattice)?
        .hierarchy(cancel)?
        .pop()
        .ok_or("Empty extraction hierarchy")?;
    let (mut mesh, normals) = extract_graph(lattice, field, &graph, &[], cancel)?;
    // Real input boundaries remain open. Only close contraction holes on a
    // closed source; source topology is checked again after valence cleanup.
    if graph
        .edge_faces
        .iter()
        .all(|f| f[0] != NONE && f[1] != NONE)
    {
        close_holes(&mut mesh.quads, &mesh.positions, &normals, cancel)?;
    }
    Ok(mesh)
}

fn extract_graph(
    lattice: &Lattice,
    field: &Field,
    graph: &Graph,
    excluded: &[bool],
    cancel: &dyn Fn() -> bool,
) -> Result<(Extraction, Vec<Vec3>)> {
    let mut dsu = Dsu::new(field.origins.len());
    for e in &lattice.edges {
        if e.d == glam::IVec2::ZERO {
            dsu.join(e.ends[0], e.ends[1]);
        }
    }
    let roots: Vec<_> = (0..field.origins.len()).map(|v| dsu.find(v)).collect();
    let mut root_samples = vec![Vec::new(); roots.len()];
    let mut root_positions = vec![Vec3::ZERO; roots.len()];
    let mut root_normals = vec![Vec3::ZERO; roots.len()];
    for (v, &r) in roots.iter().enumerate() {
        root_positions[r] += field.origins[v];
        root_normals[r] =
            (root_normals[r] * root_samples[r].len() as f64 + field.normals[v]).normalize_or_zero();
        root_samples[r].push(v);
    }
    let mut twins = vec![NONE; graph.faces.len() * 3];
    for (e, &[a, b]) in graph.edge_faces.iter().enumerate() {
        if a == NONE || b == NONE {
            continue;
        }
        let ha = a * 3
            + graph.faces[a]
                .iter()
                .position(|&v| v == e)
                .ok_or("Missing extraction edge")?;
        let hb = b * 3
            + graph.faces[b]
                .iter()
                .rposition(|&v| v == e)
                .ok_or("Missing extraction twin")?;
        twins[ha] = hb;
        twins[hb] = ha;
    }
    let mut triangles = vec![[NONE; 3]; graph.faces.len()];
    let mut mesh = Extraction {
        positions: vec![],
        quads: vec![],
        samples: vec![],
    };
    let mut normals = Vec::new();
    for h in 0..twins.len() {
        if h % 4096 == 0 {
            check(cancel)?;
        }
        if triangles[h / 3][h % 3] != NONE {
            continue;
        }
        let root = roots[field.surface.triangles[graph.source_faces[h / 3]][h % 3]];
        let id = mesh.positions.len();
        mesh.positions
            .push(root_positions[root] / root_samples[root].len() as f64);
        mesh.samples.push(root_samples[root].clone());
        normals.push(root_normals[root]);
        for corner in fan(h, &twins)? {
            triangles[corner / 3][corner % 3] = id;
        }
    }
    separate_triangle_fans(&mut mesh, &mut normals, &mut triangles, &mut twins, cancel)?;
    // Deterministic diagonal order is significant for downstream valence repair.
    let mut diagonals = BTreeMap::<[usize; 2], ([usize; 3], Option<[usize; 3]>)>::new();
    for (f, &t) in triangles.iter().enumerate() {
        if degenerate(t) || excluded.get(f) == Some(&true) {
            continue;
        }
        for k in 0..3 {
            let d = graph.offsets[graph.faces[f][k]].abs();
            if d.x == 1 && d.y == 1 {
                let tri = [t[k], t[(k + 1) % 3], t[(k + 2) % 3]];
                diagonals
                    .entry(topology::edge(tri[0], tri[1]))
                    .and_modify(|p| p.1 = Some(tri))
                    .or_insert((tri, None));
            }
        }
    }
    for (a, b) in diagonals.into_values() {
        if let Some(b) = b
            && a[2] != b[2]
        {
            let q = [a[1], a[2], a[0], b[2]];
            if q.iter().copied().collect::<BTreeSet<_>>().len() == 4 {
                mesh.quads.push(q);
            }
        }
    }
    Ok((mesh, normals))
}

#[cfg(feature = "audit")]
pub(crate) fn cut_inversions(
    lattice: &Lattice,
    field: &Field,
    halo: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<(Extraction, Vec<Vec3>, serde_json::Value)> {
    if halo > 16 {
        return Err("Patch cut exceeds the audit halo limit".into());
    }
    let graph = Graph::from_lattice(lattice)?
        .hierarchy(cancel)?
        .pop()
        .ok_or("Empty extraction hierarchy")?;
    if graph.edge_faces.iter().any(|f| f.contains(&NONE)) {
        return Err("Patch-cut audit currently needs a closed source".into());
    }
    let mut distance = vec![usize::MAX; graph.faces.len()];
    let mut pending = VecDeque::new();
    for (f, d) in distance.iter_mut().enumerate() {
        if graph.area(f) < 0 {
            *d = 0;
            pending.push_back(f);
        }
    }
    let inversions = pending.len();
    while let Some(f) = pending.pop_front() {
        if distance[f] >= halo {
            continue;
        }
        for &e in &graph.faces[f] {
            for g in graph.edge_faces[e] {
                if g != NONE && distance[g] == usize::MAX {
                    distance[g] = distance[f] + 1;
                    pending.push_back(g);
                }
            }
        }
    }
    let excluded: Vec<_> = distance.iter().map(|&d| d <= halo).collect();
    let removed = excluded.iter().filter(|&&x| x).count();
    let (mesh, normals) = extract_graph(lattice, field, &graph, &excluded, cancel)?;
    let report = serde_json::json!({"coarse_faces":graph.faces.len(),"coarse_inversions":inversions,"halo":halo,"excluded_coarse_faces":removed,"excluded_fraction":removed as f64/graph.faces.len() as f64,"remaining_quads":mesh.quads.len(),"selection":"Negative-area coarse faces and adjacency rings; no anatomy, coordinates or source topology edits."});
    Ok((mesh, normals, report))
}

#[cfg(feature = "audit")]
pub(crate) fn fill_quad_patches(
    mesh: &mut Extraction,
    normals: &[Vec3],
    rings: &[Vec<usize>],
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    // A bounded audit extension of the existing QuadEnergy construction.
    // Runtime FixHoles keeps its original <25 limit. The complete candidate
    // is checked before any fill is committed; no triangles are substituted.
    if rings
        .iter()
        .any(|r| r.len() < 4 || r.len() % 2 != 0 || r.len() > 32)
    {
        return Err("Quad patch requires an even boundary of 4..=32 vertices".into());
    }
    let mut quads = mesh.quads.clone();
    for ring in rings {
        let (_, fill) = quad_energy(ring, &mesh.positions, normals, &mut BTreeMap::new(), cancel)?;
        if fill.len() != ring.len() / 2 - 1 {
            return Err("Quad patch did not cover its boundary".into());
        }
        quads.extend(fill);
    }
    let graph = crate::quad::connections(&quads, mesh.positions.len())?;
    if graph.twins.contains(&NONE) {
        return Err("Quad patch left a boundary open".into());
    }
    mesh.quads = quads;
    Ok(())
}

/// Ordered incident triangle corners, including both directions on a boundary.
fn fan(start: usize, twins: &[usize]) -> Result<Vec<usize>> {
    let mut ring = VecDeque::new();
    let mut h = start;
    loop {
        ring.push_back(h);
        h = twins[prev(h)];
        if h == NONE || h == start {
            break;
        }
        if ring.len() > twins.len() {
            return Err("Extraction fan does not close".into());
        }
    }
    if h == NONE {
        h = start;
        loop {
            h = twins[h];
            if h == NONE {
                break;
            }
            h = next(h);
            if h == start {
                break;
            }
            ring.push_front(h);
            if ring.len() > twins.len() {
                return Err("Extraction boundary fan does not close".into());
            }
        }
    }
    Ok(ring.into_iter().collect())
}
fn separate_triangle_fans(
    mesh: &mut Extraction,
    normals: &mut Vec<Vec3>,
    triangles: &mut [[usize; 3]],
    twins: &mut [usize],
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    let budget = twins.len();
    loop {
        check(cancel)?;
        let initial = mesh.positions.len();
        let mut incident = vec![Vec::new(); initial];
        for (f, &t) in triangles.iter().enumerate() {
            if degenerate(t) {
                for k in 0..3 {
                    let h = f * 3 + k;
                    let other = twins[h];
                    if other != NONE {
                        twins[other] = NONE;
                    }
                    twins[h] = NONE;
                }
            } else {
                for k in 0..3 {
                    incident[t[k]].push(f * 3 + k);
                }
            }
        }
        let mut visited = vec![false; twins.len()];
        for (v, corners) in incident.iter().enumerate() {
            if v % 256 == 0 {
                check(cancel)?;
            }
            let mut count = 0;
            for &start in corners {
                if visited[start] {
                    continue;
                }
                let ring = fan(start, twins)?;
                let mut labels = vec![count; ring.len()];
                count += 1;
                let mut locations = BTreeMap::new();
                for (i, &h) in ring.iter().enumerate() {
                    visited[h] = true;
                    let pair = (triangles[h / 3][h % 3], triangles[h / 3][(h + 1) % 3]);
                    if let Some(&s) = locations.get(&pair) {
                        for k in s..i {
                            let c = ring[k];
                            locations
                                .remove(&(triangles[c / 3][c % 3], triangles[c / 3][(c + 1) % 3]));
                            labels[k] = count;
                        }
                        count += 1;
                    }
                    locations.insert(pair, i);
                }
                for (&h, label) in ring.iter().zip(labels) {
                    if label > 0 {
                        triangles[h / 3][h % 3] = mesh.positions.len() + label - 1;
                    }
                }
            }
            for _ in 1..count {
                mesh.positions.push(mesh.positions[v]);
                mesh.samples.push(mesh.samples[v].clone());
                normals.push(normals[v]);
            }
        }
        if mesh.positions.len() == initial {
            return Ok(());
        }
        if mesh.positions.len() > budget {
            return Err("Triangle fan repair exceeded its corner budget".into());
        }
    }
}

fn close_holes(
    quads: &mut Vec<[usize; 4]>,
    positions: &[Vec3],
    normals: &[Vec3],
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    let mut directed = BTreeMap::new();
    for (f, q) in quads.iter().enumerate() {
        for k in 0..4 {
            if directed.insert((q[k], q[(k + 1) % 4]), f * 4 + k).is_some() {
                return Err("Repeated directed edge after triangle fan repair".into());
            }
        }
    }
    let mut twins = vec![NONE; quads.len() * 4];
    for (&(a, b), &h) in &directed {
        if let Some(&t) = directed.get(&(b, a)) {
            twins[h] = t;
        }
    }
    let mut visited = vec![false; twins.len()];
    let mut edges: BTreeSet<_> = directed.keys().copied().collect();
    for start in 0..twins.len() {
        check(cancel)?;
        if visited[start] || twins[start] != NONE {
            continue;
        }
        let mut ring = Vec::new();
        let mut h = start;
        while !visited[h] {
            visited[h] = true;
            ring.push(quads[h / 4][h % 4]);
            h = crate::quad::next(h);
            let mut steps = 0;
            while twins[h] != NONE {
                h = crate::quad::next(twins[h]);
                steps += 1;
                if steps > twins.len() {
                    return Err("Grid hole walk does not close".into());
                }
            }
        }
        if ring.len() >= 25 {
            return Err("Grid hole exceeds upstream repair limit".into());
        }
        // Preserve the upstream handling of self-touching loops.
        let mut loops = Vec::new();
        let mut locations = BTreeMap::new();
        for (i, &v) in ring.iter().enumerate() {
            if let Some(&j) = locations.get(&v) {
                let mut part = Vec::new();
                if i - j > 3 && (i - j) % 2 == 0 {
                    for &u in &ring[j..i] {
                        if locations.remove(&u).is_some() {
                            part.push(u);
                        }
                    }
                }
                loops.push(part);
            }
            locations.insert(v, i);
        }
        if locations.len() >= 3 {
            loops.push(
                ring.iter()
                    .copied()
                    .filter(|v| locations.remove(v).is_some())
                    .collect(),
            );
        }
        for part in loops {
            if part.is_empty() {
                break;
            }
            let (_, fill) = quad_energy(&part, positions, normals, &mut BTreeMap::new(), cancel)?;
            for q in fill {
                if (0..4).all(|k| !edges.contains(&(q[k], q[(k + 1) % 4]))) {
                    for k in 0..4 {
                        edges.insert((q[k], q[(k + 1) % 4]));
                    }
                    quads.push(q);
                }
            }
        }
    }
    Ok(())
}

type Fill = (f64, Vec<[usize; 4]>);
/// Same angle objective and enumeration as QuadEnergy; memoization avoids
/// recomputing identical sub-polygons. No centre vertices are invented.
fn quad_energy(
    ring: &[usize],
    p: &[Vec3],
    n: &[Vec3],
    memo: &mut BTreeMap<Vec<usize>, Fill>,
    cancel: &dyn Fn() -> bool,
) -> Result<Fill> {
    check(cancel)?;
    if ring.len() < 4 {
        return Ok((0., vec![]));
    }
    if let Some(fill) = memo.get(ring) {
        return Ok(fill.clone());
    }
    let mut best = (1e30, vec![]);
    if ring.len() == 4 {
        let mut energy = 0.;
        for k in 0..4 {
            let a = (p[ring[(k + 3) % 4]] - p[ring[k]]).normalize_or_zero();
            let b = (p[ring[(k + 1) % 4]] - p[ring[k]]).normalize_or_zero();
            let cross = a.cross(b);
            let mut sine = cross.length();
            if cross.dot(n[ring[k]]) < 0. {
                sine = -sine;
            }
            // Keep the pinned upstream angle scale for reference comparisons.
            #[allow(clippy::approx_constant)]
            let mut angle = sine.atan2(a.dot(b)) / 3.141592654 * 180.;
            if angle < 0. {
                angle += 360.;
            }
            energy += angle * angle;
        }
        best = (energy, vec![[ring[0], ring[3], ring[2], ring[1]]]);
    } else {
        for a in (2..ring.len()).step_by(2) {
            for b in (a + 1..ring.len()).step_by(2) {
                let mut fill =
                    quad_energy(&[ring[0], ring[1], ring[a], ring[b]], p, n, memo, cancel)?;
                let mut parts = Vec::new();
                if a > 2 {
                    parts.push(ring[1..=a].to_vec());
                }
                if b > a + 1 {
                    parts.push(ring[a..=b].to_vec());
                }
                if b + 1 < ring.len() {
                    let mut part = ring[b..].to_vec();
                    part.push(ring[0]);
                    parts.push(part);
                }
                for part in parts {
                    let sub = quad_energy(&part, p, n, memo, cancel)?;
                    fill.0 += sub.0;
                    fill.1.extend(sub.1);
                }
                if fill.0 < best.0 {
                    best = fill;
                }
            }
        }
    }
    memo.insert(ring.to_vec(), best.clone());
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closes_missing_cube_face_without_inventing_vertices() {
        let positions = vec![
            Vec3::new(-1., -1., -1.),
            Vec3::new(1., -1., -1.),
            Vec3::new(1., 1., -1.),
            Vec3::new(-1., 1., -1.),
            Vec3::new(-1., -1., 1.),
            Vec3::new(1., -1., 1.),
            Vec3::new(1., 1., 1.),
            Vec3::new(-1., 1., 1.),
        ];
        let normals = positions.iter().map(|p| p.normalize()).collect::<Vec<_>>();
        let mut quads = vec![
            [0, 3, 2, 1],
            [0, 1, 5, 4],
            [1, 2, 6, 5],
            [2, 3, 7, 6],
            [3, 0, 4, 7],
        ];
        let before = quads.clone();
        close_holes(&mut quads, &positions, &normals, &|| false).unwrap();
        assert_eq!(&quads[..5], &before);
        assert_eq!(quads.len(), 6);
        topology::validate(&crate::Surface {
            positions,
            triangles: crate::surface::triangulate(&quads, &|| false).unwrap(),
        })
        .unwrap();
        let mut incidence = BTreeMap::new();
        for q in quads {
            for k in 0..4 {
                *incidence
                    .entry(topology::edge(q[k], q[(k + 1) % 4]))
                    .or_insert(0) += 1;
            }
        }
        assert!(incidence.values().all(|&n| n == 2));
    }
    #[test]
    fn splits_disconnected_triangle_fans_before_quad_pairing() {
        let mut triangles = [[0, 1, 2], [0, 2, 3], [0, 1, 4], [0, 4, 5]];
        let mut twins = vec![NONE; 12];
        for (a, b) in [(2, 3), (8, 9)] {
            twins[a] = b;
            twins[b] = a;
        }
        let mut mesh = Extraction {
            positions: (0..6).map(|v| Vec3::new(v as f64, 0., 0.)).collect(),
            samples: (0..6).map(|v| vec![v]).collect(),
            quads: vec![],
        };
        let mut normals = vec![Vec3::Z; 6];
        separate_triangle_fans(&mut mesh, &mut normals, &mut triangles, &mut twins, &|| {
            false
        })
        .unwrap();
        assert_ne!(triangles[0][0], triangles[2][0]);
        assert_ne!(triangles[0][1], triangles[2][1]);
        assert_eq!(mesh.positions.len(), 8);
        for v in 6..8 {
            assert_eq!(mesh.positions[v], mesh.positions[mesh.samples[v][0]]);
        }
    }
    #[test]
    fn hole_search_cancels_before_expensive_work() {
        assert!(
            quad_energy(&[0, 1, 2, 3, 4, 5], &[], &[], &mut BTreeMap::new(), &|| {
                true
            })
            .is_err()
        );
    }
}
