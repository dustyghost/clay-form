//! Resolve every input against the same scene before installing any baked output.
use super::*;
use crate::sculpt::Geometry;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BakeScope {
    Set(u64),
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BakeDestination {
    PerCopy,
    New,
    Current,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BakeRequest {
    pub scope: BakeScope,
    pub destination: BakeDestination,
    pub fuse: bool,
}

impl BakeRequest {
    pub fn new(scope: BakeScope) -> Self {
        Self {
            scope,
            destination: BakeDestination::PerCopy,
            fuse: false,
        }
    }
}

impl Geometry {
    pub(crate) fn bake_instances(
        &mut self,
        request: BakeRequest,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(), String> {
        use BakeDestination::*;
        let library = &self.scene.instances;
        let mut resolver = ArrayResolver::new(library);
        let mut meshes = Vec::new();
        let mut baked = HashSet::new();
        let mut vertices = 0;
        let mut triangles = 0;
        let mut count_mesh = |mesh: &Mesh| -> Result<(), String> {
            vertices += mesh.vertices.len();
            triangles += mesh.triangles.len();
            if vertices > MAX_VERTICES || triangles > MAX_TRIANGLES {
                return Err("Instance bake budget exceeded".into());
            }
            if mesh.triangles.is_empty() {
                return Err("A copy has no faces to bake".into());
            }
            Ok(())
        };
        let current = request.destination == Current;
        if current && !self.scene.active_metadata.visible {
            return Err("Show the selected submodel before baking into it".into());
        }
        if request.destination == PerCopy && request.fuse {
            return Err("Choose one new submodel or the selected submodel to fuse copies".into());
        }
        let mut name = "Baked instances".to_owned();
        for set in &library.sets {
            match request.scope {
                BakeScope::Set(id) if id != set.id => continue,
                BakeScope::All if !set.visible => continue,
                _ => {}
            }
            if cancel() {
                return Err("Operation cancelled".into());
            }
            let first = meshes.len();
            if let Some(copies) = set.generated(&self.paths, |s| self.instance_source(s), cancel)? {
                for copy in copies {
                    count_mesh(&copy.mesh)?;
                    meshes.push((format!("{} {}", set.name, copy.slot + 1), copy.mesh));
                }
            } else {
                for (slot, p) in resolver.placements(set, &self.paths)? {
                    if cancel() {
                        return Err("Operation cancelled".into());
                    }
                    let source = self
                        .instance_source(p.piece.source)
                        .ok_or("Instance source is missing")?;
                    count_mesh(source)?;
                    meshes.push((format!("{} {}", set.name, slot + 1), p.mesh(source)?));
                }
            }
            if meshes.len() > first {
                baked.insert(set.id);
                if let BakeScope::Set(_) = request.scope {
                    name = set.name.clone();
                }
            }
        }
        if meshes.is_empty() {
            return Err("No visible copies to bake".into());
        }
        for remaining in library.sets.iter().filter(|s| !baked.contains(&s.id)) {
            if let Layout::Array(a) = &remaining.layout
                && a.source_array.is_some_and(|id| baked.contains(&id))
            {
                return Err(format!(
                    "{} still uses this array. Show and bake the dependent array as well.",
                    remaining.name
                ));
            }
        }
        let new_models = match request.destination {
            PerCopy => meshes.len(),
            New => 1,
            Current => 0,
        };
        if self.submodels.len() + 1 + new_models > 64 {
            return Err("This bake would exceed 64 submodels. Choose one new submodel or the selected submodel.".into());
        }
        if request.destination == PerCopy {
            for (name, mesh) in meshes {
                if cancel() {
                    return Err("Operation cancelled".into());
                }
                self.import_mesh(mesh, &name)?;
            }
        } else {
            let mut surfaces: Vec<_> = meshes.into_iter().map(|(_, mesh)| mesh).collect();
            if current && !self.mesh.vertices.is_empty() {
                let meta = &self.scene.active_metadata;
                let surface = if meta.temporary_thickness > 0. {
                    self.mesh.solidify_with_metadata(meta)?
                } else {
                    self.mesh.clone()
                };
                surfaces.insert(0, surface);
            }
            let mesh = combine(surfaces, request.fuse, self.settings.resolution, cancel)?;
            if cancel() {
                return Err("Operation cancelled".into());
            }
            if current {
                self.mesh = mesh;
                self.settings.primitive = None;
                self.settings.lower.clear();
                self.settings.upper.clear();
                self.scene.active_metadata.temporary_thickness = 0.;
            } else {
                if request.fuse {
                    name.push_str(" fused");
                }
                self.import_mesh(mesh, &name)?;
            }
        }
        self.scene.instances.sets.retain(|s| !baked.contains(&s.id));
        Ok(())
    }
}

fn combine(
    mut surfaces: Vec<Mesh>,
    fuse: bool,
    resolution: u32,
    cancel: &dyn Fn() -> bool,
) -> Result<Mesh, String> {
    let capacity = crate::sculpt::capacity::current();
    if surfaces.iter().map(|m| m.vertices.len()).sum::<usize>() > capacity.vertices
        || surfaces.iter().map(|m| m.triangles.len()).sum::<usize>() > capacity.triangles
    {
        return Err(
            "Combined surface exceeds the mesh capacity. Bake one submodel per copy instead."
                .into(),
        );
    }
    let uv = if fuse {
        for mesh in &mut surfaces {
            if cancel() {
                return Err("Operation cancelled".into());
            }
            if mesh.has_hidden_faces() || mesh.validate_closed().is_err() {
                return Err("Fusion requires closed, fully visible surfaces. Turn off Fuse overlapping surfaces to keep open pieces.".into());
            }
            mesh.uv = Default::default();
        }
        None
    } else if surfaces.len() > 1 {
        Some(combined_uvs(&mut surfaces, cancel)?)
    } else {
        None
    };
    // Merge in pairs so large batches aren't repeatedly copied into a growing prefix.
    while surfaces.len() > 1 {
        let mut pairs = surfaces.into_iter();
        let mut next = Vec::new();
        while let Some(a) = pairs.next() {
            if cancel() {
                return Err("Operation cancelled".into());
            }
            next.push(match pairs.next() {
                Some(b) => a.merged_with(&b)?,
                None => a,
            });
        }
        surfaces = next;
    }
    let mut mesh = surfaces.pop().ok_or("No surfaces to combine")?;
    if let Some(mut map) = uv {
        if !map.corners.is_empty() {
            map.topology.clone_from(&mesh.triangles);
        }
        mesh.uv = map;
        mesh.validate()?;
    }
    if fuse {
        mesh.remesh_cancellable(resolution, cancel)
    } else {
        Ok(mesh)
    }
}

// A submodel has one texture per channel. Unmapped faces can coexist with mapped
// faces, but two mapped surfaces with different bindings need separate submodels.
fn combined_uvs(
    surfaces: &mut [Mesh],
    cancel: &dyn Fn() -> bool,
) -> Result<crate::sculpt::uv::UvMap, String> {
    let mut result = crate::sculpt::uv::UvMap::default();
    let mapped = surfaces.iter().any(|m| !m.uv.corners.is_empty());
    let masked = surfaces.iter().any(|m| !m.uv.masks.is_empty());
    let mut bound = false;
    let mut offset = 0;
    for mesh in surfaces {
        if cancel() {
            return Err("Operation cancelled".into());
        }
        if !mesh.uv.corners.is_empty() && !mesh.uv.current(mesh) {
            return Err(
                "Refresh stale UV maps before combining, or bake one submodel per copy.".into(),
            );
        }
        let mut map = std::mem::take(&mut mesh.uv);
        if !map.corners.is_empty() || !map.textures.is_empty() {
            if !bound {
                result.textures = std::mem::take(&mut map.textures);
                bound = true;
            } else if result.textures.len() != map.textures.len()
                || !result.textures.iter().all(|t| map.textures.contains(t))
            {
                return Err("These surfaces use different texture images. Bake one submodel per copy to keep their assignments.".into());
            }
        }
        if mapped {
            let faces = mesh.triangles.len();
            map.removed.resize(faces, map.corners.is_empty());
            map.corners.resize(faces, [[0.; 2]; 3]);
            result.corners.append(&mut map.corners);
            result.removed.append(&mut map.removed);
            if masked {
                map.masks.resize(faces, [0.; 3]);
                result.masks.append(&mut map.masks);
            }
        }
        for [a, b] in map.seams {
            if a >= mesh.vertices.len() || b >= mesh.vertices.len() {
                return Err("Refresh stale UV seams before combining these surfaces.".into());
            }
            result.seams.push([a + offset, b + offset]);
        }
        offset += mesh.vertices.len();
    }
    if result.removed.iter().all(|r| !r) {
        result.removed.clear();
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{
        Primitive, PrimitiveKind,
        texture::{Channel, Texture},
    };

    #[test]
    fn bake_combined_uvs_keep_unmapped_faces_and_reject_different_paint() {
        let blank = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        let mut mapped = blank.clone();
        mapped.uv.corners = vec![[[0., 0.], [1., 0.], [0., 1.]]; mapped.triangles.len()];
        mapped.uv.topology.clone_from(&mapped.triangles);
        mapped
            .uv
            .textures
            .push(Texture::new(Channel::Color, 16, &Default::default()).unwrap());
        let faces = blank.triangles.len();
        let merged = combine(vec![blank, mapped.clone()], false, 16, &|| false).unwrap();
        assert!(merged.uv.current(&merged));
        assert!(merged.uv.removed[..faces].iter().all(|x| *x));
        assert!(merged.uv.removed[faces..].iter().all(|x| !*x));
        assert_eq!(merged.uv.textures, mapped.uv.textures);
        let mut painted = mapped.clone();
        assert!(painted.uv.textures[0].pixels.set_pixel(0, [0., 0., 0., 0.]));
        assert!(combine(vec![mapped, painted], false, 16, &|| false).is_err());
    }
}
