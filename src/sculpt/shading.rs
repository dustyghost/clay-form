//! Display-only normal fans. Sculpt topology remains welded at hard edges.
use super::{Mesh, Vec3};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct SurfaceShading {
    /// Original mesh vertex and its normal in this smoothing fan.
    pub vertices: Vec<(usize, Vec3)>,
    pub triangles: Vec<[usize; 3]>,
}
fn root(parents: &mut [usize], mut i: usize) -> usize {
    while parents[i] != i {
        parents[i] = parents[parents[i]];
        i = parents[i];
    }
    i
}
fn join(parents: &mut [usize], a: usize, b: usize) {
    let a = root(parents, a);
    let b = root(parents, b);
    if a != b {
        parents[a.max(b)] = a.min(b);
    }
}
impl Mesh {
    /// One offset direction per welded vertex, independent of triangle area.
    pub(crate) fn angle_weighted_normals(&self) -> Vec<Vec3> {
        let mut normals = vec![Vec3::default(); self.vertices.len()];
        for t in &self.triangles {
            let face = (self.vertices[t[1]] - self.vertices[t[0]])
                .cross(self.vertices[t[2]] - self.vertices[t[0]]);
            let length = face.len();
            if length == 0. {
                continue;
            }
            let normal = face * (1. / length);
            for corner in 0..3 {
                let a = self.vertices[t[(corner + 1) % 3]] - self.vertices[t[corner]];
                let b = self.vertices[t[(corner + 2) % 3]] - self.vertices[t[corner]];
                let angle = a.cross(b).len().atan2(a.dot(b));
                normals[t[corner]] = normals[t[corner]] + normal * angle;
            }
        }
        normals.into_iter().map(Vec3::norm).collect()
    }
    pub(crate) fn surface_shading(&self) -> SurfaceShading {
        let mut parents: Vec<_> = (0..self.triangles.len() * 3).collect();
        let face_normals: Vec<_> = self
            .triangles
            .iter()
            .map(|&[a, b, c]| {
                let n = (self.vertices[b] - self.vertices[a])
                    .cross(self.vertices[c] - self.vertices[a]);
                let length = n.len();
                if length > 0. {
                    n * (1. / length)
                } else {
                    Vec3::default()
                }
            })
            .collect();
        let mut edges: HashMap<(usize, usize), (usize, usize, usize)> = HashMap::new();
        for (face, t) in self.triangles.iter().enumerate() {
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let edge = (t[a].min(t[b]), t[a].max(t[b]));
                if let Some((other, x, y)) = edges.remove(&edge) {
                    // Smooth only across consistently oriented, shallow edges.
                    // Sixty degrees keeps a planar cut distinct from curved walls.
                    if self.triangles[other][x] == t[b]
                        && face_normals[face].dot(face_normals[other]) > 0.5
                    {
                        join(&mut parents, face * 3 + a, other * 3 + y);
                        join(&mut parents, face * 3 + b, other * 3 + x);
                    }
                } else {
                    edges.insert(edge, (face, a, b));
                }
            }
        }
        let mut sums = vec![Vec3::default(); parents.len()];
        for (face, t) in self.triangles.iter().enumerate() {
            for corner in 0..3 {
                let a = self.vertices[t[(corner + 1) % 3]] - self.vertices[t[corner]];
                let b = self.vertices[t[(corner + 2) % 3]] - self.vertices[t[corner]];
                // Corner angles, rather than triangle areas, make shading stable
                // when clipping produces slivers or a large cap triangle fan.
                let angle = a.cross(b).len().atan2(a.dot(b));
                let r = root(&mut parents, face * 3 + corner);
                sums[r] = sums[r] + face_normals[face] * angle;
            }
        }
        let mut shading = SurfaceShading::default();
        let mut indices = vec![usize::MAX; parents.len()];
        for (face, t) in self.triangles.iter().enumerate() {
            let mut triangle = [0; 3];
            for corner in 0..3 {
                let r = root(&mut parents, face * 3 + corner);
                if indices[r] == usize::MAX {
                    indices[r] = shading.vertices.len();
                    shading.vertices.push((t[corner], sums[r].norm()));
                }
                triangle[corner] = indices[r];
            }
            shading.triangles.push(triangle);
        }
        shading
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, SliceMode, SliceOptions, SlicePlane};
    #[test]
    fn trimmed_and_separated_caps_do_not_pull_wall_normals_into_the_cap() {
        let source = Mesh::starting_sphere();
        for mode in [SliceMode::Trim, SliceMode::Separate] {
            let (a, b) = source
                .slice(
                    SlicePlane {
                        origin: Vec3(0., 0.23, 0.),
                        normal: Vec3(0., -1., 0.),
                    },
                    SliceOptions {
                        mode,
                        ..Default::default()
                    },
                    &|| false,
                )
                .unwrap();
            for mesh in std::iter::once(a).chain(b) {
                let before = mesh.clone();
                let shading = mesh.surface_shading();
                assert_eq!(mesh, before);
                mesh.validate_closed().unwrap();
                let mut split = false;
                let mut source_normals: HashMap<usize, Vec<Vec3>> = HashMap::new();
                for &(v, n) in &shading.vertices {
                    source_normals.entry(v).or_default().push(n);
                }
                for (fi, tri) in mesh.triangles.iter().enumerate() {
                    let shaded = shading.triangles[fi];
                    assert_eq!(shaded.map(|i| shading.vertices[i].0), *tri);
                    for i in shaded {
                        let (v, n) = shading.vertices[i];
                        if mesh.polygroups[fi] == mesh.vertex_groups.len() {
                            assert!(n.1.abs() > 0.99999, "cap normal must stay planar: {n:?}");
                            if source_normals[&v].iter().any(|other| n.dot(*other) < 0.5) {
                                split = true;
                            }
                        } else {
                            assert!(
                                n.dot(mesh.vertices[v].norm()) > 0.995,
                                "wall normal must follow the sphere: {n:?}"
                            );
                        }
                    }
                }
                assert!(
                    split,
                    "render vertices must split at the cap without disconnecting sculpt topology"
                );
            }
        }
    }
    #[test]
    fn polygroup_slicing_preserves_smooth_normals_despite_thin_cut_triangles() {
        let mesh = Mesh::starting_sphere();
        let before = mesh.surface_shading();
        let (cut, _) = mesh
            .slice(
                SlicePlane {
                    origin: Vec3(0.023, 0., 0.),
                    normal: Vec3(1., 0.31, 0.13),
                },
                SliceOptions::default(),
                &|| false,
            )
            .unwrap();
        let after = cut.surface_shading();
        assert_eq!(
            after.vertices.len(),
            cut.vertices.len(),
            "color boundaries are not hard edges"
        );
        let normals: HashMap<_, _> = after.vertices.iter().copied().collect();
        for (v, n) in before.vertices {
            assert!(n.dot(normals[&v]) > 0.99999);
        }
    }
    #[test]
    fn hard_cube_edges_split_only_display_vertices() {
        let mut primitive = Primitive::for_kind(PrimitiveKind::Cube);
        primitive.divisions = [1; 3];
        let mesh = primitive.mesh().unwrap();
        let shaded = mesh.surface_shading();
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(shaded.vertices.len(), 24);
        for (_, n) in shaded.vertices {
            assert_eq!(n.0.abs() + n.1.abs() + n.2.abs(), 1.);
        }
        mesh.validate_closed().unwrap();
    }
}
