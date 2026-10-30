//! Exact temporary normals for fixed-topology display snapshots.
use super::{DisplayMesh, Mesh, Vec3};

/// Compact, ordered source-corner adjacency. Hidden faces are included, and
/// fan IDs are deliberately not cached: release can split or merge them.
pub(super) struct VertexCorners {
    offsets: Vec<u32>,
    corners: Vec<u32>,
}
impl VertexCorners {
    pub(super) fn new(mesh: &Mesh) -> Option<Self> {
        let count = mesh.triangles.len().checked_mul(3)?;
        u32::try_from(count).ok()?;
        let mut offsets = vec![0_u32; mesh.vertices.len().checked_add(1)?];
        for &vertex in mesh.triangles.iter().flatten() {
            offsets[vertex + 1] += 1;
        }
        for vertex in 1..offsets.len() {
            offsets[vertex] += offsets[vertex - 1];
        }
        let mut corners = vec![0; count];
        // Use starts as write cursors, then shift the resulting ends back into
        // starts. This avoids a second vertex-sized cursor allocation.
        for (corner, &vertex) in mesh.triangles.iter().flatten().enumerate() {
            let cursor = &mut offsets[vertex];
            corners[*cursor as usize] = corner as u32;
            *cursor += 1;
        }
        offsets.copy_within(..mesh.vertices.len(), 1);
        offsets[0] = 0;
        Some(Self { offsets, corners })
    }
    fn incident(&self, vertex: usize) -> &[u32] {
        &self.corners[self.offsets[vertex] as usize..self.offsets[vertex + 1] as usize]
    }
    fn update(
        &self,
        mesh: &Mesh,
        fans: &[[usize; 3]],
        previous: &[(usize, Vec3)],
        changed: &[usize],
    ) -> Option<NormalUpdate> {
        let mut dirty = vec![false; previous.len()];
        let mut selected = Vec::new();
        let mut work = 0_usize;
        for &vertex in changed {
            for &corner in self.incident(vertex) {
                for fan in fans[corner as usize / 3] {
                    if !dirty[fan] {
                        // Bound repeated face evaluation, including high-valence
                        // vertices and edits spread across many separate regions.
                        work = work.checked_add(self.incident(previous[fan].0).len())?;
                        if work > mesh.triangles.len() / 2 {
                            return None;
                        }
                        dirty[fan] = true;
                        selected.push(fan);
                    }
                }
            }
        }
        drop(dirty);
        selected.sort_unstable();
        let mut vertices = previous.to_vec();
        #[cfg(test)]
        let mut evaluated_faces = 0;
        for &fan in &selected {
            let mut sum = Vec3::default();
            for &corner in self.incident(previous[fan].0) {
                let face = corner as usize / 3;
                if fans[face][corner as usize % 3] == fan {
                    let [a, b, c] = mesh.triangles[face].map(|v| mesh.vertices[v]);
                    sum = sum + (b - a).cross(c - a);
                    #[cfg(test)]
                    {
                        evaluated_faces += 1;
                    }
                }
            }
            // Re-sum complete fans in original triangle/corner order. Subtracting
            // old face contributions would accumulate floating-point drift.
            vertices[fan].1 = sum.norm();
        }
        Some(NormalUpdate {
            vertices,
            #[cfg(test)]
            local: true,
            #[cfg(test)]
            evaluated_faces,
        })
    }
}

pub(super) struct NormalUpdate {
    pub(super) vertices: Vec<(usize, Vec3)>,
    #[cfg(test)]
    pub(super) local: bool,
    #[cfg(test)]
    pub(super) evaluated_faces: usize,
}

/// Discover all changes since the displayed snapshot, including skipped dabs.
/// Broad edits keep the sequential full pass and do not allocate adjacency.
fn changed_vertices(before: &[Vec3], after: &[Vec3]) -> Option<Vec<usize>> {
    let mut changed = Vec::new();
    for (i, (a, b)) in before.iter().zip(after).enumerate() {
        if [a.0.to_bits(), a.1.to_bits(), a.2.to_bits()]
            != [b.0.to_bits(), b.1.to_bits(), b.2.to_bits()]
        {
            changed.push(i);
            if changed.len() > before.len() / 8 {
                return None;
            }
        }
    }
    Some(changed)
}

impl DisplayMesh {
    pub(super) fn update_live_normals(&self, mesh: &Mesh, fans: &[[usize; 3]]) -> NormalUpdate {
        // The first deformation converts every canonical angle-weighted normal
        // to the existing area-weighted preview. Later frames can retain fans
        // whose incident face positions have not changed.
        if self.temporary_normals
            && let Some(topology) = &self.stroke_topology
            && let Some(changed) = changed_vertices(&self.mesh.vertices, &mesh.vertices)
            && let Some(corners) = topology
                .source
                .normal_corners
                .get_or_init(|| VertexCorners::new(mesh))
            && let Some(updated) = corners.update(mesh, fans, &self.shading.vertices, &changed)
        {
            return updated;
        }
        full_normals(mesh, fans, &self.shading.vertices)
    }
}

pub(super) fn full_normals(
    mesh: &Mesh,
    fans: &[[usize; 3]],
    previous: &[(usize, Vec3)],
) -> NormalUpdate {
    let mut normals = vec![Vec3::default(); previous.len()];
    for (ids, fans) in mesh.triangles.iter().zip(fans) {
        let [a, b, c] = ids.map(|i| mesh.vertices[i]);
        let face = (b - a).cross(c - a);
        for &i in fans {
            normals[i] = normals[i] + face;
        }
    }
    NormalUpdate {
        vertices: previous
            .iter()
            .zip(normals)
            .map(|(&(vertex, _), sum)| (vertex, sum.norm()))
            .collect(),
        #[cfg(test)]
        local: false,
        #[cfg(test)]
        evaluated_faces: mesh.triangles.len(),
    }
}

#[cfg(test)]
mod tests;
