//! Session-only edge selection; mesh edits never depend on stale indices.
use super::*;
impl Viewport {
    pub(crate) fn set_edge_selection(&mut self, edges: Vec<[usize; 2]>, document: &Document) {
        self.edge_selection = edges;
        self.edge_revision = document.display_revision();
    }
    pub(crate) fn selected_edges(&mut self, document: &Document) -> &[[usize; 2]] {
        if self.edge_revision != document.display_revision() {
            self.edge_selection.clear();
            self.edge_revision = document.display_revision();
        }
        &self.edge_selection
    }
    pub(super) fn paint_edges(
        &mut self,
        document: &Document,
        scene: &Scene,
        painter: &egui::Painter,
    ) {
        let mesh = document.mesh();
        for &[a, b] in self.selected_edges(document) {
            if let Some(points) = scene.clip_segment([mesh.vertices[a], mesh.vertices[b]]) {
                painter.line_segment(
                    points,
                    egui::Stroke::new(2_f32, Color32::from_rgb(70, 225, 240)),
                );
            }
        }
    }
    pub(super) fn edge_at(point: Pos2, document: &Document, scene: &Scene) -> Option<[usize; 2]> {
        let mesh = document.mesh();
        let mut candidates = Vec::new();
        let visible: std::collections::BTreeSet<_> = mesh
            .triangles
            .iter()
            .enumerate()
            .filter(|(i, _)| mesh.face_visible(*i))
            .flat_map(|(_, t)| {
                let [a, b, c] = *t;
                [
                    [a.min(b), a.max(b)],
                    [b.min(c), b.max(c)],
                    [c.min(a), c.max(a)],
                ]
            })
            .collect();
        for edge in mesh.polygon_edges() {
            if !visible.contains(&edge) {
                continue;
            }
            let [a, b] = edge;
            let Some([aa, bb]) = scene.clip_segment([mesh.vertices[a], mesh.vertices[b]]) else {
                continue;
            };
            let direction = bb - aa;
            let t = ((point - aa).dot(direction) / direction.length_sq().max(1e-6)).clamp(0., 1.);
            let closest = aa + direction * t;
            let distance = point.distance(closest);
            if distance < 10. {
                candidates.push((distance, edge, closest));
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        candidates.into_iter().find_map(|(_, edge, p)| {
            // Picking at the closest screen point supplies the depth test. At an
            // exact silhouette, sample slightly toward the pointer as well.
            let [a, b] = edge;
            let delta = mesh.vertices[b] - mesh.vertices[a];
            let hit = scene.pick(p).or_else(|| scene.pick(p.lerp(point, 0.05)))?;
            let t =
                ((hit.0 - mesh.vertices[a]).dot(delta) / delta.dot(delta).max(1e-12)).clamp(0., 1.);
            ((hit.0 - (mesh.vertices[a] + delta * t)).len() < delta.len() * 0.03 + 1e-4)
                .then_some(edge)
        })
    }
    pub(super) fn pick_edge(
        &mut self,
        point: Pos2,
        modifiers: egui::Modifiers,
        document: &Document,
        scene: &Scene,
    ) {
        let found = Self::edge_at(point, document, scene);
        if !modifiers.shift && !modifiers.ctrl && !modifiers.command {
            self.edge_selection.clear();
        }
        if let Some(edge) = found {
            if modifiers.ctrl || modifiers.command {
                self.edge_selection.retain(|e| *e != edge);
            } else {
                self.edge_selection.push(edge);
                self.edge_selection.sort_unstable();
                self.edge_selection.dedup();
            }
        }
        self.edge_revision = document.display_revision();
    }
}
