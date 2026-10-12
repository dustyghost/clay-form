//! Normal-offset shells for temporary display and explicit solidification.
use super::{
    Mesh,
    subdivision::{MAX_TRIANGLES, MAX_VERTICES},
};
use std::collections::HashMap;
impl Mesh {
    pub fn solidify(&self, thickness: f32) -> Result<Self, String> {
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
        let normals = self.normals();
        let mut shell = Mesh {
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
        for (a, b) in boundary {
            shell.quads.push([b, a, a + n, b + n]);
            shell.triangles.extend([[b, a, a + n], [b, a + n, b + n]]);
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
