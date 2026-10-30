//! Immutable render/picking payloads, independent of the editable document mesh.
use super::{Arc, Material, Mesh, Vec3};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DisplaySurface {
    pub vertices: Arc<Vec<Vec3>>,
    pub triangles: Arc<Vec<[usize; 3]>>,
    pub quads: Arc<Vec<[usize; 4]>>,
    pub mask: Arc<Vec<f32>>,
    pub materials: Arc<Vec<Material>>,
}

impl From<Mesh> for DisplaySurface {
    fn from(mesh: Mesh) -> Self {
        // The builder grows these vectors. Shared snapshots retain them for the
        // whole stroke, so release any spare capacity before sharing.
        fn shared<T>(mut values: Vec<T>) -> Arc<Vec<T>> {
            values.shrink_to_fit();
            Arc::new(values)
        }
        Self {
            vertices: shared(mesh.vertices),
            triangles: shared(mesh.triangles),
            quads: shared(mesh.quads),
            mask: shared(mesh.mask),
            materials: shared(mesh.materials),
        }
    }
}

impl DisplaySurface {
    pub fn material(&self, i: usize) -> Material {
        self.materials.get(i).cloned().unwrap_or_default()
    }

    pub(super) fn same_positions(&self, source: &Mesh) -> bool {
        self.vertices.len() == source.vertices.len()
            && self.vertices.iter().zip(&source.vertices).all(|(a, b)| {
                [a.0.to_bits(), a.1.to_bits(), a.2.to_bits()]
                    == [b.0.to_bits(), b.1.to_bits(), b.2.to_bits()]
            })
    }

    pub(super) fn same_mask(&self, source: &Mesh) -> bool {
        bytemuck::cast_slice::<f32, u32>(&self.mask)
            == bytemuck::cast_slice::<f32, u32>(&source.mask)
    }

    /// Called only after topology/visibility validation. Compare actual payloads
    /// rather than brush kind: edits can be combined or skip display frames.
    pub(super) fn update(&self, source: &Mesh, positions_unchanged: bool) -> Self {
        Self {
            vertices: if positions_unchanged {
                self.vertices.clone()
            } else {
                Arc::new(source.vertices.clone())
            },
            triangles: self.triangles.clone(),
            quads: self.quads.clone(),
            mask: if self.same_mask(source) {
                self.mask.clone()
            } else {
                Arc::new(source.mask.clone())
            },
            materials: if Material::slices_bitwise_equal(&self.materials, &source.materials) {
                self.materials.clone()
            } else {
                Arc::new(source.materials.clone())
            },
        }
    }

    /// Owned reference mesh for existing geometry regressions only.
    #[cfg(test)]
    pub(crate) fn to_mesh(&self) -> Mesh {
        Mesh {
            vertices: (*self.vertices).clone(),
            triangles: (*self.triangles).clone(),
            quads: (*self.quads).clone(),
            mask: (*self.mask).clone(),
            materials: (*self.materials).clone(),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_payload_reuse_preserves_float_bits_and_material_presence() {
        let original = Mesh {
            vertices: vec![Vec3(0., 1., 2.)],
            mask: vec![0.],
            ..Default::default()
        };
        let display = DisplaySurface::from(original.clone());
        let mut edited = original.clone();
        edited.vertices[0].0 = -0.;
        edited.mask[0] = -0.;
        // Explicit defaults may carry different provenance than an empty array.
        edited.materials = vec![Material::default()];
        assert!(!display.same_positions(&edited));
        let next = display.update(&edited, display.same_positions(&edited));
        assert!(!Arc::ptr_eq(&display.vertices, &next.vertices));
        assert!(!Arc::ptr_eq(&display.mask, &next.mask));
        assert!(!Arc::ptr_eq(&display.materials, &next.materials));
        assert_eq!(next.vertices[0].0.to_bits(), (-0_f32).to_bits());
        assert_eq!(next.mask[0].to_bits(), (-0_f32).to_bits());
        assert_eq!(display.to_mesh(), original);
        let removed = next.update(&original, false);
        assert!(removed.materials.is_empty());
        assert_eq!(removed.mask[0].to_bits(), 0_f32.to_bits());
    }
}
