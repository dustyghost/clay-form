//! Background geometry work. Results are committed by the controller as one edit.
use crate::sculpt::Geometry;
use crate::{background_job::BackgroundJob, sculpt::ValidatedGeometry};

#[derive(Clone)]
pub(crate) enum Operation {
    BakeInstances(crate::sculpt::instances::BakeRequest),
    UvUnwrap(bool, f32),
    ImportObj(String),
    Edges(crate::sculpt::EdgeEdit),
    Mirror(crate::sculpt::MirrorOptions),
    SlicePath(crate::sculpt::SlicePath, crate::sculpt::SliceOptions),
    Subdivide,
    Remesh,
    Slice(crate::sculpt::SlicePlane, crate::sculpt::SliceOptions),
}
impl Operation {
    pub fn history_label(&self) -> &'static str {
        match self {
            Self::BakeInstances(..) => "Bake instances",
            Self::UvUnwrap(..) => "Unwrap UVs",
            Self::Edges(edit) => edit.label(),
            Self::ImportObj(_) => "Import OBJ",
            Self::Mirror(_) => "Mirror one side",
            Self::Subdivide => "Subdivide mesh",
            Self::Remesh => "Remesh",
            Self::Slice(_, options) | Self::SlicePath(_, options) => options.mode.history_label(),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::BakeInstances(..) => "Baking instances",
            Self::UvUnwrap(..) => "Unwrapping UVs",
            Self::Edges(_) => "Editing edges",
            Self::ImportObj(_) => "Importing OBJ",
            Self::Mirror(_) => "Mirroring",
            Self::Subdivide => "Subdividing",
            Self::Remesh => "Remeshing",
            Self::Slice(..) | Self::SlicePath(..) => "Slicing",
        }
    }
}
pub(crate) struct MeshJob {
    worker: BackgroundJob<ValidatedGeometry>,
    pub operation: Operation,
}
impl MeshJob {
    pub fn start(mut geometry: Geometry, operation: Operation) -> Result<Self, String> {
        let worker_operation = operation.clone();
        let worker = BackgroundJob::start("clay-mesh-operation", move |cancel| {
            match worker_operation {
                Operation::BakeInstances(request) => geometry.bake_instances(request, cancel)?,
                Operation::UvUnwrap(automatic, padding) => {
                    geometry.mesh.uv = geometry.mesh.unwrap_uv(automatic, padding, &cancel)?;
                }
                Operation::Edges(edit) => geometry.edit_edges(&edit, &cancel)?,
                Operation::Mirror(options) => {
                    geometry.mirror(options, &cancel)?;
                }
                Operation::ImportObj(path) => {
                    let mesh = crate::persistence::load_obj_cancellable(&path, &cancel)?;
                    let name = std::path::Path::new(&path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Imported OBJ");
                    geometry.import_mesh(mesh, name)?;
                }
                Operation::SlicePath(path, options) => {
                    geometry.slice_path(&path, options, &cancel)?
                }
                Operation::Slice(plane, options) => geometry.slice(plane, options, &cancel)?,
                Operation::Subdivide => return ValidatedGeometry::subdivided(geometry, &cancel),
                Operation::Remesh => {
                    if !geometry.settings.dynamic {
                        return Err("Switch to dynamic mode first".into());
                    }
                    geometry.mesh = geometry.mesh.remesh_algorithm_cancellable(
                        geometry.settings.resolution,
                        geometry.settings.remesh_topology,
                        geometry.settings.quad_remesh_algorithm,
                        &cancel,
                    )?;
                }
            }
            ValidatedGeometry::new(geometry, &cancel)
        })?;
        Ok(Self { worker, operation })
    }
    pub fn cancel(&self) {
        self.worker.cancel();
    }
    pub fn is_cancelled(&self) -> bool {
        self.worker.is_cancelled()
    }
    pub fn poll(&mut self) -> Option<Result<ValidatedGeometry, String>> {
        self.worker.poll()
    }
}

#[cfg(test)]
mod subdivision_profile {
    use super::*;
    use crate::sculpt::Mesh;

    #[test]
    #[ignore = "manual bounded subdivision timings; run with --ignored --nocapture"]
    fn subdivision_worker_profile() {
        let mut geometry = Geometry::new(Mesh::starting_sphere());
        println!("input_vertices,output_vertices,worker_ms");
        for _ in 0..3 {
            let input = geometry.mesh.vertices.len();
            let start = std::time::Instant::now();
            let mut job = MeshJob::start(geometry, Operation::Subdivide).unwrap();
            geometry = loop {
                if let Some(result) = job.poll() {
                    break result.unwrap().into_geometry();
                }
                assert!(
                    start.elapsed().as_secs() < 120,
                    "subdivision worker exceeded profile deadline"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            };
            println!(
                "{input},{},{:.2}",
                geometry.mesh.vertices.len(),
                start.elapsed().as_secs_f64() * 1000.
            );
        }
        if let Ok(path) = std::env::var("CLAY_SUBDIVISION_PROFILE") {
            std::fs::write(
                path,
                serde_json::to_vec(&(&geometry.mesh, &geometry.settings)).unwrap(),
            )
            .unwrap();
        }
    }
}
