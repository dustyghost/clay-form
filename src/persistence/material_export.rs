//! Exchange snapshots. Native projects remain the lossless editable source.
use crate::sculpt::{
    Project,
    material::layers::{VALUE_COUNT, group},
};
use std::{io::Write, path::PathBuf};
mod interchange;
pub use interchange::{ExportBundle, prepare_export};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Ply,
    Blender,
    OpenUsd,
    Fbx,
}
impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Ply => "ply",
            Self::Blender => "blend",
            Self::OpenUsd => "usda",
            Self::Fbx => "fbx",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Ply => "Export PLY",
            Self::Blender => "Export Blender",
            Self::OpenUsd => "Export OpenUSD",
            Self::Fbx => "Export FBX",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Ply => "Active model · geometry and base vertex colours only; no material layers",
            Self::Blender => {
                "Active model · UVs, embedded texture paint, vertex paint and editable material layers; requires Blender 4.0+ on PATH"
            }
            Self::OpenUsd => {
                "Active model · UVs, normals and base PBR textures + companion folder; save .clay for editable layers and masks"
            }
            Self::Fbx => {
                "Active model · UVs, normals and base PBR textures + companion folder; requires Blender 4.0+ on PATH; material support varies by importer"
            }
        }
    }
}
/// A world-space snapshot, captured on the UI thread before the export worker starts.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ExportLighting {
    lights: Vec<ExportLight>,
    ambient_fill: f32,
}
#[derive(Clone, Debug, serde::Serialize)]
struct ExportLight {
    kind: &'static str,
    vector: [f32; 3],
    colour: [f32; 3],
    intensity: f32,
}
impl ExportLighting {
    pub(crate) fn capture(rig: &crate::lighting::Lighting, camera: crate::camera::Camera) -> Self {
        let (right, up, view) = camera.basis();
        Self {
            ambient_fill: rig.fill,
            lights: rig
                .lights
                .iter()
                .map(|light| {
                    let vector = light.vector(right, up, view);
                    ExportLight {
                        kind: if light.kind == crate::lighting::LightKind::Point {
                            "POINT"
                        } else {
                            "SUN"
                        },
                        vector: [vector.0, vector.1, vector.2],
                        colour: light.colour,
                        intensity: light.intensity,
                    }
                })
                .collect(),
        }
    }
}
pub fn export_bytes(
    project: &Project,
    format: ExportFormat,
    lighting: Option<&ExportLighting>,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u8>, String> {
    project.validate()?;
    if cancel() {
        return Err("Export cancelled".into());
    }
    match format {
        ExportFormat::Ply => ply(project, cancel),
        ExportFormat::Blender => blender(project, lighting, cancel),
        ExportFormat::OpenUsd | ExportFormat::Fbx => {
            Err("Use the export bundle to retain companion textures".into())
        }
    }
}
fn ply(project: &Project, cancel: &dyn Fn() -> bool) -> Result<Vec<u8>, String> {
    let mesh = &project.mesh;
    let mut data = Vec::new();
    write!(data,"ply\nformat binary_little_endian 1.0\ncomment Clay Form base vertex colours in sRGB; material layers not included\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nelement face {}\nproperty list uchar uint vertex_indices\nend_header\n",mesh.vertices.len(),mesh.triangles.len()).map_err(|e|e.to_string())?;
    for (i, p) in mesh.vertices.iter().enumerate() {
        if i % 1024 == 0 && cancel() {
            return Err("Export cancelled".into());
        }
        for v in [p.0, p.1, p.2] {
            data.extend(v.to_le_bytes());
        }
        for c in mesh.material(i).base_color {
            data.push(
                (crate::sculpt::material::linear_to_srgb(c).clamp(0., 1.) * 255.).round() as u8,
            );
        }
    }
    for face in &mesh.triangles {
        data.push(3);
        for i in face {
            data.extend((*i as u32).to_le_bytes());
        }
    }
    Ok(data)
}
#[derive(serde::Serialize)]
struct Exchange<'a> {
    version: u32,
    name: &'a str,
    vertices: Vec<[f32; 3]>,
    faces: &'a Vec<[usize; 3]>,
    normals: Vec<[[f32; 3]; 3]>,
    /// Cached channels include arbitrary paint blending; residuals preserve it exactly.
    channels: Vec<Vec<f32>>,
    assets: Vec<ExchangeAsset<'a>>,
    uv: &'a Vec<[[f32; 2]; 3]>,
    mapped: Vec<bool>,
    textures: Vec<ExchangeTexture>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lighting: Option<&'a ExportLighting>,
}
#[derive(serde::Serialize)]
struct ExchangeTexture {
    channel: crate::sculpt::texture::Channel,
    path: String,
    linear: bool,
}
fn texture_filename(texture: &crate::sculpt::texture::Texture) -> String {
    let extension = if texture.pixels.precision() == crate::sculpt::texture::Precision::Float32 {
        "exr"
    } else {
        "png"
    };
    format!("paint-{}.{}", texture.channel.index(), extension)
}
#[derive(serde::Serialize)]
struct ExchangeAsset<'a> {
    id: String,
    name: &'a str,
    values: Vec<f32>,
    weights: Vec<[f32; 4]>,
}
fn exchange(
    project: &Project,
    lighting: Option<&ExportLighting>,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u8>, String> {
    let mesh = &project.mesh;
    if !mesh.uv.corners.is_empty() && !mesh.uv.current(mesh) {
        return Err("UV topology is stale; unwrap the model before exporting".into());
    }
    let shading = mesh.surface_shading();
    let normals = shading
        .triangles
        .iter()
        .map(|face| {
            face.map(|i| {
                let n = shading.vertices[i].1;
                [n.0, n.1, n.2]
            })
        })
        .collect();
    let mut data = Exchange {
        normals,
        version: 1,
        name: &project.model_name,
        vertices: mesh.vertices.iter().map(|p| [p.0, p.1, p.2]).collect(),
        faces: &mesh.triangles,
        channels: vec![vec![0.; mesh.vertices.len()]; VALUE_COUNT],
        assets: Vec::new(),
        uv: &mesh.uv.corners,
        mapped: (0..mesh.uv.corners.len())
            .map(|f| mesh.uv.mapped(f))
            .collect(),
        textures: mesh
            .uv
            .textures
            .iter()
            .map(|texture| ExchangeTexture {
                channel: texture.channel,
                path: texture_filename(texture),
                linear: texture.pixels.precision() == crate::sculpt::texture::Precision::Float32,
            })
            .collect(),
        lighting,
    };
    for (i, m) in (0..mesh.vertices.len()).map(|i| (i, mesh.material(i))) {
        if i % 1024 == 0 && cancel() {
            return Err("Export cancelled".into());
        }
        for (c, v) in m.values().iter().enumerate() {
            data.channels[c][i] = *v;
        }
    }
    for asset in &project.scene.materials {
        let weights: Vec<_> = (0..mesh.vertices.len())
            .map(|i| {
                mesh.material(i)
                    .links
                    .iter()
                    .find(|l| l.id == asset.id)
                    .map_or([0.; 4], |l| l.weights)
            })
            .collect();
        if weights.iter().all(|w| *w == [0.; 4]) {
            continue;
        }
        let values = asset.material.values();
        for (c, channel) in data.channels.iter_mut().enumerate() {
            for (v, weights) in channel.iter_mut().zip(&weights) {
                *v -= values[c] * weights[group(c)];
            }
        }
        data.assets.push(ExchangeAsset {
            id: asset.id.to_string(),
            name: &asset.name,
            values: values.to_vec(),
            weights,
        });
    }
    serde_json::to_vec(&data).map_err(|e| e.to_string())
}
struct Temporary(PathBuf);
impl Temporary {
    fn new() -> Result<Self, String> {
        let directory = std::env::temp_dir().join(format!(
            "clay-export-{}-{}",
            std::process::id(),
            crate::sculpt::texture::Texture::new_id()
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory).map_err(|e| e.to_string())?;
        Ok(Self(directory))
    }
}
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn blender(
    project: &Project,
    lighting: Option<&ExportLighting>,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u8>, String> {
    let temporary = Temporary::new()?;
    let input = temporary.0.join("material.json");
    let script = temporary.0.join("export.py");
    let output = temporary.0.join("model.blend");
    std::fs::write(&input, exchange(project, lighting, cancel)?).map_err(|e| e.to_string())?;
    for texture in &project.mesh.uv.textures {
        if cancel() {
            return Err("Export cancelled".into());
        }
        // Pixels are already in working sRGB. Do not restore an imported ICC
        // profile: Blender reads this PNG explicitly as sRGB.
        let mut texture = texture.clone();
        texture.profile.clear();
        crate::persistence::texture::save(&temporary.0.join(texture_filename(&texture)), &texture)?;
    }
    std::fs::write(&script, include_str!("export_blender.py")).map_err(|e| e.to_string())?;
    run_blender(&input, &output, &script, cancel)?;
    std::fs::read(output).map_err(|e| format!("Blender did not produce a file: {e}"))
}
fn run_blender(
    input: &std::path::Path,
    output: &std::path::Path,
    script: &std::path::Path,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let log_path = input.with_extension("log");
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut process = std::process::Command::new("blender")
        .env("ALSOFT_DRIVERS", "null")
        .args([
            "--background",
            "--factory-startup",
            "-noaudio",
            "--python-exit-code",
            "1",
            "--python",
        ])
        .arg(script)
        .arg("--")
        .arg(input)
        .arg(output)
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .spawn()
        .map_err(|e| {
            format!("Cannot start Blender: {e}. Install Blender 4.0 or newer and add it to PATH.")
        })?;
    let start = std::time::Instant::now();
    loop {
        if cancel() || start.elapsed() > std::time::Duration::from_secs(300) {
            let _ = process.kill();
            let _ = process.wait();
            return Err(if cancel() {
                "Export cancelled"
            } else {
                "Blender export timed out"
            }
            .into());
        }
        match process.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let log = std::fs::read_to_string(&log_path).unwrap_or_default();
                    return Err(format!(
                        "Blender export failed: {}",
                        log.chars()
                            .rev()
                            .take(2000)
                            .collect::<String>()
                            .chars()
                            .rev()
                            .collect::<String>()
                    ));
                }
                return Ok(());
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => {
                let _ = process.kill();
                let _ = process.wait();
                return Err(e.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::Document,
        sculpt::{Brush, Kind, Material, material::layers::Asset},
    };
    fn fixture() -> Project {
        let mut doc = Document::default();
        let mut m = Material::from_srgb([0.2, 0.4, 0.6], 0.3, 0.4);
        m.subsurface = 0.45;
        m.subsurface_radius = [1., 0.35, 0.2];
        m.subsurface_scale = 0.04;
        m.ior = 1.7;
        m.specular = 0.65;
        m.layers[0].weight = 0.3;
        m.layers[0].add = 1.;
        m.layers[0].surface.emission = [0.1, 0.3, 0.2];
        m.layers[0].surface.emission_strength = 0.8;
        doc.save_material(Asset {
            id: 42,
            name: "Copper coating".into(),
            material: m.clone(),
            original: None,
        })
        .unwrap();
        let mut brush = Brush::new(Kind::Paint);
        brush.material = m.with_link(42);
        brush.strength = 0.6;
        doc.fill_material(&brush).unwrap();
        doc.project()
    }
    #[test]
    fn material_exports_preserve_colour_bytes_and_reconstruct_layered_channels() {
        let project = fixture();
        let bytes = export_bytes(&project, ExportFormat::Ply, None, &|| false).unwrap();
        let marker = b"end_header\n";
        let start = bytes
            .windows(marker.len())
            .position(|w| w == marker)
            .unwrap()
            + marker.len();
        assert_eq!(
            bytes.len() - start,
            project.mesh.vertices.len() * 15 + project.mesh.triangles.len() * 13
        );
        for c in 0..3 {
            assert_eq!(
                bytes[start + 12 + c],
                (crate::sculpt::material::linear_to_srgb(project.mesh.material(0).base_color[c])
                    * 255.)
                    .round() as u8
            );
        }
        let data: serde_json::Value =
            serde_json::from_slice(&exchange(&project, None, &|| false).unwrap()).unwrap();
        for c in 0..VALUE_COUNT {
            let residual = data["channels"][c][0].as_f64().unwrap();
            let weight = data["assets"][0]["weights"][0][group(c)].as_f64().unwrap();
            let value = data["assets"][0]["values"][c].as_f64().unwrap();
            assert!(
                (residual + weight * value - project.mesh.material(0).values()[c] as f64).abs()
                    < 1e-6
            );
        }
        assert!(export_bytes(&project, ExportFormat::Ply, None, &|| true).is_err());
    }
    #[test]
    fn lighting_export_is_optional_and_freezes_camera_relative_lights() {
        use crate::lighting::{Light, LightKind, Lighting};
        let project = fixture();
        let camera = crate::camera::Camera {
            yaw: 0.7,
            pitch: -0.4,
            ..Default::default()
        };
        let mut rig = Lighting {
            lights: vec![
                Light::default(),
                Light {
                    kind: LightKind::Point,
                    camera_relative: false,
                    distance: 3.,
                    ..Default::default()
                },
            ],
            fill: 0.4,
            ..Default::default()
        };
        let snapshot = ExportLighting::capture(&rig, camera);
        let axes = camera.basis();
        let direction = rig.lights[0].vector(axes.0, axes.1, axes.2);
        assert_eq!(
            snapshot.lights[0].vector,
            [direction.0, direction.1, direction.2]
        );
        assert_eq!(snapshot.lights[0].kind, "SUN");
        assert_eq!(snapshot.lights[1].kind, "POINT");
        rig.lights[0].azimuth = 130.;
        rig.lights[1].distance = 10.;
        let with: serde_json::Value =
            serde_json::from_slice(&exchange(&project, Some(&snapshot), &|| false).unwrap())
                .unwrap();
        let without: serde_json::Value =
            serde_json::from_slice(&exchange(&project, None, &|| false).unwrap()).unwrap();
        assert!(without.get("lighting").is_none());
        assert_eq!(with["lighting"]["lights"].as_array().unwrap().len(), 2);
        for (i, expected) in [direction.0, direction.1, direction.2]
            .into_iter()
            .enumerate()
        {
            assert!(
                (with["lighting"]["lights"][0]["vector"][i].as_f64().unwrap() - expected as f64)
                    .abs()
                    < 1e-6
            );
        }
        assert_eq!(
            export_bytes(&project, ExportFormat::Ply, Some(&snapshot), &|| false).unwrap(),
            export_bytes(&project, ExportFormat::Ply, None, &|| false).unwrap()
        );
    }
    #[test]
    #[ignore = "manual Blender light interoperability validation; requires Blender on PATH"]
    fn lighting_blender_export_manual_review() {
        use crate::lighting::{Light, LightKind, Lighting};
        let directory = PathBuf::from(
            std::env::var("CLAY_EXPORT_REVIEW").unwrap_or_else(|_| "/tmp/clay-light-export".into()),
        );
        std::fs::create_dir_all(&directory).unwrap();
        let project = fixture();
        let camera = crate::camera::Camera {
            yaw: 0.7,
            pitch: -0.4,
            ..Default::default()
        };
        let rig = Lighting {
            lights: vec![
                Light::default(),
                Light {
                    kind: LightKind::Point,
                    camera_relative: false,
                    distance: 3.,
                    intensity: 0.7,
                    colour: [0.2, 0.5, 1.],
                    ..Default::default()
                },
            ],
            fill: 0.4,
            ..Default::default()
        };
        let snapshot = ExportLighting::capture(&rig, camera);
        let empty = ExportLighting::capture(
            &Lighting {
                lights: vec![],
                fill: 0.4,
                ..Default::default()
            },
            camera,
        );
        for (name, lights) in [
            ("viewport", Some(&snapshot)),
            ("neutral", None),
            ("ambient", Some(&empty)),
        ] {
            let bytes = export_bytes(&project, ExportFormat::Blender, lights, &|| false).unwrap();
            std::fs::write(directory.join(format!("{name}.blend")), bytes).unwrap();
        }
        std::fs::write(
            directory.join("lighting.json"),
            serde_json::to_vec(&snapshot).unwrap(),
        )
        .unwrap();
    }
    #[test]
    #[ignore = "manual Blender interoperability validation; requires Blender on PATH"]
    fn material_blender_export_manual_review() {
        let directory =
            std::env::var("CLAY_EXPORT_REVIEW").unwrap_or_else(|_| "/tmp/clay-layer-export".into());
        std::fs::create_dir_all(&directory).unwrap();
        let mut project = fixture();
        // Vary five independent fields, including negative residuals under the
        // shared definition, to exercise both packed RGB and alpha components.
        let count = project.mesh.materials.len() as f32;
        for (i, material) in project.mesh.materials.iter_mut().enumerate() {
            let t = i as f32 / count;
            material.base_color = [t, t * t, (t * 3.).fract()];
            material.roughness = 0.1 + 0.8 * t;
            material.metallic = 0.8 * t * t;
        }
        let bytes = export_bytes(&project, ExportFormat::Blender, None, &|| false).unwrap();
        assert!(bytes.starts_with(b"BLENDER") || bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]));
        std::fs::write(PathBuf::from(&directory).join("layered.blend"), bytes).unwrap();
        std::fs::write(
            PathBuf::from(&directory).join("exchange.json"),
            exchange(&project, None, &|| false).unwrap(),
        )
        .unwrap();
    }
    #[test]
    #[ignore = "manual Blender texture interoperability validation; requires Blender on PATH"]
    fn texture_blender_export_manual_review() {
        use crate::sculpt::texture::{Channel, Precision, Texture};
        let directory = PathBuf::from(
            std::env::var("CLAY_EXPORT_REVIEW")
                .unwrap_or_else(|_| "/tmp/clay-texture-export".into()),
        );
        std::fs::create_dir_all(&directory).unwrap();
        if let Ok(path) = std::env::var("CLAY_EXPORT_PROJECT") {
            let project = crate::persistence::load_project(&path).unwrap();
            let bytes = export_bytes(&project, ExportFormat::Blender, None, &|| false).unwrap();
            std::fs::write(directory.join("recovered.blend"), bytes).unwrap();
            return;
        }
        for precision in [Precision::U16, Precision::Float32] {
            let mut project = Document::default().project();
            project.mesh.uv = project.mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
            let mut texture =
                Texture::new_with_precision(Channel::Color, 32, &Material::default(), precision)
                    .unwrap();
            // Asymmetric colour and coverage expose flipped UVs and lost alpha.
            for i in 0..1024 {
                texture.pixels.set_pixel(
                    i,
                    if i < 512 {
                        [0.9, 0.1, 0.02, 1.]
                    } else {
                        [0.02, 0.3, 0.9, 0.5]
                    },
                );
            }
            project.mesh.uv.textures.push(texture);
            project.mesh.uv.removed = vec![false; project.mesh.triangles.len()];
            project.mesh.uv.removed[0] = true;
            let name = if precision == Precision::U16 {
                "png"
            } else {
                "exr"
            };
            let bytes = export_bytes(&project, ExportFormat::Blender, None, &|| false).unwrap();
            std::fs::write(directory.join(format!("{name}.blend")), bytes).unwrap();
            std::fs::write(
                directory.join(format!("{name}.json")),
                exchange(&project, None, &|| false).unwrap(),
            )
            .unwrap();
        }
    }
    #[test]
    fn blender_snapshot_preserves_uv_coverage_and_float_texture_metadata() {
        use crate::sculpt::texture::{Channel, Precision, Texture};
        let mut project = Document::default().project();
        project.mesh = crate::sculpt::Mesh {
            vertices: vec![
                crate::sculpt::Vec3(0., 0., 0.),
                crate::sculpt::Vec3(1., 0., 0.),
                crate::sculpt::Vec3(1., 1., 0.),
                crate::sculpt::Vec3(0., 1., 0.),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            mask: vec![0.; 4],
            ..Default::default()
        };
        project.mesh.uv.corners = vec![
            [[0.1, 0.2], [0.8, 0.2], [0.8, 0.9]],
            [[0.1, 0.2], [0.8, 0.9], [0.1, 0.9]],
        ];
        project.mesh.uv.topology = project.mesh.triangles.clone();
        project.mesh.uv.removed = vec![false; project.mesh.triangles.len()];
        project.mesh.uv.removed[0] = true;
        project.mesh.uv.textures.push(
            Texture::new_with_precision(
                Channel::Color,
                16,
                &Material::default(),
                Precision::Float32,
            )
            .unwrap(),
        );
        let snapshot: serde_json::Value =
            serde_json::from_slice(&exchange(&project, None, &|| false).unwrap()).unwrap();
        let uv: Vec<[[f32; 2]; 3]> = serde_json::from_value(snapshot["uv"].clone()).unwrap();
        assert_eq!(uv, project.mesh.uv.corners);
        assert_eq!(snapshot["mapped"][0], false);
        assert_eq!(snapshot["mapped"][1], true);
        assert_eq!(snapshot["textures"][0]["path"], "paint-0.exr");
        assert_eq!(snapshot["textures"][0]["linear"], true);
        project.mesh.uv.topology.swap(0, 1);
        assert!(
            exchange(&project, None, &|| false)
                .unwrap_err()
                .contains("stale")
        );
    }
}
