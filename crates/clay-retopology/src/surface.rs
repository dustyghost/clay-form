//! Closest-point queries for fitting and measuring reconstructed surfaces.
use crate::{Result, Surface, Vec3, check};

/// Choose distinct render diagonals without colliding with polygon edges.
/// Around poles an otherwise valid quad complex can need the other diagonal.
/// Augmenting paths avoid making a greedy choice that blocks a later quad.
pub(crate) fn triangulate(
    quads: &[[usize; 4]],
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<[usize; 3]>> {
    use std::collections::BTreeSet;
    let edges: BTreeSet<_> = quads
        .iter()
        .flat_map(|q| (0..4).map(|i| crate::topology::edge(q[i], q[(i + 1) % 4])))
        .collect();
    let choices: Vec<Vec<_>> = quads
        .iter()
        .map(|&[a, b, c, d]| {
            [crate::topology::edge(a, c), crate::topology::edge(b, d)]
                .into_iter()
                .filter(|e| !edges.contains(e))
                .collect()
        })
        .collect();
    triangulate_choices(quads, &choices, cancel)
}
fn triangulate_choices(
    quads: &[[usize; 4]],
    choices: &[Vec<[usize; 2]>],
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<[usize; 3]>> {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    let mut owner = BTreeMap::new();
    let mut chosen = vec![[usize::MAX; 2]; quads.len()];
    for root in 0..quads.len() {
        if root % 256 == 0 {
            check(cancel)?;
        }
        let mut parents = BTreeMap::new();
        let mut seen = BTreeSet::from([root]);
        let mut queue = VecDeque::from([root]);
        let mut free = None;
        while let Some(q) = queue.pop_front() {
            for &diagonal in &choices[q] {
                if let Some(&next) = owner.get(&diagonal) {
                    if seen.insert(next) {
                        parents.insert(next, (q, diagonal));
                        queue.push_back(next);
                    }
                } else {
                    free = Some((q, diagonal));
                    break;
                }
            }
            if free.is_some() {
                break;
            }
        }
        let Some((mut q, mut diagonal)) = free else {
            return Err("The quad grid has no manifold triangulation around a pole".into());
        };
        loop {
            owner.insert(diagonal, q);
            chosen[q] = diagonal;
            if q == root {
                break;
            }
            (q, diagonal) = parents[&q];
        }
    }
    Ok(quads
        .iter()
        .zip(chosen)
        .flat_map(|(&[a, b, c, d], diagonal)| {
            if diagonal == crate::topology::edge(a, c) {
                [[a, b, c], [a, c, d]]
            } else {
                [[a, b, d], [b, c, d]]
            }
        })
        .collect())
}

/// A concave or twisted quad can have only one outward render diagonal.
/// Filter both possibilities against the source, then use the same global
/// matching as the topology-only triangulator so poles cannot reuse diagonals.
fn outward_triangulation(
    source: &Bvh<'_>,
    positions: &[Vec3],
    quads: &[[usize; 4]],
    work: &mut usize,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<[usize; 3]>> {
    oriented_triangulation(source, positions, quads, work, cancel, true)
}

fn oriented_triangulation(
    source: &Bvh<'_>,
    positions: &[Vec3],
    quads: &[[usize; 4]],
    work: &mut usize,
    cancel: &dyn Fn() -> bool,
    require_outward: bool,
) -> Result<Vec<[usize; 3]>> {
    use std::collections::BTreeSet;
    let perimeter: BTreeSet<_> = quads
        .iter()
        .flat_map(|q| (0..4).map(|k| crate::topology::edge(q[k], q[(k + 1) % 4])))
        .collect();
    let mut choices = Vec::with_capacity(quads.len());
    for (i, &[a, b, c, d]) in quads.iter().enumerate() {
        check(cancel)?;
        let mut valid = Vec::new();
        for (diagonal, triangles) in [
            (crate::topology::edge(a, c), [[a, b, c], [a, c, d]]),
            (crate::topology::edge(b, d), [[a, b, d], [b, c, d]]),
        ] {
            if perimeter.contains(&diagonal) {
                continue;
            }
            let mut good = true;
            for t in triangles {
                let [p, q, r] = t.map(|v| positions[v]);
                let n = (q - p).cross(r - p);
                if n.length_squared() < 1e-24
                    || n.dot(source.nearest((p + q + r) / 3., work, cancel)?.normal) <= 0.
                {
                    good = false;
                    break;
                }
            }
            if good {
                valid.push(diagonal);
            }
        }
        if valid.is_empty() && !require_outward {
            // Keep topological alternatives only for a genuinely unresolved
            // quad. Already valid concave quads must not trigger smoothing.
            valid.extend(
                [crate::topology::edge(a, c), crate::topology::edge(b, d)]
                    .into_iter()
                    .filter(|e| !perimeter.contains(e)),
            );
        }
        if valid.is_empty() {
            return Err(format!(
                "Retopology quad {i} has no outward render diagonal near {:?}",
                [a, b, c, d].iter().map(|&v| positions[v]).sum::<Vec3>() / 4.
            ));
        }
        choices.push(valid);
    }
    triangulate_choices(quads, &choices, cancel)
}

pub(crate) struct Fit {
    pub displacement: f64,
    pub error: f64,
    pub triangles: Vec<[usize; 3]>,
}

struct Node {
    lo: Vec3,
    hi: Vec3,
    start: usize,
    end: usize,
    children: Option<[usize; 2]>,
}
pub(crate) struct Bvh<'a> {
    surface: &'a Surface,
    faces: Vec<usize>,
    nodes: Vec<Node>,
}
pub(crate) struct Hit {
    pub point: Vec3,
    pub normal: Vec3,
    pub face: usize,
    pub distance: f64,
}

impl<'a> Bvh<'a> {
    pub fn new(surface: &'a Surface, cancel: &dyn Fn() -> bool) -> Result<Self> {
        if surface.triangles.is_empty() {
            return Err("Cannot fit to an empty surface".into());
        }
        let mut bvh = Self {
            surface,
            faces: (0..surface.triangles.len()).collect(),
            nodes: Vec::new(),
        };
        bvh.build(0, bvh.faces.len(), cancel)?;
        Ok(bvh)
    }
    fn build(&mut self, start: usize, end: usize, cancel: &dyn Fn() -> bool) -> Result<usize> {
        check(cancel)?;
        let mut lo = Vec3::splat(f64::INFINITY);
        let mut hi = -lo;
        for &f in &self.faces[start..end] {
            for v in self.surface.triangles[f] {
                lo = lo.min(self.surface.positions[v]);
                hi = hi.max(self.surface.positions[v]);
            }
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            lo,
            hi,
            start,
            end,
            children: None,
        });
        if end - start > 8 {
            let extent = hi - lo;
            let axis = if extent.x >= extent.y && extent.x >= extent.z {
                0
            } else if extent.y >= extent.z {
                1
            } else {
                2
            };
            let center = |f: usize| {
                self.surface.triangles[f]
                    .iter()
                    .map(|&v| self.surface.positions[v][axis])
                    .sum::<f64>()
            };
            let middle = (start + end) / 2;
            self.faces[start..end].select_nth_unstable_by(middle - start, |&a, &b| {
                center(a).total_cmp(&center(b)).then(a.cmp(&b))
            });
            let a = self.build(start, middle, cancel)?;
            let b = self.build(middle, end, cancel)?;
            self.nodes[index].children = Some([a, b]);
        }
        Ok(index)
    }
    pub fn nearest(&self, p: Vec3, work: &mut usize, cancel: &dyn Fn() -> bool) -> Result<Hit> {
        let bound = |node: &Node| {
            ((node.lo - p).max(Vec3::ZERO) + (p - node.hi).max(Vec3::ZERO)).length_squared()
        };
        let mut stack = [0usize; 64];
        let mut count = 1;
        let mut best = f64::INFINITY;
        let mut hit = None;
        while count > 0 {
            count -= 1;
            let node = &self.nodes[stack[count]];
            if bound(node) > best {
                continue;
            }
            if let Some([a, b]) = node.children {
                if count + 2 > stack.len() {
                    return Err("Surface-fit hierarchy exceeds its depth budget".into());
                }
                let (near, far) = if bound(&self.nodes[a]) < bound(&self.nodes[b]) {
                    (a, b)
                } else {
                    (b, a)
                };
                stack[count] = far;
                stack[count + 1] = near;
                count += 2;
            } else {
                for &f in &self.faces[node.start..node.end] {
                    if *work == 0 {
                        return Err("Surface-fit query budget exceeded".into());
                    }
                    *work -= 1;
                    if (*work).is_multiple_of(4096) {
                        check(cancel)?;
                    }
                    let [a, b, c] = self.surface.triangles[f].map(|v| self.surface.positions[v]);
                    let q = closest(p, a, b, c);
                    let d = (p - q).length_squared();
                    if d < best {
                        best = d;
                        hit = Some(Hit {
                            point: q,
                            normal: (b - a).cross(c - a).normalize_or_zero(),
                            face: f,
                            distance: d.sqrt(),
                        });
                    }
                }
            }
        }
        hit.ok_or_else(|| "No finite surface-fit point was found".into())
    }
}

pub(crate) fn closest(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0. && d2 <= 0. {
        return a;
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0. && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0. && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && d4 - d3 >= 0. && d5 - d6 >= 0. {
        return b + (c - b) * ((d4 - d3) / (d4 - d3 + d5 - d6));
    }
    let inverse = 1. / (va + vb + vc);
    a + ab * (vb * inverse) + ac * (vc * inverse)
}

pub(crate) fn fit(
    source: &Surface,
    spacing: &[f64],
    positions: &mut [Vec3],
    quads: &[[usize; 4]],
    cancel: &dyn Fn() -> bool,
) -> Result<Fit> {
    let original = Bvh::new(source, cancel)?;
    let mut work = 50_000_000;
    let mut displacement = 0f64;
    for (i, p) in positions.iter_mut().enumerate() {
        if i % 256 == 0 {
            check(cancel)?;
        }
        let hit = original.nearest(*p, &mut work, cancel)?;
        let h = source.triangles[hit.face]
            .iter()
            .map(|&v| spacing[v])
            .fold(0., f64::max);
        displacement = displacement.max(hit.distance);
        if hit.distance > h {
            return Err(format!(
                "Retopology vertex {i} drifted {:.5} from the source (local grid spacing {h:.5})",
                hit.distance
            ));
        }
        *p = hit.point;
    }
    let mut triangles =
        oriented_triangulation(&original, positions, quads, &mut work, cancel, false)?;
    if !untangle(&original, positions, quads, &triangles, &mut work, cancel)? {
        triangles = outward_triangulation(&original, positions, quads, &mut work, cancel)?;
    }
    let result = Surface {
        positions: positions.to_vec(),
        triangles,
    };
    let error = measure(source, spacing, &result, &original, &mut work, cancel)?;
    Ok(Fit {
        displacement,
        error,
        triangles: result.triangles,
    })
}

/// Check a completed mesh without projecting or moving any vertex.
pub(crate) fn validate_output(
    source: &Surface,
    spacing: &[f64],
    positions: Vec<Vec3>,
    quads: &[[usize; 4]],
    cancel: &dyn Fn() -> bool,
) -> Result<Fit> {
    let original = Bvh::new(source, cancel)?;
    let mut work = 50_000_000;
    let triangles = outward_triangulation(&original, &positions, quads, &mut work, cancel)?;
    let result = Surface {
        positions,
        triangles,
    };
    let error = measure(source, spacing, &result, &original, &mut work, cancel)?;
    if crate::topology::validate(&result)? != crate::topology::validate(source)? {
        return Err("Reference mesh changed source topology".into());
    }
    Ok(Fit {
        displacement: 0.,
        error,
        triangles: result.triangles,
    })
}

fn measure(
    source: &Surface,
    spacing: &[f64],
    result: &Surface,
    original: &Bvh<'_>,
    work: &mut usize,
    cancel: &dyn Fn() -> bool,
) -> Result<f64> {
    crate::topology::validate(result)?;
    let mut error = 0f64;
    for (fi, t) in result.triangles.iter().enumerate() {
        if fi % 256 == 0 {
            check(cancel)?;
        }
        let [a, b, c] = t.map(|v| result.positions[v]);
        let normal = (b - a).cross(c - a);
        let hit = original.nearest((a + b + c) / 3., work, cancel)?;
        if normal.dot(hit.normal) <= 0. {
            return Err(format!(
                "Fitted retopology triangle {fi} folds against the source surface at {:?}",
                (a + b + c) / 3.
            ));
        }
        for p in [
            (a + b + c) / 3.,
            (a + b) * 0.5,
            (b + c) * 0.5,
            (c + a) * 0.5,
        ] {
            let hit = original.nearest(p, work, cancel)?;
            let h = source.triangles[hit.face]
                .iter()
                .map(|&v| spacing[v])
                .fold(0., f64::max);
            error = error.max(hit.distance);
            if hit.distance > h {
                return Err(format!(
                    "Retopology bridges away from the source at {p:?}: error {:.5}, local grid spacing {h:.5}",
                    hit.distance
                ));
            }
        }
    }
    let reconstructed = Bvh::new(result, cancel)?;
    let mut measure = |p: Vec3, h: f64| -> Result<()> {
        let hit = reconstructed.nearest(p, work, cancel)?;
        error = error.max(hit.distance);
        if hit.distance > h {
            return Err(format!(
                "Retopology lost source shape at {p:?}: error {:.5}, local grid spacing {h:.5}",
                hit.distance
            ));
        }
        Ok(())
    };
    for (i, &p) in source.positions.iter().enumerate() {
        if i % 256 == 0 {
            check(cancel)?;
        }
        measure(p, spacing[i])?;
    }
    for (fi, &[a, b, c]) in source.triangles.iter().enumerate() {
        if fi % 256 == 0 {
            check(cancel)?;
        }
        let [pa, pb, pc] = [a, b, c].map(|v| source.positions[v]);
        measure(
            (pa + pb + pc) / 3.,
            (spacing[a] + spacing[b] + spacing[c]) / 3.,
        )?;
        for (u, v) in [(a, b), (b, c), (c, a)] {
            measure(
                (source.positions[u] + source.positions[v]) * 0.5,
                (spacing[u] + spacing[v]) * 0.5,
            )?;
        }
    }
    Ok(error)
}

/// Repair local continuous folds after coincident lattice fans are separated.
/// Only vertices in a folded patch move, and every proposal is projected to
/// the original surface. Final geometry and two-way shape gates are unchanged.
fn untangle(
    source: &Bvh<'_>,
    positions: &mut [Vec3],
    quads: &[[usize; 4]],
    triangles: &[[usize; 3]],
    work: &mut usize,
    cancel: &dyn Fn() -> bool,
) -> Result<bool> {
    let bad_faces = |positions: &[Vec3], work: &mut usize| -> Result<Vec<usize>> {
        let mut bad = Vec::new();
        for (i, &t) in triangles.iter().enumerate() {
            if i % 256 == 0 {
                check(cancel)?;
            }
            let [a, b, c] = t.map(|v| positions[v]);
            let n = (b - a).cross(c - a);
            if n.length_squared() < 1e-24
                || n.dot(source.nearest((a + b + c) / 3., work, cancel)?.normal) <= 0.
            {
                bad.push(i);
            }
        }
        Ok(bad)
    };
    let mut bad = bad_faces(positions, work)?;
    if bad.is_empty() {
        return Ok(true);
    }
    let mut neighbours = vec![Vec::new(); positions.len()];
    let mut edges = std::collections::BTreeMap::new();
    for q in quads {
        for i in 0..4 {
            let a = q[i];
            let b = q[(i + 1) % 4];
            neighbours[a].push(b);
            neighbours[b].push(a);
            *edges.entry(crate::topology::edge(a, b)).or_insert(0) += 1;
        }
    }
    for row in &mut neighbours {
        row.sort_unstable();
        row.dedup();
    }
    let mut fixed = vec![false; positions.len()];
    for ([a, b], count) in edges {
        if count == 1 {
            fixed[a] = true;
            fixed[b] = true;
        }
    }
    let mut best = (bad.len(), positions.to_vec());
    for _ in 0..128 {
        check(cancel)?;
        let mut active = vec![false; positions.len()];
        for &f in &bad {
            for v in triangles[f] {
                active[v] = true;
                for &u in &neighbours[v] {
                    active[u] = true;
                }
            }
        }
        let immediate = active.clone();
        for (v, &dirty) in immediate.iter().enumerate() {
            if dirty {
                for &u in &neighbours[v] {
                    active[u] = true;
                }
            }
        }
        let old = positions.to_vec();
        for i in 0..positions.len() {
            if !active[i] || fixed[i] || neighbours[i].is_empty() {
                continue;
            }
            let average =
                neighbours[i].iter().map(|&j| old[j]).sum::<Vec3>() / neighbours[i].len() as f64;
            let hit = source.nearest(old[i] * 0.6 + average * 0.4, work, cancel)?;
            let before = source.nearest(old[i], work, cancel)?;
            if hit.normal.dot(before.normal) > 0. {
                positions[i] = hit.point;
            }
        }
        bad = bad_faces(positions, work)?;
        if bad.is_empty() {
            return Ok(true);
        }
        if bad.len() < best.0 {
            best = (bad.len(), positions.to_vec());
        }
    }
    positions.copy_from_slice(&best.1);
    Ok(false)
}

/// Measurements for rejected audit candidates. This never turns a failure into
/// an accepted Layout, and does not move vertices or relax the acceptance gate.
#[cfg(feature = "audit")]
pub(crate) fn quality_report(
    source: &Surface,
    spacing: &[f64],
    positions: &[Vec3],
    quads: &[[usize; 4]],
) -> Result<serde_json::Value> {
    let cancel = || false;
    let original = Bvh::new(source, &cancel)?;
    let mut work = 50_000_000;
    let mut bad = Vec::new();
    let perimeter: std::collections::BTreeSet<_> = quads
        .iter()
        .flat_map(|q| (0..4).map(|k| crate::topology::edge(q[k], q[(k + 1) % 4])))
        .collect();
    for (i, &[a, b, c, d]) in quads.iter().enumerate() {
        let mut reasons = Vec::new();
        let mut valid = 0;
        for (edge, ts) in [
            (crate::topology::edge(a, c), [[a, b, c], [a, c, d]]),
            (crate::topology::edge(b, d), [[a, b, d], [b, c, d]]),
        ] {
            let blocked = perimeter.contains(&edge);
            let mut dots = Vec::new();
            let mut degenerate = false;
            for t in ts {
                let [p, q, r] = t.map(|v| positions[v]);
                let n = (q - p).cross(r - p);
                degenerate |= n.length_squared() < 1e-24;
                dots.push(
                    n.normalize_or_zero().dot(
                        original
                            .nearest((p + q + r) / 3., &mut work, &cancel)?
                            .normal,
                    ),
                );
            }
            if !blocked && !degenerate && dots.iter().all(|&d| d > 0.) {
                valid += 1;
            }
            reasons.push(serde_json::json!({"perimeter_collision":blocked,"degenerate":degenerate,"normal_dots":dots}));
        }
        if valid == 0 {
            bad.push(serde_json::json!({"quad":i,"center":((positions[a]+positions[b]+positions[c]+positions[d])/4.).to_array(),"diagonals":reasons}));
        }
    }
    let triangles = triangulate(quads, &cancel)?;
    let result = Surface {
        positions: positions.to_vec(),
        triangles,
    };
    let topology = crate::topology::validate(&result).map(|t| format!("{t:?}"));
    let reconstructed = Bvh::new(&result, &cancel)?;
    let mut forward = (0f64, 0f64, Vec3::ZERO);
    let mut backward = forward;
    for &t in &result.triangles {
        let [a, b, c] = t.map(|v| positions[v]);
        for p in [
            a,
            b,
            c,
            (a + b) * 0.5,
            (b + c) * 0.5,
            (c + a) * 0.5,
            (a + b + c) / 3.,
        ] {
            let hit = original.nearest(p, &mut work, &cancel)?;
            let h = source.triangles[hit.face]
                .iter()
                .map(|&v| spacing[v])
                .fold(0., f64::max);
            if hit.distance / h > forward.0 {
                forward = (hit.distance / h, hit.distance, p);
            }
        }
    }
    let mut measure = |p: Vec3, h: f64| -> Result<()> {
        let hit = reconstructed.nearest(p, &mut work, &cancel)?;
        if hit.distance / h > backward.0 {
            backward = (hit.distance / h, hit.distance, p);
        }
        Ok(())
    };
    for (i, &p) in source.positions.iter().enumerate() {
        measure(p, spacing[i])?;
    }
    for &[a, b, c] in &source.triangles {
        measure(
            (source.positions[a] + source.positions[b] + source.positions[c]) / 3.,
            (spacing[a] + spacing[b] + spacing[c]) / 3.,
        )?;
        for (u, v) in [(a, b), (b, c), (c, a)] {
            measure(
                (source.positions[u] + source.positions[v]) * 0.5,
                (spacing[u] + spacing[v]) * 0.5,
            )?;
        }
    }
    Ok(
        serde_json::json!({"vertices":positions.len(),"quads":quads.len(),"topology":topology,
        "quads_without_outward_diagonal":bad.len(),"bad_quads":bad,
        "result_to_source":{"max_spacing_ratio":forward.0,"distance_at_max_ratio":forward.1,"point":forward.2.to_array()},
        "source_to_result":{"max_spacing_ratio":backward.0,"distance_at_max_ratio":backward.1,"point":backward.2.to_array()},
        "note":"Distance measurements use topology-only render diagonals; these are diagnostics, not acceptance or a self-intersection test."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitting_uses_valid_diagonals_before_moving_a_concave_patch() {
        let source = Surface {
            positions: vec![
                Vec3::ZERO,
                Vec3::X * 2.,
                (Vec3::X + Vec3::Y) * 2.,
                Vec3::Y * 2.,
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut positions: Vec<_> = (0..3)
            .flat_map(|y| (0..3).map(move |x| Vec3::new(x as f64, y as f64, 0.)))
            .collect();
        positions[4] = Vec3::new(1.8, 0.2, 0.);
        let before = positions.clone();
        let quads = [[0, 1, 4, 3], [1, 2, 5, 4], [3, 4, 7, 6], [4, 5, 8, 7]];
        let result = fit(&source, &[1.; 4], &mut positions, &quads, &|| false).unwrap();
        for (a, b) in positions.iter().zip(before) {
            assert!(
                a.distance(b) < 1e-12,
                "A valid concave patch was unnecessarily smoothed"
            );
        }
        for t in result.triangles {
            let [a, b, c] = t.map(|v| positions[v]);
            assert!((b - a).cross(c - a).z > 0.);
        }
    }
    #[test]
    fn concave_quad_uses_the_outward_diagonal_without_moving_its_vertices() {
        let source = Surface {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::X + Vec3::Y, Vec3::Y],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let positions = vec![
            Vec3::ZERO,
            Vec3::X,
            Vec3::X + Vec3::Y,
            Vec3::new(0.8, 0.2, 0.),
        ];
        let bvh = Bvh::new(&source, &|| false).unwrap();
        let triangles =
            outward_triangulation(&bvh, &positions, &[[0, 1, 2, 3]], &mut 1000, &|| false).unwrap();
        assert_eq!(triangles, [[0, 1, 3], [1, 2, 3]]);
        for t in triangles {
            let [a, b, c] = t.map(|i| positions[i]);
            assert!((b - a).cross(c - a).z > 0.);
        }
    }
    #[test]
    fn quad_triangulation_avoids_existing_polygon_edges() {
        let quads = [[0, 1, 2, 3], [0, 2, 4, 5]];
        let triangles = triangulate(&quads, &|| false).unwrap();
        assert_eq!(&triangles[..2], &[[0, 1, 3], [1, 2, 3]]);
        let blocked = [[0, 1, 2, 3], [0, 2, 4, 5], [1, 3, 6, 7]];
        assert!(triangulate(&blocked, &|| false).is_err());
        assert!(triangulate(&quads, &|| true).is_err());
    }
    #[test]
    fn surface_fitting_separates_a_coincident_interior_grid_vertex() {
        let source = Surface {
            positions: vec![
                Vec3::ZERO,
                Vec3::X * 2.,
                (Vec3::X + Vec3::Y) * 2.,
                Vec3::Y * 2.,
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        };
        let mut positions: Vec<_> = (0..3)
            .flat_map(|y| (0..3).map(move |x| Vec3::new(x as f64, y as f64, 0.)))
            .collect();
        positions[4] = positions[5];
        let quads = vec![[0, 1, 4, 3], [1, 2, 5, 4], [3, 4, 7, 6], [4, 5, 8, 7]];
        let error = fit(&source, &[1.; 4], &mut positions, &quads, &|| false)
            .unwrap()
            .error;
        assert!(positions[4].x < 1.9 && positions[4].x > 0.1);
        assert!(error < 1e-10);
    }
    #[test]
    fn nearest_surface_finds_interior_edges_and_vertices_across_nodes() {
        let mut s = Surface {
            positions: vec![],
            triangles: vec![],
        };
        for i in 0..20 {
            let n = s.positions.len();
            let x = i as f64 * 3.;
            s.positions.extend([
                Vec3::new(x, 0., 0.),
                Vec3::new(x + 1., 0., 0.),
                Vec3::new(x, 1., 0.),
            ]);
            s.triangles.push([n, n + 1, n + 2]);
        }
        let bvh = Bvh::new(&s, &|| false).unwrap();
        let mut budget = 10000;
        for i in 0..20 {
            let x = i as f64 * 3.;
            for (query, expected) in [
                (Vec3::new(x + 0.2, 0.3, 1.), Vec3::new(x + 0.2, 0.3, 0.)),
                (Vec3::new(x + 0.8, 0.8, 1.), Vec3::new(x + 0.5, 0.5, 0.)),
                (Vec3::new(x - 0.2, -0.2, 1.), Vec3::new(x, 0., 0.)),
            ] {
                let hit = bvh.nearest(query, &mut budget, &|| false).unwrap();
                assert_eq!(hit.face, i);
                assert!((hit.point - expected).length() < 1e-10);
                assert_eq!(hit.normal, Vec3::Z);
                assert!((hit.distance - (query - expected).length()).abs() < 1e-10);
            }
        }
        assert!(bvh.nearest(Vec3::ZERO, &mut 0, &|| false).is_err());
        assert!(Bvh::new(&s, &|| true).is_err());
    }
}
