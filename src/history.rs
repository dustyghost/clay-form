//! Bounded mesh snapshots, one entry per stroke or explicit mesh edit.
use crate::sculpt::Geometry;
use std::collections::VecDeque;
const UNDO_LIMIT: usize = 24;
const MEMORY_LIMIT: usize = 128 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Retention {
    pub edits: usize,
    pub memory_mib: usize,
}
impl Default for Retention {
    fn default() -> Self {
        Self {
            edits: UNDO_LIMIT,
            memory_mib: MEMORY_LIMIT / 1024 / 1024,
        }
    }
}
impl Retention {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=256).contains(&self.edits) || !(16..=1024).contains(&self.memory_mib) {
            return Err("History requires 1–256 edits and 16–1024 MiB".into());
        }
        Ok(())
    }
}
pub(crate) struct Entry {
    pub geometry: Geometry,
    pub label: &'static str,
    pub id: u64,
    pub settings: Option<crate::diagnostics::EditSettings>,
    bytes: usize,
    settings_memory: crate::diagnostics::SettingsMemory,
}
#[derive(Default)]
pub(crate) struct History {
    pub undo: VecDeque<Entry>,
    pub redo: Vec<Entry>,
    pub retention: Retention,
    pub evicted: usize,
    pub discarded_redo: usize,
    next_id: u64,
}
impl History {
    pub fn reset(&mut self) {
        *self = Self {
            retention: self.retention,
            ..Default::default()
        };
    }
    #[cfg(test)]
    pub fn record_named(&mut self, mesh: &Geometry, label: &'static str) {
        self.record_owned(mesh.clone(), label, None);
    }
    pub fn record_owned(
        &mut self,
        geometry: Geometry,
        label: &'static str,
        settings: Option<crate::diagnostics::EditSettings>,
    ) {
        self.discarded_redo += self.redo.len();
        self.redo.clear();
        self.next_id += 1;
        let bytes = geometry_bytes(&geometry);
        let settings_memory = settings
            .as_ref()
            .map_or_else(Default::default, |s| s.memory());
        self.undo.push_back(Entry {
            geometry,
            label,
            id: self.next_id,
            settings,
            bytes,
            settings_memory,
        });
        self.enforce_limits();
    }
    pub fn retained_bytes(&self) -> usize {
        let mut shared = std::collections::HashMap::new();
        let mut owned = 0;
        for entry in self.undo.iter().chain(&self.redo) {
            owned += entry.bytes + entry.settings_memory.owned;
            shared.extend(entry.settings_memory.shared.iter().copied());
        }
        owned + shared.values().sum::<usize>()
    }
    pub fn enforce_limits(&mut self) {
        while self.undo.len() + self.redo.len() > self.retention.edits
            || self.retained_bytes() > self.retention.memory_mib * 1024 * 1024
        {
            // Preserve the contiguous stages closest to the current position.
            if self.undo.pop_front().is_none() {
                self.redo.remove(0);
            }
            self.evicted += 1;
        }
    }
    pub fn undo(&mut self, mesh: &mut Geometry) -> bool {
        if let Some(mut previous) = self.undo.pop_back() {
            std::mem::swap(mesh, &mut previous.geometry);
            previous.bytes = geometry_bytes(&previous.geometry);
            self.redo.push(previous);
            self.enforce_limits();
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self, mesh: &mut Geometry) -> bool {
        if let Some(mut next) = self.redo.pop() {
            std::mem::swap(mesh, &mut next.geometry);
            next.bytes = geometry_bytes(&next.geometry);
            self.undo.push_back(next);
            self.enforce_limits();
            true
        } else {
            false
        }
    }
    pub fn undo_label(&self) -> Option<&'static str> {
        self.undo.back().map(|entry| entry.label)
    }
    pub fn redo_label(&self) -> Option<&'static str> {
        self.redo.last().map(|entry| entry.label)
    }
    pub fn counts(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

fn geometry_bytes(geometry: &Geometry) -> usize {
    let path_bytes: usize = geometry
        .paths
        .iter()
        .map(|p| {
            std::mem::size_of::<crate::sculpt::paths::SurfacePath>()
                + p.name.len()
                + p.points.len() * std::mem::size_of::<crate::sculpt::paths::PathPoint>()
        })
        .sum();
    std::iter::once(&geometry.mesh)
        .chain(geometry.settings.lower.iter())
        .chain(geometry.settings.upper.iter())
        .chain(geometry.submodels.iter().flat_map(|model| {
            std::iter::once(&model.mesh)
                .chain(model.mesh_settings.lower.iter())
                .chain(model.mesh_settings.upper.iter())
        }))
        .map(|mesh| {
            mesh.vertices.len() * std::mem::size_of::<crate::sculpt::Vec3>()
                + mesh.mask.len() * std::mem::size_of::<f32>()
                + mesh.materials.len() * std::mem::size_of::<crate::sculpt::Material>()
                + mesh.triangles.len() * std::mem::size_of::<[usize; 3]>()
                + mesh.quads.len() * std::mem::size_of::<[usize; 4]>()
                + mesh.creases.len() * std::mem::size_of::<[usize; 2]>()
                + mesh.hidden_faces.len()
                + mesh.polygroups.len() * std::mem::size_of::<usize>()
                + mesh
                    .vertex_groups
                    .iter()
                    .map(|g| g.weights.len() * 4 + g.name.len())
                    .sum::<usize>()
        })
        .sum::<usize>()
        + path_bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::{Mesh, Vec3};
    #[test]
    fn oldest_snapshots_are_evicted_at_the_limit() {
        let mut history = History::default();
        let mut mesh = Geometry::new(Mesh {
            creases: Vec::new(),
            materials: Vec::new(),
            vertex_groups: Vec::new(),
            hidden_faces: Vec::new(),
            polygroups: Vec::new(),
            quads: Vec::new(),
            vertices: vec![Vec3::default()],
            triangles: vec![],
            mask: vec![0.],
        });
        for _ in 0..UNDO_LIMIT + 3 {
            history.record_named(&mesh, "Brush stroke");
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

#[cfg(test)]
mod retention_tests {
    use super::*;
    use crate::sculpt::{Mesh, Vec3};
    #[test]
    fn oversized_snapshots_are_evicted_and_redo_shares_the_budget() {
        let mut history = History {
            retention: Retention {
                edits: 3,
                memory_mib: 16,
            },
            ..Default::default()
        };
        let mut geometry = Geometry::new(Mesh::sphere());
        for _ in 0..3 {
            history.record_named(&geometry, "edit");
        }
        for _ in 0..3 {
            assert!(history.undo(&mut geometry));
        }
        history.retention.edits = 1;
        history.enforce_limits();
        assert_eq!(history.counts(), (0, 1));
        assert_eq!(history.redo[0].id, 1, "keep the nearest redo stage");
        assert_eq!(history.evicted, 2);
        geometry.mesh.vertices = vec![Vec3::default(); 1_500_000];
        history.record_named(&geometry, "oversized");
        assert_eq!(history.counts(), (0, 0));
        assert_eq!(history.evicted, 3);
        assert_eq!(history.discarded_redo, 1);
    }
}

#[cfg(test)]
mod shared_settings_tests {
    use super::*;
    use crate::{
        diagnostics::EditSettings,
        sculpt::{Alpha, Brush, Kind, Mesh},
    };
    #[test]
    fn shared_images_keep_undo_and_are_counted_once_across_both_stacks() {
        let pixels: std::sync::Arc<[f32]> = vec![0.12345; 2048 * 2048].into();
        let mut brush = Brush::new(Kind::Standard);
        brush.alpha = Alpha::Image {
            width: 2048,
            height: 2048,
            pixels,
        };
        let settings = EditSettings {
            brushes: vec![brush.clone(); 3],
            active_brush: Some(brush),
            symmetry: Default::default(),
            topology: Default::default(),
            view_input: None,
            first_dab: None,
        };
        let mut history = History::default();
        let mut geometry = Geometry::new(Mesh::default());
        for _ in 0..8 {
            history.record_owned(geometry.clone(), "stroke", Some(settings.clone()));
        }
        assert_eq!(history.counts(), (8, 0));
        assert!((16 * 1024 * 1024..17 * 1024 * 1024).contains(&history.retained_bytes()));
        let bytes = history.retained_bytes();
        assert!(history.undo(&mut geometry));
        assert!(history.undo(&mut geometry));
        assert_eq!(history.retained_bytes(), bytes);
        assert!(history.redo(&mut geometry));
        assert_eq!(history.retained_bytes(), bytes);
        history.record_owned(geometry.clone(), "branch", None);
        assert_eq!(history.counts(), (8, 0));
        history.reset();
        assert_eq!(history.retained_bytes(), 0);
    }
}
