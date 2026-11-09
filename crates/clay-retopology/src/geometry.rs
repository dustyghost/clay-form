//! Rust adaptation of QuadriFlow's optimize_positions_fixed, optimizer.cpp,
//! commit 810b7a0967c35b0dc85b4464e3835e26a756c967. See LICENSE-QuadriFlow.
//! The normal equations and aggregation follow upstream; the linear solver is
//! a cancellable, matrix-free, diagonally preconditioned conjugate gradient.
use crate::symmetry::{Extrinsic, Frame, RoSy4};
use crate::{Result, Vec3, check, field::Field, integer::Edge, topology::Dsu};
use std::collections::BTreeMap;

pub(crate) struct FramePoint {
    pub origin: Vec3,
    pub axes: [Vec3; 2],
    pub initial: [f64; 2],
    pub fixed: [bool; 2],
}

pub(crate) struct Distance {
    pub ends: [usize; 2],
    pub vector: Vec3,
}

/// Same ideal displacement as upstream, including averaging the two spacings
/// separately from the frame directions. Recompute frame matching after edge
/// subdivision: stored integer transport can encode different singular frames.
pub(crate) fn ideal(field: &Field, edge: &Edge) -> Vec3 {
    let [a, b] = edge.ends;
    let q = field.directions[a];
    let n = field.normals[a];
    let r = field.directions[b];
    let m = field.normals[b];
    let matching = Extrinsic::match_orientation(Frame::new(q, n), Frame::new(r, m));
    let rank = (matching.rhs.1 - matching.lhs.1).rem_euclid(4);
    let rotate = |v: Vec3| match rank {
        0 => v,
        1 => m.cross(v),
        2 => -v,
        _ => -m.cross(v),
    };
    let x = (q + rotate(r)) * 0.5;
    let y = (n.cross(q) + rotate(m.cross(r))) * 0.5;
    (x * f64::from(edge.d.x) + y * f64::from(edge.d.y))
        * ((field.spacing[a] + field.spacing[b]) * 0.5)
}

pub(crate) fn fit_fixed(
    field: &mut Field,
    edges: &[Edge],
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    let n = field.origins.len();
    let mut tree = Dsu::new(n);
    for e in edges {
        if e.d == glam::IVec2::ZERO {
            tree.join(e.ends[0], e.ends[1]);
        }
    }
    let mut ids = BTreeMap::new();
    let groups: Vec<_> = (0..n)
        .map(|i| {
            let next = ids.len();
            *ids.entry(tree.find(i)).or_insert(next)
        })
        .collect();
    let mut means = vec![Vec3::ZERO; ids.len()];
    let mut counts = vec![0; ids.len()];
    for (i, &g) in groups.iter().enumerate() {
        means[g] += field.origins[i];
        counts[g] += 1;
    }
    for (p, count) in means.iter_mut().zip(counts) {
        *p /= f64::from(count);
    }
    let mut representatives = vec![usize::MAX; ids.len()];
    let mut distances = vec![f64::INFINITY; ids.len()];
    for (i, &g) in groups.iter().enumerate() {
        let distance = means[g].distance_squared(field.surface.positions[i]);
        if distance < distances[g] {
            distances[g] = distance;
            representatives[g] = i;
        }
    }
    // Hard boundary/feature constraints only. Guide strokes steer the field;
    // they must not accidentally pin the output to the path's sample positions.
    for (i, &g) in groups.iter().enumerate() {
        if field.constraints[i].weight >= 1. {
            representatives[g] = i;
            means[g] = field.origins[i];
        }
    }
    let frames: Vec<_> = representatives
        .iter()
        .enumerate()
        .map(|(g, &i)| {
            let c = field.constraints[i];
            let hard = c.weight >= 1.;
            let origin = if hard {
                c.origin
            } else {
                field.surface.positions[i]
            };
            let q = if hard { c.tangent } else { field.directions[i] };
            let axes = [q, field.normals[i].cross(q)];
            FramePoint {
                origin,
                axes,
                initial: axes.map(|axis| (means[g] - origin).dot(axis)),
                fixed: [false, hard],
            }
        })
        .collect();
    // Upstream weights each ordered pair of collapsed vertices once, averaging
    // all contributing source edges. Dense input triangulation gets no extra vote.
    let mut sums = BTreeMap::<[usize; 2], (Vec3, usize)>::new();
    for (i, e) in edges.iter().enumerate() {
        if i % 4096 == 0 {
            check(cancel)?;
        }
        let pair = e.ends.map(|v| groups[v]);
        if pair[0] == pair[1] {
            continue;
        }
        let entry = sums.entry(pair).or_default();
        entry.0 += ideal(field, e);
        entry.1 += 1;
    }
    let terms: Vec<_> = sums
        .into_iter()
        .map(|(ends, (sum, count))| Distance {
            ends,
            vector: sum / count as f64,
        })
        .collect();
    let fitted = solve(&frames, &terms, cancel)?;
    for (i, &g) in groups.iter().enumerate() {
        field.origins[i] = fitted[g];
    }
    Ok(())
}

/// Minimise sum |P_b - P_a - d_ab|² with each P expressed in its local
/// two-dimensional tangent plane. Eliminate fixed coordinates symmetrically.
pub(crate) fn solve(
    frames: &[FramePoint],
    terms: &[Distance],
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Vec3>> {
    let n = frames.len() * 2;
    let mut rhs = vec![0.; n];
    let mut diagonal = vec![0.; n];
    let mut x: Vec<_> = frames.iter().flat_map(|f| f.initial).collect();
    let free: Vec<_> = frames.iter().flat_map(|f| f.fixed.map(|v| !v)).collect();
    for term in terms {
        let [a, b] = term.ends;
        let indices = [a * 2, a * 2 + 1, b * 2, b * 2 + 1];
        let axes = [
            -frames[a].axes[0],
            -frames[a].axes[1],
            frames[b].axes[0],
            frames[b].axes[1],
        ];
        let mut target = term.vector - (frames[b].origin - frames[a].origin);
        for k in 0..4 {
            if !free[indices[k]] {
                target -= axes[k] * x[indices[k]];
            }
        }
        for k in 0..4 {
            let i = indices[k];
            if free[i] {
                rhs[i] += axes[k].dot(target);
                diagonal[i] += axes[k].length_squared();
            }
        }
    }
    let multiply = |input: &[f64], output: &mut [f64]| {
        output.fill(0.);
        for term in terms {
            let [a, b] = term.ends;
            let mut delta = Vec3::ZERO;
            for k in 0..2 {
                if free[a * 2 + k] {
                    delta -= frames[a].axes[k] * input[a * 2 + k];
                }
                if free[b * 2 + k] {
                    delta += frames[b].axes[k] * input[b * 2 + k];
                }
            }
            for k in 0..2 {
                if free[a * 2 + k] {
                    output[a * 2 + k] -= frames[a].axes[k].dot(delta);
                }
                if free[b * 2 + k] {
                    output[b * 2 + k] += frames[b].axes[k].dot(delta);
                }
            }
        }
    };
    let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    let mut ax = vec![0.; n];
    multiply(&x, &mut ax);
    let mut residual: Vec<_> = rhs.iter().zip(&ax).map(|(b, a)| b - a).collect();
    let mut z = vec![0.; n];
    let precondition = |r: &[f64], out: &mut [f64]| {
        for i in 0..n {
            out[i] = if diagonal[i] > 1e-20 {
                r[i] / diagonal[i]
            } else {
                0.
            };
        }
    };
    precondition(&residual, &mut z);
    let mut direction = z.clone();
    let mut rz = dot(&residual, &z);
    let tolerance = (dot(&rhs, &rhs) * 1e-20).max(1e-24);
    let mut converged = dot(&residual, &residual) <= tolerance;
    for _ in 0..(n * 4).clamp(64, 20_000) {
        check(cancel)?;
        if converged {
            break;
        }
        multiply(&direction, &mut ax);
        let curvature = dot(&direction, &ax);
        if !curvature.is_finite() || curvature <= 0. {
            break;
        }
        let alpha = rz / curvature;
        for i in 0..n {
            x[i] += alpha * direction[i];
            residual[i] -= alpha * ax[i];
        }
        converged = dot(&residual, &residual) <= tolerance;
        precondition(&residual, &mut z);
        let next = dot(&residual, &z);
        let beta = next / rz;
        for i in 0..n {
            direction[i] = z[i] + beta * direction[i];
        }
        rz = next;
    }
    // Check the actual normal-equation residual, not just the CG recurrence.
    multiply(&x, &mut ax);
    let error = rhs
        .iter()
        .zip(ax)
        .map(|(b, a)| (b - a).powi(2))
        .sum::<f64>();
    if !error.is_finite() || error > tolerance * 100. {
        return Err(format!(
            "Grid position solve did not converge (residual {error:e})"
        ));
    }
    let positions: Vec<_> = frames
        .iter()
        .enumerate()
        .map(|(i, f)| f.origin + f.axes[0] * x[i * 2] + f.axes[1] * x[i * 2 + 1])
        .collect();
    if positions.iter().any(|p| !p.is_finite()) {
        return Err("Grid position solve produced a non-finite position".into());
    }
    Ok(positions)
}

/// Post-extraction optimizer from QuadriFlow's optimize_positions_dynamic.
/// Reassign tangent planes by walking the source surface, interpolate missing
/// ideal edges around split poles, and solve the complete quad graph together.
/// Two deliberate corrections: normalize the transport rotation axis, and
/// handle parallel interpolation directions without producing NaNs.
pub(crate) fn fit_dynamic(
    field: &Field,
    edges: &[Edge],
    mesh: &mut crate::integer::Extraction,
    cancel: &dyn Fn() -> bool,
) -> Result<()> {
    fit_dynamic_observed(field, edges, mesh, cancel, &|_, _, _| Ok(()))
}
pub(crate) type DynamicTrace<'a> = dyn Fn(usize, &[usize], &[Vec3]) -> Result<()> + 'a;
pub(crate) fn fit_dynamic_observed(
    field: &Field,
    edges: &[Edge],
    mesh: &mut crate::integer::Extraction,
    cancel: &dyn Fn() -> bool,
    trace: &DynamicTrace<'_>,
) -> Result<()> {
    use crate::quad::{NONE, connections, next, prev, vertex};
    let graph = connections(&mesh.quads, mesh.positions.len())?;
    let (mut diffs, mut counts) = dynamic_targets(field, edges, mesh);
    let mut uncertain = vec![false; mesh.positions.len()];
    for h in 0..diffs.len() {
        if counts[h] == 0 {
            uncertain[vertex(&mesh.quads, h)] = true;
            uncertain[vertex(&mesh.quads, next(h))] = true;
        }
    }
    let mut source_adj = vec![Vec::new(); field.origins.len()];
    for t in &field.surface.triangles {
        for k in 0..3 {
            source_adj[t[k]].push(t[(k + 1) % 3]);
        }
    }
    let mut nearest = vec![NONE; mesh.positions.len()];
    let hard: Vec<_> = mesh
        .samples
        .iter()
        .map(|samples| {
            samples
                .iter()
                .copied()
                .find(|&s| field.constraints[s].weight >= 1.)
        })
        .collect();
    for iter in 0..10 {
        check(cancel)?;
        for (v, sample) in nearest.iter_mut().enumerate() {
            if *sample == NONE {
                *sample = *mesh.samples[v]
                    .iter()
                    .min_by(|&&a, &&b| {
                        field.surface.positions[a]
                            .distance_squared(mesh.positions[v])
                            .total_cmp(
                                &field.surface.positions[b].distance_squared(mesh.positions[v]),
                            )
                    })
                    .ok_or("Quad vertex lost its source membership")?;
                let s = *sample;
                let n = field.normals[s];
                let point = mesh.positions[v];
                mesh.positions[v] = point - n * (point - field.surface.positions[s]).dot(n);
            } else {
                let normal = field.normals[*sample];
                loop {
                    let old = *sample;
                    let mut best = old;
                    let mut distance =
                        field.surface.positions[old].distance_squared(mesh.positions[v]);
                    for &s in &source_adj[old] {
                        if field.normals[s].dot(normal) < 10f64.to_radians().cos() {
                            continue;
                        }
                        let d = field.surface.positions[s].distance_squared(mesh.positions[v]);
                        if d < distance {
                            distance = d;
                            best = s;
                        }
                    }
                    if best == old {
                        break;
                    }
                    let n1 = field.normals[old];
                    let n2 = field.normals[best];
                    let axis = n1.cross(n2);
                    let length = axis.length();
                    if length > 1e-12 {
                        let rotation =
                            glam::DQuat::from_axis_angle(axis / length, length.atan2(n1.dot(n2)));
                        for &h in &graph.rings[v] {
                            diffs[h] = rotation * diffs[h];
                        }
                    }
                    *sample = best;
                }
            }
        }
        // Complete edge observations by angular interpolation in ordered fans.
        loop {
            let mut changed = false;
            for (v, ring) in graph.rings.iter().enumerate() {
                let known = ring.iter().filter(|&&h| counts[h] > 0).count();
                if known < 2 || known == ring.len() {
                    continue;
                }
                let n = field.normals[nearest[v]];
                for i in 0..ring.len() {
                    if counts[ring[i]] > 0 {
                        continue;
                    }
                    let mut left = (i + ring.len() - 1) % ring.len();
                    while counts[ring[left]] == 0 {
                        left = (left + ring.len() - 1) % ring.len();
                    }
                    let mut right = (i + 1) % ring.len();
                    while counts[ring[right]] == 0 {
                        right = (right + 1) % ring.len();
                    }
                    let dl = diffs[ring[left]];
                    let dr = diffs[ring[right]];
                    let lenl = dl.length();
                    let lenr = dr.length();
                    if lenl < 1e-12 || lenr < 1e-12 {
                        continue;
                    }
                    let dl = dl / lenl;
                    let dr = dr / lenr;
                    let mut angle = dl.cross(dr).length().atan2(dl.dot(dr));
                    if dl.cross(dr).dot(n) < 0. {
                        angle = std::f64::consts::TAU - angle;
                    }
                    let perpendicular = n.cross(dl).normalize_or_zero();
                    let steps = (right + ring.len() - left) % ring.len();
                    for k in 1..steps {
                        let fraction = k as f64 / steps as f64;
                        let h = ring[(left + k) % ring.len()];
                        diffs[h] = (dl * (angle * fraction).cos()
                            + perpendicular * (angle * fraction).sin())
                            * (lenl + (lenr - lenl) * fraction);
                        counts[h] = 2;
                        let t = graph.twins[h];
                        if t != NONE {
                            diffs[t] = -diffs[h];
                            counts[t] = 2;
                        }
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let frames: Vec<_> = nearest
            .iter()
            .enumerate()
            .map(|(v, &s)| {
                let (origin, q) = if let Some(h) = hard[v] {
                    (field.constraints[h].origin, field.constraints[h].tangent)
                } else {
                    (field.surface.positions[s], field.directions[s])
                };
                let axes = [q, field.normals[s].cross(q)];
                FramePoint {
                    origin,
                    axes,
                    initial: axes.map(|a| (mesh.positions[v] - origin).dot(a)),
                    fixed: [false, hard[v].is_some()],
                }
            })
            .collect();
        let terms: Vec<_> = diffs
            .iter()
            .enumerate()
            .map(|(h, &vector)| Distance {
                ends: [vertex(&mesh.quads, h), vertex(&mesh.quads, next(h))],
                vector,
            })
            .collect();
        mesh.positions = solve(&frames, &terms, cancel)?;
        trace(iter, &nearest, &mesh.positions)?;
        if iter == 9 {
            for _ in 0..5 {
                for (v, ring) in graph.rings.iter().enumerate() {
                    if hard[v].is_some() || (ring.len() == 4 && !uncertain[v]) {
                        continue;
                    }
                    let p = mesh.positions[v];
                    let mut normal = Vec3::ZERO;
                    let mut sum = Vec3::ZERO;
                    for &h in ring {
                        let a = mesh.positions[vertex(&mesh.quads, next(h))];
                        let b = mesh.positions[vertex(&mesh.quads, prev(h))];
                        normal += (a - p).cross(b - p);
                        sum += a;
                    }
                    let normal = normal.normalize_or_zero();
                    let delta = sum / ring.len() as f64 - p;
                    mesh.positions[v] += delta - normal * normal.dot(delta);
                }
            }
        }
    }
    Ok(())
}

/// Ideal unit-edge observations shared with the upstream comparison harness.
pub(crate) fn dynamic_targets(
    field: &Field,
    edges: &[Edge],
    mesh: &crate::integer::Extraction,
) -> (Vec<Vec3>, Vec<usize>) {
    use crate::quad::{next, vertex};
    let mut original_to_output = vec![Vec::new(); field.origins.len()];
    for (v, samples) in mesh.samples.iter().enumerate() {
        for &s in samples {
            original_to_output[s].push(v);
        }
    }
    let mut directed = BTreeMap::new();
    for h in 0..mesh.quads.len() * 4 {
        directed.insert((vertex(&mesh.quads, h), vertex(&mesh.quads, next(h))), h);
    }
    let mut diffs = vec![Vec3::ZERO; mesh.quads.len() * 4];
    let mut counts = vec![0usize; diffs.len()];
    for e in edges {
        if e.d.abs().element_sum() != 1 {
            continue;
        }
        let [a, b] = e.ends;
        if original_to_output[a].len() != 1 || original_to_output[b].len() != 1 {
            continue;
        }
        let [a, b] = [original_to_output[a][0], original_to_output[b][0]];
        let d = ideal(field, e);
        if let Some(&h) = directed.get(&(a, b)) {
            diffs[h] += d;
            counts[h] += 1;
        }
        if let Some(&h) = directed.get(&(b, a)) {
            diffs[h] -= d;
            counts[h] += 1;
        }
    }
    for (d, &n) in diffs.iter_mut().zip(&counts) {
        if n > 0 {
            *d /= n as f64;
        }
    }
    (diffs, counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_stage_matches_pinned_upstream_on_curved_mixed_scale_groups() {
        let mut input = include_str!("../tests/data/fixed-curved.input").split_whitespace();
        let n: usize = input.next().unwrap().parse().unwrap();
        let e: usize = input.next().unwrap().parse().unwrap();
        let mut f = Field {
            surface: crate::Surface {
                positions: vec![],
                triangles: vec![],
            },
            normals: vec![],
            directions: vec![],
            origins: vec![],
            spacing: vec![],
            areas: vec![1.; n],
            constraints: vec![crate::field::Constraint::default(); n],
        };
        for _ in 0..n {
            let values: Vec<f64> = input
                .by_ref()
                .take(13)
                .map(|x| x.parse().unwrap())
                .collect();
            let vector = |k| Vec3::new(values[k], values[k + 1], values[k + 2]);
            f.surface.positions.push(vector(0));
            f.normals.push(vector(3));
            f.directions.push(vector(6));
            f.origins.push(vector(9));
            f.spacing.push(values[12]);
        }
        let mut edges = Vec::new();
        for _ in 0..e {
            let v: Vec<i32> = input.by_ref().take(4).map(|x| x.parse().unwrap()).collect();
            edges.push(Edge {
                ends: [v[0] as usize, v[1] as usize],
                d: glam::IVec2::new(v[2], v[3]),
                transport: 0,
                fixed_axis: None,
            });
        }
        fit_fixed(&mut f, &edges, &|| false).unwrap();
        let expected: Vec<_> = include_str!("../tests/data/fixed-curved.expected")
            .lines()
            .map(|l| {
                let v: Vec<f64> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
                Vec3::new(v[0], v[1], v[2])
            })
            .collect();
        assert_eq!(f.origins.len(), expected.len());
        for (actual, expected) in f.origins.iter().zip(expected) {
            assert!(
                actual.distance(expected) < 1e-8,
                "{actual:?} != {expected:?}"
            );
        }
        for pair in f.origins.as_chunks::<2>().0 {
            assert_eq!(pair[0], pair[1]);
        }
    }
    #[test]
    fn tangent_solve_preserves_fixed_coordinates_and_cancels() {
        let frames = vec![
            FramePoint {
                origin: Vec3::ZERO,
                axes: [Vec3::X, Vec3::Y],
                initial: [2., 3.],
                fixed: [true, true],
            },
            FramePoint {
                origin: Vec3::ZERO,
                axes: [Vec3::X, Vec3::Y],
                initial: [9., 8.],
                fixed: [false, true],
            },
        ];
        let terms = vec![Distance {
            ends: [0, 1],
            vector: Vec3::new(4., 5., 0.),
        }];
        let fitted = solve(&frames, &terms, &|| false).unwrap();
        assert!(fitted[0].distance(Vec3::new(2., 3., 0.)) < 1e-10);
        assert!(fitted[1].distance(Vec3::new(6., 8., 0.)) < 1e-10);
        assert!(
            solve(&frames, &terms, &|| true)
                .unwrap_err()
                .contains("cancelled")
        );
    }
}
