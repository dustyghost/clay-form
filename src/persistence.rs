//! Filesystem adapter: versioned projects, presets, image alphas, and OBJ export.
use crate::sculpt::{Alpha, Brush, Mesh, Project, validate_brush};
pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 128 * 1024 * 1024 {
        return Err("File exceeds 128 MB limit".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub(crate) fn write_json<T: serde::Serialize>(path: &str, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let temp = format!("{path}.tmp");
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(temp, path).map_err(|e| e.to_string())
}
pub(crate) fn same_path(a: &str, b: &str) -> bool {
    fn resolved(path: &str) -> std::path::PathBuf {
        let p = std::path::Path::new(path);
        p.canonicalize().unwrap_or_else(|_| {
            let parent = p
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            parent
                .canonicalize()
                .unwrap_or_else(|_| parent.into())
                .join(p.file_name().unwrap_or_default())
        })
    }
    resolved(a) == resolved(b)
}

pub fn load_project(path: &str) -> Result<Project, String> {
    let project: Project = read_json(path)?;
    project.validate()?;
    Ok(project)
}
pub fn save_project(path: &str, project: &Project) -> Result<(), String> {
    project.validate()?;
    write_json(path, project)
}
pub fn load_brush(path: &str) -> Result<Brush, String> {
    let brush: Brush = read_json(path)?;
    validate_brush(&brush)?;
    Ok(brush)
}
pub fn save_brush(path: &str, brush: &Brush) -> Result<(), String> {
    validate_brush(brush)?;
    write_json(path, brush)
}
pub fn load_alpha(path: &str) -> Result<Alpha, String> {
    let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
    let mut reader = reader.with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?.to_rgba8();
    Ok::<_, String>(Alpha::Image {
        width: image.width() as usize,
        height: image.height() as usize,
        pixels: image
            .pixels()
            .map(|p| {
                ((p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722) / 255.)
                    * (p[3] as f32 / 255.)
            })
            .collect::<Vec<_>>()
            .into(),
    })
}
pub fn export_obj(path: &str, mesh: &Mesh) -> Result<(), String> {
    let mut text = String::from("# Clay Form mesh\n");
    for v in &mesh.vertices {
        text.push_str(&format!("v {} {} {}\n", v.0, v.1, v.2));
    }
    for &[a, b, c] in &mesh.triangles {
        text.push_str(&format!("f {} {} {}\n", a + 1, b + 1, c + 1));
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_version_one_json_keeps_its_exact_data_shape() {
        let legacy = include_str!("../tests/fixtures/v1-project.json");
        let project: Project = serde_json::from_str(legacy).unwrap();
        project.validate().unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&serde_json::to_vec(&project).unwrap())
                .unwrap(),
            serde_json::from_str::<serde_json::Value>(legacy).unwrap()
        );
        let cloned = project.brushes[0].clone();
        if let (Alpha::Image { pixels: a, .. }, Alpha::Image { pixels: b, .. }) =
            (&project.brushes[0].alpha, &cloned.alpha)
        {
            assert!(std::sync::Arc::ptr_eq(a, b));
        } else {
            panic!("Legacy image alpha was not preserved");
        }
    }
    #[test]
    fn project_and_custom_preset_survive_disk_save() {
        let dir = std::env::temp_dir().join(format!(
            "clay-form-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let mut brush = Brush::new(crate::sculpt::Kind::Clay);
        brush.name = "My textured clay".into();
        brush.subtract = true;
        brush.pressure_radius = true;
        brush.rotation = 37.;
        brush.alpha = Alpha::Image {
            width: 2,
            height: 2,
            pixels: vec![0., 0.25, 0.75, 1.].into(),
        };
        let mut mesh = Mesh::sphere();
        mesh.mask[7] = 0.75;
        mesh.vertices[7].0 += 0.05;
        let project = Project {
            mesh_settings: crate::sculpt::MeshSettings::default(),
            version: 1,
            mesh,
            brushes: vec![brush.clone()],
            symmetry: crate::sculpt::Symmetry {
                enabled: true,
                mirror: [true, true, false],
                radial: true,
                radial_axis: crate::sculpt::Axis::Z,
                radial_count: 8,
                center: crate::sculpt::Vec3(0.1, 0.2, 0.3),
            },
        };
        let path = dir.join("project.clay");
        save_project(path.to_str().unwrap(), &project).unwrap();
        let loaded = load_project(path.to_str().unwrap()).unwrap();
        loaded.mesh.validate().unwrap();
        assert_eq!(loaded.symmetry, project.symmetry);
        assert_eq!(loaded.mesh.vertices[7].0, project.mesh.vertices[7].0);
        assert_eq!(loaded.mesh.mask[7], 0.75);
        assert!(loaded.brushes[0] == brush);
        assert_eq!(loaded.brushes[0].alpha, brush.alpha);
        let preset = dir.join("preset.brush.json");
        save_brush(preset.to_str().unwrap(), &brush).unwrap();
        let restored = load_brush(preset.to_str().unwrap()).unwrap();
        validate_brush(&restored).unwrap();
        assert_eq!(restored.alpha, brush.alpha);
        assert!(!dir.join("project.clay.tmp").exists());
        assert!(same_path(
            path.to_str().unwrap(),
            dir.join("./project.clay").to_str().unwrap()
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod mesh_storage_tests {
    use super::*;
    #[test]
    fn mesh_levels_and_dynamic_settings_survive_files_and_obj_exports_active_level() {
        let dir = std::env::temp_dir().join(format!("clay-mesh-storage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut geometry = crate::sculpt::Geometry::new(Mesh::sphere());
        geometry.subdivide().unwrap();
        geometry.set_level(1).unwrap();
        geometry.settings.resolution = 32;
        geometry.settings.auto_remesh = true;
        let project = Project {
            version: Project::VERSION,
            mesh: geometry.mesh,
            mesh_settings: geometry.settings,
            brushes: vec![Brush::new(crate::sculpt::Kind::Standard)],
            symmetry: Default::default(),
        };
        let path = dir.join("levels.clay");
        save_project(path.to_str().unwrap(), &project).unwrap();
        let loaded = load_project(path.to_str().unwrap()).unwrap();
        assert_eq!(loaded.mesh_settings, project.mesh_settings);
        assert_eq!(loaded.mesh, project.mesh);
        let obj = dir.join("active.obj");
        export_obj(obj.to_str().unwrap(), &loaded.mesh).unwrap();
        let text = std::fs::read_to_string(obj).unwrap();
        assert_eq!(
            text.lines().filter(|l| l.starts_with("f ")).count(),
            loaded.mesh.triangles.len()
        );
        assert_eq!(
            text.lines().filter(|l| l.starts_with("v ")).count(),
            loaded.mesh.vertices.len()
        );
        let mut dynamic = project;
        dynamic.mesh_settings.lower.clear();
        dynamic.mesh_settings.upper.clear();
        dynamic.mesh_settings.dynamic = true;
        save_project(path.to_str().unwrap(), &dynamic).unwrap();
        assert_eq!(
            load_project(path.to_str().unwrap()).unwrap().mesh_settings,
            dynamic.mesh_settings
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod adaptive_storage_tests {
    use super::*;
    #[test]
    fn snake_presets_and_adaptive_project_settings_survive_disk() {
        let dir =
            std::env::temp_dir().join(format!("clay-adaptive-storage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut document = crate::document::Document::default();
        document
            .set_adaptive(crate::sculpt::AdaptiveSettings {
                enabled: true,
                detail: 4.,
                relative: true,
                collapse_ratio: 0.3,
                ..Default::default()
            })
            .unwrap();
        let project = document.project();
        let path = dir.join("adaptive.clay");
        save_project(path.to_str().unwrap(), &project).unwrap();
        let loaded = load_project(path.to_str().unwrap()).unwrap();
        assert_eq!(
            loaded.mesh_settings.adaptive,
            project.mesh_settings.adaptive
        );
        let mut snake = Brush::new(crate::sculpt::Kind::Snake);
        snake.name = "My snake".into();
        snake.subtract = true;
        snake.pressure_curve = 2.;
        snake.alpha = Alpha::Image {
            width: 2,
            height: 1,
            pixels: vec![0.2, 1.].into(),
        };
        let path = dir.join("snake.brush.json");
        save_brush(path.to_str().unwrap(), &snake).unwrap();
        assert!(load_brush(path.to_str().unwrap()).unwrap() == snake);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
