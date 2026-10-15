//! Catmull–Clark face/edge/vertex rules with persistent hard creases.
use super::*;
impl Mesh {
    pub fn smooth_subdivide(
        &self,
        amount: f32,
        cancel: &impl Fn() -> bool,
    ) -> Result<Self, String> {
        self.validate()?;
        if !amount.is_finite() || !(0. ..=1.).contains(&amount) {
            return Err("Smoothness must be between 0 and 1".into());
        }
        let polygons = self.polygons();
        if polygons.is_empty() {
            return Err("The mesh has no polygons".into());
        }
        let mut incidence: BTreeMap<Edge, Vec<usize>> = BTreeMap::new();
        let mut vertex_faces = vec![Vec::new(); self.vertices.len()];
        for (i, p) in polygons.iter().enumerate() {
            for &v in &p.vertices {
                vertex_faces[v].push(i);
            }
            for (a, b) in p.edges() {
                incidence.entry(edge(a, b)).or_default().push(i);
            }
        }
        boundaries(
            &polygons,
            &vec![Some(0); polygons.len()],
            BorderTarget::Holes,
        )?;
        if self.vertices.len() + incidence.len() + polygons.len() > MAX_VERTICES
            || polygons.iter().map(|p| p.vertices.len() * 2).sum::<usize>() > MAX_TRIANGLES
        {
            return Err("Smooth subdivision exceeds the mesh size limit".into());
        }
        let centers: Vec<_> = polygons
            .iter()
            .map(|p| {
                p.vertices
                    .iter()
                    .fold(Vec3::default(), |sum, &v| sum + self.vertices[v])
                    * (1. / p.vertices.len() as f32)
            })
            .collect();
        let mut out = self.empty_topology();
        let mut neighbors = vec![Vec::new(); self.vertices.len()];
        let mut sharp = vec![Vec::new(); self.vertices.len()];
        let mut midpoint = BTreeMap::new();
        for (&e, faces) in &incidence {
            let [a, b] = e;
            neighbors[a].push(b);
            neighbors[b].push(a);
            let hard = faces.len() == 1 || self.creases.binary_search(&e).is_ok();
            if hard {
                sharp[a].push(b);
                sharp[b].push(a);
            }
            let linear = (self.vertices[a] + self.vertices[b]) * 0.5;
            let smooth = if hard {
                linear
            } else {
                (self.vertices[a] + self.vertices[b] + centers[faces[0]] + centers[faces[1]]) * 0.25
            };
            let m = out.append_blend(
                self,
                &[(a, 0.5), (b, 0.5)],
                linear * (1. - amount) + smooth * amount,
            );
            midpoint.insert(e, m);
            if self.creases.binary_search(&e).is_ok() {
                out.creases.extend([edge(a, m), edge(m, b)]);
            }
        }
        for v in 0..self.vertices.len() {
            if v % 1024 == 0 {
                cancelled(cancel)?;
            }
            let p = self.vertices[v];
            let n = neighbors[v].len();
            let smooth = if sharp[v].len() >= 3 || n < 2 {
                p
            } else if sharp[v].len() == 2 {
                (p * 6. + self.vertices[sharp[v][0]] + self.vertices[sharp[v][1]]) * 0.125
            } else {
                let f = vertex_faces[v]
                    .iter()
                    .fold(Vec3::default(), |sum, &i| sum + centers[i])
                    * (1. / vertex_faces[v].len() as f32);
                let r = neighbors[v].iter().fold(Vec3::default(), |sum, &i| {
                    sum + (p + self.vertices[i]) * 0.5
                }) * (1. / n as f32);
                (f + r * 2. + p * (n as f32 - 3.)) * (1. / n as f32)
            };
            out.vertices[v] = p * (1. - amount) + smooth * amount;
        }
        for (i, p) in polygons.iter().enumerate() {
            if i % 1024 == 0 {
                cancelled(cancel)?;
            }
            let weights: Vec<_> = p
                .vertices
                .iter()
                .map(|&v| (v, 1. / p.vertices.len() as f32))
                .collect();
            let center = out.append_blend(self, &weights, centers[i]);
            let n = p.vertices.len();
            for j in 0..n {
                let v = p.vertices[j];
                out.push_polygon(
                    &[
                        v,
                        midpoint[&edge(v, p.vertices[(j + 1) % n])],
                        center,
                        midpoint[&edge(p.vertices[(j + n - 1) % n], v)],
                    ],
                    p.group,
                    p.hidden,
                );
            }
        }
        out.creases.sort_unstable();
        out.creases.dedup();
        out.validate()?;
        if self.validate_closed().is_ok() {
            out.validate_closed()?;
        }
        cancelled(cancel)?;
        Ok(out)
    }
}
