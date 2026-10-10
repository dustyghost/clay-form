//! Bounded mesh snapshots, one entry per stroke or explicit mesh edit.
use crate::sculpt::Mesh;
use std::collections::VecDeque;
const UNDO_LIMIT: usize = 24;
#[derive(Default)]
pub(crate) struct History {
    undo: VecDeque<Mesh>,
    redo: Vec<Mesh>,
}
impl History {
    pub fn record(&mut self, mesh: &Mesh) {
        self.push_undo(mesh.clone());
        self.redo.clear();
    }
    fn push_undo(&mut self, mesh: Mesh) {
        if self.undo.len() == UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(mesh);
    }
    pub fn undo(&mut self, mesh: &mut Mesh) -> bool {
        if let Some(previous) = self.undo.pop_back() {
            self.redo.push(std::mem::replace(mesh, previous));
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, mesh: &mut Mesh) -> bool {
        if let Some(next) = self.redo.pop() {
            let previous = std::mem::replace(mesh, next);
            self.push_undo(previous);
            true
        } else {
            false
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::Vec3;
    #[test]
    fn oldest_snapshots_are_evicted_at_the_limit() {
        let mut history = History::default();
        let mut mesh = Mesh {
            vertices: vec![Vec3::default()],
            triangles: vec![],
            mask: vec![0.],
        };
        for _ in 0..UNDO_LIMIT + 3 {
            history.record(&mesh);
            mesh.vertices[0].0 += 1.;
        }
        for _ in 0..UNDO_LIMIT {
            assert!(history.undo(&mut mesh));
        }
        assert!(!history.undo(&mut mesh));
        assert_eq!(mesh.vertices[0].0, 3.);
        for _ in 0..UNDO_LIMIT {
            assert!(history.redo(&mut mesh));
        }
        assert!(!history.redo(&mut mesh));
        assert_eq!(mesh.vertices[0].0, (UNDO_LIMIT + 3) as f32);
    }
}
