//! Exchange snapshots. Native projects remain the lossless editable source.
use crate::sculpt::{
    Project,
    material::layers::{VALUE_COUNT, group},
};
use std::{io::Write, path::PathBuf};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Ply,
    Blender,
}
impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Ply => "ply",
            Self::Blender => "blend",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Ply => "Export PLY",
            Self::Blender => "Export Blender",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Ply => "Active model · geometry and base vertex colours only; no material layers",
            Self::Blender => {
                "Active model · vertex paint and editable material layers; requires Blender 4.0 or newer on PATH"
            }
        }
    }
}
pub fn export_bytes(
    project: &Project,
    format: ExportFormat,
    cancel: &dyn Fn() -> bool,
) -> Result<Vec<u8>, String> {
    project.validate()?;
    if cancel() {
        return Err("Export cancelled".into());
    }
    match format {
        ExportFormat::Ply => ply(project, cancel),
        ExportFormat::Blender => blender(project, cancel),
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
}
#[derive(serde::Serialize)]
struct ExchangeAsset<'a> {
    id: String,
    name: &'a str,
    values: Vec<f32>,
    weights: Vec<[f32; 4]>,
}
fn exchange(project: &Project, cancel: &dyn Fn() -> bool) -> Result<Vec<u8>, String> {
    let mesh = &project.mesh;
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
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn blender(project: &Project, cancel: &dyn Fn() -> bool) -> Result<Vec<u8>, String> {
    let directory = std::env::temp_dir().join(format!(
        "clay-export-{}-{}",
        std::process::id(),
        crate::sculpt::material::layers::fresh_id()
    ));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&directory).map_err(|e| e.to_string())?;
    let temporary = Temporary(directory);
    let input = temporary.0.join("material.json");
    let script = temporary.0.join("export.py");
    let output = temporary.0.join("model.blend");
    std::fs::write(&input, exchange(project, cancel)?).map_err(|e| e.to_string())?;
    std::fs::write(&script, include_str!("export_blender.py")).map_err(|e| e.to_string())?;
    let log_path = temporary.0.join("blender.log");
    let log = std::fs::File::create(&log_path).map_err(|e| e.to_string())?;
    let mut process = std::process::Command::new("blender")
        .args([
            "--background",
            "--factory-startup",
            "-noaudio",
            "--python-exit-code",
            "1",
            "--python",
        ])
        .arg(&script)
        .arg("--")
        .arg(&input)
        .arg(&output)
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
                return std::fs::read(output)
                    .map_err(|e| format!("Blender did not produce a file: {e}"));
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
        let bytes = export_bytes(&project, ExportFormat::Ply, &|| false).unwrap();
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
            serde_json::from_slice(&exchange(&project, &|| false).unwrap()).unwrap();
        for c in 0..VALUE_COUNT {
            let residual = data["channels"][c][0].as_f64().unwrap();
            let weight = data["assets"][0]["weights"][0][group(c)].as_f64().unwrap();
            let value = data["assets"][0]["values"][c].as_f64().unwrap();
            assert!(
                (residual + weight * value - project.mesh.material(0).values()[c] as f64).abs()
                    < 1e-6
            );
        }
        assert!(export_bytes(&project, ExportFormat::Ply, &|| true).is_err());
    }
    #[test]
    #[ignore = "manual Blender interoperability validation; requires Blender on PATH"]
    fn material_blender_export_manual_review() {
        let directory =
            std::env::var("CLAY_EXPORT_REVIEW").unwrap_or_else(|_| "/tmp/clay-layer-export".into());
        std::fs::create_dir_all(&directory).unwrap();
        let project = fixture();
        let bytes = export_bytes(&project, ExportFormat::Blender, &|| false).unwrap();
        assert!(bytes.starts_with(b"BLENDER") || bytes.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]));
        std::fs::write(PathBuf::from(&directory).join("layered.blend"), bytes).unwrap();
        std::fs::write(
            PathBuf::from(&directory).join("exchange.json"),
            exchange(&project, &|| false).unwrap(),
        )
        .unwrap();
    }
}
