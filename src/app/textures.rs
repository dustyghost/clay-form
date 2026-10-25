use super::*;
use crate::sculpt::texture::{Channel, Exchange, Texture};
pub(crate) enum TextureCommand {
    Create(Channel, u32),
    Paint(Option<Channel>),
    Remove(Channel),
    Copy(Channel),
    Export(Channel, String, bool),
    EditExternal(Channel),
    Reload(Channel, bool),
}

impl App {
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
            Create(channel, size) => {
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
                self.document
                    .assign_texture(Texture::new(channel, size, &brush.material)?)?;
            }
            Paint(channel) => {
                if let Some(channel) = channel {
                    get(&self.document, channel)?;
                    if !self.document.mesh().uv.current(self.document.mesh()) {
                        return Err("Generate current UVs before painting a texture".into());
                    }
                    if self
                        .document
                        .mesh()
                        .uv
                        .corners
                        .iter()
                        .any(|t| crate::sculpt::uv::area(*t).abs() < 1e-12)
                    {
                        return Err("Repair degenerate UV triangles before painting".into());
                    }
                    if self
                        .document
                        .mesh()
                        .uv
                        .corners
                        .iter()
                        .flatten()
                        .flatten()
                        .any(|v| !(0. ..=1.).contains(v))
                    {
                        return Err("Pack UV islands inside 0–1 before painting; tiled UV painting is not supported".into());
                    }
                    if let Some(index) = self
                        .document
                        .brushes()
                        .iter()
                        .position(|b| b.kind == crate::sculpt::Kind::Paint)
                    {
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
                }
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
                    .is_some_and(|s| s.eq_ignore_ascii_case("png"))
                {
                    return Err("Export texture to a .png file".into());
                }
                if path.exists() && !overwrite {
                    self.ui.uv.export_overwrite = Some(path.to_string_lossy().into());
                    return Ok(());
                }
                persistence::texture::save(path, &texture)?;
                self.ui.uv.export_overwrite = None;
                self.ui.status = format!("Exported {} as 16-bit PNG", path.display());
            }
            EditExternal(channel) => {
                let mut texture = get(&self.document, channel)?;
                let directory = std::env::temp_dir().join("clay-form-texture-exchange");
                std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                let path =
                    directory.join(format!("texture-{}-{}.png", texture.id, Texture::new_id()));
                persistence::texture::save(&path, &texture)?;
                let executable = if self.ui.preferences.gimp_path.trim().is_empty() {
                    "gimp"
                } else {
                    self.ui.preferences.gimp_path.trim()
                };
                let child = std::process::Command::new(executable)
                    .arg(&path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                match child {
                    Ok(mut child) => {
                        let _ =
                            std::thread::Builder::new()
                                .name("clay-gimp".into())
                                .spawn(move || {
                                    let _ = child.wait();
                                });
                    }
                    Err(e) => {
                        let _ = std::fs::remove_file(path);
                        return Err(format!(
                            "Could not launch GIMP: {e}. Set its executable in the UV texture controls"
                        ));
                    }
                }
                texture.exchange = Some(Exchange {
                    path: path.to_string_lossy().into(),
                    baseline: texture.fingerprint(),
                });
                self.document.assign_texture(texture)?;
                self.ui.status="In GIMP, export the edited image to the exchange PNG, then click Reload. Save layered work separately as XCF.".into();
            }
            Reload(channel, discard_local) => {
                let texture = get(&self.document, channel)?;
                let exchange = texture
                    .exchange
                    .as_ref()
                    .ok_or("Send this texture to GIMP first")?;
                if !discard_local && texture.fingerprint() != exchange.baseline {
                    self.ui.uv.reload_conflict = true;
                    return Err("This texture changed in Clay Form since export. Keep local edits, or explicitly replace them with the external image".into());
                }
                let mut next = persistence::texture::load(&exchange.path, channel)?;
                next.id = texture.id;
                next.name = texture.name;
                next.exchange = Some(Exchange {
                    path: exchange.path.clone(),
                    baseline: next.fingerprint(),
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
        app.texture_command(TextureCommand::Create(Channel::Color, 16))
            .unwrap();
        let mut texture = app.document.mesh().uv.textures[0].clone();
        let path = std::env::temp_dir().join(format!("clay-reload-{}.png", texture.id));
        texture.exchange = Some(Exchange {
            path: path.to_string_lossy().into(),
            baseline: texture.fingerprint(),
        });
        app.document.assign_texture(texture.clone()).unwrap();
        let before = app.document.geometry_snapshot();
        let mut external = Texture::new(Channel::Color, 32, &Default::default()).unwrap();
        std::sync::Arc::make_mut(&mut external.pixels)[0] = 12345;
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
        std::sync::Arc::make_mut(&mut local.pixels)[0] = 54321;
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
        assert!(
            app.texture_command(TextureCommand::EditExternal(Channel::Color))
                .is_err()
        );
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
        let p = point(&out, "Create texture");
        capture.render(&ctx, out);
        for events in [
            vec![egui::Event::PointerMoved(p), button(p, true)],
            vec![button(p, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
        assert_eq!(app.document.mesh().uv.textures.len(), 1);
        let out = frame(&mut app, vec![]);
        let p = point(&out, "Paint this texture");
        capture.render(&ctx, out);
        for events in [
            vec![egui::Event::PointerMoved(p), button(p, true)],
            vec![button(p, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
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
        let output = frame(&mut app, vec![egui::Event::PointerGone]);
        let pixels = capture.render(&ctx, output);
        capture.save(std::env::temp_dir().join("clay-uv-textures.png"), &pixels);
    }
}
