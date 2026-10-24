//! Explicit source → target controls for a private projection draft.
use super::*;
use crate::{
    document::ProjectionSource,
    sculpt::{ProjectionOptions, ProjectionReport, RelaxationOptions, RelaxationReport},
};

pub(crate) enum SurfaceReport {
    Projection(ProjectionReport),
    Relaxation(RelaxationReport),
}
impl SurfaceReport {
    pub fn moved(&self) -> usize {
        match self {
            Self::Projection(r) => r.moved,
            Self::Relaxation(r) => r.moved,
        }
    }
    pub fn summary(&self) -> String {
        match self {
            Self::Projection(r) => r.summary(),
            Self::Relaxation(r) => r.summary(),
        }
    }
}
#[derive(Default)]
pub(crate) struct ProjectionControls {
    pub open: bool,
    pub relaxation: bool,
    pub relaxation_options: RelaxationOptions,
    pub source: Option<ProjectionSource>,
    pub options: ProjectionOptions,
    pub report: Option<SurfaceReport>,
    pub busy: bool,
}
impl UiState {
    pub(super) fn projection_buttons(
        &mut self,
        ui: &mut egui::Ui,
        document: &Document,
        commands: &mut Vec<Command>,
    ) {
        let enabled = !self.mesh_busy && document.require_projection_target().is_ok();
        if design::help(ui.add_enabled(enabled, design::button(ui, Glyph::Mesh, "Relax mesh…")), "Redistribute vertices along the surface. Mask areas to protect them; preview before applying.").customize(Action::RelaxMesh).clicked() {
            commands.push(Command::OpenRelaxation);
        }
        ui.collapsing("Detail projection", |ui| {
            ui.weak("Keep a reference before remeshing or adaptive sculpting.");
            let enabled = !self.mesh_busy && document.require_projection_target().is_ok();
            if design::help(ui.add_enabled(enabled, design::button(ui, Glyph::Plus, "Keep reference copy")), "Save an unchanged, hidden submodel below the active model. Remesh or sculpt the active model, then project its detail from this reference.").customize(Action::KeepProjectionReference).clicked() {
                commands.push(Command::KeepProjectionReference);
            }
            if design::help(ui.add_enabled(enabled, design::button(ui, Glyph::Mesh, "Project details…")), Action::ProjectDetails.help().unwrap()).customize(Action::ProjectDetails).clicked() {
                commands.push(Command::OpenProjection);
            }
        });
    }
    pub(crate) fn projection_window(
        &mut self,
        ctx: &egui::Context,
        document: &Document,
    ) -> Vec<Command> {
        let mut commands = Vec::new();
        if !self.projection.open {
            return commands;
        }
        if self.projection.relaxation {
            return self.relaxation_window(ctx, document);
        }
        let mut open = true;
        egui::Window::new("Project details").open(&mut open).default_width(410.).resizable(false).show(ctx, |ui| {
            let state = &mut self.projection;
            let old = (state.source, state.options);
            let name = |source: Option<ProjectionSource>| match source {
                Some(ProjectionSource::Submodel(i)) => document.model(i).map_or("Missing reference".into(), |m| format!("{} (submodel {})", m.name, i+1)),
                Some(ProjectionSource::Level(i)) => format!("Target's subdivision level {i}"),
                None => "Choose a reference".into(),
            };
            ui.label(format!("Target: {}", document.model(document.active_model_index()).unwrap().name));
            ui.weak("Only the target moves. The reference stays unchanged.");
            ui.add_enabled_ui(!state.busy, |ui| {
                egui::ComboBox::from_id_salt("projection_source").selected_text(name(state.source)).width(370.).show_ui(ui, |ui| {
                    for i in 0..document.model_count() {
                        if i != document.active_model_index() {
                            let source = Some(ProjectionSource::Submodel(i));
                            ui.selectable_value(&mut state.source, source, name(source));
                        }
                    }
                    for i in 1..=document.mesh_settings().levels() {
                        if i != document.mesh_settings().level() {
                            let source = Some(ProjectionSource::Level(i));
                            ui.selectable_value(&mut state.source, source, name(source));
                        }
                    }
                });
                if ui.add_enabled(document.merge_down_target().is_some(), design::button(ui, Glyph::Mesh, "Use model below")).clicked() {
                    state.source = document.merge_down_target().map(ProjectionSource::Submodel);
                }
                ui.label(format!("Project from {} onto the active target", name(state.source)));
                design::help(ui.add(egui::Slider::new(&mut state.options.strength, 0. ..=1.).text("Strength")), "Blend toward the reference. Masks reduce movement; fully masked and hidden boundary vertices stay fixed.");
                ui.horizontal(|ui| {
                    ui.label("Maximum distance");
                    design::help(ui.add(egui::DragValue::new(&mut state.options.max_distance).speed(0.001).range(0.000001..=1000.)), "Maximum world-space distance to the reference. Use a small distance near thin walls or disconnected parts.");
                });
            });
            if old != (state.source, state.options) { commands.push(Command::ClearProjectionPreview); }
            ui.weak("Hidden reference submodels are usable; individually hidden reference faces are excluded. Source uses its stored active surface, not its coarse display mesh.");
            ui.weak("Nearest opposing sheets and ambiguous separate parts are skipped. Triangle safety may reduce movement. Inspect thin or overlapping surfaces before Apply.");
            ui.label(format!("Target topology, masks and groups stay intact. {} subdivision level(s) retained.", document.mesh_settings().levels()));
            if state.busy { ui.spinner(); ui.label("Computing preview…"); }
            if let Some(report) = &state.report {
                ui.separator();
                ui.label(report.summary());
                ui.weak("Drag the viewport to orbit; scroll to zoom. No geometry is edited until Apply.");
            } else if !state.busy { ui.label("Preview required before Apply."); }
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!state.busy && state.source.is_some(), design::button(ui, Glyph::Mesh, "Preview projection")).clicked() { commands.push(Command::PreviewProjection); }
                if ui.add_enabled(!state.busy && old == (state.source, state.options) && state.report.as_ref().is_some_and(|r| r.moved() > 0), design::button(ui, Glyph::Mesh, "Apply projection")).clicked() { commands.push(Command::ApplyProjection); }
                if ui.add(design::button(ui, Glyph::Delete, "Cancel")).clicked() { commands.push(Command::CancelProjection); }
            });
        });
        if !open {
            commands.push(Command::CancelProjection);
        }
        commands
    }
    fn relaxation_window(&mut self, ctx: &egui::Context, document: &Document) -> Vec<Command> {
        let mut commands = Vec::new();
        let mut open = true;
        egui::Window::new("Relax mesh").open(&mut open).default_width(410.).vscroll(true).show(ctx, |ui| {
            let state=&mut self.projection;
            let old=state.relaxation_options;
            ui.label(format!("Target: {}",document.model(document.active_model_index()).unwrap().name));
            ui.weak("Redistribute existing vertices along a fixed reference surface. Mask everything outside the region you want to relax.");
            ui.add_enabled_ui(!state.busy, |ui| {
                ui.add(egui::Slider::new(&mut state.relaxation_options.strength,0. ..=1.).text("Strength"));
                ui.add(egui::Slider::new(&mut state.relaxation_options.iterations,1..=30).text("Iterations"));
                ui.horizontal(|ui| {
                    ui.label("Surface tolerance");
                    design::help(ui.add(egui::DragValue::new(&mut state.relaxation_options.max_deviation).range(0.000001..=1.).speed(0.0001).max_decimals(6)), "Maximum sampled distance in model units. Smaller values protect fine detail but may limit redistribution.");
                });
            });
            if old!=state.relaxation_options { commands.push(Command::ClearProjectionPreview); }
            ui.weak("Boundaries, creases, sharp corners, hidden faces and fully masked vertices stay fixed. Soft masks reduce movement. Connectivity and attributes stay intact.");
            ui.weak(format!("Only the active subdivision level is relaxed; {} level(s) retained. Symmetry follows the existing mesh and mask. Consecutive applications reuse the reference until another edit or Undo.", document.mesh_settings().levels()));
            ui.weak("Deviation is sampled at face centers and edge midpoints. Inspect the wireframe, groove and silhouette before applying. Tight regions can remain unchanged.");
            if state.busy { ui.spinner(); ui.label("Computing preview…"); }
            if let Some(report)=&state.report { ui.label(report.summary()); }
            ui.weak("Orbit and zoom to inspect · Escape cancels");
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!state.busy,design::button(ui,Glyph::Mesh,"Preview relaxation")).clicked() {commands.push(Command::PreviewProjection);}
                if ui.add_enabled(!state.busy && old==state.relaxation_options && state.report.as_ref().is_some_and(|r|r.moved()>0),design::button(ui,Glyph::Mesh,"Apply relaxation")).clicked() {commands.push(Command::ApplyProjection);}
                if ui.add(design::button(ui,Glyph::Delete,"Cancel")).clicked() {commands.push(Command::CancelProjection);}
            });
        });
        if !open {
            commands.push(Command::CancelProjection);
        }
        commands
    }
}
