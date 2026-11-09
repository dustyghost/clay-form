use crate::symmetry::{Extrinsic, Frame, RoSy4};
mod reference;
pub(crate) use reference::midpoint as reference_midpoint;
mod adjacency;
use crate::{Options, Result, Surface, Vec3, check, topology};
pub(crate) use adjacency::ordered_neighbours;
use std::collections::{BTreeMap, HashMap};

#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default)]
pub(crate) struct Constraint {
    #[cfg_attr(feature = "audit", serde(with = "crate::audit::vector"))]
    pub tangent: Vec3,
    #[cfg_attr(feature = "audit", serde(with = "crate::audit::vector"))]
    pub origin: Vec3,
    pub weight: f64,
}
#[cfg_attr(feature = "audit", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct Field {
    pub surface: Surface,
    #[cfg_attr(feature = "audit", serde(with = "crate::audit::vectors"))]
    pub normals: Vec<Vec3>,
    #[cfg_attr(feature = "audit", serde(with = "crate::audit::vectors"))]
    pub directions: Vec<Vec3>,
    #[cfg_attr(feature = "audit", serde(with = "crate::audit::vectors"))]
    pub origins: Vec<Vec3>,
    pub spacing: Vec<f64>,
    pub areas: Vec<f64>,
    pub constraints: Vec<Constraint>,
}

pub(crate) fn solve(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(crate::Stage),
) -> Result<Field> {
    solve_impl(source, options, false, false, cancel, progress)
}

/// Let guides steer orientation while keeping hard boundary and crease anchors.
pub(crate) fn solve_soft_directions(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(crate::Stage),
) -> Result<Field> {
    solve_impl(source, options, true, false, cancel, progress)
}

/// Controlled offline experiment: guide the orientation solve, then discard
/// guide position constraints. Hard boundary/feature constraints are excluded
/// so coarsening cannot mix them with the soft constraints being discarded.
#[cfg(feature = "audit")]
pub(crate) fn solve_direction_guides(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(crate::Stage),
) -> Result<Field> {
    solve_impl(source, options, true, true, cancel, progress)
}

fn solve_impl(
    source: &Surface,
    options: &Options,
    direction_only: bool,
    audit_without_hard_constraints: bool,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(crate::Stage),
) -> Result<Field> {
    progress(crate::Stage::Sampling);
    let (_, _, _, area) = topology::geometry(source);
    let base = (area / options.target_quads as f64).sqrt();
    let (surface, detail) = refine(source, &options.detail, base, options.max_vertices, cancel)?;
    progress(crate::Stage::Hierarchy {
        samples: surface.positions.len(),
    });
    let (normals, areas, neighbours, _) = topology::geometry(&surface);
    let spacing = allocate_spacing(&normals, &areas, &neighbours, &detail, options);
    let constraints = constraints(&surface, &normals, options, cancel)?;
    if audit_without_hard_constraints && constraints.iter().any(|c| c.weight >= 1.) {
        return Err("Direction-only audit excludes hard boundary/sharp constraints".into());
    }
    let mut field = Field {
        surface,
        normals,
        areas,
        spacing,
        constraints,
        directions: Vec::new(),
        origins: Vec::new(),
    };
    reference::solve(&mut field, direction_only, cancel, progress)?;
    Ok(field)
}

fn allocate_spacing(
    normals: &[Vec3],
    areas: &[f64],
    neighbours: &[Vec<usize>],
    detail: &[f64],
    options: &Options,
) -> Vec<f64> {
    let mut density = vec![1.; normals.len()];
    for i in 0..normals.len() {
        let curvature = neighbours[i]
            .iter()
            .map(|&j| 1. - normals[i].dot(normals[j]))
            .fold(0., f64::max);
        density[i] = 4f64.powf(detail[i]) * (1. + 8. * options.adaptive * curvature);
    }
    let weighted: f64 = areas.iter().zip(&density).map(|(a, d)| a * d).sum();
    density
        .iter()
        .map(|d| (weighted / options.target_quads as f64 / d).sqrt())
        .collect()
}

/// Validation spacing indexed by the immutable source, never by the reordered
/// working mesh. Reuse the same refinement and density allocation as the field.
pub(crate) fn source_spacing(
    source: &Surface,
    options: &Options,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<f64>> {
    let (_, _, _, area) = topology::geometry(source);
    let base = (area / options.target_quads as f64).sqrt();
    let (sampled, detail) = refine(source, &options.detail, base, options.max_vertices, cancel)?;
    let (normals, areas, neighbours, _) = topology::geometry(&sampled);
    let mut spacing = allocate_spacing(&normals, &areas, &neighbours, &detail, options);
    spacing.truncate(source.positions.len());
    Ok(spacing)
}

#[cfg(feature = "audit")]
pub(crate) fn resolve_fields(field: &mut Field, progress: &dyn Fn(crate::Stage)) -> Result<()> {
    reference::solve(field, false, &|| false, progress)
}

fn random_tangent(n: Vec3, i: usize) -> Vec3 {
    let axis = if n.x.abs() < 0.8 { Vec3::X } else { Vec3::Y };
    let a = n.cross(axis).normalize_or_zero();
    let b = n.cross(a);
    let angle = ((i as u64).wrapping_mul(0x9e3779b97f4a7c15) >> 12) as f64 * 1e-10;
    a * angle.cos() + b * angle.sin()
}
fn tangent(q: Vec3, n: Vec3) -> Vec3 {
    let q = q - n * n.dot(q);
    if q.length_squared() > 1e-20 {
        q.normalize()
    } else {
        random_tangent(n, 0)
    }
}
pub(crate) fn rotate(q: Vec3, n: Vec3, k: i32) -> Vec3 {
    match k.rem_euclid(4) {
        0 => q,
        1 => n.cross(q),
        2 => -q,
        _ => -n.cross(q),
    }
}

fn constraints(
    s: &Surface,
    normals: &[Vec3],
    o: &Options,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<Constraint>> {
    let mut out = vec![Constraint::default(); s.positions.len()];
    let mut edges = BTreeMap::<[usize; 2], Vec<Vec3>>::new();
    for &[a, b, c] in &s.triangles {
        let n = (s.positions[b] - s.positions[a])
            .cross(s.positions[c] - s.positions[a])
            .normalize();
        for (u, v) in [(a, b), (b, c), (c, a)] {
            edges.entry(topology::edge(u, v)).or_default().push(n);
        }
    }
    let sharp_threshold = o.sharp_threshold();
    for ([a, b], ns) in edges {
        if ns.len() == 1 || sharp_threshold.is_some_and(|threshold| ns[0].dot(ns[1]) <= threshold) {
            for (i, j) in [(a, b), (b, a)] {
                out[i] = Constraint {
                    tangent: tangent(s.positions[j] - s.positions[i], normals[i]),
                    origin: s.positions[i],
                    weight: 1.,
                };
            }
        }
    }
    let radius = o.guides.iter().map(|g| g.radius).fold(0., f64::max);
    if radius > 0. {
        let key = |p: Vec3| {
            [
                (p.x / radius).floor() as i64,
                (p.y / radius).floor() as i64,
                (p.z / radius).floor() as i64,
            ]
        };
        let mut bins = HashMap::<[i64; 3], Vec<usize>>::new();
        for (i, g) in o.guides.iter().enumerate() {
            bins.entry(key(g.position)).or_default().push(i);
        }
        for (i, &p) in s.positions.iter().enumerate() {
            if i % 1024 == 0 {
                check(cancel)?;
            }
            if out[i].weight == 1. {
                continue;
            }
            let k = key(p);
            let mut sum = Vec3::ZERO;
            let mut center = Vec3::ZERO;
            let mut weight = 0.;
            for x in -1..=1 {
                for y in -1..=1 {
                    for z in -1..=1 {
                        if let Some(ids) = bins.get(&[k[0] + x, k[1] + y, k[2] + z]) {
                            for &j in ids {
                                let g = o.guides[j];
                                let d = (g.position - p).length() / g.radius;
                                if d >= 1. || normals[i].dot(g.normal.normalize()) < 0.5 {
                                    continue;
                                }
                                let w = g.strength * (1. - d).powi(2);
                                let q = tangent(g.tangent, normals[i]);
                                let q = if weight > 0. {
                                    let m = Extrinsic::match_orientation(
                                        Frame::new(sum.normalize(), normals[i]),
                                        Frame::new(q, normals[i]),
                                    );
                                    rotate(m.rhs.0, normals[i], -m.lhs.1)
                                } else {
                                    q
                                };
                                sum += q * w;
                                center += g.position * w;
                                weight += w;
                            }
                        }
                    }
                }
            }
            if weight > 0. {
                out[i] = Constraint {
                    tangent: tangent(sum, normals[i]),
                    origin: center / weight,
                    weight: weight.min(0.95),
                };
            }
        }
    }
    Ok(out)
}

fn refine(
    source: &Surface,
    detail: &[f64],
    scale: f64,
    max_vertices: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<(Surface, Vec<f64>)> {
    let mut mesh = source.clone();
    let mut detail = if detail.is_empty() {
        vec![0.; mesh.positions.len()]
    } else {
        detail.to_vec()
    };
    for _ in 0..12 {
        check(cancel)?;
        let mut mid = BTreeMap::new();
        for t in &mesh.triangles {
            for j in 0..3 {
                let [a, b] = topology::edge(t[j], t[(j + 1) % 3]);
                let local = scale * 2f64.powf(-(detail[a] + detail[b]) * 0.5) * 0.7;
                if (mesh.positions[a] - mesh.positions[b]).length() > local {
                    mid.entry([a, b]).or_insert(0usize);
                }
            }
        }
        if mid.is_empty() {
            return Ok((mesh, detail));
        }
        if mesh.positions.len() + mid.len() > max_vertices {
            return Err("Retopology sampling exceeds the configured vertex budget".into());
        }
        for (&[a, b], v) in &mut mid {
            *v = mesh.positions.len();
            mesh.positions
                .push((mesh.positions[a] + mesh.positions[b]) * 0.5);
            detail.push((detail[a] + detail[b]) * 0.5);
        }
        let mut faces = Vec::with_capacity(mesh.triangles.len() * 2);
        for &[a, b, c] in &mesh.triangles {
            let ms = [
                mid.get(&topology::edge(a, b)).copied(),
                mid.get(&topology::edge(b, c)).copied(),
                mid.get(&topology::edge(c, a)).copied(),
            ];
            match ms {
                [None, None, None] => faces.push([a, b, c]),
                [Some(x), Some(y), Some(z)] => {
                    faces.extend([[a, x, z], [x, b, y], [z, y, c], [x, y, z]])
                }
                _ => {
                    let vs = [a, b, c];
                    let count = ms.iter().filter(|x| x.is_some()).count();
                    let k = if count == 1 {
                        ms.iter().position(Option::is_some).unwrap()
                    } else {
                        ms.iter().position(Option::is_none).unwrap()
                    };
                    let [u, v, w] = [vs[k], vs[(k + 1) % 3], vs[(k + 2) % 3]];
                    if count == 1 {
                        let x = ms[k].unwrap();
                        faces.extend([[u, x, w], [x, v, w]]);
                    } else {
                        let y = ms[(k + 1) % 3].unwrap();
                        let z = ms[(k + 2) % 3].unwrap();
                        faces.extend([[w, z, y], [u, v, z], [v, y, z]]);
                    }
                }
            }
        }
        mesh.triangles = faces;
    }
    Err("Retopology could not resolve source edge lengths within its work budget".into())
}
