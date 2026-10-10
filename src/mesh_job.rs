//! Background geometry work. Results are committed by the controller as one edit.
use crate::sculpt::Geometry;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
};

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    Subdivide,
    Remesh,
}
impl Operation {
    pub fn history_label(self) -> &'static str {
        match self {
            Self::Subdivide => "Subdivide mesh",
            Self::Remesh => "Remesh",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Subdivide => "Subdividing",
            Self::Remesh => "Remeshing",
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
        std::thread::Builder::new()
            .name("clay-mesh-operation".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    match operation {
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
