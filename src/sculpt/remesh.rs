//! Voxel solid reconstruction followed by marching tetrahedra.
//! Scanline winding fills overlapping closed volumes as a union. The grid is
//! padded so extraction never ends on its boundary. No third-party mesh code.
use super::{
    Mesh, Vec3,
    math::closest_triangle,
    subdivision::{MAX_TRIANGLES, MAX_VERTICES},
};
use std::collections::HashMap;

impl Mesh {
    pub fn validate_closed(&self) -> Result<(), String> {
        self.validate()?;
        if self.triangles.is_empty() {
            return Err("Remeshing requires a closed surface with faces".into());
        }
        let mut edges: HashMap<(usize, usize), (u32, i32)> = HashMap::new();
        for &[a, b, c] in &self.triangles {
            if a == b || b == c || a == c {
                return Err("Mesh contains degenerate faces".into());
            }
            for (x, y) in [(a, b), (b, c), (c, a)] {
                let (edge, sign) = if x < y { ((x, y), 1) } else { ((y, x), -1) };
                let entry = edges.entry(edge).or_default();
                entry.0 += 1;
                entry.1 += sign;
            }
        }
        if edges
            .values()
            .any(|&(count, winding)| count != 2 || winding != 0)
        {
            return Err("Remeshing requires a closed, consistently oriented manifold mesh".into());
        }
        Ok(())
    }
    pub fn remesh(&self, resolution: u32) -> Result<Self, String> {
        self.remesh_cancellable(resolution, &|| false)
    }
    pub(crate) fn remesh_cancellable(
        &self,
        resolution: u32,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, String> {
        if !self.creases.is_empty() {
            return Err("Clear creases before voxel remeshing".into());
        }
        if self.has_hidden_faces() {
            return Err("Show all faces before remeshing".into());
        }
        if !(16..=96).contains(&resolution) {
            return Err("Remesh resolution must be between 16 and 96".into());
        }
        self.validate_closed()?;
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for v in &self.vertices {
            for (axis, value) in [v.0, v.1, v.2].into_iter().enumerate() {
                min[axis] = min[axis].min(value);
                max[axis] = max[axis].max(value);
            }
        }
        let longest = (0..3).map(|i| max[i] - min[i]).fold(0f32, f32::max);
        if !longest.is_finite() || longest < 1e-6 {
            return Err("Mesh is too small or its bounds are invalid".into());
        }
        let step = longest / resolution as f32;
        let dims: [usize; 3] =
            std::array::from_fn(|i| ((max[i] - min[i]) / step).ceil() as usize + 5);
        let origin = Vec3(min[0] - step * 2., min[1] - step * 2., min[2] - step * 2.);
        let grid = Grid { dims, origin, step };
        let mut crossings = vec![Vec::<(f32, i32)>::new(); dims[1] * dims[2]];
        // Rasterize each triangle onto yz scanlines instead of testing every ray
        // against every face. A tiny consistent offset avoids shared-edge hits.
        for (index, &[a, b, c]) in self.triangles.iter().enumerate() {
            if index % 2048 == 0 && cancelled() {
                return Err("Mesh operation cancelled".into());
            }
            let [a, b, c] = [self.vertices[a], self.vertices[b], self.vertices[c]];
            let denominator = (b.1 - a.1) * (c.2 - a.2) - (b.2 - a.2) * (c.1 - a.1);
            if denominator.abs() < step * step * 1e-10 {
                continue;
            }
            let y0 =
                (((a.1.min(b.1).min(c.1) - origin.1) / step).floor() as usize).min(dims[1] - 1);
            let y1 = (((a.1.max(b.1).max(c.1) - origin.1) / step).ceil() as usize).min(dims[1] - 1);
            let z0 =
                (((a.2.min(b.2).min(c.2) - origin.2) / step).floor() as usize).min(dims[2] - 1);
            let z1 = (((a.2.max(b.2).max(c.2) - origin.2) / step).ceil() as usize).min(dims[2] - 1);
            for z in z0..=z1 {
                for y in y0..=y1 {
                    let py = origin.1 + (y as f32 + 0.00013) * step - a.1;
                    let pz = origin.2 + (z as f32 + 0.00027) * step - a.2;
                    let u = (py * (c.2 - a.2) - pz * (c.1 - a.1)) / denominator;
                    let v = ((b.1 - a.1) * pz - (b.2 - a.2) * py) / denominator;
                    if u >= 0. && v >= 0. && u + v <= 1. {
                        crossings[y + dims[1] * z].push((
                            a.0 + u * (b.0 - a.0) + v * (c.0 - a.0),
                            if denominator > 0. { 1 } else { -1 },
                        ));
                    }
                }
            }
        }
        let mut inside = vec![false; dims[0] * dims[1] * dims[2]];
        for z in 0..dims[2] {
            if cancelled() {
                return Err("Mesh operation cancelled".into());
            }
            for y in 0..dims[1] {
                let ray = &mut crossings[y + dims[1] * z];
                ray.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
                let mut next = 0;
                let mut winding = 0;
                for x in 0..dims[0] {
                    let px = origin.0 + x as f32 * step;
                    while next < ray.len() && ray[next].0 < px {
                        winding += ray[next].1;
                        next += 1;
                    }
                    inside[grid.index(x, y, z)] = winding != 0;
                }
            }
        }
        drop(crossings);
        // Binary occupancy supplies the union's sign, but it cannot locate the
        // surface between samples. Measure actual distances in a narrow band
        // so extraction interpolates crossings instead of snapping to midpoints.
        let mut distance = vec![step * 3.; inside.len()];
        for (index, &triangle) in self.triangles.iter().enumerate() {
            if index % 1024 == 0 && cancelled() {
                return Err("Mesh operation cancelled".into());
            }
            let [a, b, c] = triangle.map(|i| self.vertices[i]);
            let coordinates = |v: Vec3| [v.0, v.1, v.2];
            let points = [coordinates(a), coordinates(b), coordinates(c)];
            let base = coordinates(origin);
            let lo: [usize; 3] = std::array::from_fn(|i| {
                ((points.iter().map(|p| p[i]).fold(f32::INFINITY, f32::min) - base[i]) / step - 2.)
                    .floor()
                    .max(0.) as usize
            });
            let hi: [usize; 3] = std::array::from_fn(|i| {
                (((points
                    .iter()
                    .map(|p| p[i])
                    .fold(f32::NEG_INFINITY, f32::max)
                    - base[i])
                    / step
                    + 2.)
                    .ceil() as usize)
                    .min(dims[i] - 1)
            });
            for z in lo[2]..=hi[2] {
                if cancelled() {
                    return Err("Mesh operation cancelled".into());
                }
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        let i = grid.index(x, y, z);
                        let p = grid.point(i);
                        let d = (closest_triangle(p, a, b, c) - p).len();
                        distance[i] = distance[i].min(d);
                    }
                }
            }
        }
        let field: Vec<f32> = distance
            .into_iter()
            .zip(&inside)
            .map(|(d, &inner)| {
                // Avoid coincident vertices on different grid edges when the input
                // surface lands exactly on a sample; retain the winding sign.
                let d = d.max(step * 1e-5);
                if inner { -d } else { d }
            })
            .collect();
        let mut mesh = Mesh {
            creases: Vec::new(),
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            quads: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
            mask: Vec::new(),
        };
        let mut edge_vertices = HashMap::new();
        const TETS: [[usize; 4]; 6] = [
            [0, 1, 2, 6],
            [0, 2, 3, 6],
            [0, 3, 7, 6],
            [0, 7, 4, 6],
            [0, 4, 5, 6],
            [0, 5, 1, 6],
        ];
        for z in 0..dims[2] - 1 {
            if cancelled() {
                return Err("Mesh operation cancelled".into());
            }
            for y in 0..dims[1] - 1 {
                for x in 0..dims[0] - 1 {
                    let corners = [
                        grid.index(x, y, z),
                        grid.index(x + 1, y, z),
                        grid.index(x + 1, y + 1, z),
                        grid.index(x, y + 1, z),
                        grid.index(x, y, z + 1),
                        grid.index(x + 1, y, z + 1),
                        grid.index(x + 1, y + 1, z + 1),
                        grid.index(x, y + 1, z + 1),
                    ];
                    let count = corners.iter().filter(|&&i| inside[i]).count();
                    if count == 0 || count == 8 {
                        continue;
                    }
                    for tet in TETS {
                        let ids = tet.map(|i| corners[i]);
                        let inner: Vec<usize> = ids.into_iter().filter(|&i| inside[i]).collect();
                        let outer: Vec<usize> = ids.into_iter().filter(|&i| !inside[i]).collect();
                        if inner.is_empty() || outer.is_empty() {
                            continue;
                        }
                        let mut edge = |a: usize, b: usize| {
                            let key = if a < b { (a, b) } else { (b, a) };
                            *edge_vertices.entry(key).or_insert_with(|| {
                                let i = mesh.vertices.len();
                                let t = field[a] / (field[a] - field[b]);
                                mesh.vertices
                                    .push(grid.point(a) + (grid.point(b) - grid.point(a)) * t);
                                mesh.mask.push(0.);
                                i
                            })
                        };
                        let triangles: Vec<[usize; 3]> = match inner.len() {
                            1 => vec![[
                                edge(inner[0], outer[0]),
                                edge(inner[0], outer[1]),
                                edge(inner[0], outer[2]),
                            ]],
                            3 => vec![[
                                edge(outer[0], inner[0]),
                                edge(outer[0], inner[1]),
                                edge(outer[0], inner[2]),
                            ]],
                            _ => {
                                let [a, b, c, d] = [
                                    edge(inner[0], outer[0]),
                                    edge(inner[0], outer[1]),
                                    edge(inner[1], outer[0]),
                                    edge(inner[1], outer[1]),
                                ];
                                vec![[a, b, c], [b, d, c]]
                            }
                        };
                        let centroid = |indices: &[usize]| {
                            indices
                                .iter()
                                .fold(Vec3::default(), |sum, &i| sum + grid.point(i))
                                * (1. / indices.len() as f32)
                        };
                        let outward = centroid(&outer) - centroid(&inner);
                        for mut triangle in triangles {
                            let [a, b, c] = triangle.map(|i| mesh.vertices[i]);
                            if (b - a).cross(c - a).dot(outward) < 0. {
                                triangle.swap(1, 2);
                            }
                            mesh.triangles.push(triangle);
                        }
                    }
                    if mesh.vertices.len() > MAX_VERTICES || mesh.triangles.len() > MAX_TRIANGLES {
                        return Err("Remesh exceeds mesh limits; choose a lower resolution".into());
                    }
                }
            }
        }
        if mesh.triangles.is_empty() {
            return Err("No surface found at this resolution; increase resolution".into());
        }
        mesh.smooth_remesh();
        self.transfer_groups_to(&mut mesh);
        mesh.validate_closed()?;
        Ok(mesh)
    }
}
struct Grid {
    dims: [usize; 3],
    origin: Vec3,
    step: f32,
}
impl Grid {
    fn index(&self, x: usize, y: usize, z: usize) -> usize {
        x + self.dims[0] * (y + self.dims[1] * z)
    }
    fn point(&self, index: usize) -> Vec3 {
        let x = index % self.dims[0];
        let y = index / self.dims[0] % self.dims[1];
        let z = index / (self.dims[0] * self.dims[1]);
        self.origin + Vec3(x as f32, y as f32, z as f32) * self.step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cube(offset: Vec3) -> Mesh {
        Mesh {
            creases: Vec::new(),
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            quads: Vec::new(),
            vertices: vec![
                Vec3(-1., -1., -1.),
                Vec3(1., -1., -1.),
                Vec3(1., 1., -1.),
                Vec3(-1., 1., -1.),
                Vec3(-1., -1., 1.),
                Vec3(1., -1., 1.),
                Vec3(1., 1., 1.),
                Vec3(-1., 1., 1.),
            ]
            .into_iter()
            .map(|v| v + offset)
            .collect(),
            triangles: vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
            mask: vec![0.8; 8],
        }
    }
    fn components(mesh: &Mesh) -> usize {
        let mut neighbors = vec![Vec::new(); mesh.vertices.len()];
        for &[a, b, c] in &mesh.triangles {
            for (i, j) in [(a, b), (b, c), (c, a)] {
                neighbors[i].push(j);
                neighbors[j].push(i);
            }
        }
        let mut seen = vec![false; mesh.vertices.len()];
        let mut count = 0;
        for i in 0..seen.len() {
            if seen[i] {
                continue;
            }
            count += 1;
            seen[i] = true;
            let mut stack = vec![i];
            while let Some(v) = stack.pop() {
                for &n in &neighbors[v] {
                    if !seen[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
        }
        count
    }
    fn merged(offset: Vec3) -> Mesh {
        let mut mesh = cube(Vec3::default());
        let second = cube(offset);
        let n = mesh.vertices.len();
        mesh.vertices.extend(second.vertices);
        mesh.mask.extend(second.mask);
        mesh.triangles
            .extend(second.triangles.into_iter().map(|t| t.map(|i| i + n)));
        mesh
    }
    #[test]
    fn remesh_produces_closed_outward_surface_and_resolution_changes_density() {
        let cube = cube(Vec3(0.3, -0.2, 0.1));
        cube.validate_closed().unwrap();
        let low = cube.remesh(16).unwrap();
        let high = cube.remesh(32).unwrap();
        low.validate_closed().unwrap();
        high.validate_closed().unwrap();
        assert!(high.vertices.len() > low.vertices.len() * 2);
        assert_eq!(components(&low), 1);
        assert!(low.mask.iter().all(|&m| m == 0.));
        let volume = low
            .triangles
            .iter()
            .map(|&[a, b, c]| low.vertices[a].dot(low.vertices[b].cross(low.vertices[c])) / 6.)
            .sum::<f32>();
        assert!(volume > 6. && volume < 10., "volume={volume}");
    }
    #[test]
    fn remesh_unions_overlapping_shells_and_preserves_disconnected_volumes() {
        let united = merged(Vec3(0.8, 0.2, 0.1)).remesh(24).unwrap();
        united.validate_closed().unwrap();
        assert_eq!(components(&united), 1);
        let separate = merged(Vec3(4., 0., 0.)).remesh(24).unwrap();
        separate.validate_closed().unwrap();
        assert_eq!(components(&separate), 2);
    }
    #[test]
    fn open_invalid_and_cancelled_meshes_are_rejected() {
        let mesh = cube(Vec3::default());
        assert!(mesh.remesh_cancellable(16, &|| true).is_err());
        assert!(mesh.remesh(100).is_err());
        let mut open = mesh;
        open.triangles.pop();
        assert!(open.remesh(16).is_err());
    }
    #[test]
    fn sculpt_sphere_remesh_preserves_scale_and_surface() {
        let sphere = Mesh::sphere();
        let remeshed = sphere.remesh(24).unwrap();
        remeshed.validate_closed().unwrap();
        let average_error = remeshed
            .vertices
            .iter()
            .map(|v| (v.len() - 1.).abs())
            .sum::<f32>()
            / remeshed.vertices.len() as f32;
        assert!(average_error < 0.06, "error={average_error}");
    }
    #[test]
    fn high_resolution_sphere_has_smooth_normals_without_voxel_bands() {
        let mesh = Mesh::sphere().remesh(64).unwrap();
        let normals = mesh.normals();
        let error = mesh
            .vertices
            .iter()
            .zip(normals)
            .map(|(p, n)| 1. - p.norm().dot(n))
            .sum::<f32>()
            / mesh.vertices.len() as f32;
        assert!(error < 0.001, "normal error={error}");
    }
    #[test]
    fn repeated_remeshing_does_not_reintroduce_bands_or_large_shape_drift() {
        let mut mesh = Mesh::sphere();
        for _ in 0..3 {
            mesh = mesh.remesh(48).unwrap();
            mesh.validate_closed().unwrap();
            let normals = mesh.normals();
            let error = mesh
                .vertices
                .iter()
                .zip(normals)
                .map(|(p, n)| 1. - p.norm().dot(n))
                .sum::<f32>()
                / mesh.vertices.len() as f32;
            assert!(error < 0.002, "normal error={error}");
            let radial_error = mesh
                .vertices
                .iter()
                .map(|p| (p.len() - 1.).abs())
                .sum::<f32>()
                / mesh.vertices.len() as f32;
            assert!(radial_error < 0.01, "shape drift={radial_error}");
        }
    }
}
