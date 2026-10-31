//! One prepared view per rendering role; immutable mesh identity controls reuse.
use super::{Arc, Camera, DisplayMesh, Mutex, Scene, egui};

#[derive(PartialEq, Eq)]
struct Key {
    bits: [u32; 12],
    perspective: bool,
    sort: bool,
}
impl Key {
    fn new(rect: egui::Rect, camera: &Camera, sort: bool) -> Self {
        Self {
            bits: [
                rect.min.x,
                rect.min.y,
                rect.max.x,
                rect.max.y,
                camera.yaw,
                camera.pitch,
                camera.roll,
                camera.zoom,
                camera.pan.x,
                camera.pan.y,
                camera.field_of_view,
                camera.perspective_radius,
            ]
            .map(f32::to_bits),
            perspective: camera.perspective,
            sort,
        }
    }
}
#[derive(Default)]
pub(in crate::viewport) struct Cache {
    view: Option<(Key, Scene)>,
}
impl Cache {
    pub fn clear(&mut self) {
        self.view = None;
    }
    pub fn get(
        &mut self,
        rect: egui::Rect,
        camera: &Camera,
        geometry: Arc<DisplayMesh>,
        sort: bool,
    ) -> Scene {
        let key = Key::new(rect, camera, sort);
        if let Some((old_key, old)) = &self.view
            && *old_key == key
            && Arc::ptr_eq(&old.geometry.mesh.vertices, &geometry.mesh.vertices)
            && Arc::ptr_eq(&old.geometry.mesh.triangles, &geometry.mesh.triangles)
            && Arc::ptr_eq(&old.geometry.owners, &geometry.owners)
            && old.active == geometry.active
            && old.geometry.active_vertices == geometry.active_vertices
            && old.geometry.depth_extent.to_bits() == geometry.depth_extent.to_bits()
        {
            let mut scene = old.clone();
            // Projection/bounds depend only on geometry and view. A hit also
            // contains a shading normal, which can change on stroke release.
            if !Arc::ptr_eq(&old.geometry, &geometry) {
                scene.last_pick = Arc::new(Mutex::new(None));
            }
            scene.geometry = geometry;
            self.view = Some((key, scene.clone()));
            return scene;
        }
        let scene = Scene::from_geometry(rect, camera, geometry, sort);
        self.view = Some((key, scene.clone()));
        scene
    }
}
