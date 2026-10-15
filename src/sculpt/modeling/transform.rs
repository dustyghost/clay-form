//! Component transforms keep the source topology and reflect displacement across symmetry.
use super::*;
#[derive(Clone, Copy)]
pub enum ComponentTransform {
    Move(Vec3),
    Rotate(Vec3, f32),
    Scale(Vec3, f32),
}
impl Mesh {
    /// Protected source polygons must survive with the same oriented corners.
    pub fn check_model_protection(&self, out: &Mesh, mask: bool) -> Result<(), String> {
        let key = |mesh: &Mesh, p: &Polygon| {
            let points: Vec<_> = p
                .vertices
                .iter()
                .map(|&v| {
                    let p = mesh.vertices[v];
                    [p.0.to_bits(), p.1.to_bits(), p.2.to_bits()]
                })
                .collect();
            (0..points.len())
                .map(|i| {
                    points[i..]
                        .iter()
                        .chain(&points[..i])
                        .copied()
                        .collect::<Vec<_>>()
                })
                .min()
                .unwrap()
        };
        let remaining: BTreeSet<_> = out.polygons().iter().map(|p| key(out, p)).collect();
        for p in self.polygons() {
            if (!p.visible || (mask && p.vertices.iter().any(|&v| self.mask[v] > 0.)))
                && !remaining.contains(&key(self, &p))
            {
                return Err(if !p.visible {
                    "Show hidden adjoining faces before editing"
                } else {
                    "This edit touches masked geometry; clear the mask or turn off Protect mask"
                }
                .into());
            }
        }
        Ok(())
    }

    pub fn protect_model_vertices(&self, vertices: &[usize], mask: bool) -> Result<(), String> {
        if vertices.is_empty() || vertices.iter().any(|&v| v >= self.vertices.len()) {
            return Err("Select components on the current mesh".into());
        }
        let selected: BTreeSet<_> = vertices.iter().copied().collect();
        if self
            .polygons()
            .iter()
            .any(|p| !p.visible && p.vertices.iter().any(|v| selected.contains(v)))
        {
            return Err("This edit touches hidden faces; show them before editing".into());
        }
        if mask && vertices.iter().any(|&v| self.mask[v] > 0.) {
            return Err(
                "This edit touches masked vertices; clear the mask or turn off Protect mask".into(),
            );
        }
        Ok(())
    }
    /// Use the same reference-side vertices for the gizmo and the symmetric edit.
    pub(crate) fn component_transform_frame(
        &self,
        selected: &[usize],
        anchor: Vec3,
        symmetry: &ModelingSymmetry,
    ) -> (Vec<usize>, Vec3) {
        let all: BTreeSet<_> = selected
            .iter()
            .flat_map(|&v| symmetry.vertex_copies(v))
            .collect();
        let mut seeds = Vec::new();
        let mut visited = BTreeSet::new();
        for &v in &all {
            if visited.contains(&v) {
                continue;
            }
            let copies = symmetry.vertex_copies(v);
            let seed = *copies
                .iter()
                .min_by(|&&a, &&b| {
                    (self.vertices[a] - anchor)
                        .len()
                        .total_cmp(&(self.vertices[b] - anchor).len())
                })
                .unwrap();
            visited.extend(copies);
            seeds.push(seed);
        }
        let pivot = seeds
            .iter()
            .fold(Vec3::default(), |a, &v| a + self.vertices[v])
            * (1. / seeds.len().max(1) as f32);
        (seeds, pivot)
    }
    pub fn transform_components(
        &self,
        selected: &[usize],
        anchor: Vec3,
        transform: ComponentTransform,
        protect: bool,
        symmetry: &ModelingSymmetry,
    ) -> Result<Mesh, String> {
        let all: BTreeSet<_> = selected
            .iter()
            .flat_map(|&v| symmetry.vertex_copies(v))
            .collect();
        self.protect_model_vertices(&all.iter().copied().collect::<Vec<_>>(), protect)?;
        let (seeds, pivot) = self.component_transform_frame(selected, anchor, symmetry);
        let mut out = self.clone();
        for seed in seeds {
            let p = self.vertices[seed];
            let delta = match transform {
                ComponentTransform::Move(v) => v,
                ComponentTransform::Rotate(axis, angle) => {
                    (p - pivot).rotated(axis.norm(), angle) - (p - pivot)
                }
                ComponentTransform::Scale(axis, factor) => {
                    if !factor.is_finite() || factor <= 0. {
                        return Err("Scale must be greater than zero".into());
                    }
                    let offset = p - pivot;
                    if axis.len() < 1e-8 {
                        offset * (factor - 1.)
                    } else {
                        axis.norm() * (offset.dot(axis.norm()) * (factor - 1.))
                    }
                }
            };
            // Stabilizers on mirror planes contribute more than once; average their
            // displacements to keep shared seam vertices on the symmetry plane.
            let mut moves = BTreeMap::<usize, (Vec3, usize)>::new();
            for (mapping, basis) in symmetry.vertices.iter().zip(&symmetry.bases) {
                let value = basis[0] * delta.0 + basis[1] * delta.1 + basis[2] * delta.2;
                let e = moves.entry(mapping[seed]).or_default();
                e.0 = e.0 + value;
                e.1 += 1;
            }
            for (v, (delta, n)) in moves {
                out.vertices[v] = self.vertices[v] + delta * (1. / n as f32);
            }
        }
        manifold(&out)?;
        symmetry.validate_result(&out)?;
        Ok(out)
    }
}
