//! Experimental, source-surface retopology. Decimation allocates vertices according
//! to painted density. Path tangents rank convex pairings of the remaining triangles.
use super::{
    Mesh, Vec3,
    paths::{PathSample, SurfacePath},
    uv::edge,
};
use std::collections::{BTreeMap, HashMap};

const MAX_INPUT: usize = 24_000;
pub(crate) type GuideCurves = std::sync::Arc<[Vec<PathSample>]>;

pub(crate) struct Layout {
    pub mesh: Mesh,
    pub polygons: usize,
    pub guides: GuideCurves,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Options {
    pub target: usize,
    pub flow: f32,
    pub density: f32,
    pub mirror_path_x: bool,
    pub preserve_sharp: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            target: 500,
            flow: 1.,
            density: 1.,
            mirror_path_x: false,
            preserve_sharp: true,
        }
    }
}
fn check(cancel: &dyn Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Retopology cancelled".into())
    } else {
        Ok(())
    }
}

type EdgeFaces = BTreeMap<[usize; 2], (usize, Option<usize>)>;
struct Surface {
    edges: EdgeFaces,
    normals: Vec<Vec3>,
}
impl Surface {
    fn new(mesh: &Mesh, cancel: &impl Fn() -> bool) -> Result<Self, String> {
        let mut edges = EdgeFaces::new();
        let mut normals = Vec::with_capacity(mesh.triangles.len());
        let mut incident = vec![Vec::new(); mesh.vertices.len()];
        for (face, &[a, b, c]) in mesh.triangles.iter().enumerate() {
            if face % 256 == 0 {
                check(cancel)?;
            }
            let [p, q, r] = [a, b, c].map(|v| mesh.vertices[v]);
            let n = (q - p).cross(r - p);
            if !n.len().is_finite() || n.len() <= ((q - p).len() * (r - p).len() * 1e-6).max(1e-12)
            {
                return Err("Retopology needs nondegenerate triangles".into());
            }
            normals.push(n * (1. / n.len()));
            for (x, y, z) in [(a, b, c), (b, c, a), (c, a, b)] {
                incident[x].push(face);
                let (first, second) = edges.entry(edge(x, y)).or_insert((face, None));
                if *first != face {
                    let t = mesh.triangles[*first];
                    if second.is_some()
                        || t.contains(&z)
                        || !(0..3).any(|j| t[j] == y && t[(j + 1) % 3] == x)
                    {
                        return Err(
                            "Retopology requires consistently oriented manifold faces".into()
                        );
                    }
                    *second = Some(face);
                }
            }
        }
        // Edge incidence alone misses two disks/shells touching at a single vertex.
        let mut visited = vec![usize::MAX; mesh.triangles.len()];
        let mut stack = Vec::new();
        for (v, fan) in incident.iter().enumerate() {
            if v % 256 == 0 {
                check(cancel)?;
            }
            if fan.is_empty() || fan.len() > 64 {
                return Err(
                    "Remove loose vertices or reduce vertex valence to 64 for this prototype"
                        .into(),
                );
            }
            stack.push(fan[0]);
            let mut count = 0;
            while let Some(f) = stack.pop() {
                if visited[f] == v {
                    continue;
                }
                visited[f] = v;
                count += 1;
                for w in mesh.triangles[f] {
                    if w != v {
                        let (a, b) = edges[&edge(v, w)];
                        if let Some(b) = b {
                            stack.push(if a == f { b } else { a });
                        }
                    }
                }
            }
            if count != fan.len() {
                return Err("Retopology found a nonmanifold vertex fan".into());
            }
        }
        Ok(Self { edges, normals })
    }
    fn feature(&self, mesh: &Mesh, e: [usize; 2], a: usize, b: usize, sharp: bool) -> bool {
        mesh.creases.binary_search(&e).is_ok()
            || mesh.polygroups.get(a) != mesh.polygroups.get(b)
            || (sharp && self.normals[a].dot(self.normals[b]) < std::f32::consts::FRAC_1_SQRT_2)
    }
}
impl Mesh {
    pub(crate) fn require_retopology_input(&self) -> Result<(), String> {
        if self.triangles.is_empty()
            || self.triangles.len() > MAX_INPUT
            || self.vertices.len() > 3 * MAX_INPUT
        {
            return Err(format!(
                "The prototype supports 1–{MAX_INPUT} source triangles with no loose vertices"
            ));
        }
        if !self.uv.is_empty() {
            return Err("Use a source without UVs or textures for guided retopology".into());
        }
        Ok(())
    }
    pub(crate) fn guided_retopology(
        &self,
        options: Options,
        path: Option<&SurfacePath>,
        cancel: &impl Fn() -> bool,
    ) -> Result<Layout, String> {
        check(cancel)?;
        self.require_retopology_input()?;
        self.validate()?;
        if self.has_hidden_faces() {
            return Err("Show all faces before guided retopology".into());
        }
        if options.target == 0
            || options.target > self.triangles.len().div_ceil(2)
            || ![options.flow, options.density]
                .iter()
                .all(|x| (0. ..=1.).contains(x))
        {
            return Err("Invalid retopology target or constraint strength".into());
        }
        let mut surface = Surface::new(self, cancel)?;
        let (mut lo, mut hi) = (self.vertices[0], self.vertices[0]);
        for &Vec3(x, y, z) in &self.vertices {
            lo = Vec3(lo.0.min(x), lo.1.min(y), lo.2.min(z));
            hi = Vec3(hi.0.max(x), hi.1.max(y), hi.2.max(z));
        }
        let radius = (hi - lo).len() * 0.12;
        let mut guides = Vec::new();
        if let Some(path) = path.filter(|_| options.flow > 0.) {
            if path.mode != super::paths::PathMode::Surface {
                return Err("Select a saved surface path".into());
            }
            let mut samples = path.samples_on_mesh_cancelled(self, radius / 8., cancel)?;
            if path.closed {
                samples.push(samples[0]);
            }
            if options.mirror_path_x {
                let mirror = |v: Vec3| Vec3(-v.0, v.1, v.2);
                guides.push(
                    samples
                        .iter()
                        .map(|s| PathSample {
                            position: mirror(s.position),
                            normal: mirror(s.normal),
                            tangent: mirror(s.tangent),
                            binormal: mirror(s.binormal) * (-1.),
                            ..*s
                        })
                        .collect(),
                );
            }
            guides.push(samples);
        }
        let mut mesh = self.clone();
        mesh.quads.clear();
        // Leave some triangles unmatched at poles, group boundaries and sharp edges.
        let triangle_target = (options.target * 17 / 10).max(4);
        for _ in 0..24 {
            let limit = mesh.triangles.len().saturating_sub(triangle_target) / 2;
            if limit == 0 || !mesh.reduce_for_layout(&surface, options, limit, cancel)? {
                break;
            }
            surface = Surface::new(&mesh, cancel)?;
        }
        mesh.pair_layout_triangles(&surface, options, &guides, radius, cancel)?;
        check(cancel)?;
        mesh.validate()?;
        Ok(Layout {
            guides: guides.into(),
            polygons: mesh.triangles.len() - mesh.quads.len(),
            mesh,
        })
    }
    fn pair_layout_triangles(
        &mut self,
        surface: &Surface,
        options: Options,
        guides: &[Vec<PathSample>],
        radius: f32,
        cancel: &impl Fn() -> bool,
    ) -> Result<(), String> {
        let mut pairs = Vec::new();
        if surface
            .edges
            .len()
            .saturating_mul(guides.iter().map(Vec::len).sum())
            > 16_000_000
        {
            return Err(
                "Guide search limit reached. Shorten the guide or reduce the input mesh".into(),
            );
        }
        for (i, (&e, &(a, b))) in surface.edges.iter().enumerate() {
            if i % 128 == 0 {
                check(cancel)?;
            }
            let Some(b) = b else { continue };
            if surface.feature(self, e, a, b, options.preserve_sharp) {
                continue;
            }
            let t = self.triangles[a];
            let k = t.iter().position(|v| !e.contains(v)).unwrap();
            let other = *self.triangles[b].iter().find(|v| !e.contains(v)).unwrap();
            let quad = [t[(k + 2) % 3], t[k], t[(k + 1) % 3], other];
            let p = quad.map(|v| self.vertices[v]);
            let normal = (surface.normals[a] + surface.normals[b]).norm();
            let sides: [Vec3; 4] = std::array::from_fn(|j| (p[(j + 1) % 4] - p[j]).norm());
            let quality = (0..4)
                .map(|j| sides[j].cross(sides[(j + 1) % 4]).dot(normal))
                .fold(1., f32::min);
            if quality < 0.25 || surface.normals[a].dot(surface.normals[b]) < 0.5 {
                continue;
            }
            let center = (p[0] + p[1] + p[2] + p[3]) * 0.25;
            let nearest = guides
                .iter()
                .flatten()
                .filter(|s| s.normal.dot(normal) > 0.5)
                .map(|s| ((center - s.position).len() / radius, s.tangent))
                .filter(|(d, _)| *d < 1.)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            // Suppress a diagonal crossing the desired flow. Reward an opposite
            // pair of sides that both track the tangent, independent of path direction.
            let flow = nearest.map_or(0., |(d, t)| {
                let axis = |j: usize| sides[j].dot(t).abs().min(sides[j + 2].dot(t).abs());
                let diagonal = (p[2] - p[0]).norm().dot(t).abs();
                (1. - d) * (axis(0).max(axis(1)) - diagonal)
            });
            pairs.push((quality + 3. * options.flow * flow, a, b, quad));
        }
        pairs.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        let mut used = vec![false; self.triangles.len()];
        for (_, a, b, quad) in pairs {
            if self.triangles.len() - self.quads.len() <= options.target {
                break;
            }
            if !used[a] && !used[b] {
                self.quads.push(quad);
                used[a] = true;
                used[b] = true;
            }
        }
        Ok(())
    }
    fn reduce_for_layout(
        &mut self,
        surface: &Surface,
        options: Options,
        limit: usize,
        cancel: &impl Fn() -> bool,
    ) -> Result<bool, String> {
        let mut locked = vec![false; self.vertices.len()];
        let mut candidates = Vec::new();
        for (&e, &(a, b)) in &surface.edges {
            let [x, y] = e;
            if b.is_none_or(|b| surface.feature(self, e, a, b, options.preserve_sharp)) {
                locked[x] = true;
                locked[y] = true;
            } else {
                let length = (self.vertices[x] - self.vertices[y]).len();
                let heat = (self.mask[x] + self.mask[y]) * 0.5;
                candidates.push((length * (1. + 4. * options.density * heat), e, length));
            }
        }
        candidates.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let (neighbors, incident) = self.collapse_adjacency(&vec![true; self.vertices.len()]);
        let mut merged = HashMap::new();
        // Rebuild adjacency when a cool region is blocked for this sweep.
        // Comparing paint alone here avoids starving larger edges near a pole.
        let mut sweep_heat = f32::INFINITY;
        for (i, (_, e, length)) in candidates.into_iter().enumerate() {
            if i % 64 == 0 {
                check(cancel)?;
            }
            let [a, b] = e;
            let heat = options.density * (self.mask[a] + self.mask[b]) * 0.5;
            if heat > sweep_heat
                || locked[a]
                || locked[b]
                || !self.safe_edge_collapse(e, &neighbors, &incident, length, true)
            {
                continue;
            }
            if merged.is_empty() {
                sweep_heat = heat + 0.4;
            }
            merged.insert(b, a);
            // Decimate's fold test includes adjacent faces. Keep their positions
            // fixed until the batch has been installed and adjacency rebuilt.
            for &v in neighbors[a].iter().chain(&neighbors[b]) {
                locked[v] = true;
                for &w in &neighbors[v] {
                    locked[w] = true;
                }
            }
            if merged.len() >= limit {
                break;
            }
        }
        if merged.is_empty() {
            return Ok(false);
        }
        self.apply_edge_collapses(&merged);
        Ok(true)
    }
}

#[cfg(test)]
pub(crate) mod tests;
