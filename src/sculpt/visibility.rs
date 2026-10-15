//! Face visibility is independent of masks and polygroup membership.
use super::Mesh;
impl Mesh {
    pub fn has_hidden_faces(&self) -> bool {
        self.hidden_faces.iter().any(|&hidden| hidden)
            || (self.vertex_groups.iter().any(|g| !g.visible)
                && self
                    .polygroups
                    .iter()
                    .any(|&id| id != 0 && !self.vertex_groups[id - 1].visible))
    }
    pub fn hidden_face_count(&self) -> usize {
        if !self.has_hidden_faces() {
            return 0;
        }
        (0..self.triangles.len())
            .filter(|&i| !self.face_visible(i))
            .count()
    }
    pub fn show_all_faces(&mut self) {
        self.hidden_faces.clear();
        for group in &mut self.vertex_groups {
            group.visible = true;
        }
    }
    /// Resolve group and rectangle hiding to one effective face set.
    pub fn set_face_visibility(&mut self, visible: Vec<bool>) {
        assert_eq!(visible.len(), self.triangles.len());
        self.show_all_faces();
        if visible.iter().any(|&v| !v) {
            self.hidden_faces = visible.into_iter().map(|v| !v).collect();
        }
    }
    pub fn invert_visibility(&mut self) {
        self.set_face_visibility(
            (0..self.triangles.len())
                .map(|i| !self.face_visible(i))
                .collect(),
        );
    }
    /// Extract a face set without welding or capping its boundary. Both halves
    /// retain masks, weights, categories, and only complete surviving quads.
    pub fn visibility_part(&self, visible: bool) -> Result<Self, String> {
        let old_faces: Vec<_> = (0..self.triangles.len())
            .filter(|&i| self.face_visible(i) == visible)
            .collect();
        let mut result = Self {
            creases: Vec::new(),
            vertices: Vec::new(),
            triangles: Vec::new(),
            quads: Vec::new(),
            mask: Vec::new(),
            vertex_groups: Vec::new(),
            polygroups: Vec::new(),
            hidden_faces: Vec::new(),
        };
        let mut indices = vec![usize::MAX; self.vertices.len()];
        let mut old_vertices = Vec::new();
        for &fi in &old_faces {
            for &i in &self.triangles[fi] {
                if indices[i] == usize::MAX {
                    indices[i] = result.vertices.len();
                    old_vertices.push(i);
                    result.vertices.push(self.vertices[i]);
                    result.mask.push(self.mask[i]);
                }
            }
            result
                .triangles
                .push(self.triangles[fi].map(|i| indices[i]));
        }
        let key = |mut t: [usize; 3]| {
            t.sort_unstable();
            t
        };
        let faces: std::collections::HashSet<_> =
            old_faces.iter().map(|&i| key(self.triangles[i])).collect();
        for &[a, b, c, d] in &self.quads {
            if (faces.contains(&key([a, b, c])) && faces.contains(&key([a, c, d])))
                || (faces.contains(&key([a, b, d])) && faces.contains(&key([b, c, d])))
            {
                result.quads.push([a, b, c, d].map(|i| indices[i]));
            }
        }
        self.reindex_groups(&mut result, &old_vertices, &old_faces);
        result.show_all_faces();
        result.validate()?;
        Ok(result)
    }
}
