//! Background geometry work. Results are committed by the controller as one edit.
use crate::sculpt::Geometry;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

#[derive(Clone)]
pub(crate) enum Operation {
    ImportObj(String),
    Mirror(crate::sculpt::MirrorOptions),
    SlicePath(crate::sculpt::SlicePath, crate::sculpt::SliceOptions),
    Subdivide,
    Remesh,
    Slice(crate::sculpt::SlicePlane, crate::sculpt::SliceOptions),
}
impl Operation {
    pub fn history_label(&self) -> &'static str {
        match self {
            Self::ImportObj(_) => "Import OBJ",
            Self::Mirror(_) => "Mirror one side",
            Self::Subdivide => "Subdivide mesh",
            Self::Remesh => "Remesh",
            Self::Slice(_, options) | Self::SlicePath(_, options) => options.mode.history_label(),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::ImportObj(_) => "Importing OBJ",
            Self::Mirror(_) => "Mirroring",
            Self::Subdivide => "Subdividing",
            Self::Remesh => "Remeshing",
            Self::Slice(..) | Self::SlicePath(..) => "Slicing",
        }
    }
}
pub(crate) struct MeshJob {
    receiver: Receiver<Result<Geometry, String>>,
    cancel: Arc<AtomicBool>,
    pub operation: Operation,
}
impl MeshJob {
    pub fn start(mut geometry: Geometry, operation: Operation) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let worker_operation = operation.clone();
        std::thread::Builder::new()
            .name("clay-mesh-operation".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    match worker_operation {
                        Operation::Mirror(options) => {
                            geometry.mirror(options, &|| flag.load(Ordering::Relaxed))?;
                        }
                        Operation::ImportObj(path) => {
                            let mesh = crate::persistence::load_obj_cancellable(&path, &|| {
                                flag.load(Ordering::Relaxed)
                            })?;
                            let name = std::path::Path::new(&path)
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("Imported OBJ");
                            geometry.import_mesh(mesh, name)?;
                        }
                        Operation::SlicePath(path, options) => {
                            geometry.slice_path(&path, options, &|| flag.load(Ordering::Relaxed))?
                        }
                        Operation::Slice(plane, options) => {
                            geometry.slice(plane, options, &|| flag.load(Ordering::Relaxed))?
                        }
                        Operation::Subdivide => geometry.subdivide()?,
                        Operation::Remesh => {
                            if !geometry.settings.dynamic {
                                return Err("Switch to dynamic mode first".into());
                            }
                            geometry.mesh = geometry
                                .mesh
                                .remesh_cancellable(geometry.settings.resolution, &|| {
                                    flag.load(Ordering::Relaxed)
                                })?;
                        }
                    }
                    Ok(geometry)
                }))
                .unwrap_or_else(|_| {
                    Err("Mesh operation failed; the original mesh was kept".into())
                });
                let _ = sender.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            receiver,
            cancel,
            operation,
        })
    }
    pub fn poll(&self) -> Option<Result<Geometry, String>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("Mesh worker stopped unexpectedly".into())),
        }
    }
}
impl Drop for MeshJob {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
