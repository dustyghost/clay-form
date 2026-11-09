// Hierarchy and field optimization adapted from QuadriFlow (BSD-3-Clause).
// Copyright (c) 2018, Jingwei Huang, Yichao Zhou, Matthias Niessner, Jonathan
// Shewchuk, Leonidas Guibas. Full terms: ../../LICENSE-QuadriFlow.
// Reference: 810b7a0967c35b0dc85b4464e3835e26a756c967, hierarchy.cpp,
// optimizer.cpp and field-math.hpp. No C++ or libc runtime calls.
use super::{Constraint, Field};
use crate::{Result, Stage, Vec3, check};

const EPS: f64 = 2.938735877055719e-39;

#[derive(Clone, Copy)]
struct Link {
    id: usize,
    weight: f64,
}

struct Level {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    areas: Vec<f64>,
    adjacency: Vec<Vec<Link>>,
    children: Vec<[usize; 2]>,
    phases: Vec<Vec<usize>>,
    spacing: Vec<f64>,
    constraints: Vec<Constraint>,
    q: Vec<Vec3>,
    o: Vec<Vec3>,
}

/// Local, portable reproduction of the Linux reference's srand(0)/rand()
/// sequence: a degree-31 additive recurrence, lag 3, Park-Miller initialization.
/// Defining the sequence here avoids platform-dependent libc and global state.
struct InitialRandom {
    state: [u32; 31],
    cursor: usize,
}
impl InitialRandom {
    fn new() -> Self {
        let mut state = [1; 31];
        for i in 1..31 {
            state[i] = (u64::from(state[i - 1]) * 16807 % 2147483647) as u32;
        }
        let mut rng = Self { state, cursor: 3 };
        for _ in 0..310 {
            rng.next();
        }
        rng
    }
    fn next(&mut self) -> u32 {
        let i = self.cursor;
        self.state[i] = self.state[i].wrapping_add(self.state[(i + 28) % 31]);
        self.cursor = (i + 1) % 31;
        self.state[i] >> 1
    }
    fn unit(&mut self) -> f64 {
        f64::from(self.next()) / 2147483647.
    }
}

fn normalized(v: Vec3) -> Vec3 {
    let len = v.length();
    if len > EPS { v / len } else { v }
}

fn axes(n: Vec3) -> (Vec3, Vec3) {
    let t = if n.x.abs() > n.y.abs() {
        let r = 1. / (n.x * n.x + n.z * n.z).sqrt();
        Vec3::new(n.z * r, 0., -n.x * r)
    } else {
        let r = 1. / (n.y * n.y + n.z * n.z).sqrt();
        Vec3::new(0., n.z * r, -n.y * r)
    };
    (t.cross(n), t)
}

fn compatible(q0: Vec3, n0: Vec3, q1: Vec3, n1: Vec3) -> (Vec3, Vec3) {
    let a = [q0, n0.cross(q0)];
    let b = [q1, n1.cross(q1)];
    let mut best = (0, 0, f64::NEG_INFINITY);
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            let score = x.dot(*y).abs();
            // The vector-valued reference matcher deliberately has a tie margin;
            // its integer-index matcher does not. Do not replace one with the other.
            if score > best.2 + 1e-6 {
                best = (i, j, score);
            }
        }
    }
    let dot = a[best.0].dot(b[best.1]);
    (a[best.0], b[best.1] * dot.signum())
}

pub(crate) fn midpoint(p0: Vec3, n0: Vec3, p1: Vec3, n1: Vec3) -> Vec3 {
    let n0p0 = n0.dot(p0);
    let n0p1 = n0.dot(p1);
    let n1p0 = n1.dot(p0);
    let n1p1 = n1.dot(p1);
    let dot = n0.dot(n1);
    // Upstream's literal is single precision, then promoted to double.
    let denom = 1. / (1. - dot * dot + f64::from(1e-4_f32));
    let l0 = 2. * (n0p1 - n0p0 - dot * (n1p0 - n1p1)) * denom;
    let l1 = 2. * (n1p0 - n1p1 - dot * (n0p1 - n0p0)) * denom;
    (p0 + p1) * 0.5 - (n0 * l0 + n1 * l1) * 0.25
}

fn floor_origin(o: Vec3, q: Vec3, n: Vec3, p: Vec3, h: f64) -> Vec3 {
    let t = n.cross(q);
    let d = p - o;
    let inv = 1. / h;
    o + q * (q.dot(d) * inv).floor() * h + t * (t.dot(d) * inv).floor() * h
}
fn round_origin(o: Vec3, q: Vec3, n: Vec3, p: Vec3, h: f64) -> Vec3 {
    let t = n.cross(q);
    let d = p - o;
    let inv = 1. / h;
    o + q * (q.dot(d) * inv).round() * h + t * (t.dot(d) * inv).round() * h
}

fn compatible_positions(
    a: (Vec3, Vec3, Vec3, Vec3, f64),
    b: (Vec3, Vec3, Vec3, Vec3, f64),
) -> (Vec3, Vec3) {
    let (p0, n0, q0, o0, h0) = a;
    let (p1, n1, q1, o1, h1) = b;
    let middle = midpoint(p0, n0, p1, n1);
    let a = floor_origin(o0, q0, n0, middle, h0);
    let b = floor_origin(o1, q1, n1, middle, h1);
    let t0 = n0.cross(q0);
    let t1 = n1.cross(q1);
    let mut best = (a, b, f64::INFINITY);
    for i in 0..4 {
        let x = a + (q0 * f64::from(i & 1) * h0 + t0 * f64::from((i & 2) >> 1) * h0);
        for j in 0..4 {
            let y = b + (q1 * f64::from(j & 1) * h1 + t1 * f64::from((j & 2) >> 1) * h1);
            let cost = (x - y).length_squared();
            if cost < best.2 {
                best = (x, y, cost);
            }
        }
    }
    (best.0, best.1)
}

fn coloring(adjacency: &[Vec<Link>], cancel: &dyn Fn() -> bool) -> Result<Vec<Vec<usize>>> {
    // The pinned pcg32::shuffle immediately returns for begin <= end. Thus
    // the sequential reference visits vertices in their existing order.
    let mut colors = vec![usize::MAX; adjacency.len()];
    let mut phases: Vec<Vec<usize>> = vec![];
    let mut available = Vec::new();
    for (i, row) in adjacency.iter().enumerate() {
        if i % 4096 == 0 {
            check(cancel)?;
        }
        available.resize(phases.len(), true);
        available.fill(true);
        for link in row {
            if colors[link.id] != usize::MAX {
                available[colors[link.id]] = false;
            }
        }
        let color = available.iter().position(|&ok| ok).unwrap_or(phases.len());
        if color == phases.len() {
            phases.push(vec![]);
        }
        colors[i] = color;
        phases[color].push(i);
    }
    Ok(phases)
}

fn coarsen(fine: &Level, cancel: &dyn Fn() -> bool) -> Result<Level> {
    let n = fine.positions.len();
    let mut entries = Vec::new();
    for (i, row) in fine.adjacency.iter().enumerate() {
        if i % 4096 == 0 {
            check(cancel)?;
        }
        for link in row {
            let j = link.id;
            let (a, b) = (fine.areas[i], fine.areas[j]);
            let ratio = if a > b { a / b } else { b / a };
            entries.push((fine.normals[i].dot(fine.normals[j]) * ratio, i, j));
        }
    }
    // Keep both directed links and stable ties, exactly as DownsampleGraph.
    entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    check(cancel)?;
    let mut parent = vec![usize::MAX; n];
    let mut children = Vec::new();
    for (_, i, j) in entries {
        if parent[i] == usize::MAX && parent[j] == usize::MAX {
            parent[i] = children.len();
            parent[j] = children.len();
            children.push([i, j]);
        }
    }
    for (i, p) in parent.iter_mut().enumerate() {
        if *p == usize::MAX {
            *p = children.len();
            children.push([i, usize::MAX]);
        }
    }
    let mut level = Level {
        positions: vec![],
        normals: vec![],
        areas: vec![],
        adjacency: vec![],
        children,
        phases: vec![],
        spacing: vec![],
        constraints: vec![],
        q: vec![],
        o: vec![],
    };
    for (id, &[a, b]) in level.children.iter().enumerate() {
        if id % 4096 == 0 {
            check(cancel)?;
        }
        if b == usize::MAX {
            level.positions.push(fine.positions[a]);
            level.normals.push(fine.normals[a]);
            level.areas.push(fine.areas[a]);
            level.spacing.push(fine.spacing[a]);
            level.constraints.push(fine.constraints[a]);
        } else {
            let (wa, wb) = (fine.areas[a], fine.areas[b]);
            let area = wa + wb;
            let p = if area > EPS {
                (fine.positions[a] * wa + fine.positions[b] * wb) / area
            } else {
                (fine.positions[a] + fine.positions[b]) * 0.5
            };
            let sum = fine.normals[a] * wa + fine.normals[b] * wb;
            let normal = if sum.length() > EPS {
                sum / sum.length()
            } else {
                Vec3::X
            };
            level.positions.push(p);
            level.normals.push(normal);
            level.areas.push(area);
            // Exactly constant h stays constant; no repeated area averaging drift.
            level.spacing.push(if fine.spacing[a] == fine.spacing[b] {
                fine.spacing[a]
            } else {
                (fine.spacing[a] * wa + fine.spacing[b] * wb) / area
            });
            level
                .constraints
                .push(merge_constraint(fine, a, b, normal, area));
        }
        let mut links = Vec::new();
        for v in [a, b].into_iter().filter(|&v| v != usize::MAX) {
            for link in &fine.adjacency[v] {
                if parent[link.id] != id {
                    links.push(Link {
                        id: parent[link.id],
                        weight: link.weight,
                    });
                }
            }
        }
        links.sort_by_key(|link| link.id);
        let mut combined: Vec<Link> = Vec::new();
        for link in links {
            if let Some(last) = combined.last_mut().filter(|last| last.id == link.id) {
                last.weight += link.weight;
            } else {
                combined.push(link);
            }
        }
        level.adjacency.push(combined);
    }
    level.phases = coloring(&level.adjacency, cancel)?;
    Ok(level)
}

// Preserve Clay's soft guide weighting when aggregating. Unlike upstream's
// hard-boundary propagator, a weak guide must not become weight one at depth.
// This extension is separate from the uniform unconstrained parity target.
fn merge_constraint(fine: &Level, a: usize, b: usize, n: Vec3, area: f64) -> Constraint {
    let (ca, cb) = (fine.constraints[a], fine.constraints[b]);
    if ca.weight == 1. || cb.weight == 1. {
        // Preserve hard boundary/crease constraints throughout the hierarchy.
        // Use actual normals and project the origin itself: upstream's
        // propagateConstraints has CQ-for-N and CQ-for-CO transcription errors.
        let (q, mut o) = if ca.weight == 1. && cb.weight == 1. {
            let (qa, qb) = compatible(ca.tangent, fine.normals[a], cb.tangent, fine.normals[b]);
            let (oa, ob) = compatible_positions(
                (
                    fine.positions[a],
                    fine.normals[a],
                    ca.tangent,
                    ca.origin,
                    fine.spacing[a],
                ),
                (
                    fine.positions[b],
                    fine.normals[b],
                    cb.tangent,
                    cb.origin,
                    fine.spacing[b],
                ),
            );
            (qa + qb, (oa + ob) * 0.5)
        } else if ca.weight == 1. {
            (ca.tangent, ca.origin)
        } else {
            (cb.tangent, cb.origin)
        };
        let p = (fine.positions[a] * fine.areas[a] + fine.positions[b] * fine.areas[b]) / area;
        o -= n * n.dot(o - p);
        return Constraint {
            tangent: normalized(q - n * n.dot(q)),
            origin: o,
            weight: 1.,
        };
    }
    let (wa, wb) = (fine.areas[a] * ca.weight, fine.areas[b] * cb.weight);
    if wa + wb == 0. {
        return Constraint::default();
    }
    let (qa, qb) = if wa > 0. && wb > 0. {
        compatible(ca.tangent, fine.normals[a], cb.tangent, fine.normals[b])
    } else {
        (ca.tangent, cb.tangent)
    };
    let q = qa * wa + qb * wb;
    Constraint {
        tangent: normalized(q - n * n.dot(q)),
        origin: (ca.origin * wa + cb.origin * wb) / (wa + wb),
        weight: ((wa + wb) / area).min(1.),
    }
}

fn initialize(levels: &mut [Level]) {
    let mut rng = InitialRandom::new();
    for l in levels {
        for i in 0..l.positions.len() {
            let (s, t) = axes(l.normals[i]);
            let angle = rng.unit() * 2. * std::f64::consts::PI;
            let x = rng.unit() * 2. - 1.;
            let y = rng.unit() * 2. - 1.;
            l.q.push(s * angle.cos() + t * angle.sin());
            l.o.push(l.positions[i] + (s * x + t * y) * l.spacing[i]);
        }
    }
}

fn orientations(levels: &mut [Level], cancel: &dyn Fn() -> bool) -> Result<()> {
    for k in (0..levels.len()).rev() {
        let l = &mut levels[k];
        for _ in 0..6 {
            check(cancel)?;
            for phase in &l.phases {
                for &i in phase {
                    if i % 2048 == 0 {
                        check(cancel)?;
                    }
                    let n = l.normals[i];
                    let mut sum = l.q[i];
                    let mut weights = 0.;
                    for link in &l.adjacency[i] {
                        let (a, b) = compatible(sum, n, l.q[link.id], l.normals[link.id]);
                        sum = a * weights + b * link.weight;
                        sum -= n * n.dot(sum);
                        weights += link.weight;
                        sum = normalized(sum);
                    }
                    let c = l.constraints[i];
                    if c.weight > 0. {
                        let (a, b) = compatible(sum, n, c.tangent, n);
                        sum = a * (1. - c.weight) + b * c.weight;
                        sum = normalized(sum - n * n.dot(sum));
                    }
                    if weights > 0. {
                        l.q[i] = sum;
                    }
                }
            }
        }
        if k > 0 {
            let (low, high) = levels.split_at_mut(k);
            let fine = &mut low[k - 1];
            let coarse = &high[0];
            for (i, children) in coarse.children.iter().enumerate() {
                for &v in children {
                    if v != usize::MAX {
                        let n = fine.normals[v];
                        let q = coarse.q[i];
                        fine.q[v] = q - n * n.dot(q);
                    }
                }
            }
        }
    }
    for k in 1..levels.len() {
        check(cancel)?;
        let (low, high) = levels.split_at_mut(k);
        let fine = &low[k - 1];
        let coarse = &mut high[0];
        for (i, &[a, b]) in coarse.children.iter().enumerate() {
            let mut q = if b == usize::MAX {
                fine.q[a]
            } else {
                let (a, b) = compatible(fine.q[a], fine.normals[a], fine.q[b], fine.normals[b]);
                a + b
            };
            let n = coarse.normals[i];
            q -= n * n.dot(q);
            coarse.q[i] = if q.length_squared() > EPS {
                q / q.length()
            } else {
                q
            };
        }
    }
    Ok(())
}

fn positions(levels: &mut [Level], cancel: &dyn Fn() -> bool) -> Result<()> {
    for k in (0..levels.len()).rev() {
        let l = &mut levels[k];
        for _ in 0..6 {
            check(cancel)?;
            for phase in &l.phases {
                for &i in phase {
                    if i % 1024 == 0 {
                        check(cancel)?;
                    }
                    let (n, p) = (l.normals[i], l.positions[i]);
                    let q = normalized(l.q[i]);
                    let mut sum = l.o[i];
                    let mut weights = 0.;
                    for link in &l.adjacency[i] {
                        let j = link.id;
                        let (a, b) = compatible_positions(
                            (p, n, q, sum, l.spacing[i]),
                            (
                                l.positions[j],
                                l.normals[j],
                                normalized(l.q[j]),
                                l.o[j],
                                l.spacing[j],
                            ),
                        );
                        sum = a * weights + b * link.weight;
                        weights += link.weight;
                        if weights > EPS {
                            sum /= weights;
                        }
                        sum -= n * (sum - p).dot(n);
                    }
                    let c = l.constraints[i];
                    if c.weight > 0. {
                        let d = c.origin - sum;
                        sum += (d - c.tangent * c.tangent.dot(d)) * c.weight;
                        sum -= n * (sum - p).dot(n);
                    }
                    if weights > 0. {
                        l.o[i] = round_origin(sum, q, n, p, l.spacing[i]);
                    }
                }
            }
        }
        if k > 0 {
            let (low, high) = levels.split_at_mut(k);
            let fine = &mut low[k - 1];
            let coarse = &high[0];
            for (i, children) in coarse.children.iter().enumerate() {
                for &v in children {
                    if v != usize::MAX {
                        let n = fine.normals[v];
                        let o = coarse.o[i];
                        fine.o[v] = o - n * n.dot(o - fine.positions[v]);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn solve(
    field: &mut Field,
    direction_only: bool,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(Stage),
) -> Result<()> {
    let neighbours = super::ordered_neighbours(&field.surface, cancel)?;
    let adjacency: Vec<_> = neighbours
        .into_iter()
        .map(|row| row.into_iter().map(|id| Link { id, weight: 1. }).collect())
        .collect();
    let phases = coloring(&adjacency, cancel)?;
    let mut levels = vec![Level {
        positions: field.surface.positions.clone(),
        normals: field.normals.clone(),
        areas: field.areas.clone(),
        adjacency,
        children: vec![],
        phases,
        spacing: field.spacing.clone(),
        constraints: field.constraints.clone(),
        q: vec![],
        o: vec![],
    }];
    let mut samples = field.surface.positions.len();
    for _ in 0..25 {
        check(cancel)?;
        let coarse = coarsen(levels.last().unwrap(), cancel)?;
        samples += coarse.positions.len();
        if samples > field.surface.positions.len().saturating_mul(4) {
            return Err("Field hierarchy exceeds the aggregate sample budget".into());
        }
        let done = coarse.positions.len() == 1;
        progress(Stage::Hierarchy {
            samples: coarse.positions.len(),
        });
        levels.push(coarse);
        if done {
            break;
        }
    }
    initialize(&mut levels);
    progress(Stage::Orientation);
    orientations(&mut levels, cancel)?;
    if direction_only {
        for l in &mut levels {
            for constraint in &mut l.constraints {
                if constraint.weight < 1. {
                    *constraint = Constraint::default();
                }
            }
        }
    }
    progress(Stage::Position);
    positions(&mut levels, cancel)?;
    let fine = levels.remove(0);
    if fine.q.iter().chain(&fine.o).any(|v| !v.is_finite()) {
        return Err("Field optimization produced a nonfinite result".into());
    }
    field.directions = fine.q;
    field.origins = fine.o;
    field.constraints = fine.constraints;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fields_match_pinned_cpp_on_asymmetric_surface() {
        let mut input = include_str!("../../tests/data/field-asymmetric.input").split_whitespace();
        assert_eq!(input.next(), Some("CLAYFIELD1"));
        let fingerprint = input.next().unwrap();
        let n: usize = input.next().unwrap().parse().unwrap();
        let nf: usize = input.next().unwrap().parse().unwrap();
        let spacing: f64 = input.next().unwrap().parse().unwrap();
        let mut scalar = || input.next().unwrap().parse::<f64>().unwrap();
        let mut points = Vec::new();
        let mut normals = Vec::new();
        let mut areas = Vec::new();
        for _ in 0..n {
            points.push(Vec3::new(scalar(), scalar(), scalar()));
            normals.push(Vec3::new(scalar(), scalar(), scalar()));
            areas.push(scalar());
        }
        let triangles = (0..nf)
            .map(|_| std::array::from_fn(|_| input.next().unwrap().parse().unwrap()))
            .collect();
        let mut field = Field {
            surface: crate::Surface {
                positions: points,
                triangles,
            },
            normals,
            areas,
            spacing: vec![spacing; n],
            constraints: vec![Constraint::default(); n],
            directions: vec![],
            origins: vec![],
        };
        solve(&mut field, false, &|| false, &|_| {}).unwrap();
        let mut output =
            include_str!("../../tests/data/field-asymmetric-fan.expected").split_whitespace();
        assert_eq!(output.next(), Some("CLAYFIELD1"));
        assert_eq!(output.next(), Some(fingerprint));
        assert_eq!(output.next().unwrap().parse::<usize>().unwrap(), n);
        for i in 0..n {
            let mut scalar = || output.next().unwrap().parse::<f64>().unwrap();
            let q = Vec3::new(scalar(), scalar(), scalar());
            let o = Vec3::new(scalar(), scalar(), scalar());
            assert!((field.directions[i] - q).length() < 1e-9, "direction {i}");
            assert!(
                (field.origins[i] - o).length() < spacing * 1e-9,
                "origin {i}"
            );
        }
        assert!(output.next().is_none());
    }

    #[test]
    fn initialization_matches_linux_reference_seed_zero() {
        let mut rng = InitialRandom::new();
        let expected = [
            1804289383, 846930886, 1681692777, 1714636915, 1957747793, 424238335, 719885386,
            1649760492, 596516649, 1189641421,
        ];
        for x in expected {
            assert_eq!(rng.next(), x);
        }
    }
}
