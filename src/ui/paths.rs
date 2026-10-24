//! Path controls edit a private draft and emit commands for persistent operations.
use super::*;
use crate::{
    app::paths::PathState,
    command::PathCommand,
    sculpt::paths::PathMode,
    viewport::paths::{PathEditor, PathTool, RingAxis, RingMode},
};

pub(crate) fn show(
    ctx: &egui::Context,
    editor: &mut PathEditor,
    state: &mut PathState,
    document: &Document,
    selected_brush: &mut usize,
) -> Vec<Command> {
    let mut commands = Vec::new();
    if !editor.active {
        return commands;
    }
    let mut open = true;
    egui::Window::new("Editable paths").id(egui::Id::new("editable_paths"))
        .open(&mut open).default_pos(egui::pos2(20.,80.)).default_width(330.).default_height(620.)
        .resizable(true).vscroll(true).show(ctx,|ui| {
        let busy=state.busy();
        let preview=state.has_preview();
        ui.spacing_mut().interact_size.y = 28.;
        ui.spacing_mut().button_padding = egui::vec2(8.,5.);
        ui.spacing_mut().item_spacing = egui::vec2(6.,6.);
        let name=document.model(document.active_model_index()).map_or("Active submodel",|model|model.name);
        ui.label(format!("Target: {name}"));
        let history_ready=!busy && !editor.dirty(document);
        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(history_ready && document.can_undo(),egui::Button::new("Undo"))
                .on_hover_text(document.undo_label().map_or("Nothing to undo".into(),|label|format!("Undo {label}; keep Editable paths open"))).clicked() {
                commands.push(Command::Undo);
            }
            if ui.add_enabled(history_ready && document.can_redo(),egui::Button::new("Redo"))
                .on_hover_text(document.redo_label().map_or("Nothing to redo".into(),|label|format!("Redo {label}; keep Editable paths open"))).clicked() {
                commands.push(Command::Redo);
            }
        });
        if !history_ready { ui.weak(if busy {"Wait or cancel the running stroke before Undo / Redo."} else {"Save or discard path edits before Undo / Redo."}); }
        if busy {
            ui.spinner(); ui.label(if state.applying_entire_path() { "Stroking the entire path…" } else { "Computing brush preview…" });
            if ui.button(if state.applying_entire_path() { "Cancel stroke" } else { "Cancel preview" }).clicked() { commands.push(Command::Path(PathCommand::CancelPreview)); }
        }
        if preview { ui.label("Brush preview — Apply or Cancel to continue editing."); }
        ui.label("Save your surface path, then sculpt near it. The brush follows the section you draw.");
        ui.add_enabled_ui(!busy && !preview,|ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("New ring").on_hover_text("Create a complete loop with one pen press, then drag to position it.").clicked() { commands.push(Command::Path(PathCommand::NewRing)); }
                if ui.button("New surface path").clicked() { commands.push(Command::Path(PathCommand::New(PathMode::Surface))); }
                if ui.button("New free path").clicked() { commands.push(Command::Path(PathCommand::New(PathMode::Free))); }
            });
            if editor.tool==PathTool::Ring {
                ui.strong("Ring from one pen drag");
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut editor.ring_mode,RingMode::Level,"Level ring");
                    ui.selectable_value(&mut editor.ring_mode,RingMode::Shortest,"Shortest surface route");
                });
                ui.horizontal(|ui| {
                    ui.label("Around axis");
                    ui.selectable_value(&mut editor.ring_axis,RingAxis::X,"X");
                    ui.selectable_value(&mut editor.ring_axis,RingAxis::Y,"Y");
                    ui.selectable_value(&mut editor.ring_axis,RingAxis::Z,"Z");
                });
                ui.weak(match editor.ring_mode {
                    RingMode::Level=>"Press on the side to create a complete level loop. Drag along the axis to move its plane, then release and Save & sculpt.",
                    RingMode::Shortest=>"Press on the side for a short surface loop around the axis. Drag to reposition it. This follows mesh edges, so the shortest route is approximate.",
                });
            }
            let active=editor.draft.as_ref().map(|p|p.id);
            egui::ComboBox::from_id_salt("saved_paths").selected_text(
                editor.draft.as_ref().map_or("Choose saved path",|p|p.name.as_str()))
                .show_ui(ui,|ui| {
                    for path in document.paths().iter().filter(|p|p.target==document.active_model_id()) {
                        if ui.selectable_label(active==Some(path.id),&path.name).clicked() {
                            commands.push(Command::Path(PathCommand::Select(path.id)));
                        }
                    }
                });
            if editor.draft.as_ref().is_some_and(|path|path.id!=0) {
            ui.separator();
            ui.strong("Automatic whole-path stroke");
            egui::ComboBox::from_id_salt("path_brush").selected_text(&document.brushes()[*selected_brush].name)
                .show_ui(ui,|ui| { for (i,brush) in document.brushes().iter().enumerate() { ui.selectable_value(selected_brush,i,&brush.name); } });
            let brush=&document.brushes()[*selected_brush];
            ui.label(format!("Strength {:.0}% · Spacing {:.0}% of radius",brush.strength*100.,brush.spacing*100.));
            let linked=state.use_brush_size;
            if design::checkbox(ui, &mut state.use_brush_size,"Use active brush size").changed()
                && linked && !state.use_brush_size
                && let Some(radius)=state.brush_radius_world { state.options.radius=radius; }
            if state.use_brush_size {
                ui.label(format!("Brush radius {:.0} px · Pressure {:.0}%",brush.radius,state.options.pressure*100.));
                if let Some(radius)=state.brush_radius_world { ui.weak(format!("{radius:.3} model units at the nearest side of the path; held constant for this stroke.")); }
            } else {
                ui.label(format!("Custom radius {:.3} model units · Pressure {:.0}%",state.options.radius,state.options.pressure*100.));
            }
            ui.label(if document.mesh_settings().adaptive.enabled { "Adaptive sculpt: on — uses your current mesh settings" } else { "Adaptive sculpt: off — uses the existing mesh" });
            egui::CollapsingHeader::new("Automatic stroke settings").default_open(false).show(ui,|ui| {
            let mut brush=document.brushes()[*selected_brush].clone();
            ui.weak("Brush settings are shared with manual sculpting.");
            let mut changed=false;
            if state.use_brush_size {
                changed|=ui.add(egui::Slider::new(&mut brush.radius,1. ..=500.).text("Brush radius (px)")).changed();
            } else {
                ui.add(egui::DragValue::new(&mut state.options.radius).range(0.00001..=1000.).speed(0.01).prefix("Custom radius (model units) "));
            }
            changed|=ui.add(egui::Slider::new(&mut brush.strength,0. ..=1.).text("Brush strength")).changed();
            changed|=ui.add(egui::Slider::new(&mut brush.spacing,0.02..=1.).text("Brush spacing / radius")).changed();
            if changed { commands.push(Command::UpdateBrush(*selected_brush,brush)); }
            ui.add(egui::Slider::new(&mut state.options.pressure,0. ..=1.).text("Automatic pressure"));
            ui.weak("Pressure stays constant. Reduce it to match a lighter pen stroke; the brush pressure curve still applies.");
            ui.horizontal_wrapped(|ui| {
                design::checkbox(ui, &mut state.options.reverse,"Run in reverse");
                design::checkbox(ui, &mut state.options.invert,"Invert brush effect");
            });
            });
            let ready=editor.draft.as_ref().is_some_and(|p|p.id!=0 && p.mode==PathMode::Surface && !p.is_stale(document.mesh())) && !editor.dirty(document);
            ui.horizontal_wrapped(|ui| {
                let response=ui.add_enabled(ready,egui::Button::new(egui::RichText::new("Stroke entire path").strong()));
                if design::help(response,"Automatically apply the active brush over this entire saved path using the displayed size, constant pressure and current adaptive settings. Commits one undo step when finished; Cancel discards unfinished work.").clicked() { commands.push(Command::Path(PathCommand::StrokeEntirePath)); }
                if ui.add_enabled(ready,egui::Button::new("Preview stroke")).on_hover_text("Inspect the result before choosing Apply stroke.").clicked() { commands.push(Command::Path(PathCommand::Preview)); }
            });
            ui.weak("Save edits first. Uses the active brush, masks, symmetry and adaptive sculpt settings; disable the stencil.");
            }
            ui.separator();
            let dirty=editor.dirty(document);
            if let Some(path)=editor.draft.as_mut() {
                ui.text_edit_singleline(&mut path.name);
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut editor.tool,PathTool::Add,"Add / draw");
                    ui.selectable_value(&mut editor.tool,PathTool::Move,"Move points");
                    ui.selectable_value(&mut editor.tool,PathTool::Insert,"Insert point");
                    ui.add_enabled_ui(path.mode==PathMode::Surface,|ui| {
                        ui.selectable_value(&mut editor.tool,PathTool::Ring,"Ring");
                    });
                });
                ui.weak(if editor.tool==PathTool::Ring {
                    "One press creates the whole ring. Drag to position it; Esc restores the previous draft."
                } else if path.mode==PathMode::Surface {
                    "Click or drag on the surface. Right-drag to orbit and continue around the model."
                } else {
                    "New points use the view plane through the last point (origin initially). Orbit then drag a point to edit depth."
                });
                ui.label(format!("{} points · {}",path.points.len(),if path.closed { "Closed loop" } else { "Open path" }));
                let explicit_rounding=(0..path.points.len()).any(|i|path.corner_rounding(i)>0.);
                ui.add_enabled(!explicit_rounding,egui::Slider::new(&mut path.smoothing,0. ..=1.).text("Smoothing"));
                if explicit_rounding { ui.weak("Individual rounded corners replace global smoothing."); }
                ui.horizontal_wrapped(|ui| {
                    if ui.button(if path.closed { "Open loop" } else { "Close loop" }).clicked() {
                        match path.set_closed(!path.closed) {
                            Ok(())=>editor.selected=None,
                            Err(e)=>editor.notice=Some(e),
                        }
                    }
                    if ui.button("Reverse direction").clicked() { path.reverse(); editor.selected=None; }
                });
                if let Some(index)=editor.selected.filter(|i|*i<path.points.len()) {
                    ui.label(format!("Selected point {}",index+1));
                    ui.horizontal_wrapped(|ui| {
                        let point=&mut path.points[index].position;
                        ui.add(egui::DragValue::new(&mut point.0).speed(0.01).prefix("X "));
                        ui.add(egui::DragValue::new(&mut point.1).speed(0.01).prefix("Y "));
                        ui.add(egui::DragValue::new(&mut point.2).speed(0.01).prefix("Z "));
                    });
                    let limit=path.corner_limit(index);
                    let mut rounding=path.corner_rounding(index);
                    ui.add_enabled_ui(limit>0.,|ui| {
                        ui.horizontal(|ui| {
                            ui.label("Corner");
                            if ui.selectable_label(rounding==0.,"Sharp").clicked() { rounding=0.; }
                            if ui.selectable_label(rounding>0.,"Round").clicked() && rounding==0. { rounding=limit*0.5; }
                        });
                        ui.add(egui::Slider::new(&mut rounding,0. ..=limit.max(1e-6)).text("Rounding (model units)"))
                            .on_hover_text("Distance trimmed along each adjacent edge, using the same rounding as Slice.");
                    });
                    if rounding!=path.corner_rounding(index)
                        && let Err(error)=path.set_corner_rounding(index,rounding) { editor.notice=Some(error); }
                    ui.weak(if limit>0. { "Move points: drag the diamond or either round handle to adjust this corner." } else { "Open endpoints and straight segments stay sharp." });
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Delete point").clicked() {
                            match path.remove_point(index) {
                                Ok(())=>editor.selected=None,
                                Err(error)=>editor.notice=Some(error),
                            }
                        }
                        if ui.add_enabled(path.closed,egui::Button::new("Use as seam")).clicked()
                            && let Err(e)=path.set_seam(index) { editor.notice=Some(e); }
                    });
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(path.mode==PathMode::Surface && path.points.len()>=if path.closed {3} else {2},egui::Button::new("Save & sculpt")).clicked() { commands.push(Command::Path(PathCommand::SaveAndSculpt)); }
                    if ui.add_enabled(dirty,egui::Button::new("Save path")).clicked() { commands.push(Command::Path(PathCommand::Save)); }
                    if ui.button("Discard edits").clicked() { commands.push(Command::Path(PathCommand::Discard)); }
                    if ui.add_enabled(path.id!=0,egui::Button::new("Delete path")).clicked() { commands.push(Command::Path(PathCommand::Delete)); }
                });
                if dirty { ui.weak("Unsaved path edits — Save path creates one undo step."); }
                if path.id!=0 {
                    if path.is_stale(document.mesh()) { ui.colored_label(egui::Color32::from_rgb(245,150,90),"Surface changed. Reproject before editing or applying this path."); }
                    ui.horizontal_wrapped(|ui| {
                        ui.add(egui::DragValue::new(&mut state.reproject_distance).range(0.0001..=100.).speed(0.01).prefix("Distance "));
                        if ui.button(if path.mode==PathMode::Free { "Attach saved path to surface" } else { "Reproject saved path" }).clicked() { commands.push(Command::Path(PathCommand::Reproject)); }
                    });
                }
            }
        });
        if preview {
            ui.horizontal(|ui| {
                if ui.button("Apply stroke").clicked() { commands.push(Command::Path(PathCommand::Apply)); }
                if ui.button("Cancel preview").clicked() { commands.push(Command::Path(PathCommand::CancelPreview)); }
            });
        }
        ui.separator();
        if ui.button("Done").clicked() { commands.push(Command::Path(PathCommand::Close)); }
        ui.weak("Done discards unsaved edits. Esc cancels an active point, corner or ring drag or brush preview.");
    });
    if !open {
        commands.push(Command::Path(PathCommand::Close));
    }
    commands
}
