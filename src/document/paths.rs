//! Transactional persistent path edits share the geometry undo timeline.
use super::*;
use crate::sculpt::paths::{self, PathMode, PathPoint, PathSample, SurfacePath};

impl Document {
    pub fn paths(&self) -> &[SurfacePath] {
        &self.geometry.paths
    }
    pub fn path(&self, id: u64) -> Option<&SurfacePath> {
        self.paths().iter().find(|p| p.id == id)
    }
    pub fn active_model_id(&self) -> u64 {
        self.geometry.scene.active_metadata.model_id
    }
    pub fn create_path(
        &mut self,
        name: String,
        mode: PathMode,
        points: Vec<PathPoint>,
        closed: bool,
        smoothing: f32,
    ) -> Result<u64, String> {
        self.create_path_with_rounding(name, mode, points, closed, smoothing, Vec::new())
    }
    pub fn create_path_with_rounding(
        &mut self,
        name: String,
        mode: PathMode,
        points: Vec<PathPoint>,
        closed: bool,
        smoothing: f32,
        corner_rounding: Vec<f32>,
    ) -> Result<u64, String> {
        if self.paths().len() >= paths::MAX_PATHS {
            return Err("Paths are limited to 128 per project".into());
        }
        let mut next = self.geometry.clone();
        if next.scene.active_metadata.model_id == 0 {
            let last = std::iter::once(next.scene.active_metadata.model_id)
                .chain(next.submodels.iter().map(|m| m.metadata.model_id))
                .chain(next.paths.iter().map(|p| p.target))
                .max()
                .unwrap_or(0);
            next.scene.active_metadata.model_id =
                last.checked_add(1).ok_or("Model identity limit reached")?;
        }
        let id = next
            .paths
            .iter()
            .map(|p| p.id)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Path identity limit reached")?;
        let mut path = SurfacePath {
            id,
            name,
            target: next.scene.active_metadata.model_id,
            mode,
            points,
            closed,
            smoothing,
            corner_rounding,
            seam: 0,
            surface_signature: paths::mesh_signature(self.mesh()),
            attached_route: None,
        };
        path.validate()?;
        if mode == PathMode::Surface {
            if !self.active_visible() {
                return Err("Show the target model before creating a surface path".into());
            }
            // Picked positions are already on the target; accommodate floating
            // point ray/triangle error without allowing a jump to another sheet.
            path.reproject(self.mesh(), 0.01)?;
        }
        next.paths.push(path);
        self.replace_geometry_named(next, "Create path")?;
        Ok(id)
    }
    pub fn update_path(&mut self, mut path: SurfacePath) -> Result<(), String> {
        path.validate()?;
        let old = self.path(path.id).ok_or("Path no longer exists")?;
        if path.target != old.target || path.mode != old.mode {
            return Err("Use explicit reprojection to change a path attachment".into());
        }
        if path == *old {
            return Ok(());
        }
        let shape_changed = path.points != old.points
            || path.smoothing != old.smoothing
            || path.corner_rounding != old.corner_rounding
            || path.closed != old.closed
            || path.seam != old.seam;
        let preserves_route = path.has_attached_route();
        if shape_changed && !preserves_route {
            path.attached_route = None;
        }
        self.require_path_target(old)?;
        if old.is_stale(self.mesh()) && shape_changed {
            return Err("Path surface changed; reproject it before editing its shape".into());
        }
        path.surface_signature = old.surface_signature;
        if path.mode == PathMode::Surface && shape_changed && !preserves_route {
            path.reproject(self.mesh(), 0.01)?;
        }
        let mut next = self.geometry.clone();
        let id = path.id;
        *next.paths.iter_mut().find(|p| p.id == id).unwrap() = path;
        self.replace_geometry_named(next, "Edit path")
    }
    pub fn delete_path(&mut self, id: u64) -> Result<(), String> {
        if self.path(id).is_none() {
            return Err("Path no longer exists".into());
        }
        let mut next = self.geometry.clone();
        next.paths.retain(|p| p.id != id);
        self.replace_geometry_named(next, "Delete path")
    }
    pub fn reproject_path(&mut self, id: u64, max_distance: f32) -> Result<(), String> {
        let mut path = self.path(id).ok_or("Path no longer exists")?.clone();
        self.require_path_target(&path)?;
        path.reproject(self.mesh(), max_distance)?;
        if self.path(id) == Some(&path) {
            return Ok(());
        }
        let mut next = self.geometry.clone();
        *next.paths.iter_mut().find(|p| p.id == id).unwrap() = path;
        self.replace_geometry_named(next, "Reproject path")
    }
    pub fn path_samples(&self, id: u64, spacing: f32) -> Result<Vec<PathSample>, String> {
        let path = self.path(id).ok_or("Path no longer exists")?;
        self.require_path_target(path)?;
        path.samples_on_mesh(self.mesh(), spacing)
    }
    fn require_path_target(&self, path: &SurfacePath) -> Result<(), String> {
        if path.target != self.active_model_id() {
            return Err("Select this path's original target model before using it".into());
        }
        if !self.active_visible() {
            return Err("Show the path's target model before using it".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn points() -> Vec<PathPoint> {
        vec![
            PathPoint {
                position: crate::sculpt::Vec3(0., 0., 0.),
                normal: crate::sculpt::Vec3(0., 0., 1.),
            },
            PathPoint {
                position: crate::sculpt::Vec3(1., 0., 0.),
                normal: crate::sculpt::Vec3(0., 0., 1.),
            },
        ]
    }
    #[test]
    fn individual_corner_rounding_creates_saves_and_undoes_in_one_edit() {
        let mut document = Document::default();
        let mut points = points();
        points.push(PathPoint {
            position: crate::sculpt::Vec3(1., 1., 0.),
            normal: crate::sculpt::Vec3(0., 0., 1.),
        });
        let id = document
            .create_path_with_rounding(
                "Rounded guide".into(),
                PathMode::Free,
                points,
                false,
                0.,
                vec![0., 0.2, 0.],
            )
            .unwrap();
        assert_eq!(document.history_counts(), (1, 0));
        assert_eq!(document.path(id).unwrap().corner_rounding(1), 0.2);
        let project = document.project();
        let mut json = serde_json::to_value(&project).unwrap();
        let restored: Project = serde_json::from_value(json.clone()).unwrap();
        assert!(restored == project);
        json["paths"][0]
            .as_object_mut()
            .unwrap()
            .remove("corner_rounding");
        let legacy: Project = serde_json::from_value(json).unwrap();
        assert!(legacy.paths[0].corner_rounding.is_empty());
        document.undo();
        assert!(document.paths().is_empty());
        document.redo();
        assert_eq!(document.path(id).unwrap().corner_rounding(1), 0.2);
        let mut path = document.path(id).unwrap().clone();
        path.set_corner_rounding(1, 0.3).unwrap();
        document.update_path(path).unwrap();
        document.undo();
        assert_eq!(document.path(id).unwrap().corner_rounding(1), 0.2);
    }
    #[test]
    fn path_edits_roundtrip_and_share_mesh_history() {
        let mut document = Document::default();
        let id = document
            .create_path("Guide".into(), PathMode::Free, points(), false, 0.5)
            .unwrap();
        let target = document.active_model_id();
        assert_ne!(target, 0);
        let project = document.project();
        project.validate().unwrap();
        let json = serde_json::to_vec(&project).unwrap();
        let decoded: Project = serde_json::from_slice(&json).unwrap();
        assert!(decoded == project);
        document.undo();
        assert!(document.paths().is_empty());
        assert_eq!(document.active_model_id(), 0);
        document.redo();
        assert_eq!(document.path(id).unwrap().target, target);
        document.delete_path(id).unwrap();
        assert!(document.paths().is_empty());
        document.undo();
        assert!(document.path(id).is_some());
    }
    #[test]
    fn attachment_identity_survives_duplicate_selection_and_target_deletion() {
        let mut document = Document::default();
        let id = document
            .create_path("Guide".into(), PathMode::Free, points(), false, 0.)
            .unwrap();
        let target = document.active_model_id();
        document.duplicate_model(false).unwrap();
        assert_ne!(document.active_model_id(), target);
        assert!(document.path_samples(id, 0.1).is_err());
        document.select_submodel(1).unwrap();
        assert_eq!(document.active_model_id(), target);
        assert!(document.path_samples(id, 0.1).is_ok());
        document.delete_submodel();
        assert!(document.path(id).is_some());
        assert!(document.path_samples(id, 0.1).is_err());
        document.project().validate().unwrap();
        document.undo();
        assert!(document.path_samples(id, 0.1).is_ok());
    }
    #[test]
    fn older_projects_default_to_no_paths_and_unassigned_model_ids() {
        let mut json = serde_json::to_value(Document::default().project()).unwrap();
        json.as_object_mut().unwrap().remove("paths");
        let project: Project = serde_json::from_value(json).unwrap();
        project.validate().unwrap();
        assert!(project.paths.is_empty());
        assert_eq!(project.scene.active_metadata.model_id, 0);
    }
    #[test]
    fn invalid_path_open_preserves_document() {
        let mut document = Document::default();
        document
            .create_path("Guide".into(), PathMode::Free, points(), false, 0.)
            .unwrap();
        let before = document.project();
        let mut invalid = before.clone();
        invalid.paths[0].points[0].normal = crate::sculpt::Vec3::default();
        assert!(document.replace_project(invalid, "bad.clay").is_err());
        assert!(document.project() == before);
    }
    #[test]
    fn guide_reattachment_shares_the_completed_strokes_undo_entry() {
        use crate::sculpt::Vec3;
        let mesh = Mesh {
            vertices: vec![
                Vec3(-1., -1., 0.),
                Vec3(1., -1., 0.),
                Vec3(1., 1., 0.),
                Vec3(-1., 1., 0.),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            mask: vec![0.; 4],
            ..Default::default()
        };
        let mut document = Document::default();
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        let id = document
            .create_path("Guide".into(), PathMode::Surface, points(), false, 0.)
            .unwrap();
        let before = document.project();
        let undo_before = document.history_counts().0;
        let mut next = document.geometry_snapshot();
        for p in &mut next.mesh.vertices {
            p.2 += 0.1;
        }
        next.paths[0]
            .reattach_after_stroke(document.mesh(), &next.mesh, 0.01, &|| false)
            .unwrap();
        document
            .replace_geometry_named(next, "Guided brush stroke")
            .unwrap();
        assert_eq!(document.history_counts().0, undo_before + 1);
        assert!(!document.path(id).unwrap().is_stale(document.mesh()));
        assert!(document.path(id).unwrap().attached_route.is_some());
        assert!(document.path_samples(id, 0.1).is_ok());
        let after = document.project();
        let encoded = serde_json::to_vec(&after).unwrap();
        let mut reopened = Document::default();
        reopened
            .replace_project(serde_json::from_slice(&encoded).unwrap(), "reattached.clay")
            .unwrap();
        assert_eq!(
            reopened.path_samples(id, 0.1).unwrap(),
            document.path_samples(id, 0.1).unwrap()
        );
        document.undo();
        assert!(document.project() == before);
        document.redo();
        assert!(document.project() == after);
        assert!(document.path_samples(id, 0.1).is_ok());
        let mut reversed = document.path(id).unwrap().clone();
        reversed.reverse();
        let expected = reversed.attached_route.clone();
        document.update_path(reversed).unwrap();
        assert_eq!(document.path(id).unwrap().attached_route, expected);
        document.undo();
        assert!(document.project() == after);
    }
}
