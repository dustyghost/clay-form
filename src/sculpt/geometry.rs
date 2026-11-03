//! Persistent scene geometry, independent of mesh algorithms.
use super::{Mesh, MeshSettings};

#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    pub paths: Vec<super::paths::SurfacePath>,
    pub scene: super::SceneSettings,
    pub name: String,
    pub submodels: Vec<super::project::Submodel>,
    pub mesh: Mesh,
    pub settings: MeshSettings,
}
impl Geometry {
    pub fn new(mesh: Mesh) -> Self {
        Self {
            mesh,
            paths: Vec::new(),
            name: "Model".into(),
            scene: super::SceneSettings::default(),
            submodels: Vec::new(),
            settings: MeshSettings::default(),
        }
    }
}

/// A result validated before crossing the worker/document commit boundary.
pub(crate) struct ValidatedGeometry(Geometry);
impl ValidatedGeometry {
    pub fn new(geometry: Geometry, cancel: &impl Fn() -> bool) -> Result<Self, String> {
        geometry
            .settings
            .validate_cancellable(&geometry.mesh, cancel)?;
        if cancel() {
            return Err("Operation cancelled".into());
        }
        Ok(Self(geometry))
    }
    /// Validate the incoming stack, then retain the proof from subdivision's
    /// validated output. Retained levels are unchanged and child indices come
    /// from the shared refinement traversal.
    pub(crate) fn subdivided(
        mut geometry: Geometry,
        cancel: &impl Fn() -> bool,
    ) -> Result<Self, String> {
        geometry
            .settings
            .validate_cancellable(&geometry.mesh, cancel)?;
        geometry.subdivide_cancellable(cancel)?;
        geometry.settings.validate_stack_memory(&geometry.mesh)?;
        if cancel() {
            return Err("Operation cancelled".into());
        }
        Ok(Self(geometry))
    }
    pub fn geometry(&self) -> &Geometry {
        &self.0
    }
    pub fn into_geometry(self) -> Geometry {
        self.0
    }
}

#[cfg(test)]
mod subdivision_tests {
    use super::*;
    use crate::sculpt::{
        Primitive, PrimitiveKind, Vec3,
        capacity::{self, MeshCapacity},
    };

    #[test]
    fn checked_subdivision_matches_full_validation_and_rejects_invalid_stack() {
        for kind in [
            PrimitiveKind::Cube,
            PrimitiveKind::UvSphere,
            PrimitiveKind::Plane,
        ] {
            let source = Geometry::new(Primitive::for_kind(kind).mesh().unwrap());
            let mut reference = source.clone();
            reference.subdivide().unwrap();
            let actual = ValidatedGeometry::subdivided(source, &|| false)
                .unwrap()
                .into_geometry();
            assert_eq!(actual, reference);
            ValidatedGeometry::new(actual.clone(), &|| false).unwrap();
            let mut damaged = actual;
            damaged.mesh.triangles[0].swap(0, 1);
            assert!(ValidatedGeometry::subdivided(damaged, &|| false).is_err());
        }
    }

    #[test]
    fn checked_subdivision_enforces_total_stack_memory_and_cancellation() {
        capacity::with_test_capacity(
            MeshCapacity {
                vertices: 140,
                triangles: 1000,
            },
            || {
                let mut mesh = Mesh {
                    vertices: vec![Vec3(0., 0., 0.), Vec3(1., 0., 0.), Vec3(0., 1., 0.)],
                    triangles: vec![[0, 1, 2]],
                    mask: vec![0.; 3],
                    ..Default::default()
                };
                mesh.vertices.resize(103, Vec3::default());
                mesh.mask.resize(103, 0.);
                let mut g = Geometry::new(mesh);
                g.subdivide().unwrap();
                let result = ValidatedGeometry::subdivided(g, &|| false);
                assert!(result.err().unwrap().contains("stack exceeds memory"));
            },
        );
        let checks = std::cell::Cell::new(0);
        let source = Geometry::new(Mesh::starting_sphere());
        assert!(
            ValidatedGeometry::subdivided(source, &|| {
                checks.set(checks.get() + 1);
                checks.get() > 30
            })
            .is_err()
        );
    }
}
