use super::*;
use crate::{
    background_job::BackgroundJob,
    sculpt::{
        Mesh,
        fusion::{self, Boundary},
    },
};
use std::sync::Arc;

enum ResultMesh {
    Openings(Mesh, [Vec<Boundary>; 2]),
    Preview(Mesh, fusion::FusionReport),
}
struct Worker {
    job: BackgroundJob<ResultMesh>,
    revision: u64,
    sources: [usize; 2],
    key: Option<crate::ui::FusionKey>,
}
struct Draft {
    mesh: Mesh,
    revision: u64,
    key: crate::ui::FusionKey,
}
#[derive(Default)]
pub(super) struct FusionState {
    pub display: Option<crate::viewport::JoinDisplay>,
    pub patch: Vec<[crate::sculpt::Vec3; 2]>,
    pub removed: Vec<[crate::sculpt::Vec3; 2]>,
    worker: Option<Worker>,
    prepared: Option<(Arc<Mesh>, u64, [usize; 2])>,
    draft: Option<Draft>,
}
impl App {
    pub(super) fn open_fusion(&mut self) -> Result<(), String> {
        self.viewport.cancel(&mut self.document);
        let first = self.document.active_model_index();
        let second = self.document.merge_down_target().unwrap_or(first);
        self.ui.fusion.open = true;
        self.ui.fusion.sources = [first, second];
        self.discover_fusion()
    }
    pub(super) fn clear_fusion(&mut self) {
        self.fusion.worker = None;
        self.fusion.draft = None;
        self.fusion.display = None;
        self.fusion.patch.clear();
        self.fusion.removed.clear();
        self.viewport.session.join_patch.clear();
        self.viewport.session.join_removed.clear();
        self.viewport.set_join_preview(None);
        self.ui.fusion.ready = false;
        self.ui.fusion.busy = false;
        self.ui.fusion.notice = if self.ui.fusion.intersections {
            "Preview the local intersection patches."
        } else {
            "Choose both openings, then preview the join."
        }
        .into();
    }
    pub(super) fn cancel_fusion(&mut self) {
        self.clear_fusion();
        self.fusion.prepared = None;
        self.ui.fusion.open = false;
        self.ui.fusion.loops = Default::default();
        self.viewport.session.join_rims.clear();
        self.viewport.session.join_pick = None;
        self.viewport.wait_for_release();
    }
    pub(super) fn discover_fusion(&mut self) -> Result<(), String> {
        self.clear_fusion();
        self.fusion.prepared = None;
        self.ui.fusion.loops = Default::default();
        self.ui.fusion.selected = [None, None];
        let sources = self.ui.fusion.sources;
        let [a, b] = self.document.fusion_sources(sources)?;
        let job = BackgroundJob::start("clay-join-openings", move |cancel| {
            fusion::cancelled(cancel)?;
            let first = fusion::boundaries(&a)?;
            let mut second = if sources[0] == sources[1] {
                first.clone()
            } else {
                fusion::boundaries(&b)?
            };
            let mesh = if sources[0] == sources[1] {
                a
            } else {
                for boundary in &mut second {
                    for v in &mut boundary.vertices {
                        *v += a.vertices.len();
                    }
                }
                fusion::merge(&a, &b)?
            };
            Ok(ResultMesh::Openings(mesh, [first, second]))
        })?;
        self.fusion.worker = Some(Worker {
            job,
            revision: self.document.display_revision(),
            sources,
            key: None,
        });
        self.ui.fusion.busy = true;
        self.ui.fusion.notice = "Finding open boundary loops…".into();
        Ok(())
    }
    pub(super) fn preview_fusion(&mut self) -> Result<(), String> {
        if self.ui.fusion.busy {
            return Err("Wait for the current preview or cancel it first".into());
        }
        let key = self.ui.fusion.key();
        let (mesh, revision, sources) = self
            .fusion
            .prepared
            .clone()
            .ok_or("Choose two valid source models first")?;
        if revision != self.document.display_revision() || sources != self.ui.fusion.sources {
            return Err("The sources changed; choose them again".into());
        }
        let selected = self.ui.fusion.selected;
        let options = self.ui.fusion.options;
        let intersections = self.ui.fusion.intersections;
        let collar = self.ui.fusion.collar;
        let job = if intersections {
            if sources[0] == sources[1] {
                return Err("Choose two different models for intersection fusion".into());
            }
            let [a, b] = self.document.fusion_sources(sources)?;
            BackgroundJob::start("clay-local-fusion", move |cancel| {
                fusion::fuse(&a, &b, collar, options, cancel)
                    .map(|(mesh, report)| ResultMesh::Preview(mesh, report))
            })?
        } else {
            let first = self.ui.fusion.loops[0]
                .get(selected[0].ok_or("Choose the first opening")?)
                .ok_or("Choose a current first opening")?
                .vertices
                .clone();
            let second = self.ui.fusion.loops[1]
                .get(selected[1].ok_or("Choose the second opening")?)
                .ok_or("Choose a current second opening")?
                .vertices
                .clone();
            BackgroundJob::start("clay-local-join", move |cancel| {
                fusion::join_open_ends(&mesh, &first, &second, options, cancel)
                    .map(|(mesh, report)| ResultMesh::Preview(mesh, report))
            })?
        };
        self.clear_fusion();
        self.fusion.worker = Some(Worker {
            job,
            revision,
            sources,
            key: Some(key),
        });
        self.ui.fusion.busy = true;
        self.ui.fusion.notice = "Building the local connector…".into();
        Ok(())
    }
    pub(super) fn poll_fusion(&mut self) {
        if self.fusion.draft.as_ref().is_some_and(|d| {
            d.revision != self.document.display_revision() || d.key != self.ui.fusion.key()
        }) {
            self.clear_fusion();
            self.ui.fusion.notice = "Scene or join settings changed; preview the join again".into();
        }
        if let Some(result) = self.fusion.worker.as_mut().and_then(|w| w.job.poll()) {
            let worker = self.fusion.worker.take().unwrap();
            self.ui.fusion.busy = false;
            let result = if worker.revision != self.document.display_revision()
                || worker.sources != self.ui.fusion.sources
                || worker.key.is_some_and(|k| k != self.ui.fusion.key())
            {
                Err("Scene or join settings changed; preview the join again".into())
            } else {
                result
            };
            match result {
                Ok(ResultMesh::Openings(mesh, loops)) => {
                    self.ui.fusion.notice = format!(
                        "{} and {} openings found. Select the intended pair.",
                        loops[0].len(),
                        loops[1].len()
                    );
                    self.ui.fusion.loops = loops;
                    self.fusion.prepared = Some((Arc::new(mesh), worker.revision, worker.sources));
                }
                Ok(ResultMesh::Preview(mesh, report)) => {
                    if let Some((source, _, _)) = &self.fusion.prepared {
                        use std::collections::BTreeSet;
                        let start = report.retained_triangles;
                        let edges = |triangles: &[[usize; 3]]| {
                            triangles
                                .iter()
                                .flat_map(|t| {
                                    (0..3)
                                        .map(move |i| crate::sculpt::uv::edge(t[i], t[(i + 1) % 3]))
                                })
                                .collect::<BTreeSet<_>>()
                        };
                        self.fusion.patch = edges(&mesh.triangles[start..])
                            .into_iter()
                            .map(|e| e.map(|v| mesh.vertices[v]))
                            .collect();
                        let retained = mesh.triangles[..start]
                            .iter()
                            .copied()
                            .collect::<BTreeSet<_>>();
                        let removed = source
                            .triangles
                            .iter()
                            .filter(|t| !retained.contains(*t))
                            .copied()
                            .collect::<Vec<_>>();
                        self.fusion.removed = edges(&removed)
                            .into_iter()
                            .map(|e| e.map(|v| source.vertices[v]))
                            .collect();
                    }
                    let result = self
                        .document
                        .fusion_geometry(mesh.clone(), worker.sources)
                        .and_then(|g| {
                            let mut d = Document::default();
                            d.replace_geometry_named(g, "Join preview")?;
                            Ok(d)
                        });
                    match result {
                        Ok(document) => {
                            self.fusion.display =
                                Some(crate::viewport::JoinDisplay::new(&document));
                            self.viewport.set_join_preview(self.fusion.display.clone());
                            self.ui.fusion.notice = if self.ui.fusion.intersections {
                                format!(
                                    "{} join(s) · {} + {} source polygons removed · {} original polygons retained. Inspect the new local patches.",
                                    report.joins,
                                    report.removed_faces[0],
                                    report.removed_faces[1],
                                    report.retained_faces
                                )
                            } else {
                                "Preview ready. Inspect the connector, then Apply or Cancel.".into()
                            };
                            self.ui.fusion.has_removed_faces = !self.fusion.removed.is_empty();
                            if report.enclosed_faces > 0 {
                                self.ui.fusion.notice.push_str(&format!(
                                    " Culled {} enclosed polygons.",
                                    report.enclosed_faces
                                ));
                            }
                            self.ui.fusion.ready = true;
                            self.fusion.draft = Some(Draft {
                                mesh,
                                revision: worker.revision,
                                key: worker.key.expect("preview job has settings"),
                            });
                        }
                        Err(e) => self.ui.fusion.notice = e,
                    }
                }
                Err(e) => self.ui.fusion.notice = e,
            }
        }
        if self.ui.fusion.open
            && let Some((side, index)) = self.viewport.session.join_pick.take()
        {
            self.ui.fusion.selected[side] = Some(index);
            self.clear_fusion();
        }
        self.viewport.session.join_rims.clear();
        if self.ui.fusion.open
            && !self.ui.fusion.intersections
            && let Some((mesh, _, _)) = &self.fusion.prepared
        {
            for side in 0..2 {
                for (index, b) in self.ui.fusion.loops[side].iter().enumerate() {
                    self.viewport
                        .session
                        .join_rims
                        .push(crate::viewport::JoinRim {
                            side,
                            index,
                            selected: self.ui.fusion.selected[side] == Some(index),
                            pickable: side == self.ui.fusion.picking && !self.ui.fusion.busy,
                            points: b.vertices.iter().map(|&v| mesh.vertices[v]).collect(),
                        });
                }
            }
        }
    }
    pub(super) fn apply_fusion(&mut self) -> Result<(), String> {
        let draft = self
            .fusion
            .draft
            .take()
            .ok_or("Preview the join before applying it")?;
        if draft.key != self.ui.fusion.key() {
            self.clear_fusion();
            return Err("Join settings changed; preview the join again".into());
        }
        if let Err(error) =
            self.document
                .apply_fusion(draft.mesh, draft.key.sources, draft.revision)
        {
            self.clear_fusion();
            return Err(error);
        }
        self.cancel_fusion();
        self.ui.status = "Joined model created; original sources retained hidden.".into();
        Ok(())
    }
}
