//! Normal-offset shells for temporary display and explicit solidification.
use super::{
    Mesh,
    subdivision::{MAX_TRIANGLES, MAX_VERTICES},
};
use std::collections::HashMap;
impl Mesh {
    pub fn solidify(&self, thickness: f32) -> Result<Self, String> {
        self.solidify_impl(thickness, None)
    }
    pub fn solidify_with_metadata(&self, metadata: &super::ModelMetadata) -> Result<Self, String> {
        metadata.validate()?;
        self.solidify_impl(metadata.temporary_thickness, Some(metadata))
    }
    fn solidify_impl(
        &self,
        thickness: f32,
        metadata: Option<&super::ModelMetadata>,
    ) -> Result<Self, String> {
        self.validate()?;
        if !thickness.is_finite() || thickness <= 0. || thickness > 100. {
            return Err("Thickness must be greater than zero and at most 100 model units".into());
        }
        if self.triangles.is_empty() {
            return Err("Thickness requires a surface with faces".into());
        }
        let mut edges: HashMap<(usize, usize), (usize, i32, usize, usize)> = HashMap::new();
        for &[a, b, c] in &self.triangles {
            if a == b
                || b == c
                || a == c
                || (self.vertices[b] - self.vertices[a])
                    .cross(self.vertices[c] - self.vertices[a])
                    .len()
                    < 1e-10
            {
                return Err("Thickness requires nondegenerate faces".into());
            }
            for (a, b) in [(a, b), (b, c), (c, a)] {
                let (key, sign) = if a < b { ((a, b), 1) } else { ((b, a), -1) };
                let edge = edges.entry(key).or_insert((0, 0, a, b));
                edge.0 += 1;
                edge.1 += sign;
            }
        }
        if edges.values().any(|e| e.0 > 2 || (e.0 == 2 && e.1 != 0)) {
            return Err("Thickness requires consistently oriented manifold faces".into());
        }
        let mut boundary: Vec<_> = edges
            .values()
            .filter(|e| e.0 == 1)
            .map(|e| (e.2, e.3))
            .collect();
        if boundary.is_empty() {
            return Err("Thickness is for open meshes; this surface is already closed".into());
        }
        let mut boundary_vertices: HashMap<usize, (usize, usize)> = HashMap::new();
        for &(a, b) in &boundary {
            boundary_vertices.entry(a).or_default().0 += 1;
            boundary_vertices.entry(b).or_default().1 += 1;
        }
        if boundary_vertices.values().any(|&counts| counts != (1, 1)) {
            return Err("Thickness requires nonbranching boundary loops".into());
        }
        boundary.sort_unstable();
        let n = self.vertices.len();
        if n * 2 > MAX_VERTICES || self.triangles.len() * 2 + boundary.len() * 2 > MAX_TRIANGLES {
            return Err(
                "Thickness would exceed the mesh size limit; use a lower resolution".into(),
            );
        }
        let mut normals = self.angle_weighted_normals();
        // A cut can leave arbitrarily short edges whose endpoints have very
        // different one-sided normal estimates. Bound offset-field variation on
        // those edges before building the shell, so they cannot fold back.
        let mut normal_edges: Vec<_> = edges
            .keys()
            .filter(|(a, b)| boundary_vertices.contains_key(a) || boundary_vertices.contains_key(b))
            .map(|&(a, b)| ((self.vertices[b] - self.vertices[a]).len(), a, b))
            .collect();
        normal_edges.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        for _ in 0..32 {
            let mut changed = false;
            for &(length, a, b) in &normal_edges {
                let limit = (length / (2. * thickness)).min(2.);
                let difference = normals[b] - normals[a];
                if difference.len() > limit * 1.001 {
                    if (normals[a] + normals[b]).len() < 1e-6 {
                        return Err("Thickness cannot offset this folded boundary; adjust the surface first".into());
                    }
                    let middle = (normals[a] + normals[b]).norm();
                    let tangent = difference.norm();
                    let sine = limit * 0.5;
                    let cosine = (1. - sine * sine).sqrt();
                    normals[a] = middle * cosine - tangent * sine;
                    normals[b] = middle * cosine + tangent * sine;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut shell = Mesh {
            vertex_groups: self
                .vertex_groups
                .iter()
                .map(|g| super::VertexGroup {
                    visible: g.visible,
                    name: g.name.clone(),
                    weights: g
                        .weights
                        .iter()
                        .copied()
                        .chain(g.weights.iter().copied())
                        .collect(),
                })
                .collect(),
            polygroups: if self.polygroups.is_empty() {
                Vec::new()
            } else {
                self.polygroups
                    .iter()
                    .copied()
                    .chain(self.polygroups.iter().copied())
                    .collect()
            },
            vertices: self
                .vertices
                .iter()
                .zip(&normals)
                .map(|(p, n)| *p + *n * (thickness * 0.5))
                .chain(
                    self.vertices
                        .iter()
                        .zip(&normals)
                        .map(|(p, n)| *p - *n * (thickness * 0.5)),
                )
                .collect(),
            mask: self
                .mask
                .iter()
                .copied()
                .chain(self.mask.iter().copied())
                .collect(),
            triangles: self.triangles.clone(),
            quads: self.quads.clone(),
        };
        shell.triangles.extend(
            self.triangles
                .iter()
                .map(|&[a, b, c]| [a + n, c + n, b + n]),
        );
        shell.quads.extend(
            self.quads
                .iter()
                .map(|&[a, b, c, d]| [a + n, d + n, c + n, b + n]),
        );
        let boundary_vertices: std::collections::HashSet<_> =
            boundary.iter().flat_map(|&(a, b)| [a, b]).collect();
        for (a, b) in boundary {
            shell.quads.push([b, a, a + n, b + n]);
            shell.triangles.extend([[b, a, a + n], [b, a + n, b + n]]);
            if !self.polygroups.is_empty() {
                shell.polygroups.extend([0; 2]);
            }
        }
        if let Some(options) = metadata {
            if options.thickness_mask_rim {
                for &i in &boundary_vertices {
                    shell.mask[i] = 1.;
                    shell.mask[i + n] = 1.;
                }
            }
            if options.thickness_groups {
                if shell.vertex_groups.len() + 3 > 64 {
                    return Err("Thickness groups would exceed the 64-group limit".into());
                }
                if shell.polygroups.is_empty() {
                    shell.polygroups = vec![0; shell.triangles.len()];
                }
                for (part, name) in ["Thickness front", "Thickness back", "Thickness rim"]
                    .into_iter()
                    .enumerate()
                {
                    let name = shell.unique_group_name(name);
                    let id = shell.vertex_groups.len() + 1;
                    let weights = (0..n * 2)
                        .map(|i| {
                            let selected = match part {
                                0 => i < n,
                                1 => i >= n,
                                _ => boundary_vertices.contains(&(i % n)),
                            };
                            if selected { 1. } else { 0. }
                        })
                        .collect();
                    shell.vertex_groups.push(super::VertexGroup {
                        visible: true,
                        name,
                        weights,
                    });
                    let faces = self.triangles.len();
                    let range = match part {
                        0 => 0..faces,
                        1 => faces..faces * 2,
                        _ => faces * 2..shell.triangles.len(),
                    };
                    for i in range {
                        shell.polygroups[i] = id;
                    }
                }
            }
            if options.thickness_group_mask
                && shell
                    .mask
                    .iter()
                    .any(|&m| super::mask_selected(m, options.mask_threshold))
            {
                let name = shell.unique_group_name("Thickness masked");
                shell.create_vertex_group(name, options.mask_threshold, false)?;
            }
        }
        shell.canonicalize_polygroups();
        for &[a, b, c] in &self.triangles {
            let source_normal =
                (self.vertices[b] - self.vertices[a]).cross(self.vertices[c] - self.vertices[a]);
            for offset in [0, n] {
                let offset_normal = (shell.vertices[b + offset] - shell.vertices[a + offset])
                    .cross(shell.vertices[c + offset] - shell.vertices[a + offset]);
                if offset_normal.dot(source_normal) <= 0. {
                    return Err("This thickness folds surface faces; reduce thickness or refine the boundary".into());
                }
            }
        }
        for rim in shell.triangles[self.triangles.len() * 2..].chunks_exact(2) {
            let face_normal = |[a, b, c]: [usize; 3]| {
                (shell.vertices[b] - shell.vertices[a]).cross(shell.vertices[c] - shell.vertices[a])
            };
            if face_normal(rim[0]).dot(face_normal(rim[1])) <= 0. {
                return Err(
                    "This thickness folds a rim face; reduce thickness or refine the boundary"
                        .into(),
                );
            }
        }
        if shell.triangles.iter().any(|&[a, b, c]| {
            (shell.vertices[b] - shell.vertices[a])
                .cross(shell.vertices[c] - shell.vertices[a])
                .len()
                <= 1e-12
        }) {
            return Err("This thickness collapses shell faces; choose a different thickness or adjust the mesh coordinates".into());
        }
        shell.validate_closed()?;
        Ok(shell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Primitive, PrimitiveKind};
    #[test]
    fn thickness_closes_a_plane_with_quads_and_preserves_masks() {
        let p = Primitive::for_kind(PrimitiveKind::Plane);
        let mut mesh = p.mesh().unwrap();
        mesh.mask = vec![0., 0.25, 0.5, 1.];
        let shell = mesh.solidify(0.2).unwrap();
        assert_eq!(shell.vertices.len(), 8);
        assert_eq!(shell.quads.len(), 6);
        assert_eq!(shell.triangles.len(), 12);
        assert_eq!(&shell.mask[..4], &mesh.mask);
        assert_eq!(&shell.mask[4..], &mesh.mask);
        assert!(shell.vertices[..4].iter().all(|p| (p.2 - 0.1).abs() < 1e-6));
        assert!(shell.vertices[4..].iter().all(|p| (p.2 + 0.1).abs() < 1e-6));
        shell.validate_closed().unwrap();
        let volume: f32 = shell
            .triangles
            .iter()
            .map(|&[a, b, c]| {
                shell.vertices[a].dot(shell.vertices[b].cross(shell.vertices[c])) / 6.
            })
            .sum();
        assert!(
            (volume - 0.8).abs() < 1e-5,
            "shell winding must point outward"
        );
    }
    #[test]
    fn thickness_rejects_offsets_lost_to_coordinate_precision() {
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        for p in &mut mesh.vertices {
            p.2 = 1_000_000.;
        }
        assert!(mesh.solidify(0.0001).is_err());
        mesh.solidify(1.).unwrap().validate_closed().unwrap();
    }
    #[test]
    fn thickness_closes_cut_sphere_holes_and_rejects_unsuitable_inputs() {
        let mut mesh = Mesh::sphere();
        for (p, m) in mesh.vertices.iter().zip(&mut mesh.mask) {
            if p.1 > 0.6 {
                *m = 1.;
            }
        }
        let cut = mesh.delete_masked().unwrap().unwrap();
        cut.solidify(0.05).unwrap().validate_closed().unwrap();
        assert!(mesh.solidify(0.05).is_err());
        for value in [0., -0.1, f32::NAN, f32::INFINITY, 101.] {
            assert!(cut.solidify(value).is_err());
        }
        let mut malformed = cut;
        malformed.triangles.push(malformed.triangles[0]);
        assert!(malformed.solidify(0.05).is_err());
    }
}

#[cfg(test)]
mod slice_rim_tests {
    use super::*;
    use crate::sculpt::{SliceMode, SliceOptions, SlicePlane, Vec3};
    #[test]
    fn sliced_hemisphere_thickness_keeps_thin_faces_and_rim_quads_unfolded() {
        for (height, normal) in [
            (0., Vec3(0., -1., 0.)),
            (0.017, Vec3(-0.37, -1., -0.21)),
            (0.23, Vec3(0.2, -1., 0.6)),
        ] {
            let (mesh, _) = Mesh::sphere()
                .slice(
                    SlicePlane {
                        origin: Vec3(0., height, 0.),
                        normal,
                    },
                    SliceOptions {
                        mode: SliceMode::Trim,
                        cap: false,
                        flip: false,
                    },
                    &|| false,
                )
                .unwrap();
            let original = mesh.clone();
            for thickness in [0.01, 0.05, 0.18, 0.4] {
                let shell = mesh.solidify(thickness).unwrap();
                let n = mesh.vertices.len();
                assert_eq!(mesh, original);
                for i in 0..n {
                    assert!(
                        ((shell.vertices[i] + shell.vertices[i + n]) * 0.5 - mesh.vertices[i])
                            .len()
                            < 1e-6
                    );
                    assert!(
                        ((shell.vertices[i] - shell.vertices[i + n]).len() - thickness).abs()
                            < 1e-6
                    );
                }
                for &[a, b, c] in &mesh.triangles {
                    let original = (mesh.vertices[b] - mesh.vertices[a])
                        .cross(mesh.vertices[c] - mesh.vertices[a]);
                    for offset in [0, n] {
                        let generated = (shell.vertices[b + offset] - shell.vertices[a + offset])
                            .cross(shell.vertices[c + offset] - shell.vertices[a + offset]);
                        assert!(
                            generated.dot(original) > 0.,
                            "shell face folded after slice"
                        );
                    }
                }
                for pair in shell.triangles[mesh.triangles.len() * 2..].chunks_exact(2) {
                    let normals: Vec<_> = pair
                        .iter()
                        .map(|&[a, b, c]| {
                            (shell.vertices[b] - shell.vertices[a])
                                .cross(shell.vertices[c] - shell.vertices[a])
                        })
                        .collect();
                    assert!(
                        normals[0].dot(normals[1]) > 0.,
                        "rim quad folded over itself"
                    );
                }
                shell.validate_closed().unwrap();
            }
        }
    }
}
