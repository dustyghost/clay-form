//! Audit-only geometric direction hints, with no object labels or region masks.
//! Fits a symmetric normal derivative in connected surface neighbourhoods.
//! All distances are relative to requested output spacing. This is a local
//! least-squares estimator, not a port of Rusinkiewicz's per-face tensor method.
use crate::{Result, Surface, Vec3, check, topology};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BinaryHeap};

#[derive(Clone, serde::Serialize)]
pub struct Hint {
    pub vertex: usize,
    pub position: [f64; 3],
    pub normal: [f64; 3],
    pub tangent: [f64; 3],
    pub radius: f64,
    pub strength: f64,
    pub curvatures: [f64; 2],
    pub confidence: f64,
    pub relative_fit_residual: f64,
    pub neighbourhood_vertices: usize,
}

#[derive(serde::Serialize)]
pub struct Report {
    pub spacing: f64,
    pub neighbourhood_radius: f64,
    pub candidate_count: usize,
    pub visited_edges: usize,
    pub work_budget_edges: usize,
    pub guides: Vec<Hint>,
}

#[derive(PartialEq)]
struct Distance(f64, usize);
impl Eq for Distance {}
impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0).then(other.1.cmp(&self.1))
    }
}

fn neighbourhood(
    graph: &[Vec<(usize, f64)>],
    seed: usize,
    radius: f64,
    work: &mut usize,
    limit: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<BTreeMap<usize, f64>> {
    let mut d = BTreeMap::from([(seed, 0.)]);
    let mut heap = BinaryHeap::from([Distance(0., seed)]);
    while let Some(Distance(value, u)) = heap.pop() {
        if value > d[&u] {
            continue;
        }
        for &(v, length) in &graph[u] {
            if (*work).is_multiple_of(4096) {
                check(cancel)?;
                if *work >= limit {
                    return Err("Curvature hints exceed their neighbourhood work budget".into());
                }
            }
            *work += 1;
            let next = value + length;
            if next < radius && next < d.get(&v).copied().unwrap_or(f64::INFINITY) {
                d.insert(v, next);
                heap.push(Distance(next, v));
            }
        }
    }
    Ok(d)
}

fn fit(
    s: &Surface,
    normals: &[Vec3],
    areas: &[f64],
    i: usize,
    neighbours: &BTreeMap<usize, f64>,
    h: f64,
) -> Option<Hint> {
    let radius = 3. * h;
    let n = normals[i];
    // This basis only represents a 2x2 tensor; its eigenvector is returned in
    // surface coordinates. It does not select a preferred world-space direction.
    let helper = if n.x.abs() < 0.8 { Vec3::X } else { Vec3::Y };
    let u = n.cross(helper).normalize_or_zero();
    let v = n.cross(u);
    let mut matrix = glam::DMat3::ZERO;
    let mut rhs = Vec3::ZERO;
    let mut energy = 0.;
    let mut count = 0;
    for (&j, &distance) in neighbours {
        if i == j || normals[j].dot(n) <= 0.5 {
            continue;
        }
        let delta = (s.positions[j] - s.positions[i]) / radius;
        let dn = normals[j] - n;
        let (x, y) = (delta.dot(u), delta.dot(v));
        let weight = areas[j] / (radius * radius) * (1. - distance / radius).max(0.05).powi(2);
        for (row, value) in [
            (Vec3::new(x, y, 0.), dn.dot(u)),
            (Vec3::new(0., x, y), dn.dot(v)),
        ] {
            matrix += glam::DMat3::from_cols(row * row.x, row * row.y, row * row.z) * weight;
            rhs += row * (weight * value);
            energy += weight * value * value;
        }
        count += 1;
    }
    let trace = matrix.x_axis.x + matrix.y_axis.y + matrix.z_axis.z;
    if count < 12 || energy <= 1e-24 || matrix.determinant() <= 1e-12 * trace.powi(3) {
        return None;
    }
    let tensor = matrix.inverse() * rhs;
    let residual =
        ((tensor.dot(matrix * tensor) - 2. * tensor.dot(rhs) + energy).max(0.) / energy).sqrt();
    let mid = (tensor.x + tensor.z) * 0.5;
    let spread = ((tensor.x - tensor.z) * 0.5).hypot(tensor.y);
    let values = [mid - spread, mid + spread];
    let along = usize::from(values[1].abs() < values[0].abs());
    let (small, large) = (values[along].abs(), values[1 - along].abs());
    let confidence = 1. - small / large.max(1e-30);
    // Dimensionless curvature threshold: flat/isotropic/noisy areas supply no
    // preferred axis. The existing field solve is free to choose there.
    if large < 0.2 || confidence < 0.65 || residual > 0.65 {
        return None;
    }
    let theta = (2. * tensor.y).atan2(tensor.x - tensor.z) * 0.5;
    let direction = if along == 0 {
        -u * theta.sin() + v * theta.cos()
    } else {
        u * theta.cos() + v * theta.sin()
    };
    Some(Hint {
        vertex: i,
        position: s.positions[i].to_array(),
        normal: n.to_array(),
        tangent: direction.normalize().to_array(),
        radius: 2.25 * h,
        strength: 0.7 * confidence,
        curvatures: values.map(|k| k / radius),
        confidence,
        relative_fit_residual: residual,
        neighbourhood_vertices: count,
    })
}

/// Generate direction hints at an output resolution. Geometry is unchanged.
/// This offline control uses uniform spacing; signed/adaptive density continues
/// to be evaluated separately by the solver, not inferred by this estimator.
pub fn estimate(
    source: &Surface,
    target_quads: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<Report> {
    check(cancel)?;
    if target_quads < 4 || source.positions.len() > 150_000 {
        return Err("Invalid curvature-hint sample budget".into());
    }
    topology::validate(source)?;
    let (_, areas, adjacent, area) = topology::geometry(source);
    let h = (area / target_quads as f64).sqrt();
    let mut normals = vec![Vec3::ZERO; source.positions.len()];
    for &[a, b, c] in &source.triangles {
        let cross = (source.positions[b] - source.positions[a])
            .cross(source.positions[c] - source.positions[a]);
        for i in [a, b, c] {
            normals[i] += cross;
        }
    }
    for n in &mut normals {
        *n = n.normalize_or_zero();
    }
    let graph: Vec<Vec<_>> = adjacent
        .iter()
        .enumerate()
        .map(|(i, row)| {
            row.iter()
                .map(|&j| (j, source.positions[i].distance(source.positions[j])))
                .collect()
        })
        .collect();
    // Scale the total traversal budget with input size, with a fixed ceiling.
    // This changes allowed work, not neighbourhood sizes or selected geometry.
    let work_budget = source
        .positions
        .len()
        .saturating_mul(8192)
        .clamp(40_000_000, 400_000_000);
    let mut work = 0;
    let mut candidates = Vec::new();
    for i in 0..source.positions.len() {
        check(cancel)?;
        let neighbours = neighbourhood(&graph, i, 3. * h, &mut work, work_budget, cancel)?;
        if let Some(g) = fit(source, &normals, &areas, i, &neighbours, h) {
            candidates.push(g);
        }
    }
    // Intrinsic thinning replaces the old world-axis voxel bins. Quantized
    // confidence makes numerical ties stable under rigid motions/scaling.
    let score = |g: &Hint| (g.confidence * (1. - g.relative_fit_residual) * 1e6).round() as i64;
    candidates.sort_by(|a, b| score(b).cmp(&score(a)).then(a.vertex.cmp(&b.vertex)));
    let candidate_count = candidates.len();
    let mut covered = vec![false; source.positions.len()];
    let mut guides = Vec::new();
    for g in candidates {
        if covered[g.vertex] {
            continue;
        }
        for &i in neighbourhood(&graph, g.vertex, h, &mut work, work_budget, cancel)?.keys() {
            covered[i] = true;
        }
        guides.push(g);
    }
    guides.sort_by_key(|g| g.vertex);
    Ok(Report {
        spacing: h,
        neighbourhood_radius: 3. * h,
        candidate_count,
        visited_edges: work,
        work_budget_edges: work_budget,
        guides,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn torus() -> Surface {
        let (n, m) = (64, 24);
        let mut positions = Vec::new();
        for i in 0..n {
            let a = i as f64 * std::f64::consts::TAU / n as f64;
            for j in 0..m {
                let b = j as f64 * std::f64::consts::TAU / m as f64;
                positions.push(Vec3::new(
                    (3. + 0.6 * b.cos()) * a.cos(),
                    (3. + 0.6 * b.cos()) * a.sin(),
                    0.6 * b.sin(),
                ));
            }
        }
        let mut triangles = Vec::new();
        for i in 0..n {
            for j in 0..m {
                let a = i * m + j;
                let b = ((i + 1) % n) * m + j;
                let c = ((i + 1) % n) * m + (j + 1) % m;
                let d = i * m + (j + 1) % m;
                triangles.extend([[a, b, c], [a, c, d]]);
            }
        }
        Surface {
            positions,
            triangles,
        }
    }
    #[test]
    fn geometry_hints_follow_non_anatomical_torus_and_are_similarity_invariant() {
        let source = torus();
        let report = estimate(&source, 700, &|| false).unwrap();
        assert!(report.guides.len() > 50);
        for g in &report.guides {
            let p = Vec3::from_array(g.position);
            let tangent = Vec3::new(-p.y, p.x, 0.).normalize();
            assert!(Vec3::from_array(g.tangent).dot(tangent).abs() > 0.99);
        }
        let r = glam::DQuat::from_axis_angle(Vec3::new(0.3, -0.7, 0.5).normalize(), 0.713);
        let mut transformed = source.clone();
        for p in &mut transformed.positions {
            *p = r * *p * 7. + Vec3::new(12., -4., 3.);
        }
        let actual = estimate(&transformed, 700, &|| false).unwrap();
        assert_eq!(report.guides.len(), actual.guides.len());
        for (a, b) in report.guides.iter().zip(&actual.guides) {
            assert_eq!(a.vertex, b.vertex);
            assert!(
                (r * Vec3::from_array(a.tangent))
                    .dot(Vec3::from_array(b.tangent))
                    .abs()
                    > 1. - 1e-10
            );
            assert!((a.radius * 7. - b.radius).abs() < 1e-10);
        }
        assert!(estimate(&source, 700, &|| true).is_err());
    }
    #[test]
    fn geometry_hints_leave_flat_and_isotropic_regions_automatic() {
        let source = torus();
        let (normals, areas, adjacent, _) = topology::geometry(&source);
        let graph: Vec<Vec<_>> = adjacent
            .iter()
            .enumerate()
            .map(|(i, row)| {
                row.iter()
                    .map(|&j| (j, source.positions[i].distance(source.positions[j])))
                    .collect()
            })
            .collect();
        let ns = neighbourhood(&graph, 0, 1., &mut 0, 40_000_000, &|| false).unwrap();
        assert!(neighbourhood(&graph, 0, 1., &mut 0, 0, &|| false).is_err());
        let flat = vec![normals[0]; normals.len()];
        assert!(fit(&source, &flat, &areas, 0, &ns, 1. / 3.).is_none());
        // Exact spherical normals have equal principal curvatures, regardless
        // of the irregular distribution of sample positions in the patch.
        let mut sphere = source.clone();
        for p in &mut sphere.positions {
            *p = p.normalize();
        }
        assert!(fit(&sphere, &sphere.positions, &areas, 0, &ns, 1. / 3.).is_none());
    }
}
