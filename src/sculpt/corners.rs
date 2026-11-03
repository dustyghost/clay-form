use super::Mesh;

/// Compact, ordered source-corner adjacency. Hidden faces are included, and
/// fan IDs are deliberately not cached: release can split or merge them.
pub(crate) struct VertexCorners {
    pub(crate) offsets: Vec<u32>,
    pub(crate) corners: Vec<u32>,
}
impl VertexCorners {
    pub(crate) fn new(mesh: &Mesh) -> Option<Self> {
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
    pub(crate) fn incident(&self, vertex: usize) -> &[u32] {
        &self.corners[self.offsets[vertex] as usize..self.offsets[vertex + 1] as usize]
    }
}
