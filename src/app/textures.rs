use super::*;
use crate::sculpt::texture::{Channel, Exchange, Precision, Texture};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TextureStart {
    #[default]
    Surface,
    Transparent,
    Solid,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum WorkingFormat {
    #[default]
    Auto,
    Psd,
    Xcf,
}
#[derive(Clone)]
pub(crate) enum TextureCommand {
    #[cfg(test)]
    Create(Channel, u32, Precision),
    Start(Channel, u32, Precision, TextureStart, bool),
    Cancel,
    NewSnapshot(Channel),
    Relink(Channel, String),
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
    resume_paint: Option<Channel>,
    progress: Option<std::sync::Arc<std::sync::atomic::AtomicU32>>,
    description: &'static str,
    retry: TextureCommand,
}

impl App {
    pub(super) fn sample_surface_material(
        &mut self,
        sampled: crate::sculpt::Material,
    ) -> Result<(), String> {
        use crate::sculpt::PaintChannels;
        let mut brush = self.document.brushes()[self.ui.selected].clone();
        let channels = match self.document.texture_paint {
            Some(Channel::Color) => PaintChannels::Color,
            Some(Channel::Roughness) => PaintChannels::Roughness,
            Some(Channel::Metallic) => PaintChannels::Metallic,
            None => brush.paint_channels,
        };
        let float_colour =
            self.document.texture_paint == Some(Channel::Color)
                && self.document.mesh().uv.textures.iter().any(|t| {
                    t.channel == Channel::Color && t.pixels.precision() == Precision::Float32
                });
        let mut bounded = sampled.clone().unlinked();
        bounded.base_color = sampled.base_color.map(|v| v.clamp(0., 1.));
        bounded.roughness = sampled.roughness.clamp(0., 1.);
        bounded.metallic = sampled.metallic.clamp(0., 1.);
        let clipped = match channels {
            PaintChannels::Material => {
                bounded.base_color != sampled.base_color
                    || bounded.roughness != sampled.roughness
                    || bounded.metallic != sampled.metallic
            }
            PaintChannels::Color => !float_colour && bounded.base_color != sampled.base_color,
            PaintChannels::Roughness => bounded.roughness != sampled.roughness,
            PaintChannels::Metallic => bounded.metallic != sampled.metallic,
        };
        brush.material = brush.material.unlinked();
        match channels {
            PaintChannels::Material => {
                brush.material = bounded;
                brush.texture_colour = None;
            }
            PaintChannels::Color => {
                brush.material.base_color = bounded.base_color;
                brush.texture_colour = float_colour.then_some(sampled.base_color);
            }
            PaintChannels::Roughness => brush.material.roughness = bounded.roughness,
            PaintChannels::Metallic => brush.material.metallic = bounded.metallic,
        }
        self.document.update_brush(self.ui.selected, brush)?;
        self.ui.status = if clipped { "Sampled values were limited to 0–1 for this target. Float colour textures retain HDR/negative colour samples." }
            else { "Sampled surface values into the brush for the selected paint channel." }.into();
        Ok(())
    }

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
        let format = self.ui.uv.working_format;
        let gimp_editor = editor.is_empty()
            || std::path::Path::new(editor)
                .file_stem()
                .is_some_and(|name| name.to_string_lossy().to_lowercase().starts_with("gimp"));
        let xcf = format == WorkingFormat::Xcf
            || (format == WorkingFormat::Auto
                && gimp_editor
                && texture.pixels.precision() == Precision::Float32);
        if create && !xcf && gimp_editor && texture.pixels.precision() == Precision::Float32 {
            return Err("GIMP cannot save float PSD. Choose Automatic or XCF working format, or choose a float-PSD-capable external app.".into());
        }
        let directory = if self.document.has_saved_path() {
            std::path::Path::new(self.document.current_path()).with_extension("textures")
        } else {
            crate::preferences::path()
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .join("external-paint")
        }
        .join(format!("session-{}", Texture::new_id()));
        let channel = texture.channel;
        let description = if create {
            if xcf {
                "Preparing float-safe XCF with UV guide"
            } else {
                "Preparing layered PSD with UV guide"
            }
        } else {
            "Reading saved paint layers; excluding UV guide"
        };
        let worker =
            crate::background_job::BackgroundJob::start("clay-paint-exchange", move |cancel| {
                if create {
                    persistence::gimp::prepare_working(
                        texture,
                        &uv,
                        &directory,
                        xcf,
                        &executable,
                        cancel,
                    )
                    .map(|t| vec![t])
                } else {
                    persistence::gimp::reload(&texture, &executable, cancel).map(|t| vec![t])
                }
            })?;
        self.texture_exchange = Some(ExchangeJob {
            geometry,
            worker,
            open_in,
            bake: false,
            resume_paint: None,
            progress: None,
            description,
            retry: if create {
                TextureCommand::NewSnapshot(channel)
            } else {
                TextureCommand::Reload(channel, false)
            },
        });
        self.ui.uv.exchange_busy = true;
        self.ui.uv.completed = false;
        self.ui.uv.operation = description.into();
        self.ui.uv.operation_progress = None;
        self.ui.uv.retry = None;
        self.ui.status = format!("{description}…");
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn finish_texture_exchange_for_test(&mut self, ctx: &egui::Context) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while self.texture_exchange.is_some() {
            assert!(
                std::time::Instant::now() < deadline,
                "Texture operation timed out"
            );
            self.poll_texture_exchange(ctx);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    pub(super) fn poll_texture_exchange(&mut self, ctx: &egui::Context) {
        let Some(job) = &mut self.texture_exchange else {
            return;
        };
        self.ui.uv.operation = job.description.into();
        self.ui.uv.operation_progress = job
            .progress
            .as_ref()
            .map(|p| p.load(std::sync::atomic::Ordering::Relaxed) as f32 / 1000.);
        let Some(result) = job.worker.poll() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        };
        let job = self
            .texture_exchange
            .take()
            .expect("completed texture exchange");
        self.ui.uv.exchange_busy = false;
        let resume_paint = job.resume_paint;
        let retry = job.retry.clone();
        let cancelled = job.worker.is_cancelled();
        let result = result.and_then(|textures| {
            if !self.document.geometry_matches(&job.geometry) {
                return Err("The document changed while the texture operation was running; current edits kept. Retry when ready".into());
            }
            self.viewport.cancel(&mut self.document);
            if let Some(executable) = job.open_in {
                let texture = &textures[0];
                let path = texture.exchange.as_ref().and_then(|e| e.layered_path.as_ref()).ok_or("Missing layered working copy")?;
                let path = path.clone();
                // Persist the working path even if the chosen editor cannot launch.
                self.document.assign_textures(textures)?;
                persistence::gimp::open(&executable, std::path::Path::new(&path))?;
                Ok("Working document ready. Save back to it, then Reload. The separate UV guide is excluded automatically.")
            } else {
                self.document.assign_textures(textures)?;
                self.ui.uv.reload_conflict = false;
                Ok(if job.bake { "Baked unlit paint/material maps. Use Paint on model to continue painting, or Edit in external paint app. Undo restores the previous textures." } else { "Reloaded saved paint layers without the UV guide; Undo restores the previous texture" })
            }
        });
        match result {
            Ok(message) => {
                self.ui.status = message.into();
                self.ui.uv.completed = true;
                self.ui.uv.retry = None;
                if let Some(channel) = resume_paint
                    && let Err(error) = self.texture_command(TextureCommand::Paint(Some(channel)))
                {
                    self.ui.status = error;
                }
            }
            Err(error) => {
                self.ui.status = error.clone();
                self.ui.uv.operation = error;
                self.ui.uv.retry = (!cancelled).then_some(retry);
            }
        }
    }

    fn start_texture_bake(
        &mut self,
        channels: Vec<Channel>,
        options: crate::sculpt::texture::bake::BakeOptions,
        resume_paint: Option<Channel>,
        retry: TextureCommand,
    ) -> Result<(), String> {
        if self.texture_exchange.is_some() {
            return Err("Wait for or cancel the current texture operation".into());
        }
        let geometry = self.document.geometry_snapshot();
        let mesh = self.document.mesh().clone();
        let progress = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let worker_progress = progress.clone();
        let worker =
            crate::background_job::BackgroundJob::start("clay-texture-bake", move |cancel| {
                mesh.bake_texture_channels(&channels, options, cancel, &|value| {
                    worker_progress
                        .store((value * 1000.) as u32, std::sync::atomic::Ordering::Relaxed);
                })
            })?;
        let description = if options.composite {
            "Baking surface colour and material maps"
        } else {
            "Capturing vertex surface into texture"
        };
        self.texture_exchange = Some(ExchangeJob {
            geometry,
            worker,
            open_in: None,
            bake: true,
            resume_paint,
            progress: Some(progress),
            description,
            retry,
        });
        self.ui.uv.exchange_busy = true;
        self.ui.uv.completed = false;
        self.ui.uv.operation = description.into();
        self.ui.uv.operation_progress = Some(0.);
        self.ui.uv.retry = None;
        self.ui.status = format!("{description}…");
        Ok(())
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
            Cancel => {
                if let Some(job) = &self.texture_exchange {
                    job.worker.cancel();
                }
                self.ui.uv.operation = "Cancelling texture operation…".into();
            }
            Bake(all, size, precision) => {
                let channels = if all {
                    Channel::ALL.to_vec()
                } else {
                    vec![Channel::Color]
                };
                self.start_texture_bake(
                    channels,
                    crate::sculpt::texture::bake::BakeOptions {
                        size,
                        precision,
                        preserve_existing: !self.ui.uv.bake_override,
                        composite: all,
                    },
                    None,
                    Bake(all, size, precision),
                )?;
            }
            CreateAndPaint(channel, size, precision) => {
                self.texture_command(Start(channel, size, precision, self.ui.uv.initial, true))?;
            }
            Start(channel, size, precision, initial, paint) => {
                if !self.document.mesh().uv.current(self.document.mesh()) {
                    return Err(
                        "Generate UVs using Automatic unwrap below, then start texture painting."
                            .into(),
                    );
                }
                if self
                    .document
                    .mesh()
                    .uv
                    .textures
                    .iter()
                    .any(|t| t.channel == channel)
                {
                    return Err("This channel already has a texture. Choose Paint on model or remove it before creating another.".into());
                }
                if initial == TextureStart::Surface {
                    self.start_texture_bake(
                        vec![channel],
                        crate::sculpt::texture::bake::BakeOptions {
                            size,
                            precision,
                            preserve_existing: false,
                            composite: false,
                        },
                        paint.then_some(channel),
                        Start(channel, size, precision, initial, paint),
                    )?;
                } else {
                    let brush = &self.document.brushes()[self.ui.selected];
                    let mut material = brush.material.clone();
                    if channel == Channel::Color && precision == Precision::Float32 {
                        material.base_color = brush.texture_colour.unwrap_or(material.base_color);
                    }
                    let mut texture =
                        Texture::new_with_precision(channel, size, &material, precision)?;
                    if initial == TextureStart::Transparent {
                        match &mut texture.pixels {
                            crate::sculpt::texture::Pixels::U16(p) => {
                                std::sync::Arc::make_mut(p).fill(0)
                            }
                            crate::sculpt::texture::Pixels::Float32(p) => {
                                std::sync::Arc::make_mut(p).fill(0.)
                            }
                        }
                    }
                    self.document.assign_texture(texture)?;
                    if paint {
                        self.texture_command(Paint(Some(channel)))?;
                    }
                }
            }
            #[cfg(test)]
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
                if let Some(exchange) = &texture.exchange {
                    let path = exchange.layered_path.as_ref().unwrap_or(&exchange.path);
                    if !std::path::Path::new(path).is_file() {
                        return Err("Working file is missing. Choose working file to relink it, or create a New snapshot.".into());
                    }
                    let app = self.ui.preferences.external_paint_app.trim();
                    let reader = self.ui.preferences.gimp_path.trim();
                    let app = if !app.is_empty() {
                        app
                    } else if !reader.is_empty() {
                        reader
                    } else {
                        "gimp"
                    };
                    persistence::gimp::open(app, std::path::Path::new(path))?;
                    self.ui.status =
                        "Opened existing working document. Save there, then Reload here.".into();
                } else {
                    self.start_texture_exchange(texture, true)?;
                }
            }
            NewSnapshot(channel) => {
                self.start_texture_exchange(get(&self.document, channel)?, true)?;
            }
            Relink(channel, path) => {
                let mut texture = get(&self.document, channel)?;
                let file = std::path::Path::new(&path);
                let metadata = std::fs::metadata(file)
                    .map_err(|e| format!("Cannot open working file: {e}"))?;
                if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 * 1024 {
                    return Err("Choose a regular working file smaller than 2 GiB".into());
                }
                let extension = file
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                if !["psd", "xcf", "png", "exr"].contains(&extension.as_str()) {
                    return Err("Choose a PSD, XCF, PNG or EXR working file".into());
                }
                let path = file
                    .canonicalize()
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .into_owned();
                texture.exchange = Some(Exchange {
                    baseline: texture
                        .exchange
                        .as_ref()
                        .map_or_else(|| texture.fingerprint(), |e| e.baseline),
                    layered_path: ["psd", "xcf"]
                        .contains(&extension.as_str())
                        .then(|| path.clone()),
                    path,
                });
                self.document.assign_texture(texture)?;
                self.ui.status = "Working file linked. Reload to import saved paint; current pixels are unchanged.".into();
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
        app.finish_texture_exchange_for_test(&egui::Context::default());
    }
    #[test]
    fn hdr_sampling_paints_float_colour_and_bounds_vertex_materials() {
        let mut app = App::default();
        let mut mesh = crate::sculpt::texture::tests::sheets();
        mesh.uv.textures[0] = Texture::new_with_precision(
            Channel::Color,
            64,
            &Default::default(),
            Precision::Float32,
        )
        .unwrap();
        app.document
            .replace_geometry(crate::sculpt::Geometry::new(mesh))
            .unwrap();
        app.texture_command(TextureCommand::Paint(Some(Channel::Color)))
            .unwrap();
        let sampled = crate::sculpt::Material {
            base_color: [2., -0.25, 0.5],
            ..Default::default()
        };
        app.sample_surface_material(sampled.clone()).unwrap();
        let mut brush = app.document.brushes()[app.ui.selected].clone();
        assert_eq!(brush.texture_colour, Some(sampled.base_color));
        let decoded: crate::sculpt::Brush =
            serde_json::from_str(&serde_json::to_string(&brush).unwrap()).unwrap();
        assert_eq!(decoded.texture_colour, brush.texture_colour);
        brush.strength = 1.;
        app.document.fill_all_material(&brush).unwrap();
        let p = app.document.mesh().uv.textures[0].linear_pixel(32 * 64 + 16);
        assert_eq!(&p[..3], &sampled.base_color);
        app.texture_command(TextureCommand::Paint(None)).unwrap();
        app.sample_surface_material(sampled).unwrap();
        let brush = &app.document.brushes()[app.ui.selected];
        assert_eq!(brush.material.base_color, [1., 0., 0.5]);
        assert!(brush.texture_colour.is_none());
        assert!(app.ui.status.contains("limited to 0–1"));
    }

    #[test]
    fn texture_setup_preserves_surface_and_cancel_keeps_document() {
        let mut app = App::default();
        let mut mesh = crate::sculpt::texture::tests::sheets();
        mesh.uv.textures.clear();
        let material = crate::sculpt::Material::from_srgb([0.8, 0.2, 0.4], 0.3, 0.7);
        mesh.materials = vec![material.clone(); mesh.vertices.len()];
        app.document
            .replace_geometry(crate::sculpt::Geometry::new(mesh))
            .unwrap();
        let before = app.document.geometry_snapshot();
        app.texture_command(TextureCommand::CreateAndPaint(
            Channel::Color,
            64,
            Precision::Float32,
        ))
        .unwrap();
        finish_exchange(&mut app);
        assert_eq!(app.document.texture_paint, Some(Channel::Color));
        let p = app.document.mesh().uv.textures[0].linear_pixel(32 * 64 + 16);
        for (actual, expected) in p[..3].iter().zip(material.base_color) {
            assert!((actual - expected).abs() < 1e-6);
        }
        app.document.undo();
        assert_eq!(app.document.geometry_snapshot(), before);
        assert_eq!(app.document.texture_paint, None);
        app.texture_command(TextureCommand::Start(
            Channel::Color,
            64,
            Precision::Float32,
            TextureStart::Transparent,
            false,
        ))
        .unwrap();
        assert_eq!(app.document.mesh().uv.textures[0].linear_pixel(0), [0.; 4]);
        let before_cancel = app.document.geometry_snapshot();
        app.texture_command(TextureCommand::Bake(true, 64, Precision::Float32))
            .unwrap();
        app.texture_command(TextureCommand::Cancel).unwrap();
        finish_exchange(&mut app);
        assert_eq!(app.document.geometry_snapshot(), before_cancel);
        assert!(app.ui.status.contains("cancelled"));
    }

    #[test]
    fn working_file_relink_preserves_pixels_and_conflict_baseline() {
        let mut app = App::default();
        app.document
            .replace_geometry(crate::sculpt::Geometry::new(
                crate::sculpt::texture::tests::sheets(),
            ))
            .unwrap();
        let before = app.document.mesh().uv.textures[0].clone();
        let path = std::env::temp_dir().join(format!("clay-relink-{}.png", before.id));
        persistence::texture::save(&path, &before).unwrap();
        app.texture_command(TextureCommand::Relink(
            Channel::Color,
            path.to_string_lossy().into(),
        ))
        .unwrap();
        let linked = app.document.mesh().uv.textures[0].clone();
        assert_eq!(linked.pixels, before.pixels);
        assert_eq!(
            linked.exchange.as_ref().unwrap().baseline,
            before.fingerprint()
        );
        let mut edited = linked.clone();
        edited.pixels.set_pixel(0, [0.1, 0.2, 0.3, 1.]);
        app.document.assign_texture(edited).unwrap();
        app.texture_command(TextureCommand::Relink(
            Channel::Color,
            path.to_string_lossy().into(),
        ))
        .unwrap();
        assert!(
            app.texture_command(TextureCommand::Reload(Channel::Color, false))
                .is_err()
        );
        std::fs::remove_file(path).unwrap();
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
        let error = app
            .texture_command(TextureCommand::EditExternal(Channel::Color))
            .unwrap_err();
        assert!(error.contains("Could not open external paint app"));
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
        finish_exchange(&mut app);
        assert_eq!(
            app.document.mesh().uv.textures.len(),
            1,
            "{}",
            app.ui.status
        );
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
        let p = point(&out, "Capture and bake");
        capture.render(&ctx, out);
        for events in [
            vec![egui::Event::PointerMoved(p), button(p, true)],
            vec![button(p, false)],
        ] {
            capture.render(&ctx, frame(&mut app, events));
        }
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
