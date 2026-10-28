//! Filesystem adapter: versioned projects, presets, image alphas, and OBJ import/export.
pub(crate) mod gimp;
pub mod material_export;
mod obj;
pub mod texture;
pub use obj::load_obj;
pub(crate) use obj::load_obj_cancellable;

use crate::sculpt::{Alpha, Brush, Mesh, Project, validate_brush};
pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > 2 * 1024 * 1024 * 1024 {
        return Err("File exceeds 2 GiB limit".into());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub(crate) fn json_bytes<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    if bytes.len() > 2 * 1024 * 1024 * 1024 {
        return Err(
            "File exceeds 2 GiB limit; reduce mesh, texture or group data before saving".into(),
        );
    }
    Ok(bytes)
}
pub(crate) fn write_json<T: serde::Serialize>(path: &str, value: &T) -> Result<(), String> {
    write_bytes(std::path::Path::new(path), &json_bytes(value)?)
}
/// Flush a uniquely named sibling before atomic replacement. Failed writes leave
/// the previous destination intact; abandoned temporary files are never recovery candidates.
pub(crate) fn write_bytes(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::{
        io::Write,
        sync::atomic::{AtomicU64, Ordering},
    };
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let mut temporary = path.as_os_str().to_os_string();
    temporary.push(format!(
        ".tmp-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = std::path::PathBuf::from(temporary);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        #[cfg(unix)]
        std::fs::File::open(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new(".")),
        )?
        .sync_all()?;
        Ok::<(), std::io::Error>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(|e| e.to_string())
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
    let mut brush = brush.clone();
    brush.ensure_identity();
    write_json(path, &brush)
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct BrushLibrary {
    pub version: u32,
    pub brushes: Vec<Brush>,
}
impl BrushLibrary {
    fn validate(&self) -> Result<(), String> {
        if self.version != 1 || self.brushes.len() > 256 {
            return Err("Invalid brush library version or size".into());
        }
        for brush in &self.brushes {
            validate_brush(brush)?;
        }
        Ok(())
    }
}
pub fn save_brush_library(path: &str, brushes: &[Brush]) -> Result<(), String> {
    let mut brushes = brushes.to_vec();
    for b in &mut brushes {
        b.ensure_identity();
    }
    let library = BrushLibrary {
        version: 1,
        brushes,
    };
    library.validate()?;
    write_json(path, &library)
}
pub fn load_brush_library(path: &str) -> Result<Vec<Brush>, String> {
    let library: BrushLibrary = read_json(path)?;
    library.validate()?;
    Ok(library.brushes)
}
pub fn load_alpha(path: &str) -> Result<Alpha, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Alpha image exceeds 32 MB limit".into());
    }
    let reader = image::ImageReader::open(path).map_err(|e| e.to_string())?;
    let mut reader = reader.with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    reader.limits(limits);
    let image = reader.decode().map_err(|e| e.to_string())?.to_rgba16();
    Ok::<_, String>(Alpha::Image {
        width: image.width() as usize,
        height: image.height() as usize,
        pixels: image
            .pixels()
            .map(|p| {
                ((p[0] as f32 * 0.2126 + p[1] as f32 * 0.7152 + p[2] as f32 * 0.0722) / 65535.)
                    * (p[3] as f32 / 65535.)
            })
            .collect::<Vec<_>>()
            .into(),
    })
}
/// PNG height export at 16-bit precision. Source images and brushes are untouched.
pub fn save_alpha(path: &str, alpha: &Alpha) -> Result<(), String> {
    use image::ImageEncoder;
    if !std::path::Path::new(path)
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("png"))
    {
        return Err("Choose a .png file for alpha export".into());
    }
    alpha.validate()?;
    let (width, height) = match alpha {
        Alpha::Image { width, height, .. } => (*width, *height),
        _ => (256, 256),
    };
    let mut pixels = Vec::with_capacity(width * height * 2);
    for y in 0..height {
        for x in 0..width {
            let v = alpha.sample(
                x as f32 / (width - 1).max(1) as f32,
                y as f32 / (height - 1).max(1) as f32,
            );
            pixels.extend_from_slice(&((v * 65535.).round() as u16).to_ne_bytes());
        }
    }
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            &pixels,
            width as u32,
            height as u32,
            image::ExtendedColorType::L16,
        )
        .map_err(|e| e.to_string())?;
    write_bytes(std::path::Path::new(path), &bytes)
}

pub fn export_obj(path: &str, mesh: &Mesh) -> Result<(), String> {
    mesh.validate()?;
    if !mesh.materials.is_empty() {
        return Err("OBJ cannot preserve painted material properties. Save as .clay to keep colour, roughness and metallic. Material exchange is not yet supported.".into());
    }
    if mesh.uv.current(mesh) {
        return obj::export_uv_obj(path, mesh);
    }
    if !mesh.uv.corners.is_empty() {
        return Err(
            "UVs are out of date after topology editing. Unwrap again before OBJ export".into(),
        );
    }
    let mut text = String::from("# Clay Form mesh\n");
    for v in &mesh.vertices {
        text.push_str(&format!("v {} {} {}\n", v.0, v.1, v.2));
    }
    let key = |mut t: [usize; 3]| {
        t.sort_unstable();
        t
    };
    let face_groups: std::collections::HashMap<_, _> = mesh
        .triangles
        .iter()
        .enumerate()
        .map(|(i, &t)| (key(t), mesh.polygroups.get(i).copied().unwrap_or(0)))
        .collect();
    let mut previous_group = usize::MAX;
    let mut emit_group = |text: &mut String, id: usize| {
        if !mesh.polygroups.is_empty() && previous_group != id {
            let name = if id == 0 {
                "ungrouped".to_string()
            } else {
                format!(
                    "group_{}_{}",
                    id,
                    mesh.vertex_groups[id - 1]
                        .name
                        .chars()
                        .map(|c| if c.is_whitespace() || c.is_control() || c == '#' {
                            '_'
                        } else {
                            c
                        })
                        .collect::<String>()
                )
            };
            text.push_str(&format!("g {name}\n"));
            previous_group = id;
        }
    };
    let mut quad_triangles = std::collections::HashSet::new();
    for &[a, b, c, d] in &mesh.quads {
        let id = [[a, b, c], [a, c, d], [a, b, d], [b, c, d]]
            .into_iter()
            .find_map(|t| face_groups.get(&key(t)).copied())
            .unwrap_or(0);
        emit_group(&mut text, id);
        text.push_str(&format!("f {} {} {} {}\n", a + 1, b + 1, c + 1, d + 1));
        for t in [[a, b, c], [a, c, d], [a, b, d], [b, c, d]] {
            quad_triangles.insert(key(t));
        }
    }
    for &[a, b, c] in &mesh.triangles {
        if quad_triangles.contains(&key([a, b, c])) {
            continue;
        }
        emit_group(&mut text, face_groups[&key([a, b, c])]);
        text.push_str(&format!("f {} {} {}\n", a + 1, b + 1, c + 1));
    }
    std::fs::write(path, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removed_head_recipes_keep_saved_surfaces_in_active_and_other_submodels() {
        use crate::sculpt::{MeshSettings, ModelMetadata, Primitive, PrimitiveKind, Submodel};
        let mut expected = crate::document::Document::default().project();
        // Recipe-only legacy settings predate the quad remesh option. The
        // active model inherits triangles; the submodel's explicit choice stays.
        expected.mesh_settings.remesh_topology = crate::sculpt::Topology::Triangles;
        expected.model_name = "Saved head".into();
        expected.mesh = Primitive::for_kind(PrimitiveKind::Cube).mesh().unwrap();
        expected.mesh.mask[0] = 0.75;
        expected.submodels.push(Submodel {
            name: "Hidden head".into(),
            mesh: expected.mesh.clone(),
            mesh_settings: MeshSettings::default(),
            metadata: ModelMetadata {
                visible: false,
                ..Default::default()
            },
        });
        // The stored mesh is authoritative, independent of the old recipe's
        // anatomy version, dimensions, or generator. No regeneration is needed.
        for anatomy_version in [None, Some(0), Some(1), Some(2)] {
            let mut recipe = serde_json::json!({"kind":"HumanHead", "head":{}});
            if let Some(version) = anatomy_version {
                recipe["head"]["anatomy_version"] = version.into();
            }
            let mut saved = serde_json::to_value(&expected).unwrap();
            saved["mesh_settings"] = serde_json::json!({"primitive": recipe.clone()});
            saved["submodels"][0]["mesh_settings"]["primitive"] = recipe;
            let loaded: Project = serde_json::from_value(saved).unwrap();
            loaded.validate().unwrap();
            assert!(loaded == expected);
            let mut document = crate::document::Document::default();
            document
                .replace_project(loaded, "legacy-head.clay")
                .unwrap();
            assert!(document.primitive().is_none());
            assert_eq!(document.mesh(), &expected.mesh);
            let roundtrip: Project =
                serde_json::from_slice(&json_bytes(&document.project()).unwrap()).unwrap();
            assert_eq!(roundtrip.mesh, expected.mesh);
            assert_eq!(roundtrip.submodels, expected.submodels);
        }
        assert!(
            serde_json::from_value::<MeshSettings>(
                serde_json::json!({"primitive":{"kind":"UnknownShape"}})
            )
            .is_err()
        );
        assert!(
            PrimitiveKind::ALL
                .iter()
                .all(|kind| kind.label() != "Human head")
        );
    }

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
        brush.stabilize = true;
        brush.stabilizer_distance = 45.;
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
            paths: Vec::new(),
            alphas: Vec::new(),
            stencil: None,
            references: Vec::new(),
            scene: Default::default(),
            model_name: "Model".into(),
            submodels: Vec::new(),
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
    fn obj_exports_quad_polygroups_with_safe_distinct_names() {
        let mut mesh = crate::sculpt::Primitive::for_kind(crate::sculpt::PrimitiveKind::Plane)
            .mesh()
            .unwrap();
        mesh.create_vertex_group("A B\n#C".into(), 0.5, true)
            .unwrap();
        let path = std::env::temp_dir().join(format!("clay-groups-{}.obj", std::process::id()));
        export_obj(path.to_str().unwrap(), &mesh).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("g group_1_A_B__C\n"));
        assert_eq!(text.lines().filter(|l| l.starts_with("f ")).count(), 1);
        assert_eq!(
            text.lines()
                .find(|l| l.starts_with("f "))
                .unwrap()
                .split_whitespace()
                .count(),
            5
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn mesh_levels_and_dynamic_settings_survive_files_and_obj_exports_active_level() {
        let dir = std::env::temp_dir().join(format!("clay-mesh-storage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut geometry = crate::sculpt::Geometry::new(Mesh::sphere());
        geometry.subdivide().unwrap();
        geometry.set_level(1).unwrap();
        geometry.settings.resolution = 32;
        let project = Project {
            paths: Vec::new(),
            alphas: Vec::new(),
            stencil: None,
            references: Vec::new(),
            scene: Default::default(),
            model_name: "Model".into(),
            submodels: Vec::new(),
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
            loaded.mesh.triangles.len() - loaded.mesh.quads.len()
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

#[cfg(test)]
mod brush_library_tests {
    use super::*;
    #[test]
    fn brush_library_and_icon_roundtrip_with_compact_png() {
        let dir = std::env::temp_dir().join(format!("clay-brush-library-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut brush = Brush::new(crate::sculpt::Kind::Standard);
        brush.stabilize = true;
        brush.stabilizer_distance = 75.;
        brush.icon = Some(
            crate::brush_icon::capture(
                crate::document::Document::default().mesh(),
                &crate::camera::Camera::default(),
            )
            .unwrap(),
        );
        let encoded = serde_json::to_vec(&brush).unwrap();
        assert!(encoded.len() < 100_000);
        assert!(serde_json::from_slice::<Brush>(&encoded).unwrap() == brush);
        let preset = dir.join("icon.brush.json");
        save_brush(preset.to_str().unwrap(), &brush).unwrap();
        assert!(load_brush(preset.to_str().unwrap()).unwrap() == brush);
        let library = dir.join("library.brushes.json");
        save_brush_library(library.to_str().unwrap(), &[brush.clone()]).unwrap();
        assert!(load_brush_library(library.to_str().unwrap()).unwrap() == vec![brush.clone()]);
        let mut value = serde_json::to_value(&brush).unwrap();
        value["icon"]["size"] = serde_json::json!(64);
        assert!(serde_json::from_value::<Brush>(value).is_err());
        let mut value = serde_json::to_value(&brush).unwrap();
        value["icon"]["png"] = serde_json::json!([1, 2, 3]);
        assert!(serde_json::from_value::<Brush>(value).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
