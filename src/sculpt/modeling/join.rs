//! Cancel the existing side patches swept by a matching-face join. A patch may
//! contain many depth loops; its boundary must tile the whole swept wall exactly.
use super::*;

type Wall = ([usize; 4], usize);
type Chain = Vec<(f32, usize)>;
pub(super) struct JoinedWalls {
    pub remove: BTreeSet<usize>,
    pub walls: Vec<(Vec<usize>, usize)>,
}

fn segment_parameter(p: Vec3, a: Vec3, b: Vec3, tolerance: f32) -> Option<f32> {
    let d = b - a;
    let t = (p - a).dot(d) / d.dot(d).max(1e-20);
    ((-1e-5..=1. + 1e-5).contains(&t) && (p - a - d * t).len() <= tolerance)
        .then_some(t.clamp(0., 1.))
}

/// Test full patch coverage by its boundary intervals, rather than summed areas:
/// holes, doubled coverage, and a strip stopping short cannot pass this check.
fn covers_wall(
    source: &Mesh,
    wall: [usize; 4],
    boundary: &[(usize, usize)],
    tolerance: f32,
) -> bool {
    let mut intervals: [Vec<(f32, f32)>; 4] = Default::default();
    for &(a, b) in boundary {
        let side = (0..4).find_map(|i| {
            let start = source.vertices[wall[i]];
            let end = source.vertices[wall[(i + 1) % 4]];
            let ta = segment_parameter(source.vertices[a], start, end, tolerance)?;
            let tb = segment_parameter(source.vertices[b], start, end, tolerance)?;
            // Existing wall boundaries have the opposite winding to the new wall.
            (ta > tb).then_some((i, tb, ta))
        });
        let Some((i, start, end)) = side else {
            return false;
        };
        intervals[i].push((start, end));
    }
    intervals.iter_mut().all(|parts| {
        parts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut end = 0.;
        for &(a, b) in parts.iter() {
            if (a - end).abs() > 1e-5 {
                return false;
            }
            end = b;
        }
        (end - 1.).abs() <= 1e-5
    })
}

impl Mesh {
    pub(super) fn joined_walls(
        &self,
        polygons: &[Polygon],
        sides: &[Wall],
        excluded: &BTreeSet<usize>,
        out: &mut Mesh,
    ) -> Result<JoinedWalls, String> {
        let mut incidence = BTreeMap::<_, Vec<usize>>::new();
        for (i, p) in polygons.iter().enumerate() {
            if !excluded.contains(&i) {
                for (a, b) in p.edges() {
                    incidence.entry(edge(a, b)).or_default().push(i);
                }
            }
        }
        let mut result = JoinedWalls {
            remove: BTreeSet::new(),
            walls: Vec::new(),
        };
        let mut cancelled = BTreeSet::new();
        let mut chains = BTreeMap::<usize, Chain>::new();
        let mut destinations = BTreeMap::new();
        for &([a, b, bb, aa], _) in sides {
            chains.entry(a).or_insert_with(|| vec![(0., a), (1., aa)]);
            chains.entry(b).or_insert_with(|| vec![(0., b), (1., bb)]);
            destinations.insert(a, aa);
            destinations.insert(b, bb);
        }
        let mut fixed = BTreeSet::new();
        for (wall_index, &(wall, _)) in sides.iter().enumerate() {
            let [a, b, _, aa] = wall;
            let origin = self.vertices[a];
            let u = self.vertices[b] - origin;
            let v = self.vertices[aa] - origin;
            let n = u.cross(v).norm();
            let tolerance = u.len().max(v.len()) * 1e-5;
            let inside = |point: Vec3| {
                let d = point - origin;
                let x = d.dot(u) / u.dot(u);
                let y = d.dot(v) / v.dot(v);
                d.dot(n).abs() <= tolerance
                    && (-1e-5..=1. + 1e-5).contains(&x)
                    && (-1e-5..=1. + 1e-5).contains(&y)
            };
            let mut patch = BTreeSet::new();
            let mut pending = incidence.get(&edge(a, b)).cloned().unwrap_or_default();
            if pending.iter().any(|&i| {
                let p = &polygons[i];
                normal(self, p).dot(n) < -0.9999
                    && p.vertices
                        .iter()
                        .all(|&vertex| (self.vertices[vertex] - origin).dot(n).abs() <= tolerance)
                    && p.vertices
                        .iter()
                        .any(|&vertex| !inside(self.vertices[vertex]))
            }) {
                return Err("The adjoining side extends beyond this join; split its boundary before extruding through".into());
            }
            while let Some(i) = pending.pop() {
                if patch.contains(&i) {
                    continue;
                }
                let p = &polygons[i];
                if normal(self, p).dot(n) > -0.9999
                    || p.vertices
                        .iter()
                        .any(|&vertex| !inside(self.vertices[vertex]))
                {
                    continue;
                }
                if !p.visible {
                    return Err("Show the adjoining side faces before joining this tunnel".into());
                }
                patch.insert(i);
                for (a, b) in p.edges() {
                    pending.extend(incidence[&edge(a, b)].iter().copied());
                }
            }
            if patch.is_empty() {
                continue;
            }
            let mut boundary = BTreeMap::new();
            for &i in &patch {
                for (a, b) in polygons[i].edges() {
                    if boundary.remove(&edge(a, b)).is_none() {
                        boundary.insert(edge(a, b), (a, b));
                    }
                }
            }
            let boundary: Vec<_> = boundary.into_values().collect();
            if !covers_wall(self, wall, &boundary, tolerance) {
                return Err("The adjoining side only partly covers this join; align its boundary before extruding through".into());
            }
            for source in [wall[0], wall[1]] {
                let target = destinations[&source];
                for vertex in boundary.iter().flat_map(|&(a, b)| [a, b]) {
                    if let Some(t) = segment_parameter(
                        self.vertices[vertex],
                        self.vertices[source],
                        self.vertices[target],
                        tolerance,
                    ) {
                        chains.get_mut(&source).unwrap().push((t, vertex));
                    }
                }
                fixed.insert(source);
            }
            cancelled.insert(wall_index);
            result.remove.extend(patch);
        }
        // Keep existing boundary vertices, including every depth-loop junction.
        // Free paths adopt the same depths so regular strips stay all-quads.
        for chain in chains.values_mut() {
            chain.sort_by(|a, b| a.0.total_cmp(&b.0));
            chain.dedup_by(|a, b| a.1 == b.1);
            if chain.windows(2).any(|pair| pair[1].0 - pair[0].0 < 1e-6) {
                return Err("The adjoining wall has overlapping boundary vertices".into());
            }
        }
        let mut depths: Vec<_> = chains
            .values()
            .flat_map(|c| c.iter().map(|&(t, _)| t))
            .collect();
        depths.sort_by(f32::total_cmp);
        depths.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        for (&source, chain) in &mut chains {
            if fixed.contains(&source) {
                continue;
            }
            let target = destinations[&source];
            for &t in &depths {
                if t <= 1e-6 || t >= 1. - 1e-6 {
                    continue;
                }
                if out.vertices.len() >= max_vertices() {
                    return Err("Tunnel join would exceed the vertex limit".into());
                }
                let vertex = out.append_blend(
                    self,
                    &[(source, 1. - t), (target, t)],
                    self.vertices[source] * (1. - t) + self.vertices[target] * t,
                );
                chain.push((t, vertex));
            }
            chain.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        for (i, &([a, b, _, _], group)) in sides.iter().enumerate() {
            if cancelled.contains(&i) {
                continue;
            }
            let (a, b) = (&chains[&a], &chains[&b]);
            let (mut j, mut k) = (0, 0);
            while j + 1 < a.len() || k + 1 < b.len() {
                let ta = a.get(j + 1).map_or(f32::INFINITY, |v| v.0);
                let tb = b.get(k + 1).map_or(f32::INFINITY, |v| v.0);
                let face = if (ta - tb).abs() < 1e-6 {
                    let face = vec![a[j].1, b[k].1, b[k + 1].1, a[j + 1].1];
                    j += 1;
                    k += 1;
                    face
                } else if ta < tb {
                    let face = vec![a[j].1, b[k].1, a[j + 1].1];
                    j += 1;
                    face
                } else {
                    let face = vec![a[j].1, b[k].1, b[k + 1].1];
                    k += 1;
                    face
                };
                result.walls.push((face, group));
                if result.walls.len() > max_triangles() / 2 {
                    return Err("Tunnel join would exceed the face limit".into());
                }
            }
        }
        Ok(result)
    }
}
