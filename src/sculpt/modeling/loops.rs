//! Oriented quad strips and atomic sets of symmetry-related loop cuts.
use super::*;
pub struct LoopCut {
    pub mesh: Mesh,
    pub cuts: Vec<[Vec3; 2]>,
}
impl LoopCut {
    pub const MAX_COUNT: u32 = 16;
    pub fn center(count: u32, position: f32) -> f32 {
        let half_span = (count.max(1) - 1) as f32 / (2. * (count.max(1) + 1) as f32);
        position.clamp(half_span + 0.01, 1. - half_span - 0.01)
    }
}
impl Mesh {
    pub fn insert_edge_loop(&self, seed: [usize; 2]) -> Result<Mesh, String> {
        self.insert_edge_loops(seed, 1, 0.5).map(|cut| cut.mesh)
    }
    pub fn insert_edge_loops(
        &self,
        seed: [usize; 2],
        count: u32,
        position: f32,
    ) -> Result<LoopCut, String> {
        self.insert_loop_copies(&[seed], count, position)
    }
    pub fn insert_symmetric_edge_loops(
        &self,
        seed: [usize; 2],
        count: u32,
        position: f32,
        symmetry: &ModelingSymmetry,
    ) -> Result<LoopCut, String> {
        let result = self.insert_loop_copies(&symmetry.edge_copies(seed), count, position)?;
        symmetry.validate_result(&result.mesh)?;
        Ok(result)
    }
    fn insert_loop_copies(
        &self,
        seeds: &[[usize; 2]],
        count: u32,
        position: f32,
    ) -> Result<LoopCut, String> {
        if !(1..=LoopCut::MAX_COUNT).contains(&count) || !position.is_finite() {
            return Err("Choose 1–16 loops and a finite position".into());
        }
        manifold(self)?;
        let center = LoopCut::center(count, position);
        let parameters: Vec<_> = (0..count)
            .map(|i| center + (i as f32 - (count - 1) as f32 * 0.5) / (count + 1) as f32)
            .collect();
        let polygons = self.polygons();
        let mut incidence = BTreeMap::<_, Vec<_>>::new();
        for (i, p) in polygons.iter().enumerate() {
            for (a, b) in p.edges() {
                incidence.entry(edge(a, b)).or_default().push(i);
            }
        }
        let mut splits = BTreeMap::<[usize; 2], Vec<f32>>::new();
        for seed in seeds {
            let key = edge(seed[0], seed[1]);
            if !incidence.contains_key(&key) {
                return Err("Choose one polygon edge for a loop cut".into());
            }
            let mut starts = BTreeMap::from([(key, seed[0])]);
            let mut pending = vec![key];
            while let Some(e) = pending.pop() {
                for &i in &incidence[&e] {
                    let p = &polygons[i];
                    if !p.visible || p.vertices.len() != 4 {
                        return Err("Loop cuts need an uninterrupted strip of visible quads".into());
                    }
                    let edges: Vec<_> = p.edges().map(|(a, b)| edge(a, b)).collect();
                    let k = edges.iter().position(|&x| x == e).unwrap();
                    let opposite = edges[(k + 2) % 4];
                    let start =
                        p.vertices[(k + if starts[&e] == p.vertices[k] { 3 } else { 2 }) % 4];
                    if let Some(&existing) = starts.get(&opposite) {
                        if existing != start {
                            return Err(
                                "The quad strip twists back onto itself; choose another edge"
                                    .into(),
                            );
                        }
                    } else {
                        starts.insert(opposite, start);
                        pending.push(opposite);
                    }
                }
            }
            if polygons.iter().any(|p| {
                p.edges()
                    .filter(|&(a, b)| starts.contains_key(&edge(a, b)))
                    .count()
                    > 2
            }) {
                return Err("The loop crosses itself; choose another edge".into());
            }
            for (e, a) in starts {
                splits.entry(e).or_default().extend(
                    parameters
                        .iter()
                        .map(|&t| if e[0] == a { t } else { 1. - t }),
                );
            }
        }
        for ts in splits.values_mut() {
            ts.sort_by(f32::total_cmp);
            ts.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
        }
        let params = |a, b| {
            let mut ts = splits.get(&edge(a, b)).cloned().unwrap_or_default();
            if a > b {
                ts = ts.into_iter().rev().map(|t| 1. - t).collect();
            }
            ts
        };
        let mut extra = splits.values().map(Vec::len).sum::<usize>();
        let mut triangles = 0;
        for p in &polygons {
            if p.vertices.len() != 4 {
                triangles += 1;
                continue;
            }
            let v = &p.vertices;
            let u = params(v[0], v[1]);
            let vv = params(v[0], v[3]);
            let same = |a: &[f32], b: &[f32]| {
                a.len() == b.len() && a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5)
            };
            if !same(&u, &params(v[3], v[2])) || !same(&vv, &params(v[1], v[2])) {
                return Err("Symmetric loop parameters disagree across a face".into());
            }
            extra += u.len() * vv.len();
            triangles += 2 * (u.len() + 1) * (vv.len() + 1);
        }
        if self.vertices.len() + extra > MAX_VERTICES || triangles > MAX_TRIANGLES {
            return Err("Loop cuts would exceed the mesh size limit".into());
        }
        let mut out = self.empty_topology();
        let mids: BTreeMap<_, Vec<_>> = splits
            .iter()
            .map(|(&[a, b], ts)| {
                (
                    edge(a, b),
                    ts.iter()
                        .map(|&t| {
                            out.append_blend(
                                self,
                                &[(a, 1. - t), (b, t)],
                                self.vertices[a] * (1. - t) + self.vertices[b] * t,
                            )
                        })
                        .collect(),
                )
            })
            .collect();
        let chain = |a, b| {
            let mut vs = mids.get(&edge(a, b)).cloned().unwrap_or_default();
            if a > b {
                vs.reverse();
            }
            std::iter::once(a).chain(vs).chain([b]).collect::<Vec<_>>()
        };
        let mut cuts = Vec::new();
        for p in &polygons {
            if p.vertices.len() != 4 || !p.edges().any(|(a, b)| splits.contains_key(&edge(a, b))) {
                out.push_polygon(&p.vertices, p.group, p.hidden);
                continue;
            }
            let v = &p.vertices;
            let u = params(v[0], v[1]);
            let vv = params(v[0], v[3]);
            let nx = u.len() + 2;
            let ny = vv.len() + 2;
            let bottom = chain(v[0], v[1]);
            let top = chain(v[3], v[2]);
            let left = chain(v[0], v[3]);
            let right = chain(v[1], v[2]);
            let mut grid = vec![vec![0; nx]; ny];
            for j in 0..ny {
                for i in 0..nx {
                    grid[j][i] = if j == 0 {
                        bottom[i]
                    } else if j == ny - 1 {
                        top[i]
                    } else if i == 0 {
                        left[j]
                    } else if i == nx - 1 {
                        right[j]
                    } else {
                        let (u, t) = (u[i - 1], vv[j - 1]);
                        let weights = [
                            (v[0], (1. - u) * (1. - t)),
                            (v[1], u * (1. - t)),
                            (v[2], u * t),
                            (v[3], (1. - u) * t),
                        ];
                        let position = weights
                            .iter()
                            .fold(Vec3::default(), |sum, &(v, w)| sum + self.vertices[v] * w);
                        out.append_blend(self, &weights, position)
                    };
                }
            }
            for j in 0..ny - 1 {
                for i in 0..nx - 1 {
                    out.push_polygon(
                        &[
                            grid[j][i],
                            grid[j][i + 1],
                            grid[j + 1][i + 1],
                            grid[j + 1][i],
                        ],
                        p.group,
                        p.hidden,
                    );
                    if i > 0 {
                        cuts.push([out.vertices[grid[j][i]], out.vertices[grid[j + 1][i]]]);
                    }
                    if j > 0 {
                        cuts.push([out.vertices[grid[j][i]], out.vertices[grid[j][i + 1]]]);
                    }
                }
            }
        }
        for &[a, b] in &self.creases {
            out.creases
                .extend(chain(a, b).windows(2).map(|p| edge(p[0], p[1])));
        }
        out.retain_creases();
        manifold(&out)?;
        Ok(LoopCut { mesh: out, cuts })
    }
}
