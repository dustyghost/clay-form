//! Find complete connected side strips at a translated face cross-section.
use super::*;
type Contact = (f32, BTreeMap<usize, usize>);
impl Mesh {
    pub(super) fn loop_contact(
        &self,
        polygons: &[Polygon],
        face: usize,
        direction: Vec3,
        distance: f32,
        snap: f32,
    ) -> Result<Option<Contact>, String> {
        let source = &polygons[face];
        let origin = self.vertices[source.vertices[0]];
        let size = source
            .edges()
            .map(|(a, b)| (self.vertices[a] - self.vertices[b]).len())
            .fold(f32::INFINITY, f32::min);
        let tolerance = (size * 1e-4).max(1e-7);
        let mut candidates: Vec<_> = self
            .vertices
            .iter()
            .filter_map(|&p| {
                let d = (p - origin).dot(direction);
                (d < -tolerance
                    && d.abs() <= distance.abs() + size * snap
                    && (p - origin - direction * d).len() <= tolerance)
                    .then_some(d)
            })
            .collect();
        candidates.sort_by(|a, b| b.total_cmp(a));
        candidates.dedup_by(|a, b| (*a - *b).abs() < tolerance);
        for depth in candidates {
            let mut mapping = BTreeMap::new();
            for &v in &source.vertices {
                let p = self.vertices[v] + direction * depth;
                let matches: Vec<_> = self
                    .vertices
                    .iter()
                    .enumerate()
                    .filter(|(w, q)| !source.vertices.contains(w) && (**q - p).len() <= tolerance)
                    .map(|(w, _)| w)
                    .collect();
                if matches.len() != 1 {
                    break;
                }
                mapping.insert(v, matches[0]);
            }
            if mapping.len() != source.vertices.len() {
                continue;
            }
            // Opposing caps belong to the independent tunnel-join setting.
            let target: BTreeSet<_> = mapping.values().copied().collect();
            if polygons.iter().any(|p| {
                p.vertices.len() == target.len() && p.vertices.iter().all(|v| target.contains(v))
            }) {
                continue;
            }
            let sides: Vec<_> = source
                .edges()
                .map(|(a, b)| ([a, b, mapping[&b], mapping[&a]], source.group))
                .collect();
            let mut out = self.empty_topology();
            match self.joined_walls(polygons, &sides, &BTreeSet::from([face]), &mut out) {
                Ok(walls) if !walls.remove.is_empty() => return Ok(Some((depth, mapping))),
                _ => continue,
            }
        }
        Ok(None)
    }
}
