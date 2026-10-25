use super::*;
use crate::sculpt::texture::{Channel, Exchange, Precision, Texture};
pub(crate) enum TextureCommand {
    Create(Channel, u32, Precision),
    Bake(bool, u32, Precision),
    CreateAndPaint(Channel, u32, Precision),
    Paint(Option<Channel>),
    Remove(Channel),
    Copy(Channel),
    Export(Channel, String, bool),
    EditExternal(Channel),
    Reload(Channel, bool),
}

pub(super) struct ExchangeJob {
    geometry: crate::sculpt::Geometry,
    worker: crate::background_job::BackgroundJob<Vec<Texture>>,
    open_in: Option<String>,
    bake: bool,
}

impl App {
    fn start_texture_exchange(&mut self, texture: Texture, create: bool) -> Result<(), String> {
        if self.texture_exchange.is_some() {
            return Err("An external paint exchange is already running".into());
        }
        if !self.document.mesh().uv.current(self.document.mesh()) {
            return Err("Generate current UVs before editing externally".into());
        }
        let geometry = self.document.geometry_snapshot();
        let uv = self.document.mesh().uv.clone();
        let executable = self.ui.preferences.gimp_path.trim();
        let executable = if executable.is_empty() {
            "gimp"
        } else {
            executable
        }
        .to_owned();
        let editor = self.ui.preferences.external_paint_app.trim();
        let open_in = create.then(|| {
            if editor.is_empty() {
                executable.clone()
            } else {
                editor.to_owned()
            }
        });
        let worker =
            crate::background_job::BackgroundJob::start("clay-gimp-exchange", move |cancel| {
                if create {
                    persistence::gimp::prepare(texture, &uv, cancel).map(|t| vec![t])
                } else {
                    persistence::gimp::reload(&texture, &executable, cancel).map(|t| vec![t])
                }
            })?;
        self.texture_exchange = Some(ExchangeJob {
            geometry,
            worker,
            open_in,
            bake: false,
        });
        self.ui.uv.exchange_busy = true;
        self.ui.status = if create {
            "Preparing a layered PSD with a UV guide…"
        } else {
            "Reloading saved PSD paint layers without the UV guide…"
        }
        .into();
        Ok(())
    }

    pub(super) fn poll_texture_exchange(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.texture_exchange else {
            return;
        };
        let Some(result) = job.worker.poll() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        };
        let job = self
            .texture_exchange
            .take()
            .expect("completed texture exchange");
        self.ui.uv.exchange_busy = false;
        self.ui.status = match result.and_then(|textures| {
            if !self.document.geometry_matches(&job.geometry) {
                return Err("The document changed while the external paint exchange was running; current edits kept. Try the exchange again".into());
            }
            self.viewport.cancel(&mut self.document);
            if let Some(executable) = job.open_in {
                let texture = &textures[0];
                let path = texture.exchange.as_ref().and_then(|e| e.layered_path.as_ref()).ok_or("Missing PSD working copy")?;
                persistence::gimp::open(&executable, std::path::Path::new(path))?;
                self.document.assign_textures(textures)?;
                Ok("Save or export back to the working PSD, then Reload. The UV guide is excluded automatically.")
            } else {
                self.document.assign_textures(textures)?;
                self.ui.uv.reload_conflict = false;
                Ok(if job.bake { "Baked unlit paint/material maps. Use Paint on model to continue painting, or Edit in external paint app. Undo restores the previous textures." } else { "Reloaded saved PSD paint layers without the UV guide; Undo restores the previous texture" })
            }
        }) {
            Ok(message) => message.into(),
            Err(error) => error,
        };
    }

    pub(super) fn texture_command(&mut self, command: TextureCommand) -> Result<(), String> {
        use TextureCommand::*;
        self.viewport.cancel(&mut self.document);
        let get = |document: &Document, channel: Channel| {
            document
                .mesh()
                .uv
                .textures
                .iter()
                .find(|t| t.channel == channel)
                .cloned()
                .ok_or_else(|| "Create or import a texture for this channel first".to_string())
        };
        match command {
            Bake(all, size, precision) => {
                if self.texture_exchange.is_some() {
                    return Err("Wait for the current texture operation".into());
                }
                let geometry = self.document.geometry_snapshot();
                let mesh = self.document.mesh().clone();
                let worker = crate::background_job::BackgroundJob::start(
                    "clay-texture-bake",
                    move |cancel| mesh.bake_material_textures(all, size, precision, cancel),
                )?;
                self.texture_exchange = Some(ExchangeJob {
                    geometry,
                    worker,
                    open_in: None,
                    bake: true,
                });
                self.ui.uv.exchange_busy = true;
                self.ui.status =
                    "Baking unlit colour and material properties into UV textures…".into();
            }
            CreateAndPaint(channel, size, precision) => {
                if !self.document.mesh().uv.current(self.document.mesh()) {
                    return Err(
                        "Use Automatic unwrap in UV layout before creating a paint texture".into(),
                    );
                }
                self.texture_command(Create(channel, size, precision))?;
                self.texture_command(Paint(Some(channel)))?;
            }
            Create(channel, size, precision) => {
                if self
                    .document
                    .mesh()
                    .uv
                    .textures
                    .iter()
                    .any(|t| t.channel == channel)
                {
                    return Err(
                        "This channel already has a texture. Remove it first to create a new one"
                            .into(),
                    );
                }
                let brush = &self.document.brushes()[self.ui.selected];
                self.document.assign_texture(Texture::new_with_precision(
                    channel,
                    size,
                    &brush.material,
                    precision,
                )?)?;
            }
            Paint(channel) => {
                if let Some(channel) = channel {
                    get(&self.document, channel)?;
                    self.document.require_texture_history()?;
                    if !self.document.mesh().uv.current(self.document.mesh()) {
                        return Err("Generate current UVs before painting a texture".into());
                    }
                    if self
                        .document
                        .mesh()
                        .uv
                        .corners
                        .iter()
                        .enumerate()
                        .any(|(f, t)| {
                            self.document.mesh().uv.mapped(f)
                                && crate::sculpt::uv::area(*t).abs() < 1e-12
                        })
                    {
                        return Err("Repair degenerate UV triangles before painting".into());
                    }
                    let uv = &self.document.mesh().uv;
                    if !(0..uv.corners.len()).any(|f| uv.mapped(f)) {
                        return Err(
                            "Generate UVs before painting; all UV islands were removed".into()
                        );
                    }
                    if uv
                        .corners
                        .iter()
                        .enumerate()
                        .filter(|(f, _)| uv.mapped(*f))
                        .flat_map(|(_, t)| t.iter().flatten())
                        .any(|v| !(0. ..=1.).contains(v))
                    {
                        return Err("Pack UV islands inside 0–1 before painting; tiled UV painting is not supported".into());
                    }
                }
                let brushes = self.document.brushes();
                let paint_index = brushes
                    .get(self.ui.selected)
                    .filter(|b| b.kind == crate::sculpt::Kind::Paint)
                    .map(|_| self.ui.selected)
                    .or_else(|| {
                        brushes
                            .iter()
                            .position(|b| b.kind == crate::sculpt::Kind::Paint)
                    });
                if let Some(index) = paint_index {
                    self.ui.selected = index;
                } else {
                    self.ui.selected = self
                        .document
                        .append_brush(crate::sculpt::Brush::new(crate::sculpt::Kind::Paint))?;
                }
                self.ui.transform_mode = crate::viewport::TransformMode::Sculpt;
                self.ui.slice_active = false;
                self.ui.selection_tool = None;
                self.ui.edges.selecting = false;
                self.ui.modeling.active = false;
                self.ui.uv.checker = false;
                self.ui.show_polygroups = false;
                self.ui.uv.open = false;
                self.ui.status = format!(
                    "Paint {} directly on the 3D model. Choose a colour in Brush settings.",
                    channel.map_or("vertex materials", |c| c.label())
                );
                self.document.texture_paint = channel;
            }
            Remove(channel) => {
                self.document.remove_texture(channel)?;
                if self.document.texture_paint == Some(channel) {
                    self.document.texture_paint = None;
                }
            }
            Copy(channel) => {
                let mut texture = get(&self.document, channel)?;
                texture.id = Texture::new_id();
                texture.exchange = None;
                self.document.assign_texture(texture)?;
            }
            Export(channel, path, overwrite) => {
                let texture = get(&self.document, channel)?;
                let path = std::path::Path::new(&path);
                if !path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("png") || s.eq_ignore_ascii_case("exr"))
                {
                    return Err("Export texture to .png (16-bit) or .exr (float)".into());
                }
                if path.exists() && !overwrite {
                    self.ui.uv.export_overwrite = Some(path.to_string_lossy().into());
                    return Ok(());
                }
                persistence::texture::save(path, &texture)?;
                self.ui.uv.export_overwrite = None;
                self.ui.status = format!("Exported texture to {}", path.display());
            }
            EditExternal(channel) => {
                let texture = get(&self.document, channel)?;
                self.start_texture_exchange(texture, true)?;
            }
            Reload(channel, discard_local) => {
                let texture = get(&self.document, channel)?;
                let exchange = texture
                    .exchange
                    .as_ref()
                    .ok_or("Send this texture to the external paint app first")?;
                if !discard_local && texture.fingerprint() != exchange.baseline {
                    self.ui.uv.reload_conflict = true;
                    return Err("This texture changed in Clay Form since export. Keep local edits, or explicitly replace them with the external image".into());
                }
                if exchange.layered_path.is_some() {
                    return self.start_texture_exchange(texture, false);
                }
                let mut next = persistence::texture::load(&exchange.path, channel)?;
                if texture.pixels.precision() == Precision::Float32
                    && next.pixels.precision() != Precision::Float32
                {
                    return Err("External image lost float precision. Export a float EXR from the editor before reloading; the current image is unchanged".into());
                }
                next.id = texture.id;
                next.name = texture.name;
                next.exchange = Some(Exchange {
                    path: exchange.path.clone(),
                    baseline: next.fingerprint(),
                    layered_path: None,
                });
                self.document.assign_texture(next)?;
                self.ui.uv.reload_conflict = false;
                self.ui.status =
                    "Reloaded texture and all linked uses; Undo restores the previous image".into();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn finish_exchange(app: &mut App) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while app.texture_exchange.is_some() {
            assert!(std::time::Instant::now() < deadline);
            app.poll_texture_exchange(&egui::Context::default());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    #[test]
    fn texture_bake_is_one_undo_and_rejects_stale_background_results() {
        let mut app = App::default();
        app.document
            .replace_geometry(crate::sculpt::Geometry::new(
                crate::sculpt::texture::tests::sheets(),
            ))
            .unwrap();
        let before = app.document.geometry_snapshot();
        let history = app.document.history_counts();
        app.texture_command(TextureCommand::Bake(true, 64, Precision::Float32))
            .unwrap();
        finish_exchange(&mut app);
        assert_eq!(
            app.document.mesh().uv.textures.len(),
            3,
            "{}",
            app.ui.status
        );
        assert_eq!(app.document.history_counts().0, history.0 + 1);
        let baked = app.document.geometry_snapshot();
        app.document.undo();
        assert_eq!(app.document.geometry_snapshot(), before);
        app.document.redo();
        assert_eq!(app.document.geometry_snapshot(), baked);
        app.texture_command(TextureCommand::Bake(false, 64, Precision::Float32))
            .unwrap();
        app.document.remove_texture(Channel::Metallic).unwrap();
        let changed = app.document.geometry_snapshot();
        finish_exchange(&mut app);
        assert_eq!(app.document.geometry_snapshot(), changed);
        assert!(app.ui.status.contains("document changed"));
    }
    #[test]
    fn paint_target_keeps_custom_preset_and_leaves_component_selection() {
        let mut app = App::default();
        let mut brush = crate::sculpt::Brush::new(crate::sculpt::Kind::Paint);
        brush.paint_effect.mode = crate::sculpt::PaintMode::Spray;
        brush.name = "Custom spray".into();
        let index = app.document.append_brush(brush).unwrap();
        app.ui.selected = index;
        app.ui.modeling.active = true;
        app.ui.uv.open = true;
        app.texture_command(TextureCommand::Paint(None)).unwrap();
        assert_eq!(app.ui.selected, index);
        assert_eq!(
            app.document.brushes()[index].paint_effect.mode,
            crate::sculpt::PaintMode::Spray
        );
        assert!(!app.ui.modeling.active && !app.ui.uv.open);
        assert_eq!(app.document.texture_paint, None);
    }
    #[test]
    fn external_texture_reload_preserves_identity_checks_conflicts_and_keeps_last_valid_image() {
        let mut app = App::default();
        let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        mesh.uv = mesh.unwrap_uv(true, 0.01, &|| false).unwrap();
        app.document
            .replace_geometry(crate::sculpt::Geometry::new(mesh))
            .unwrap();
        app.texture_command(TextureCommand::Create(Channel::Color, 16, Precision::U16))
            .unwrap();
        let mut texture = app.document.mesh().uv.textures[0].clone();
        let path = std::env::temp_dir().join(format!("clay-reload-{}.png", texture.id));
        texture.exchange = Some(Exchange {
            path: path.to_string_lossy().into(),
            baseline: texture.fingerprint(),
            layered_path: None,
        });
        app.document.assign_texture(texture.clone()).unwrap();
        let before = app.document.geometry_snapshot();
        let mut external = Texture::new(Channel::Color, 32, &Default::default()).unwrap();
        external.pixels.u16_mut()[0] = 12345;
        persistence::texture::save(&path, &external).unwrap();
        app.texture_command(TextureCommand::Reload(Channel::Color, false))
            .unwrap();
        let updated = app.document.mesh().uv.textures[0].clone();
        assert_eq!(updated.id, texture.id);
        assert_eq!(updated.width, 32);
        assert_eq!(updated.pixels, external.pixels);
        app.document.undo();
        assert_eq!(app.document.geometry_snapshot(), before);
        app.document.redo();
        let mut local = updated.clone();
        local.pixels.u16_mut()[0] = 54321;
        app.document.assign_texture(local).unwrap();
        let local = app.document.geometry_snapshot();
        assert!(
            app.texture_command(TextureCommand::Reload(Channel::Color, false))
                .is_err()
        );
        assert_eq!(app.document.geometry_snapshot(), local);
        std::fs::write(&path, b"incomplete image").unwrap();
        assert!(
            app.texture_command(TextureCommand::Reload(Channel::Color, true))
                .is_err()
        );
        assert_eq!(app.document.geometry_snapshot(), local);
        persistence::texture::save(&path, &external).unwrap();
        app.texture_command(TextureCommand::Reload(Channel::Color, true))
            .unwrap();
        assert_eq!(app.document.mesh().uv.textures[0].pixels, external.pixels);
        app.ui.preferences.gimp_path = "/nonexistent/clay-gimp".into();
        let saved = app.document.geometry_snapshot();
        app.texture_command(TextureCommand::EditExternal(Channel::Color))
            .unwrap();
        finish_exchange(&mut app);
        assert!(app.ui.status.contains("Could not open external paint app"));
        assert_eq!(app.document.geometry_snapshot(), saved);
        let _ = std::fs::remove_file(path);
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires Vulkan; clicks UV/texture controls and paints through the viewport"]
    fn uv_texture_controls_offscreen_review() {
        use eframe::egui::{Pos2, Vec2};
        let mut capture = crate::viewport::UiCapture::new([1400, 1000]);
        let ctx = egui::Context::default();
        ctx.style_mut(|s| s.animation_time = 0.);
        let mut app = App::default();
        app.viewport.initialize_renderer(&capture.state).unwrap();
        app.camera.yaw = 0.;
        app.camera.pitch = 0.;
        let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        mesh.uv = mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
        app.document
            .replace_geometry(crate::sculpt::Geometry::new(mesh))
            .unwrap();
        let frame = |app: &mut App, events: Vec<egui::Event>| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(1400., 1000.),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    for command in app.ui.toolbar(ctx, &app.document, &app.camera) {
                        app.dispatch(command, ctx);
                    }
                    for command in app.ui.uv_window(
                        ctx,
                        &app.document,
                        &app.viewport.modeling.selected_faces(),
                    ) {
                        app.dispatch(command, ctx);
                    }
                    app.viewport.set_uv_checker(app.ui.uv.checker);
                    egui::CentralPanel::default().show(ctx, |ui| app.show_viewport(ui));
                },
            )
        };
        let point = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .find_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        (t.galley.text() == label).then_some(t.pos + t.galley.size() * 0.5)
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| panic!("Missing visible control: {label}"))
        };
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let out = frame(&mut app, vec![]);
        capture.render(&ctx, out);
        let out = frame(&mut app, vec![]);
        let p = point(&out, "UVs / Textures");
        capture.render(&ctx, out);
        for events in [
            vec![egui::Event::PointerMoved(p), button(p, true)],
            vec![button(p, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
        assert!(app.ui.uv.open);
        let out = frame(&mut app, vec![]);
        let p = point(&out, "Texture images");
        capture.render(&ctx, out);
        for events in [
            vec![egui::Event::PointerMoved(p), button(p, true)],
            vec![button(p, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
        capture.render(&ctx, frame(&mut app, vec![]));
        let out = frame(&mut app, vec![]);
        let p = point(&out, "Create & paint on model");
        capture.render(&ctx, out);
        for events in [
            vec![egui::Event::PointerMoved(p), button(p, true)],
            vec![button(p, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
        assert_eq!(app.document.mesh().uv.textures.len(), 1);
        assert!(!app.ui.uv.open, "painting returns to the 3D model");
        assert_eq!(app.document.texture_paint, Some(Channel::Color));
        let mut brush = app.document.brushes()[app.ui.selected].clone();
        brush.material = crate::sculpt::Material::from_srgb([0.95, 0.04, 0.01], 0.7, 0.);
        brush.radius = 100.;
        brush.strength = 1.;
        app.dispatch(Command::UpdateBrush(app.ui.selected, brush), &ctx);
        let before = app.document.mesh().uv.textures[0].pixels.clone();
        let start = Pos2::new(820., 500.);
        let end = start + Vec2::new(50., 25.);
        for events in [
            vec![egui::Event::PointerMoved(start), button(start, true)],
            vec![egui::Event::PointerMoved(end)],
            vec![button(end, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
        assert_ne!(
            app.document.mesh().uv.textures[0].pixels,
            before,
            "a viewport stroke changes image pixels"
        );
        let after = app.document.mesh().uv.textures[0].pixels.clone();
        assert!(
            after
                .u16()
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| p[0] as u32 > p[1] as u32 * 2 && p[0] as u32 > p[2] as u32 * 2),
            "painted pixels retain the chosen red brush colour"
        );
        persistence::texture::save(
            &std::env::temp_dir().join("clay-painted-image.png"),
            &app.document.mesh().uv.textures[0],
        )
        .unwrap();
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.mesh().uv.textures[0].pixels, before);
        app.dispatch(Command::Redo, &ctx);
        assert_eq!(app.document.mesh().uv.textures[0].pixels, after);
        app.ui.uv.open = true;
        capture.render(&ctx, frame(&mut app, vec![egui::Event::PointerGone]));
        let out = frame(&mut app, vec![]);
        let p = point(&out, "Bake material maps");
        capture.render(&ctx, out);
        for events in [
            vec![egui::Event::PointerMoved(p), button(p, true)],
            vec![button(p, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
        let out = frame(&mut app, vec![]);
        let p = point(&out, "Replace with bake");
        capture.render(&ctx, out);
        let history = app.document.history_counts();
        for events in [
            vec![egui::Event::PointerMoved(p), button(p, true)],
            vec![button(p, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
        finish_exchange(&mut app);
        assert_eq!(
            app.document.mesh().uv.textures.len(),
            3,
            "{}",
            app.ui.status
        );
        assert_eq!(app.document.history_counts().0, history.0 + 1);
        app.dispatch(Command::Undo, &ctx);
        assert_eq!(app.document.mesh().uv.textures.len(), 1);
        app.dispatch(Command::Redo, &ctx);
        let output = frame(&mut app, vec![egui::Event::PointerGone]);
        let pixels = capture.render(&ctx, output);
        capture.save(std::env::temp_dir().join("clay-uv-textures.png"), &pixels);
    }
}
