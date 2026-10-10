//! Bounded mesh snapshots, one entry per stroke or explicit mesh edit.
use crate::sculpt::Geometry;
use std::collections::VecDeque;
const UNDO_LIMIT: usize = 24;
const MEMORY_LIMIT: usize = 128 * 1024 * 1024;
#[derive(Default)]
pub(crate) struct History {
    undo: VecDeque<Geometry>,
    redo: Vec<Geometry>,
}
impl History {
    pub fn record(&mut self, mesh: &Geometry) {
        self.redo.clear();
        self.push_undo(mesh.clone());
    }
    fn push_undo(&mut self, mesh: Geometry) {
        if self.undo.len() == UNDO_LIMIT {
            self.undo.pop_front();
        }
        self.undo.push_back(mesh);
        while self.undo.len() > 1
            && self
                .undo
                .iter()
                .chain(self.redo.iter())
                .map(geometry_bytes)
                .sum::<usize>()
                > MEMORY_LIMIT
        {
            self.undo.pop_front();
        }
    }
    pub fn undo(&mut self, mesh: &mut Geometry) -> bool {
        if let Some(previous) = self.undo.pop_back() {
            self.redo.push(std::mem::replace(mesh, previous));
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, mesh: &mut Geometry) -> bool {
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

fn geometry_bytes(geometry: &Geometry) -> usize {
    std::iter::once(&geometry.mesh)
        .chain(geometry.settings.lower.iter())
        .chain(geometry.settings.upper.iter())
        .map(|mesh| {
            mesh.vertices.len() * std::mem::size_of::<crate::sculpt::Vec3>()
                + mesh.mask.len() * std::mem::size_of::<f32>()
                + mesh.triangles.len() * std::mem::size_of::<[usize; 3]>()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Mesh, Vec3};
    #[test]
    fn oldest_snapshots_are_evicted_at_the_limit() {
        let mut history = History::default();
        let mut mesh = Geometry::new(Mesh {
            vertices: vec![Vec3::default()],
            triangles: vec![],
            mask: vec![0.],
        });
        for _ in 0..UNDO_LIMIT + 3 {
            history.record(&mesh);
            mesh.mesh.vertices[0].0 += 1.;
        }
        for _ in 0..UNDO_LIMIT {
            assert!(history.undo(&mut mesh));
        }
        assert!(!history.undo(&mut mesh));
        assert_eq!(mesh.mesh.vertices[0].0, 3.);
        for _ in 0..UNDO_LIMIT {
            assert!(history.redo(&mut mesh));
        }
        assert!(!history.redo(&mut mesh));
        assert_eq!(mesh.mesh.vertices[0].0, (UNDO_LIMIT + 3) as f32);
    }
}
