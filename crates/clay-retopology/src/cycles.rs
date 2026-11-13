//! Experimental automatic closed-row selection on a frozen, closed field.
//! Geodesic balls supply candidate cross-sections. Geometry identifies tubular
//! regions; integer transport and pole clearance decide which row to constrain.
//! This audit-only search is not a general anatomical segmenter or a promise
//! that a selected source cycle becomes a regular loop in the final quad mesh.
use crate::{Result, Vec3, check, field::Field, integer::Lattice, topology::Dsu};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

#[derive(Clone, serde::Serialize)]
pub struct Candidate {
    pub vertices: Vec<usize>,
    pub seed: usize,
    pub level_over_spacing: f64,
    pub center: [f64; 3],
    pub axis: [f64; 3],
    pub radius_over_spacing: f64,
    pub perimeter_over_spacing: f64,
    pub normal_alignment: f64,
    pub planarity: f64,
    pub axis_alignment: f64,
    pub rotation: i32,
    pub period: [i32; 2],
    pub along_axis: usize,
    pub clearance_over_spacing: f64,
    pub period_to_projected_length: f64,
    pub eligible: bool,
}
impl Candidate {
    fn rank(&self) -> f64 {
        self.normal_alignment
            * (1. - self.planarity)
            * self.axis_alignment
            * (self.clearance_over_spacing / 2.).min(1.)
            // Penalize both under- and over-allocation relative to geometric
            // circumference. Clamping only at one rewards oversized periods.
            * self.period_to_projected_length.min(1. / self.period_to_projected_length)
    }
}

#[derive(serde::Serialize)]
pub struct Cluster {
    pub members: Vec<usize>,
    pub selected: usize,
}
#[derive(serde::Serialize)]
pub struct Search {
    pub spacing: f64,
    pub seeds: Vec<usize>,
    pub candidates: Vec<Candidate>,
    pub clusters: Vec<Cluster>,
    pub selected: Vec<Candidate>,
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

fn distances(
    graph: &[Vec<(usize, f64)>],
    seed: usize,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<f64>> {
    let mut d = vec![f64::INFINITY; graph.len()];
    d[seed] = 0.;
    let mut heap = BinaryHeap::from([Distance(0., seed)]);
    let mut steps = 0usize;
    while let Some(Distance(value, u)) = heap.pop() {
        if steps.is_multiple_of(4096) {
            check(cancel)?;
        }
        steps += 1;
        if value > d[u] {
            continue;
        }
        for &(v, length) in &graph[u] {
            let next = value + length;
            if next < d[v] {
                d[v] = next;
                heap.push(Distance(next, v));
            }
        }
    }
    Ok(d)
}

struct Context<'a> {
    field: &'a Field,
    lattice: &'a Lattice,
    ids: BTreeMap<[usize; 2], usize>,
    defects: Vec<Vec3>,
    h: f64,
}
impl Context<'_> {
    fn candidate(&self, path: Vec<usize>, seed: usize, level: f64) -> Option<Candidate> {
        let f = self.field;
        let p = &f.surface.positions;
        let pairs: Vec<_> = path
            .iter()
            .copied()
            .zip(path.iter().copied().cycle().skip(1))
            .collect();
        let lengths: Vec<_> = pairs.iter().map(|&(a, b)| p[a].distance(p[b])).collect();
        let perimeter: f64 = lengths.iter().sum();
        if perimeter <= 6. * self.h || perimeter >= 40. * self.h {
            return None;
        }
        let center = pairs
            .iter()
            .zip(&lengths)
            .map(|(&(a, b), &w)| (p[a] + p[b]) * (w * 0.5))
            .sum::<Vec3>()
            / perimeter;
        let axis = pairs
            .iter()
            .map(|&(a, b)| (p[a] - center).cross(p[b] - center))
            .sum::<Vec3>()
            .normalize_or_zero();
        if axis == Vec3::ZERO {
            return None;
        }
        let mut radius = 0.;
        let mut min_radius = f64::INFINITY;
        let mut planarity = 0.;
        let mut normal_alignment = 0.;
        for (&v, &w) in path.iter().zip(&lengths) {
            let offset = p[v] - center;
            let axial = offset.dot(axis);
            let radial = offset - axis * axial;
            let r = radial.length();
            radius += w * r / perimeter;
            min_radius = min_radius.min(r);
            planarity += w * axial * axial / perimeter;
            normal_alignment += w * f.normals[v].dot(radial).abs() / (perimeter * r.max(1e-30));
        }
        planarity /= (radius * radius).max(1e-30);
        if planarity > 0.15 || normal_alignment < 0.75 || min_radius < 0.4 * radius {
            return None;
        }
        let mut rotation = 0i32;
        let mut period = glam::IVec2::ZERO;
        let mut scores = [0.; 2];
        for (&(u, v), &length) in pairs.iter().zip(&lengths) {
            let e = self.lattice.edges[self.ids[&crate::topology::edge(u, v)]];
            let forward = u == e.ends[0];
            let edge_rotation = rotation - if forward { 0 } else { e.transport };
            let delta = p[v] - p[u];
            let tangent = (delta - f.normals[u] * delta.dot(f.normals[u])).normalize_or_zero();
            let basis = [f.directions[u], f.normals[u].cross(f.directions[u])];
            for (a, score) in scores.iter_mut().enumerate() {
                *score += basis[(a as i32 - rotation).rem_euclid(2) as usize]
                    .dot(tangent)
                    .abs()
                    * length;
            }
            let d = match edge_rotation.rem_euclid(4) {
                0 => e.d,
                1 => glam::IVec2::new(-e.d.y, e.d.x),
                2 => -e.d,
                _ => glam::IVec2::new(e.d.y, -e.d.x),
            };
            period += if forward { d } else { -d };
            rotation += if forward { e.transport } else { -e.transport };
        }
        let along = usize::from(scores[1] > scores[0]);
        let clearance = path
            .iter()
            .flat_map(|&v| self.defects.iter().map(move |d| p[v].distance(*d) / self.h))
            .fold(100., f64::min);
        let eligible = rotation.rem_euclid(4) == 0
            && period[along].abs() >= 4
            && period[1 - along].abs() <= period[along].abs()
            && clearance >= 1.;
        Some(Candidate {
            vertices: path,
            seed,
            level_over_spacing: level / self.h,
            center: center.to_array(),
            axis: axis.to_array(),
            radius_over_spacing: radius / self.h,
            perimeter_over_spacing: perimeter / self.h,
            normal_alignment,
            planarity,
            axis_alignment: scores[along] / perimeter,
            rotation: rotation.rem_euclid(4),
            period: period.to_array(),
            along_axis: along,
            clearance_over_spacing: clearance,
            period_to_projected_length: period[along].abs() as f64 * self.h
                / scores[along].max(1e-30),
            eligible,
        })
    }
}

pub(crate) fn select(
    field: &Field,
    lattice: &Lattice,
    cancel: &dyn Fn() -> bool,
) -> Result<Search> {
    check(cancel)?;
    let p = &field.surface.positions;
    if p.is_empty() {
        return Err("Cycle search needs a nonempty field".into());
    }
    let mut spacing = field.spacing.clone();
    spacing.sort_by(f64::total_cmp);
    let h = (spacing[(spacing.len() - 1) / 2] + spacing[spacing.len() / 2]) * 0.5;
    if !h.is_finite() || h <= 0. {
        return Err("Cycle search needs positive finite spacing".into());
    }
    let mut sides = vec![Vec::new(); lattice.edges.len()];
    let mut defects = Vec::new();
    for (fi, face) in field.surface.triangles.iter().enumerate() {
        let mut rotation = 0;
        for k in 0..3 {
            let id = lattice.faces[fi].edges[k];
            sides[id].push((fi, face[k], face[(k + 1) % 3]));
            rotation += lattice.edges[id].transport
                * if face[k] == lattice.edges[id].ends[0] {
                    1
                } else {
                    -1
                };
        }
        if rotation % 4 != 0 {
            defects.push((p[face[0]] + p[face[1]] + p[face[2]]) / 3.);
        }
    }
    if sides.iter().any(|s| s.len() != 2) {
        return Err("Experimental cycle search requires a closed manifold field".into());
    }
    let context = Context {
        field,
        lattice,
        h,
        defects,
        ids: lattice
            .edges
            .iter()
            .enumerate()
            .map(|(i, e)| (e.ends, i))
            .collect(),
    };
    let mut graph = vec![Vec::new(); p.len()];
    for e in &lattice.edges {
        let [a, b] = e.ends;
        let length = p[a].distance(p[b]);
        graph[a].push((b, length));
        graph[b].push((a, length));
    }
    let centroid = p
        .iter()
        .zip(&field.areas)
        .map(|(&v, &a)| v * a)
        .sum::<Vec3>()
        / field.areas.iter().sum::<f64>();
    // Equal distances use the lowest vertex ID, not a world-axis preference.
    let farthest = |d: &[f64]| {
        (0..d.len())
            .max_by(|&a, &b| d[a].total_cmp(&d[b]).then(b.cmp(&a)))
            .unwrap()
    };
    let mut seed = farthest(&p.iter().map(|v| v.distance(centroid)).collect::<Vec<_>>());
    let mut minimum = vec![f64::INFINITY; p.len()];
    let mut seeds = Vec::new();
    let mut candidates = Vec::new();
    let mut known = BTreeSet::new();
    for _ in 0..16 {
        check(cancel)?;
        seeds.push(seed);
        let d = distances(&graph, seed, cancel)?;
        let maximum = d.iter().copied().fold(0., f64::max);
        if !maximum.is_finite() {
            return Err("Experimental cycle search requires one connected component".into());
        }
        if maximum / h > 512. {
            return Err("Experimental cycle search exceeds its section budget".into());
        }
        for (m, &value) in minimum.iter_mut().zip(&d) {
            *m = m.min(value);
        }
        let face_distances: Vec<_> = field
            .surface
            .triangles
            .iter()
            .map(|t| d[t[0]].max(d[t[1]]).max(d[t[2]]))
            .collect();
        // Half-spacing sections avoid skipping the narrow interval between
        // nearby poles on a short tube. Keep the physical search extent fixed;
        // the existing 512-spacing guard bounds this to 1024 section levels.
        for step in 4..(2. * maximum / h).ceil() as usize {
            check(cancel)?;
            let level = step as f64 * 0.5 * h;
            let mut next = vec![None; p.len()];
            let mut ambiguous = vec![false; p.len()];
            let mut starts = Vec::new();
            for side in &sides {
                let inside = [
                    face_distances[side[0].0] < level,
                    face_distances[side[1].0] < level,
                ];
                if inside[0] == inside[1] {
                    continue;
                }
                let (_, a, b) = side[usize::from(!inside[0])];
                if next[a].replace(b).is_some() {
                    ambiguous[a] = true;
                }
                starts.push(a);
            }
            let mut used = BTreeSet::new();
            for start in starts {
                if used.contains(&start) {
                    continue;
                }
                let mut path = Vec::new();
                let mut seen = BTreeSet::new();
                let mut cur = start;
                while !seen.contains(&cur) && !ambiguous[cur] {
                    let Some(v) = next[cur] else {
                        break;
                    };
                    seen.insert(cur);
                    path.push(cur);
                    cur = v;
                }
                used.extend(path.iter().copied());
                if cur != start || path.len() < 3 {
                    continue;
                }
                let mut key = path.clone();
                key.sort_unstable();
                if known.insert(key)
                    && let Some(c) = context.candidate(path, seed, level)
                {
                    candidates.push(c);
                }
            }
        }
        seed = farthest(&minimum);
        if minimum[seed] < 4. * h {
            break;
        }
    }
    // Group *geometric* sections before filtering lattice suitability. A short
    // tube can have several good sections but only one clear of a nearby pole.
    let mut groups = Dsu::new(candidates.len());
    for (i, a) in candidates.iter().enumerate() {
        check(cancel)?;
        for (j, b) in candidates[..i].iter().enumerate() {
            let axis = Vec3::from_array(a.axis);
            let delta = Vec3::from_array(a.center) - Vec3::from_array(b.center);
            let radius = (a.radius_over_spacing + b.radius_over_spacing) * h * 0.5;
            let alignment = axis.dot(Vec3::from_array(b.axis)).abs();
            if (alignment > 0.9
                && (delta - axis * delta.dot(axis)).length() < radius * 0.65
                && delta.dot(axis).abs() < radius * 2.5)
                || (alignment > 0.5 && delta.length() < radius * 1.2)
            {
                groups.join(i, j);
            }
        }
    }
    let mut members = BTreeMap::<usize, Vec<usize>>::new();
    for i in 0..candidates.len() {
        members.entry(groups.find(i)).or_default().push(i);
    }
    let mut clusters = Vec::new();
    let mut selected = Vec::new();
    for ids in members.into_values().filter(|ids| ids.len() >= 3) {
        let best = ids
            .iter()
            .copied()
            .filter(|&i| candidates[i].eligible)
            .max_by(|&a, &b| {
                candidates[a].rank().total_cmp(&candidates[b].rank()).then(
                    candidates[b]
                        .vertices
                        .iter()
                        .min()
                        .cmp(&candidates[a].vertices.iter().min()),
                )
            });
        if let Some(best) = best {
            selected.push(candidates[best].clone());
            clusters.push(Cluster {
                members: ids,
                selected: best,
            });
        }
    }
    selected.sort_by_key(|c| *c.vertices.iter().min().unwrap());
    Ok(Search {
        spacing: h,
        seeds,
        candidates,
        clusters,
        selected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tube() -> Field {
        let (rings, count) = (12, 24);
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut directions = Vec::new();
        for ring in 0..rings {
            for i in 0..count {
                let angle = i as f64 * std::f64::consts::TAU / count as f64;
                let n = Vec3::new(angle.cos(), angle.sin(), 0.);
                let radius = (1. + 0.07 * angle.sin() + 0.03 * (3. * angle).cos())
                    * (1. - 0.01 * ring as f64);
                positions.push(n * radius + Vec3::Z * (ring as f64 * 0.53));
                normals.push(n);
                directions.push(Vec3::Z);
            }
        }
        let mut triangles = Vec::new();
        for ring in 0..rings - 1 {
            for i in 0..count {
                let a = ring * count + i;
                let b = ring * count + (i + 1) % count;
                triangles.extend([[a, b, b + count], [a, b + count, a + count]]);
            }
        }
        for end in [0, rings - 1] {
            let center = positions.len();
            positions.push(Vec3::Z * (end as f64 * 0.53));
            normals.push(if end == 0 { -Vec3::Z } else { Vec3::Z });
            directions.push(Vec3::X);
            for i in 0..count {
                let a = end * count + i;
                let b = end * count + (i + 1) % count;
                triangles.push(if end == 0 {
                    [center, b, a]
                } else {
                    [center, a, b]
                });
            }
        }
        let len = positions.len();
        Field {
            origins: positions.clone(),
            surface: crate::Surface {
                positions,
                triangles,
            },
            normals,
            directions,
            areas: vec![1.; len],
            spacing: vec![0.35; len],
            constraints: vec![crate::field::Constraint::default(); len],
        }
    }

    #[test]
    fn automatic_cycles_find_tube_and_preserve_selection_under_similarity() {
        let mut f = tube();
        let lattice = Lattice::new(&f, &|| false).unwrap();
        let original = select(&f, &lattice, &|| false).unwrap();
        assert!(!original.selected.is_empty());
        // Sections may be oblique. Require a full circuit around the tube,
        // rather than a closed disk-shaped patch on one side of its surface.
        for c in &original.selected {
            let winding: f64 = c
                .vertices
                .iter()
                .zip(c.vertices.iter().cycle().skip(1))
                .map(|(&a, &b)| {
                    let a = f.surface.positions[a];
                    let b = f.surface.positions[b];
                    (a.x * b.y - a.y * b.x).atan2(a.x * b.x + a.y * b.y)
                })
                .sum();
            assert!((winding.abs() - std::f64::consts::TAU).abs() < 1e-10);
        }
        assert!(
            original
                .selected
                .iter()
                .all(|c| c.eligible && c.rotation == 0)
        );
        let rotation = glam::DQuat::from_axis_angle(Vec3::new(0.3, -0.7, 0.5).normalize(), 0.713);
        for p in &mut f.surface.positions {
            *p = rotation * *p * 7. + Vec3::new(12., -4., 3.);
        }
        for n in &mut f.normals {
            *n = rotation * *n;
        }
        for q in &mut f.directions {
            *q = rotation * *q;
        }
        for a in &mut f.areas {
            *a *= 49.;
        }
        for h in &mut f.spacing {
            *h *= 7.;
        }
        let transformed = select(&f, &lattice, &|| false).unwrap();
        let paths = |report: &Search| {
            let mut result: Vec<_> = report
                .selected
                .iter()
                .map(|c| {
                    let mut ids = c.vertices.clone();
                    ids.sort_unstable();
                    ids
                })
                .collect();
            result.sort();
            result
        };
        assert_eq!(original.seeds, transformed.seeds);
        assert_eq!(paths(&original), paths(&transformed));
    }

    #[test]
    fn automatic_cycles_reject_disk_sections_open_fields_and_cancellation() {
        let mut f = tube();
        let lattice = Lattice::new(&f, &|| false).unwrap();
        assert!(select(&f, &lattice, &|| true).is_err());
        // A planar disk has a closed contour too, but its normals point through
        // the contour plane rather than radially out of a tube.
        let ids = lattice
            .edges
            .iter()
            .enumerate()
            .map(|(i, e)| (e.ends, i))
            .collect();
        let path: Vec<_> = (24 * 5..24 * 6).collect();
        let context = Context {
            field: &f,
            lattice: &lattice,
            ids,
            defects: Vec::new(),
            h: 0.35,
        };
        assert!(context.candidate(path.clone(), 0, 1.).is_some());
        let Context { ids, .. } = context;
        for &v in &path {
            f.normals[v] = Vec3::Z;
        }
        let context = Context {
            field: &f,
            lattice: &lattice,
            ids,
            defects: Vec::new(),
            h: 0.35,
        };
        assert!(context.candidate(path, 0, 1.).is_none());
        f.surface.triangles.pop();
        let open = Lattice::new(&f, &|| false).unwrap();
        assert!(
            select(&f, &open, &|| false)
                .err()
                .unwrap()
                .contains("closed manifold")
        );
    }
}
