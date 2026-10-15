//! Combine surfaces without welding vertices or changing their positions.
use super::{
    Mesh, VertexGroup,
    subdivision::{MAX_TRIANGLES, MAX_VERTICES},
};
impl Mesh {
    pub fn merged_with(&self, other: &Mesh) -> Result<Self, String> {
        self.validate()?;
        other.validate()?;
        if self.vertices.len() + other.vertices.len() > MAX_VERTICES
            || self.triangles.len() + other.triangles.len() > MAX_TRIANGLES
        {
            return Err("Merged mesh would exceed the mesh size limit".into());
        }
        if self.vertex_groups.len() + other.vertex_groups.len() > 64 {
            return Err(
                "Merged mesh would exceed 64 vertex groups; remove unused groups first".into(),
            );
        }
        let offset = self.vertices.len();
        let group_offset = self.vertex_groups.len();
        let mut merged = self.clone();
        merged.vertices.extend_from_slice(&other.vertices);
        merged.mask.extend_from_slice(&other.mask);
        merged
            .triangles
            .extend(other.triangles.iter().map(|t| t.map(|i| i + offset)));
        merged
            .quads
            .extend(other.quads.iter().map(|q| q.map(|i| i + offset)));
        for group in &mut merged.vertex_groups {
            group.weights.resize(merged.vertices.len(), 0.);
        }
        for group in &other.vertex_groups {
            let name = if merged.vertex_groups.iter().any(|g| g.name == group.name) {
                let mut base = group.name.clone();
                while base.len() > 120 {
                    base.pop();
                }
                merged.unique_group_name(&base)
            } else {
                group.name.clone()
            };
            let mut weights = vec![0.; offset];
            weights.extend_from_slice(&group.weights);
            merged.vertex_groups.push(VertexGroup {
                name,
                weights,
                visible: group.visible,
            });
        }
        if !self.polygroups.is_empty() || !other.polygroups.is_empty() {
            merged.polygroups.resize(self.triangles.len(), 0);
            merged
                .polygroups
                .extend((0..other.triangles.len()).map(|i| {
                    let id = other.polygroups.get(i).copied().unwrap_or(0);
                    if id == 0 { 0 } else { id + group_offset }
                }));
            merged.canonicalize_polygroups();
        }
        if !self.hidden_faces.is_empty() || !other.hidden_faces.is_empty() {
            merged.hidden_faces.resize(self.triangles.len(), false);
            merged.hidden_faces.extend(
                (0..other.triangles.len())
                    .map(|i| other.hidden_faces.get(i).copied().unwrap_or(false)),
            );
        }
        merged
            .creases
            .extend(other.creases.iter().map(|&[a, b]| [a + offset, b + offset]));
        merged.creases.sort_unstable();
        merged.validate()?;
        Ok(merged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn surface() -> Mesh {
        let mut primitive = super::super::Primitive::for_kind(super::super::PrimitiveKind::Plane);
        primitive.divisions = [1; 3];
        primitive.mesh().unwrap()
    }
    #[test]
    fn merge_preserves_topology_masks_groups_and_visibility_without_welding() {
        let mut a = surface();
        a.mask.fill(0.25);
        a.create_vertex_group("Base".into(), 0.5, true).unwrap();
        a.hidden_faces = vec![true, false];
        let mut b = surface();
        b.mask.fill(0.75);
        b.create_vertex_group("Base".into(), 0.5, true).unwrap();
        b.vertex_groups[0].visible = false;
        let merged = a.merged_with(&b).unwrap();
        assert_eq!(
            merged.vertices.len(),
            8,
            "coincident vertices remain separate"
        );
        assert_eq!(merged.triangles.len(), 4);
        assert_eq!(merged.quads.len(), 2);
        assert_eq!(&merged.mask[..4], &[0.25; 4]);
        assert_eq!(&merged.mask[4..], &[0.75; 4]);
        assert_eq!(merged.polygroups, [1, 1, 2, 2]);
        assert_eq!(merged.vertex_groups[1].name, "Base 2");
        assert_eq!(
            merged.vertex_groups[0].weights,
            [1., 1., 1., 1., 0., 0., 0., 0.]
        );
        assert_eq!(
            merged.vertex_groups[1].weights,
            [0., 0., 0., 0., 1., 1., 1., 1.]
        );
        assert_eq!(merged.hidden_face_count(), 3);
        merged.validate().unwrap();
        assert_eq!(a.vertices.len(), 4);
        assert_eq!(b.vertex_groups[0].name, "Base");
    }
    #[test]
    fn merge_rejects_limits_and_handles_long_duplicate_group_names() {
        let mut a = surface();
        let name = "é".repeat(64);
        a.create_vertex_group(name, 0.5, true).unwrap();
        a.merged_with(&a).unwrap().validate().unwrap();
        for i in 1..64 {
            a.create_vertex_group(format!("Group {i}"), 0.5, true)
                .unwrap();
        }
        let mut b = surface();
        b.create_vertex_group("Extra".into(), 0.5, true).unwrap();
        assert!(a.merged_with(&b).unwrap_err().contains("64 vertex groups"));
        let mut large = surface();
        large
            .vertices
            .resize(MAX_VERTICES / 2 + 1, super::super::Vec3::default());
        large.mask.resize(large.vertices.len(), 0.);
        assert!(
            large
                .merged_with(&large)
                .unwrap_err()
                .contains("size limit")
        );
    }
}
