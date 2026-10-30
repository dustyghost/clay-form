//! Display-only normal fans. Sculpt topology remains welded at hard edges.
use super::{Mesh, Vec3};
use std::collections::HashMap;

#[derive(Clone, Default)]
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
        let normals = face_normals(self);
        // Uncached surfaces join edges directly, without allocating an adjacency list.
        SurfaceShadingTopology::for_each_pair(self, |a, x| {
            join_smooth_edge(&mut parents, &normals, a, x);
        });
        finish_shading(self, parents, &normals, None)
    }
}

/// Edge adjacency independent of vertex positions. Reuse only while triangle
/// indices and explicit creases are unchanged; sharpness is evaluated on every shade.
pub(crate) struct SurfaceShadingTopology {
    paired_corners: Vec<[usize; 2]>,
    angles: CornerAngles,
}

/// A fixed reference snapshot, shared with the topology across stroke frames.
/// Changed faces recompute weights; old weights never approximate new geometry.
struct CornerAngles {
    positions: Vec<Vec3>,
    values: Vec<[f32; 3]>,
}
impl CornerAngles {
    fn new(mesh: &Mesh) -> Self {
        Self {
            positions: mesh.vertices.clone(),
            values: mesh
                .triangles
                .iter()
                .map(|t| corner_angles(mesh, t))
                .collect(),
        }
    }
    fn unchanged(&self, mesh: &Mesh, triangle: &[usize; 3]) -> bool {
        triangle.iter().all(|&i| {
            let a = self.positions[i];
            let b = mesh.vertices[i];
            a.0.to_bits() == b.0.to_bits()
                && a.1.to_bits() == b.1.to_bits()
                && a.2.to_bits() == b.2.to_bits()
        })
    }
    fn get(&self, mesh: &Mesh, face: usize, triangle: &[usize; 3]) -> [f32; 3] {
        if self.unchanged(mesh, triangle) {
            self.values[face]
        } else {
            corner_angles(mesh, triangle)
        }
    }
}
fn corner_angles(mesh: &Mesh, t: &[usize; 3]) -> [f32; 3] {
    std::array::from_fn(|corner| {
        let a = mesh.vertices[t[(corner + 1) % 3]] - mesh.vertices[t[corner]];
        let b = mesh.vertices[t[(corner + 2) % 3]] - mesh.vertices[t[corner]];
        a.cross(b).len().atan2(a.dot(b))
    })
}
impl SurfaceShadingTopology {
    pub(crate) fn new(mesh: &Mesh) -> Self {
        let mut paired_corners = Vec::new();
        Self::for_each_pair(mesh, |a, x| paired_corners.push([a, x]));
        Self {
            paired_corners,
            angles: CornerAngles::new(mesh),
        }
    }

    fn for_each_pair(mesh: &Mesh, mut pair: impl FnMut(usize, usize)) {
        let mut edges: HashMap<(usize, usize), (usize, usize, usize)> = HashMap::new();
        for (face, t) in mesh.triangles.iter().enumerate() {
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let edge = (t[a].min(t[b]), t[a].max(t[b]));
                if let Some((other, x, _)) = edges.remove(&edge) {
                    if mesh.creases.binary_search(&[edge.0, edge.1]).is_err()
                        && mesh.triangles[other][x] == t[b]
                    {
                        pair(face * 3 + a, other * 3 + x);
                    }
                } else {
                    edges.insert(edge, (face, a, b));
                }
            }
        }
    }

    pub(crate) fn shade(&self, mesh: &Mesh) -> SurfaceShading {
        let mut parents: Vec<_> = (0..mesh.triangles.len() * 3).collect();
        let normals = face_normals(mesh);
        for &[a, x] in &self.paired_corners {
            join_smooth_edge(&mut parents, &normals, a, x);
        }
        finish_shading(mesh, parents, &normals, Some(&self.angles))
    }
}

fn face_normals(mesh: &Mesh) -> Vec<Vec3> {
    mesh.triangles
        .iter()
        .map(|&[a, b, c]| {
            let n =
                (mesh.vertices[b] - mesh.vertices[a]).cross(mesh.vertices[c] - mesh.vertices[a]);
            let length = n.len();
            if length > 0. {
                n * (1. / length)
            } else {
                Vec3::default()
            }
        })
        .collect()
}

fn join_smooth_edge(parents: &mut [usize], normals: &[Vec3], a: usize, x: usize) {
    let face = a / 3;
    let other = x / 3;
    // Recheck the sixty-degree threshold after deformation, including
    // edges that were sharp before the stroke and are now smooth.
    if normals[face].dot(normals[other]) > 0.5 {
        let b = face * 3 + (a % 3 + 1) % 3;
        let y = other * 3 + (x % 3 + 1) % 3;
        join(parents, a, y);
        join(parents, b, x);
    }
}

fn finish_shading(
    mesh: &Mesh,
    mut parents: Vec<usize>,
    normals: &[Vec3],
    angles: Option<&CornerAngles>,
) -> SurfaceShading {
    let (sums, fan_count) = accumulate_normals(mesh, &mut parents, normals, angles);
    emit_shading(mesh, &mut parents, &sums, fan_count)
}

fn accumulate_normals(
    mesh: &Mesh,
    parents: &mut [usize],
    normals: &[Vec3],
    angles: Option<&CornerAngles>,
) -> (Vec<Vec3>, usize) {
    let mut sums = vec![Vec3::default(); parents.len()];
    let mut fan_count = 0;
    for (face, t) in mesh.triangles.iter().enumerate() {
        // Keep exact corner weights and accumulation order, including slivers
        // and degenerate faces. Only the unchanged reference faces skip atan2.
        let weights =
            angles.map_or_else(|| corner_angles(mesh, t), |cache| cache.get(mesh, face, t));
        for (corner, angle) in weights.into_iter().enumerate() {
            let id = face * 3 + corner;
            let r = root(parents, id);
            // Joining always chooses the smallest corner as root. Flatten once
            // while accumulating; emission can reuse this table for output IDs.
            parents[id] = r;
            fan_count += usize::from(r == id);
            sums[r] = sums[r] + normals[face] * angle;
        }
    }
    (sums, fan_count)
}

fn emit_shading(
    mesh: &Mesh,
    parents: &mut [usize],
    sums: &[Vec3],
    fan_count: usize,
) -> SurfaceShading {
    let mut shading = SurfaceShading {
        vertices: Vec::with_capacity(fan_count),
        triangles: Vec::with_capacity(mesh.triangles.len()),
    };
    for (face, t) in mesh.triangles.iter().enumerate() {
        let mut triangle = [0; 3];
        for corner in 0..3 {
            let id = face * 3 + corner;
            let r = parents[id];
            debug_assert!(r <= id);
            // A root is encountered before every other corner in its fan.
            // Its slot already contains the emitted index for later corners.
            let index = if r == id {
                let index = shading.vertices.len();
                shading.vertices.push((t[corner], sums[r].norm()));
                index
            } else {
                parents[r]
            };
            parents[id] = index;
            triangle[corner] = index;
        }
        shading.triangles.push(triangle);
    }
    debug_assert_eq!(shading.vertices.len(), fan_count);
    shading
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind, SliceMode, SliceOptions, SlicePlane};

    // Frozen pre-cache implementation: exact fan ordering and normals are part
    // of the contract, including open, degenerate and non-manifold input.
    fn reference_shading(mesh: &Mesh) -> SurfaceShading {
        let mut parents: Vec<_> = (0..mesh.triangles.len() * 3).collect();
        let face_normals: Vec<_> = mesh
            .triangles
            .iter()
            .map(|&[a, b, c]| {
                let n = (mesh.vertices[b] - mesh.vertices[a])
                    .cross(mesh.vertices[c] - mesh.vertices[a]);
                let length = n.len();
                if length > 0. {
                    n * (1. / length)
                } else {
                    Vec3::default()
                }
            })
            .collect();
        let mut edges: HashMap<(usize, usize), (usize, usize, usize)> = HashMap::new();
        for (face, t) in mesh.triangles.iter().enumerate() {
            for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                let edge = (t[a].min(t[b]), t[a].max(t[b]));
                if let Some((other, x, y)) = edges.remove(&edge) {
                    // Smooth only across consistently oriented, shallow edges.
                    // Sixty degrees keeps a planar cut distinct from curved walls.
                    if mesh.creases.binary_search(&[edge.0, edge.1]).is_err()
                        && mesh.triangles[other][x] == t[b]
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
        for (face, t) in mesh.triangles.iter().enumerate() {
            for corner in 0..3 {
                let a = mesh.vertices[t[(corner + 1) % 3]] - mesh.vertices[t[corner]];
                let b = mesh.vertices[t[(corner + 2) % 3]] - mesh.vertices[t[corner]];
                // Corner angles, rather than triangle areas, make shading stable
                // when clipping produces slivers or a large cap triangle fan.
                let angle = a.cross(b).len().atan2(a.dot(b));
                let r = root(&mut parents, face * 3 + corner);
                sums[r] = sums[r] + face_normals[face] * angle;
            }
        }
        let mut shading = SurfaceShading::default();
        let mut indices = vec![usize::MAX; parents.len()];
        for (face, t) in mesh.triangles.iter().enumerate() {
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

    #[test]
    fn shading_fans_keep_reference_order_for_reordered_disconnected_and_empty_faces() {
        let mut mesh = Mesh {
            vertices: vec![
                Vec3(9., 9., 9.),
                Vec3(0., 0., 0.),
                Vec3(1., 0., 0.),
                Vec3(0., 1., 0.),
                Vec3(0., -1., 0.),
                Vec3(2., 0., 0.),
                Vec3(3., 0., 0.),
                Vec3(2., 1., 0.),
            ],
            triangles: vec![[1, 2, 3], [2, 1, 4], [5, 6, 7], [1, 1, 2]],
            ..Default::default()
        };
        for pass in 0..6 {
            if pass == 5 {
                mesh.triangles.clear();
            } else {
                mesh.triangles.rotate_left(1);
            }
            let expected = reference_shading(&mesh);
            for actual in [
                mesh.surface_shading(),
                SurfaceShadingTopology::new(&mesh).shade(&mesh),
            ] {
                assert_eq!(actual.triangles, expected.triangles);
                assert_eq!(actual.vertices.len(), expected.vertices.len());
                for ((i, a), (j, b)) in actual.vertices.iter().zip(&expected.vertices) {
                    assert_eq!(i, j);
                    assert_eq!(
                        [a.0.to_bits(), a.1.to_bits(), a.2.to_bits()],
                        [b.0.to_bits(), b.1.to_bits(), b.2.to_bits()]
                    );
                }
            }
        }
    }

    #[test]
    fn cached_corner_angles_match_reference_after_local_global_and_restored_edits() {
        let original = Mesh::starting_sphere();
        let topology = SurfaceShadingTopology::new(&original);
        let mut mesh = original.clone();
        assert_eq!(topology.angle_cache_stats(&mesh).1, mesh.triangles.len());
        for edit in 0..5 {
            match edit {
                0 => mesh.vertices[0].2 += 0.13,
                1 => mesh.vertices[7].0 -= 0.28,
                2 => {
                    for p in &mut mesh.vertices {
                        p.1 += 0.07;
                    }
                }
                3 => mesh = original.clone(),
                _ => {
                    let p = mesh.vertices.iter_mut().find(|p| p.0 == 0.).unwrap();
                    p.0 = if p.0.to_bits() == 0 { -0. } else { 0. };
                }
            }
            let unchanged = topology.angle_cache_stats(&mesh).1;
            match edit {
                0 | 1 | 4 => assert!(unchanged > 0 && unchanged < mesh.triangles.len()),
                2 => assert_eq!(unchanged, 0),
                _ => assert_eq!(unchanged, mesh.triangles.len()),
            }
            let expected = reference_shading(&mesh);
            let actual = topology.shade(&mesh);
            assert_eq!(actual.triangles, expected.triangles);
            assert_eq!(actual.vertices.len(), expected.vertices.len());
            for ((i, a), (j, b)) in actual.vertices.iter().zip(&expected.vertices) {
                assert_eq!(i, j);
                assert_eq!(
                    [a.0.to_bits(), a.1.to_bits(), a.2.to_bits()],
                    [b.0.to_bits(), b.1.to_bits(), b.2.to_bits()]
                );
            }
        }
    }

    #[test]
    fn cached_shading_matches_reference_across_edge_cases() {
        let mut cube = Primitive::for_kind(PrimitiveKind::Cube);
        cube.divisions = [2; 3];
        let mut meshes = vec![Mesh::starting_sphere(), cube.mesh().unwrap()];
        let mut open = meshes[1].clone();
        open.triangles.truncate(7);
        meshes.push(open);
        let mut irregular = meshes[1].clone();
        irregular.triangles.extend_from_within(..3);
        irregular.triangles.push([0, 0, 1]);
        irregular.triangles.push([0, 1, 0]);
        meshes.push(irregular);
        let mut creased = meshes[0].clone();
        creased.creases = creased.polygon_edges().into_iter().take(20).collect();
        creased.creases.sort_unstable();
        meshes.push(creased);
        for mut mesh in meshes {
            let topology = SurfaceShadingTopology::new(&mesh);
            for offset in [0., 0.8, -0.4, 0.] {
                mesh.vertices[0].2 = offset;
                let expected = reference_shading(&mesh);
                let actual = topology.shade(&mesh);
                assert_eq!(actual.triangles, expected.triangles);
                assert_eq!(actual.vertices, expected.vertices);
                let uncached = mesh.surface_shading();
                assert_eq!(uncached.triangles, expected.triangles);
                assert_eq!(uncached.vertices, expected.vertices);
            }
        }
    }

    #[test]
    fn cached_shading_splits_and_merges_when_edge_crosses_sharpness_threshold() {
        let mut mesh = Mesh {
            vertices: vec![
                Vec3(0., 0., 0.),
                Vec3(1., 0., 0.),
                Vec3(0., 1., 0.),
                Vec3(0., -1., 0.),
            ],
            triangles: vec![[0, 1, 2], [1, 0, 3]],
            ..Default::default()
        };
        let topology = SurfaceShadingTopology::new(&mesh);
        for (height, fans) in [(0., 4), (2., 6), (0., 4), (-2., 6), (0., 4)] {
            mesh.vertices[3].2 = height;
            let shaded = topology.shade(&mesh);
            assert_eq!(shaded.vertices.len(), fans);
            let reference = reference_shading(&mesh);
            assert_eq!(shaded.triangles, reference.triangles);
            assert_eq!(shaded.vertices, reference.vertices);
        }
    }
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

#[cfg(test)]
impl SurfaceShadingTopology {
    pub(crate) fn profile_shade(
        &self,
        mesh: &Mesh,
        cached: bool,
    ) -> (SurfaceShading, Vec<(&'static str, f64)>) {
        fn timed<T>(
            out: &mut Vec<(&'static str, f64)>,
            name: &'static str,
            f: impl FnOnce() -> T,
        ) -> T {
            let start = std::time::Instant::now();
            let result = f();
            out.push((name, start.elapsed().as_secs_f64() * 1000.));
            result
        }
        let mut times = Vec::new();
        let mut parents: Vec<_> = timed(&mut times, "shading.parents", || {
            (0..mesh.triangles.len() * 3).collect()
        });
        let normals = timed(&mut times, "shading.face_normals", || face_normals(mesh));
        timed(&mut times, "shading.join_fans", || {
            for &[a, x] in &self.paired_corners {
                join_smooth_edge(&mut parents, &normals, a, x);
            }
        });
        let (sums, fan_count) = timed(&mut times, "shading.angle_sums", || {
            accumulate_normals(mesh, &mut parents, &normals, cached.then_some(&self.angles))
        });
        let shading = timed(&mut times, "shading.emit", || {
            emit_shading(mesh, &mut parents, &sums, fan_count)
        });
        (shading, times)
    }
    pub(crate) fn angle_cache_stats(&self, mesh: &Mesh) -> (usize, usize) {
        let bytes = self.angles.positions.len() * std::mem::size_of::<Vec3>()
            + self.angles.values.len() * std::mem::size_of::<[f32; 3]>();
        let unchanged = mesh
            .triangles
            .iter()
            .filter(|t| self.angles.unchanged(mesh, t))
            .count();
        (bytes, unchanged)
    }
}
