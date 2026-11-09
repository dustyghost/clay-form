//! Geometry-preserving working triangles for the field solver, not output quads.
//! Split/collapse/swap/relax/project follows Botsch & Kobbelt (SGP 2004).
//! Local quality, normal and source-distance guards were informed by VCGLib's
//! isotropic_remeshing.h (Visual Computing Lab, ISTI-CNR, GPL-2.0-or-later).
//! Reference: cnr-isti-vclab/vcglib at 5cae2abc2f9056785b0537dcbe156c40da2aea20,
//! vcg/complex/algorithms/isotropic_remeshing.h. This is not an exact port:
//! original-surface coverage witnesses additionally guard every local edit.
//! This Rust implementation is GPL-3.0-only; it has no C++ dependency.
use crate::{Options, Result, Surface, Vec3, check, surface::Bvh, topology};
use std::collections::{BTreeMap, BTreeSet};

#[cfg_attr(feature = "audit", derive(serde::Serialize))]
#[derive(Debug, Default)]
pub struct Step {
    pub iteration: usize,
    pub vertices: usize,
    pub splits: usize,
    pub collapses: usize,
    pub flips: usize,
    pub moved: usize,
}
#[cfg_attr(feature = "audit", derive(serde::Serialize))]
#[derive(Debug)]
pub struct Statistics {
    pub vertices: usize,
    pub triangles: usize,
    pub edge_length_p05_p50_p95: [f64; 3],
    pub triangle_quality_p05_p50_p95: [f64; 3],
}
#[cfg_attr(feature = "audit", derive(serde::Serialize))]
#[derive(Debug)]
pub struct Report {
    pub spacing: f64,
    pub edge_target: f64,
    pub local_surface_limit: f64,
    pub sampled_surface_limit: f64,
    pub before: Statistics,
    pub after: Statistics,
    pub steps: Vec<Step>,
    pub source_to_working: f64,
    pub working_to_source: f64,
    pub triangle_queries: usize,
}
pub struct Prepared {
    pub surface: Surface,
    pub detail: Vec<f64>,
    pub report: Report,
}

fn normal(p: [Vec3; 3]) -> Vec3 {
    (p[1] - p[0]).cross(p[2] - p[0])
}
fn quality(p: [Vec3; 3]) -> f64 {
    let sum: f64 = (0..3).map(|k| p[k].distance_squared(p[(k + 1) % 3])).sum();
    2. * 3f64.sqrt() * normal(p).length() / sum.max(1e-30)
}
fn statistics(mesh: &Surface) -> Statistics {
    let mut lengths = BTreeMap::new();
    let mut qualities = Vec::with_capacity(mesh.triangles.len());
    for &t in &mesh.triangles {
        qualities.push(quality(t.map(|v| mesh.positions[v])));
        for k in 0..3 {
            let [a, b] = topology::edge(t[k], t[(k + 1) % 3]);
            lengths
                .entry([a, b])
                .or_insert_with(|| mesh.positions[a].distance(mesh.positions[b]));
        }
    }
    let quantiles = |mut a: Vec<f64>| {
        a.sort_by(f64::total_cmp);
        [0.05, 0.5, 0.95].map(|p| a[((a.len() - 1) as f64 * p).round() as usize])
    };
    Statistics {
        vertices: mesh.positions.len(),
        triangles: mesh.triangles.len(),
        edge_length_p05_p50_p95: quantiles(lengths.into_values().collect()),
        triangle_quality_p05_p50_p95: quantiles(qualities),
    }
}

struct Working {
    positions: Vec<Vec3>,
    faces: Vec<Option<[usize; 3]>>,
    incident: Vec<BTreeSet<usize>>,
    locked: Vec<bool>,
    active: Vec<bool>,
    detail: Vec<f64>,
    vertices: usize,
    // Every original vertex and face centroid has one current owner triangle.
    // Local edits must retain these witnesses; this prevents cumulative loss
    // that one-way projection to the source cannot detect.
    witnesses: Vec<Vec<Vec3>>,
}
impl Working {
    fn new(source: &Surface, options: &Options) -> Self {
        let mut w = Self {
            positions: source.positions.clone(),
            faces: vec![],
            incident: vec![BTreeSet::new(); source.positions.len()],
            locked: vec![false; source.positions.len()],
            active: vec![true; source.positions.len()],
            detail: if options.detail.is_empty() {
                vec![0.; source.positions.len()]
            } else {
                options.detail.clone()
            },
            vertices: source.positions.len(),
            witnesses: vec![],
        };
        for &t in &source.triangles {
            w.add(t);
            w.witnesses
                .last_mut()
                .unwrap()
                .push(t.map(|v| source.positions[v]).into_iter().sum::<Vec3>() / 3.);
        }
        for (v, &p) in source.positions.iter().enumerate() {
            w.witnesses[*w.incident[v].first().unwrap()].push(p);
        }
        let sharp_threshold = options.sharp_threshold();
        for [a, b] in w.edges() {
            let fs = w.edge_faces(a, b);
            if fs.len() == 1
                || sharp_threshold.is_some_and(|threshold| {
                    normal(w.points(w.faces[fs[0]].unwrap()))
                        .normalize()
                        .dot(normal(w.points(w.faces[fs[1]].unwrap())).normalize())
                        <= threshold
                })
            {
                w.locked[a] = true;
                w.locked[b] = true;
            }
        }
        w
    }
    fn add(&mut self, t: [usize; 3]) {
        let f = self.faces.len();
        self.faces.push(Some(t));
        self.witnesses.push(Vec::new());
        for v in t {
            self.incident[v].insert(f);
        }
    }
    fn erase(&mut self, f: usize) {
        self.witnesses[f].clear();
        if let Some(t) = self.faces[f].take() {
            for v in t {
                self.incident[v].remove(&f);
            }
        }
    }
    fn replace(&mut self, f: usize, t: [usize; 3]) {
        self.erase(f);
        self.faces[f] = Some(t);
        for v in t {
            self.incident[v].insert(f);
        }
    }
    fn points(&self, t: [usize; 3]) -> [Vec3; 3] {
        t.map(|v| self.positions[v])
    }
    fn edges(&self) -> Vec<[usize; 2]> {
        self.faces
            .iter()
            .flatten()
            .flat_map(|t| (0..3).map(|k| topology::edge(t[k], t[(k + 1) % 3])))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    fn edge_faces(&self, a: usize, b: usize) -> Vec<usize> {
        self.incident[a]
            .intersection(&self.incident[b])
            .copied()
            .collect()
    }
    fn neighbours(&self, v: usize) -> BTreeSet<usize> {
        self.incident[v]
            .iter()
            .flat_map(|&f| self.faces[f].unwrap())
            .filter(|&u| u != v)
            .collect()
    }
    fn compact(&self) -> (Surface, Vec<f64>) {
        let mut map = vec![usize::MAX; self.positions.len()];
        let mut positions = Vec::new();
        let mut detail = Vec::new();
        for (v, &p) in self.positions.iter().enumerate() {
            if self.active[v] {
                map[v] = positions.len();
                positions.push(p);
                detail.push(self.detail[v]);
            }
        }
        (
            Surface {
                positions,
                triangles: self
                    .faces
                    .iter()
                    .flatten()
                    .map(|t| t.map(|v| map[v]))
                    .collect(),
            },
            detail,
        )
    }
    fn pack(&mut self) {
        let (surface, detail) = self.compact();
        let witnesses = std::mem::take(&mut self.witnesses)
            .into_iter()
            .zip(&self.faces)
            .filter_map(|(points, face)| face.is_some().then_some(points))
            .collect::<Vec<_>>();
        let locked = self
            .locked
            .iter()
            .zip(&self.active)
            .filter_map(|(&lock, &active)| active.then_some(lock))
            .collect();
        *self = Self {
            incident: vec![BTreeSet::new(); surface.positions.len()],
            active: vec![true; surface.positions.len()],
            vertices: surface.positions.len(),
            positions: surface.positions,
            faces: vec![],
            detail,
            locked,
            witnesses: vec![],
        };
        for (t, points) in surface.triangles.into_iter().zip(witnesses) {
            self.add(t);
            *self.witnesses.last_mut().unwrap() = points;
        }
    }
}

struct Reference<'a> {
    source: &'a Surface,
    density: &'a [f64],
    bvh: Bvh<'a>,
    local: f64,
    coverage: f64,
    target: f64,
    work: usize,
    cancel: &'a dyn Fn() -> bool,
}
impl Reference<'_> {
    fn transfer(
        &mut self,
        w: &Working,
        faces: &[usize],
        candidates: &[[Vec3; 3]],
    ) -> Result<Option<Vec<Vec<Vec3>>>> {
        let mut points = vec![Vec::new(); candidates.len()];
        for p in faces.iter().flat_map(|&f| w.witnesses[f].iter()).copied() {
            let mut nearest = (usize::MAX, f64::INFINITY);
            for (i, &[a, b, c]) in candidates.iter().enumerate() {
                if self.work == 0 {
                    return Err("Input resampling query budget exceeded".into());
                }
                self.work -= 1;
                if self.work.is_multiple_of(4096) {
                    check(self.cancel)?;
                }
                let distance = p.distance_squared(crate::surface::closest(p, a, b, c));
                if distance < nearest.1 {
                    nearest = (i, distance);
                }
            }
            if nearest.1 > self.coverage * self.coverage {
                return Ok(None);
            }
            points[nearest.0].push(p);
        }
        Ok(Some(points))
    }
    fn target(&self, detail: f64) -> f64 {
        self.target * 2f64.powf(-detail)
    }
    fn project(&mut self, point: Vec3) -> Result<(Vec3, f64)> {
        let hit = self.bvh.nearest(point, &mut self.work, self.cancel)?;
        let mut detail = 0.;
        if !self.density.is_empty() {
            let ids = self.source.triangles[hit.face];
            let [a, b, c] = ids.map(|v| self.source.positions[v]);
            let area = normal([a, b, c]).length();
            let weights = [
                normal([hit.point, b, c]).length(),
                normal([a, hit.point, c]).length(),
                normal([a, b, hit.point]).length(),
            ];
            for k in 0..3 {
                detail += weights[k] / area * self.density[ids[k]];
            }
        }
        Ok((hit.point, detail.clamp(-1., 1.)))
    }
    fn triangle(&mut self, p: [Vec3; 3], old: [Vec3; 3], quality_ratio: f64) -> Result<bool> {
        let n = normal(p);
        let previous = normal(old);
        if n.length_squared() < 1e-24
            || n.normalize().dot(previous.normalize()) < 0.7
            || quality(p) < quality_ratio * quality(old)
        {
            return Ok(false);
        }
        let center = (p[0] + p[1] + p[2]) / 3.;
        let hit = self.bvh.nearest(center, &mut self.work, self.cancel)?;
        if hit.distance > self.local || n.normalize().dot(hit.normal) < 0.7 {
            return Ok(false);
        }
        for k in 0..3 {
            if self
                .bvh
                .nearest((p[k] + p[(k + 1) % 3]) * 0.5, &mut self.work, self.cancel)?
                .distance
                > self.local
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn split(w: &mut Working, r: &mut Reference<'_>, budget: usize) -> Result<usize> {
    let mut count = 0;
    for (i, [a, b]) in w.edges().into_iter().enumerate() {
        if i % 512 == 0 {
            check(r.cancel)?;
        }
        if w.positions[a].distance(w.positions[b])
            <= r.target((w.detail[a] + w.detail[b]) * 0.5) * 4. / 3.
        {
            continue;
        }
        let faces = w.edge_faces(a, b);
        if faces.is_empty() {
            continue;
        }
        if w.vertices == budget {
            return Err("Input resampling exceeds the vertex budget".into());
        }
        let locked = faces.len() == 1 || (w.locked[a] && w.locked[b]);
        let midpoint = (w.positions[a] + w.positions[b]) * 0.5;
        let (p, d) = if locked {
            (midpoint, (w.detail[a] + w.detail[b]) * 0.5)
        } else {
            r.project(midpoint)?
        };
        let v = w.positions.len();
        let mut replacements = Vec::new();
        let mut valid = true;
        for f in faces {
            let t = w.faces[f].unwrap();
            let k = (0..3)
                .find(|&k| topology::edge(t[k], t[(k + 1) % 3]) == [a, b])
                .unwrap();
            let (u, z, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
            for q in [[u, v, c], [v, z, c]] {
                let pts = q.map(|x| if x == v { p } else { w.positions[x] });
                valid &= r.triangle(pts, w.points(t), 0.)?;
            }
            let candidates =
                [[u, v, c], [v, z, c]].map(|q| q.map(|x| if x == v { p } else { w.positions[x] }));
            let Some(witnesses) = r.transfer(w, &[f], &candidates)? else {
                valid = false;
                break;
            };
            replacements.push((f, [u, v, c], [v, z, c], witnesses));
        }
        if !valid {
            continue;
        }
        w.positions.push(p);
        w.detail.push(d);
        w.locked.push(locked);
        w.active.push(true);
        w.incident.push(BTreeSet::new());
        w.vertices += 1;
        for (f, t, u, mut witnesses) in replacements {
            w.replace(f, t);
            w.add(u);
            *w.witnesses.last_mut().unwrap() = witnesses.pop().unwrap();
            w.witnesses[f] = witnesses.pop().unwrap();
        }
        count += 1;
    }
    Ok(count)
}

fn collapse(w: &mut Working, r: &mut Reference<'_>) -> Result<usize> {
    let mut edges = w.edges();
    edges.sort_by(|a, b| {
        w.positions[a[0]]
            .distance_squared(w.positions[a[1]])
            .total_cmp(&w.positions[b[0]].distance_squared(w.positions[b[1]]))
            .then(a.cmp(b))
    });
    let mut count = 0;
    for (i, [a, b]) in edges.into_iter().enumerate() {
        if i % 512 == 0 {
            check(r.cancel)?;
        }
        if !w.active[a] || !w.active[b] || w.locked[a] || w.locked[b] {
            continue;
        }
        let fs = w.edge_faces(a, b);
        if fs.len() != 2
            || w.positions[a].distance(w.positions[b])
                >= r.target((w.detail[a] + w.detail[b]) * 0.5) * 4. / 5.
        {
            continue;
        }
        let na = w.neighbours(a);
        let nb = w.neighbours(b);
        let opposite: BTreeSet<_> = fs
            .iter()
            .flat_map(|&f| w.faces[f].unwrap())
            .filter(|&v| v != a && v != b)
            .collect();
        if na.intersection(&nb).copied().collect::<BTreeSet<_>>() != opposite || opposite.len() != 2
        {
            continue;
        }
        let (p, d) = r.project((w.positions[a] + w.positions[b]) * 0.5)?;
        if na
            .union(&nb)
            .copied()
            .filter(|&v| v != a && v != b)
            .any(|v| p.distance(w.positions[v]) > 4. / 3. * r.target((d + w.detail[v]) * 0.5))
        {
            continue;
        }
        let affected: Vec<_> = w.incident[a].union(&w.incident[b]).copied().collect();
        let mut seen = BTreeSet::new();
        let mut changes = Vec::new();
        let mut valid = true;
        for &f in &affected {
            if fs.contains(&f) {
                continue;
            }
            let old = w.faces[f].unwrap();
            let t = old.map(|v| if v == b { a } else { v });
            let mut key = t;
            key.sort_unstable();
            if !seen.insert(key) {
                valid = false;
                break;
            }
            if !r.triangle(
                t.map(|v| if v == a { p } else { w.positions[v] }),
                w.points(old),
                0.5,
            )? {
                valid = false;
                break;
            }
            changes.push((f, t));
        }
        if !valid {
            continue;
        }
        let candidates = changes
            .iter()
            .map(|&(_, t)| t.map(|v| if v == a { p } else { w.positions[v] }))
            .collect::<Vec<_>>();
        let Some(witnesses) = r.transfer(w, &affected, &candidates)? else {
            continue;
        };
        for f in affected {
            w.erase(f);
        }
        w.positions[a] = p;
        w.detail[a] = d;
        w.active[b] = false;
        w.vertices -= 1;
        for ((f, t), points) in changes.into_iter().zip(witnesses) {
            w.replace(f, t);
            w.witnesses[f] = points;
        }
        count += 1;
    }
    Ok(count)
}

fn flip(w: &mut Working, r: &mut Reference<'_>) -> Result<usize> {
    let mut count = 0;
    for (i, [a, b]) in w.edges().into_iter().enumerate() {
        if i % 512 == 0 {
            check(r.cancel)?;
        }
        let fs = w.edge_faces(a, b);
        if fs.len() != 2 {
            continue;
        }
        let t = w.faces[fs[0]].unwrap();
        let u = w.faces[fs[1]].unwrap();
        let k = (0..3)
            .find(|&k| topology::edge(t[k], t[(k + 1) % 3]) == [a, b])
            .unwrap();
        let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
        let d = *u.iter().find(|&&v| v != a && v != b).unwrap();
        if [a, b, c, d].iter().any(|&v| w.locked[v]) || !w.edge_faces(c, d).is_empty() {
            continue;
        }
        let degrees = [a, b, c, d].map(|v| w.neighbours(v).len() as i32);
        let cost = |degrees: [i32; 4]| degrees.map(|n| (n - 6).pow(2)).iter().sum::<i32>();
        let old_cost = cost(degrees);
        let new_cost = cost([
            degrees[0] - 1,
            degrees[1] - 1,
            degrees[2] + 1,
            degrees[3] + 1,
        ]);
        if new_cost > old_cost {
            continue;
        }
        let (v, z) = ([c, d, b], [d, c, a]);
        let old_quality = quality(w.points(t)).min(quality(w.points(u)));
        let new_quality = quality(w.points(v)).min(quality(w.points(z)));
        if new_quality < old_quality * 0.8
            || (new_cost == old_cost && new_quality <= old_quality + 1e-6)
        {
            continue;
        }
        if !r.triangle(w.points(v), w.points(t), 0.)?
            || !r.triangle(w.points(z), w.points(u), 0.)?
        {
            continue;
        }
        let Some(mut witnesses) = r.transfer(w, &fs, &[w.points(v), w.points(z)])? else {
            continue;
        };
        w.replace(fs[0], v);
        w.replace(fs[1], z);
        w.witnesses[fs[1]] = witnesses.pop().unwrap();
        w.witnesses[fs[0]] = witnesses.pop().unwrap();
        count += 1;
    }
    Ok(count)
}

fn relax(w: &mut Working, r: &mut Reference<'_>) -> Result<usize> {
    let mut moves = vec![Vec3::ZERO; w.positions.len()];
    for (v, delta) in moves.iter_mut().enumerate() {
        if v % 512 == 0 {
            check(r.cancel)?;
        }
        if !w.active[v] || w.locked[v] {
            continue;
        }
        let row = w.neighbours(v);
        let mean = row.iter().map(|&u| w.positions[u]).sum::<Vec3>() / row.len() as f64;
        let n = w.incident[v]
            .iter()
            .map(|&f| normal(w.points(w.faces[f].unwrap())))
            .sum::<Vec3>()
            .normalize_or_zero();
        let d = mean - w.positions[v];
        *delta = (d - n * n.dot(d)) * 0.2;
    }
    let mut count = 0;
    for (v, delta) in moves.into_iter().enumerate() {
        if v % 512 == 0 {
            check(r.cancel)?;
        }
        if delta.length_squared() < 1e-24 {
            continue;
        }
        for alpha in [1., 0.5, 0.25, 0.125] {
            let (p, d) = r.project(w.positions[v] + delta * alpha)?;
            let mut valid = true;
            for &f in &w.incident[v] {
                let t = w.faces[f].unwrap();
                if !r.triangle(
                    t.map(|u| if u == v { p } else { w.positions[u] }),
                    w.points(t),
                    0.5,
                )? {
                    valid = false;
                    break;
                }
            }
            if valid {
                let faces = w.incident[v].iter().copied().collect::<Vec<_>>();
                let candidates = faces
                    .iter()
                    .map(|&f| {
                        w.faces[f]
                            .unwrap()
                            .map(|u| if u == v { p } else { w.positions[u] })
                    })
                    .collect::<Vec<_>>();
                let Some(witnesses) = r.transfer(w, &faces, &candidates)? else {
                    continue;
                };
                w.positions[v] = p;
                w.detail[v] = d;
                for (f, points) in faces.into_iter().zip(witnesses) {
                    w.witnesses[f] = points;
                }
                count += 1;
                break;
            }
        }
    }
    Ok(count)
}

fn distance(
    from: &Surface,
    to: &Bvh<'_>,
    work: &mut usize,
    cancel: &dyn Fn() -> bool,
) -> Result<f64> {
    let mut maximum = 0f64;
    for p in from.positions.iter().copied().chain(
        from.triangles
            .iter()
            .map(|t| t.iter().map(|&v| from.positions[v]).sum::<Vec3>() / 3.),
    ) {
        maximum = maximum.max(to.nearest(p, work, cancel)?.distance);
    }
    Ok(maximum)
}

pub fn prepare(
    source: &Surface,
    options: &Options,
    iterations: usize,
    cancel: &dyn Fn() -> bool,
    progress: &dyn Fn(&Step),
) -> Result<Prepared> {
    check(cancel)?;
    if !(1..=10).contains(&iterations)
        || !(4..=options.max_vertices).contains(&options.target_quads)
        || source.positions.len() > options.max_vertices
        || (!options.detail.is_empty() && options.detail.len() != source.positions.len())
        || options
            .detail
            .iter()
            .any(|d| !d.is_finite() || !(-1. ..=1.).contains(d))
    {
        return Err("Invalid input resampling budget or detail paint".into());
    }
    let topo = topology::validate(source)?;
    let (_, _, _, area) = topology::geometry(source);
    let h = (area / options.target_quads as f64).sqrt();
    let before = statistics(source);
    let budget = 500_000_000;
    let mut reference = Reference {
        source,
        density: &options.detail,
        bvh: Bvh::new(source, cancel)?,
        local: h * 0.01,
        coverage: h * 0.15 * (1. - 1e-10),
        target: h * 0.5,
        work: budget,
        cancel,
    };
    let mut w = Working::new(source, options);
    let mut steps = Vec::new();
    for iteration in 1..=iterations {
        check(cancel)?;
        let splits = split(&mut w, &mut reference, options.max_vertices)?;
        let collapses = collapse(&mut w, &mut reference)?;
        let flips = flip(&mut w, &mut reference)?;
        let moved = relax(&mut w, &mut reference)?;
        let step = Step {
            iteration,
            vertices: w.vertices,
            splits,
            collapses,
            flips,
            moved,
        };
        progress(&step);
        steps.push(step);
        w.pack();
        if w.witnesses.iter().map(Vec::len).sum::<usize>()
            != source.positions.len() + source.triangles.len()
        {
            return Err("Input resampling lost a source coverage witness".into());
        }
    }
    let (surface, detail) = w.compact();
    drop(w);
    if topology::validate(&surface)? != topo {
        return Err("Input resampling changed source topology".into());
    }
    let after = statistics(&surface);
    let working_to_source = distance(&surface, &reference.bvh, &mut reference.work, cancel)?;
    let rebuilt = Bvh::new(&surface, cancel)?;
    let source_to_working = distance(source, &rebuilt, &mut reference.work, cancel)?;
    if working_to_source.max(source_to_working) > h * 0.15 {
        return Err(format!(
            "Input resampling lost source shape: {:.6} / {:.6}, limit {:.6}",
            source_to_working,
            working_to_source,
            h * 0.15
        ));
    }
    let report = Report {
        spacing: h,
        edge_target: reference.target,
        local_surface_limit: reference.local,
        sampled_surface_limit: h * 0.15,
        before,
        after,
        steps,
        source_to_working,
        working_to_source,
        triangle_queries: budget - reference.work,
    };
    Ok(Prepared {
        surface,
        detail,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn patch() -> Surface {
        let xs = [
            0., 0.003, 0.008, 0.02, 0.05, 0.10, 0.25, 0.40, 0.55, 0.70, 0.85, 1.,
        ];
        let mut positions = Vec::new();
        let mut triangles = Vec::new();
        for y in 0..13 {
            for x in xs {
                positions.push(Vec3::new(x, y as f64 / 12., 0.));
            }
        }
        for y in 0..12 {
            for x in 0..11 {
                let a = y * 12 + x;
                triangles.extend([[a, a + 1, a + 13], [a, a + 13, a + 12]]);
            }
        }
        Surface {
            positions,
            triangles,
        }
    }
    fn options() -> Options {
        Options {
            target_quads: 64,
            max_vertices: 10000,
            preserve_sharp: false,
            ..Options::default()
        }
    }
    #[test]
    fn source_witnesses_prevent_erasing_a_peak() {
        let source = Surface {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::new(0.25, 0.25, 0.2)],
            triangles: vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
        };
        let w = Working::new(&source, &options());
        let mut r = Reference {
            source: &source,
            density: &[],
            bvh: Bvh::new(&source, &|| false).unwrap(),
            local: 0.01,
            coverage: 0.05,
            target: 0.2,
            work: 10000,
            cancel: &|| false,
        };
        let flattened = [[Vec3::ZERO, Vec3::X, Vec3::Y]];
        assert!(r.transfer(&w, &[0, 1, 2], &flattened).unwrap().is_none());
        let unchanged = source
            .triangles
            .iter()
            .map(|&t| w.points(t))
            .collect::<Vec<_>>();
        let transferred = r.transfer(&w, &[0, 1, 2], &unchanged).unwrap().unwrap();
        assert_eq!(
            transferred.iter().map(Vec::len).sum::<usize>(),
            source.positions.len() + source.triangles.len()
        );
    }
    #[test]
    fn closed_curved_surface_keeps_topology_and_two_way_coverage() {
        let source = Surface {
            positions: vec![
                Vec3::X * 1.17,
                Vec3::Y,
                -Vec3::X,
                -Vec3::Y,
                Vec3::Z,
                -Vec3::Z * 0.83,
            ],
            triangles: vec![
                [0, 1, 4],
                [1, 2, 4],
                [2, 3, 4],
                [3, 0, 4],
                [1, 0, 5],
                [2, 1, 5],
                [3, 2, 5],
                [0, 3, 5],
            ],
        };
        let result = prepare(&source, &options(), 5, &|| false, &|_| {}).unwrap();
        assert_eq!(
            topology::validate(&source).unwrap(),
            topology::validate(&result.surface).unwrap()
        );
        assert!(result.report.source_to_working <= result.report.sampled_surface_limit);
        assert!(result.report.working_to_source <= result.report.local_surface_limit * 1.001);
    }
    #[test]
    fn redistributes_dense_input_preserving_boundary_shape_and_source() {
        let source = patch();
        let original = source.clone();
        let before = topology::validate(&source).unwrap();
        let result = prepare(&source, &options(), 5, &|| false, &|_| {}).unwrap();
        assert_eq!(source.positions, original.positions);
        assert_eq!(source.triangles, original.triangles);
        assert_eq!(topology::validate(&result.surface).unwrap(), before);
        assert!(
            result
                .report
                .steps
                .iter()
                .map(|s| s.collapses)
                .sum::<usize>()
                > 10
        );
        assert!(result.report.steps.iter().map(|s| s.splits).sum::<usize>() > 10);
        assert!(
            result.report.after.triangle_quality_p05_p50_p95[1]
                > result.report.before.triangle_quality_p05_p50_p95[1]
        );
        assert!(result.report.source_to_working < 1e-10 && result.report.working_to_source < 1e-10);
        for p in original
            .positions
            .iter()
            .filter(|p| p.x == 0. || p.x == 1. || p.y == 0. || p.y == 1.)
        {
            assert!(result.surface.positions.contains(p));
        }
    }
    #[test]
    fn signed_detail_is_resampled_from_the_original_surface() {
        let source = patch();
        let mut options = options();
        options.detail = source.positions.iter().map(|p| 2. * p.x - 1.).collect();
        let result = prepare(&source, &options, 3, &|| false, &|_| {}).unwrap();
        for (p, d) in result.surface.positions.iter().zip(result.detail) {
            assert!((d - (2. * p.x - 1.)).abs() < 1e-10);
        }
    }
    #[test]
    fn protected_crease_vertices_stay_on_the_original_fold() {
        let mut source = patch();
        for p in &mut source.positions {
            p.z = 2. * (p.x - 0.4).max(0.);
        }
        let mut o = options();
        o.preserve_sharp = true;
        let crease = source
            .positions
            .iter()
            .position(|p| p.x == 0.4 && p.y > 0. && p.y < 1.)
            .unwrap();
        assert!(Working::new(&source, &o).locked[crease]);
        o.sharp_angle_degrees = 75.;
        assert!(!Working::new(&source, &o).locked[crease]);
        o.sharp_angle_degrees = 45.;
        let result = prepare(&source, &o, 3, &|| false, &|_| {}).unwrap();
        for p in source.positions.iter().filter(|p| p.x == 0.4) {
            assert!(result.surface.positions.contains(p));
        }
        assert_eq!(
            topology::validate(&source).unwrap(),
            topology::validate(&result.surface).unwrap()
        );
        assert!(result.report.source_to_working <= result.report.sampled_surface_limit);
    }
    #[test]
    fn nearby_disconnected_sheets_remain_separate() {
        let mut source = patch();
        let n = source.positions.len();
        let other = source
            .triangles
            .iter()
            .map(|t| t.map(|v| v + n))
            .collect::<Vec<_>>();
        source.positions.extend(
            source
                .positions
                .clone()
                .into_iter()
                .map(|p| p + Vec3::Z * 0.001),
        );
        source.triangles.extend(other);
        let mut o = options();
        o.target_quads *= 2;
        let result = prepare(&source, &o, 2, &|| false, &|_| {}).unwrap();
        assert_eq!(
            topology::validate(&source).unwrap(),
            topology::validate(&result.surface).unwrap()
        );
        assert!(
            result
                .surface
                .positions
                .iter()
                .all(|p| p.z.abs() < 1e-12 || (p.z - 0.001).abs() < 1e-12)
        );
    }
    #[test]
    fn cancellation_and_budget_failure_return_no_partial_preparation() {
        let source = patch();
        let original = source.clone();
        let calls = std::cell::Cell::new(0);
        assert!(
            prepare(
                &source,
                &options(),
                5,
                &|| {
                    calls.set(calls.get() + 1);
                    calls.get() > 30
                },
                &|_| {}
            )
            .is_err()
        );
        let mut o = options();
        o.max_vertices = source.positions.len();
        assert!(prepare(&source, &o, 5, &|| false, &|_| {}).is_err());
        assert_eq!(source.positions, original.positions);
        assert_eq!(source.triangles, original.triangles);
    }
}
