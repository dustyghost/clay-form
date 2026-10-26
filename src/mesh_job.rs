//! Background geometry work. Results are committed by the controller as one edit.
use crate::sculpt::Geometry;
use crate::{background_job::BackgroundJob, sculpt::ValidatedGeometry};

#[derive(Clone)]
pub(crate) enum Operation {
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
                Operation::Subdivide => geometry.subdivide_cancellable(&cancel)?,
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
