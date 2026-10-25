//! Static model interchange. The native project remains the editing master.
use super::{ExportFormat, ExportLighting, Temporary, export_bytes, run_blender};
use crate::sculpt::{
    Material, Project,
    texture::{Channel, Precision, Texture},
};
use std::{fmt::Write as _, path::Path};

pub struct ExportBundle {
    bytes: Vec<u8>,
    assets: Option<(Temporary, String)>,
}
impl ExportBundle {
    /// Publish a fresh companion directory first, then atomically replace the model.
    /// Never overwrite an old export's textures, or files belonging to the user.
    pub fn write(self, path: &Path) -> Result<(), String> {
        if let Some((temporary, name)) = &self.assets {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let destination = parent.join(name);
            std::fs::create_dir(&destination).map_err(|e| e.to_string())?;
            let copy = (|| {
                for entry in std::fs::read_dir(temporary.0.join(name))? {
                    let entry = entry?;
                    let target = destination.join(entry.file_name());
                    std::fs::copy(entry.path(), &target)?;
                    std::fs::OpenOptions::new()
                        .write(true)
                        .open(target)?
                        .sync_all()?;
                }
                #[cfg(unix)]
                std::fs::File::open(&destination)?.sync_all()?;
                Ok::<_, std::io::Error>(())
            })();
            if let Err(error) = copy {
                let _ = std::fs::remove_dir_all(destination);
                return Err(error.to_string());
            }
            // On a model-write failure keep these assets: rename may have succeeded
            // before a directory sync failed. Removing them could break the model.
        }
        crate::persistence::write_bytes(path, &self.bytes)
    }
}

#[derive(serde::Serialize)]
struct Map {
    channel: Channel,
    path: String,
    linear: bool,
}

pub fn prepare_export(
    project: &Project,
    format: ExportFormat,
    lighting: Option<&ExportLighting>,
    cancel: &dyn Fn() -> bool,
) -> Result<ExportBundle, String> {
    if !matches!(format, ExportFormat::OpenUsd | ExportFormat::Fbx) {
        return Ok(ExportBundle {
            bytes: export_bytes(project, format, lighting, cancel)?,
            assets: None,
        });
    }
    project.validate()?;
    check(cancel)?;
    let mesh = &project.mesh;
    let uv = &mesh.uv;
    if !uv.corners.is_empty() && !uv.current(mesh) {
        return Err("UV topology is stale; unwrap the model before exporting".into());
    }
    if uv
        .corners
        .iter()
        .enumerate()
        .filter(|(f, _)| uv.mapped(*f))
        .any(|(_, t)| t.iter().flatten().any(|v| !(0. ..=1.).contains(v)))
    {
        return Err(
            "Arrange mapped UV islands inside the 0–1 atlas before exporting PBR textures".into(),
        );
    }
    let temporary = Temporary::new()?;
    let name = format!("clay-assets-{}", Texture::new_id());
    let directory = temporary.0.join(&name);
    std::fs::create_dir(&directory).map_err(|e| e.to_string())?;
    let mut maps = Vec::new();
    if uv.current(mesh) && (0..mesh.triangles.len()).any(|f| uv.mapped(f)) {
        for channel in Channel::ALL {
            check(cancel)?;
            let source = uv.textures.iter().find(|t| t.channel == channel);
            let texture = preview_texture(project, channel, source, cancel)?;
            let extension = if texture.pixels.precision() == Precision::Float32 {
                "exr"
            } else {
                "png"
            };
            let filename = format!("{}.{}", channel_name(channel), extension);
            crate::persistence::texture::save(&directory.join(&filename), &texture)?;
            maps.push(Map {
                channel,
                path: format!("{name}/{filename}"),
                linear: channel != Channel::Color || extension == "exr",
            });
        }
    }
    // Original paint assets retain their coverage, precision, profile and dimensions.
    // Preview maps flatten coverage onto the base surface; alpha is not transparency.
    for texture in &uv.textures {
        check(cancel)?;
        let extension = if texture.pixels.precision() == Precision::Float32 {
            "exr"
        } else {
            "png"
        };
        crate::persistence::texture::save(
            &directory.join(format!(
                "source_{}.{}",
                channel_name(texture.channel),
                extension
            )),
            texture,
        )?;
    }
    let materials: Vec<_> = (0..mesh.vertices.len()).map(|i| mesh.material(i)).collect();
    let shading = mesh.surface_shading();
    let normals: Vec<_> = shading
        .triangles
        .iter()
        .map(|t| {
            t.map(|i| {
                let p = shading.vertices[i].1;
                [p.0, p.1, p.2]
            })
        })
        .collect();
    let mut uv_metadata = uv.clone();
    uv_metadata.textures.clear();
    let metadata = serde_json::json!({
        "version": 1, "name": project.model_name,
        "uv": uv_metadata, "materials": materials, "material_assets": project.scene.materials,
        "textures": uv.textures.iter().map(|t| serde_json::json!({
            "id": t.id, "name": t.name, "channel": t.channel,
            "width": t.width, "height": t.height,
            "source": format!("source_{}.{}", channel_name(t.channel), if t.pixels.precision() == Precision::Float32 { "exr" } else { "png" })
        })).collect::<Vec<_>>(),
        "notes": "Clay Form editing metadata. Base PBR preview only; layered BSDFs are not translated. Positions are Y-up, one unit exported as one metre. Save .clay for complete editing state."
    });
    std::fs::write(
        directory.join("clay_metadata.json"),
        serde_json::to_vec(&metadata).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let bytes = if format == ExportFormat::OpenUsd {
        usd(project, &materials, &normals, &maps, &name, cancel)?.into_bytes()
    } else {
        let input = temporary.0.join("exchange.json");
        let output = temporary.0.join("model.fbx");
        let script = temporary.0.join("export.py");
        let data = serde_json::json!({
            "name": project.model_name,
            "vertices": mesh.vertices.iter().map(|p| [p.0,p.1,p.2]).collect::<Vec<_>>(),
            "faces": mesh.triangles, "normals": normals, "uv": uv_metadata,
            "materials": materials, "maps": maps, "metadata": format!("{name}/clay_metadata.json")
        });
        std::fs::write(
            &input,
            serde_json::to_vec(&data).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(&script, include_str!("../export_fbx.py")).map_err(|e| e.to_string())?;
        run_blender(&input, &output, &script, cancel)?;
        std::fs::read(output).map_err(|e| format!("Blender did not produce FBX: {e}"))?
    };
    check(cancel)?;
    Ok(ExportBundle {
        bytes,
        assets: Some((temporary, name)),
    })
}
fn check(cancel: &dyn Fn() -> bool) -> Result<(), String> {
    if cancel() {
        Err("Export cancelled".into())
    } else {
        Ok(())
    }
}
fn channel_name(channel: Channel) -> &'static str {
    match channel {
        Channel::Color => "color",
        Channel::Roughness => "roughness",
        Channel::Metallic => "metallic",
    }
}
fn value(material: &Material, channel: Channel) -> [f32; 3] {
    match channel {
        Channel::Color => material.base_color,
        Channel::Roughness => [material.roughness; 3],
        Channel::Metallic => [material.metallic; 3],
    }
}
fn preview_texture(
    project: &Project,
    channel: Channel,
    source: Option<&Texture>,
    cancel: &dyn Fn() -> bool,
) -> Result<Texture, String> {
    let mesh = &project.mesh;
    let mut texture = if let Some(t) = source {
        let mut t = t.clone();
        t.profile.clear(); // Working U16 colour samples are already sRGB.
        t
    } else {
        Texture::new(channel, 1024, &mesh.material(0))?
    };
    // Preserve opaque HDR values even outside islands; avoid treating coverage as opacity.
    for i in 0..texture.pixels.len() / 4 {
        if i % 65536 == 0 {
            check(cancel)?;
        }
        let mut p = texture.pixels.pixel(i);
        p[3] = 1.;
        texture.pixels.set_pixel(i, p);
    }
    let width = texture.width as usize;
    let height = texture.height as usize;
    let mut occupied = vec![false; width * height];
    for (f, face) in mesh.triangles.iter().enumerate() {
        if f % 128 == 0 {
            check(cancel)?;
        }
        if !mesh.uv.mapped(f) {
            continue;
        }
        let values = face.map(|i| value(&mesh.material(i), channel));
        crate::sculpt::texture::raster(
            mesh.uv.corners[f].map(|p| [p[0], 1. - p[1]]),
            width,
            height,
            |x, y, w| {
                let i = y * width + x;
                occupied[i] = true;
                let painted = source.map(|t| t.linear_pixel(i));
                let mut pixel = [0., 0., 0., 1.];
                for c in 0..3 {
                    pixel[c] = (0..3).map(|k| values[k][c] * w[k]).sum();
                    if let Some(p) = painted {
                        pixel[c] = pixel[c] * (1. - p[3]) + p[c] * p[3];
                    }
                    if channel == Channel::Color && texture.pixels.precision() == Precision::U16 {
                        pixel[c] = crate::sculpt::material::linear_to_srgb(pixel[c]);
                    }
                }
                texture.pixels.set_pixel(i, pixel);
            },
        );
    }
    crate::sculpt::texture::pad_pixels(&mut texture, occupied);
    Ok(texture)
}

// USD identifiers and asset paths below are generated, never taken from user names.
// JSON quoting is valid for USD string values and escapes quotes/newlines/backslashes.
fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
fn array<T>(items: impl IntoIterator<Item = T>, f: impl Fn(T) -> String) -> String {
    format!(
        "[{}]",
        items.into_iter().map(f).collect::<Vec<_>>().join(", ")
    )
}
fn tuple<const N: usize>(p: [f32; N]) -> String {
    format!("({})", p.map(|v| v.to_string()).join(", "))
}
fn usd(
    project: &Project,
    materials: &[Material],
    normals: &[[[f32; 3]; 3]],
    maps: &[Map],
    folder: &str,
    cancel: &dyn Fn() -> bool,
) -> Result<String, String> {
    let mesh = &project.mesh;
    let uv = &mesh.uv;
    let mut out = String::from(
        "#usda 1.0\n(\n    defaultPrim = \"Model\"\n    upAxis = \"Y\"\n    metersPerUnit = 1\n)\ndef Xform \"Model\" {\n",
    );
    writeln!(
        out,
        "    custom string clay:metadata = {}",
        quoted(&format!("{folder}/clay_metadata.json"))
    )
    .unwrap();
    writeln!(out, "    def Mesh \"Mesh\" (prepend apiSchemas = [\"MaterialBindingAPI\"]) {{\n        custom string clay:name = {}", quoted(&project.model_name)).unwrap();
    writeln!(out, "        uniform token subdivisionScheme = \"none\"\n        uniform token orientation = \"rightHanded\"").unwrap();
    writeln!(
        out,
        "        point3f[] points = {}",
        array(&mesh.vertices, |p| tuple([p.0, p.1, p.2]))
    )
    .unwrap();
    writeln!(
        out,
        "        int[] faceVertexCounts = {}",
        array(&mesh.triangles, |_| "3".into())
    )
    .unwrap();
    writeln!(
        out,
        "        int[] faceVertexIndices = {}",
        array(mesh.triangles.iter().flatten(), |i| i.to_string())
    )
    .unwrap();
    writeln!(
        out,
        "        normal3f[] normals = {} (interpolation = \"faceVarying\")",
        array(normals.iter().flatten(), |p| tuple(*p))
    )
    .unwrap();
    for (name, ty, values) in [
        (
            "displayColor",
            "color3f",
            array(materials, |m| tuple(m.base_color)),
        ),
        (
            "roughness",
            "float",
            array(materials, |m| m.roughness.to_string()),
        ),
        (
            "metallic",
            "float",
            array(materials, |m| m.metallic.to_string()),
        ),
    ] {
        writeln!(
            out,
            "        {ty}[] primvars:{name} = {values} (interpolation = \"vertex\")"
        )
        .unwrap();
    }
    if uv.current(mesh) {
        writeln!(
            out,
            "        texCoord2f[] primvars:st = {} (interpolation = \"faceVarying\")",
            array(
                uv.corners
                    .iter()
                    .enumerate()
                    .flat_map(|(f, t)| if uv.mapped(f) { *t } else { [[0.; 2]; 3] }),
                tuple
            )
        )
        .unwrap();
        writeln!(
            out,
            "        float[] primvars:clay:uvMask = {} (interpolation = \"faceVarying\")",
            array(
                (0..mesh.triangles.len()).flat_map(|f| (0..3).map(move |c| uv.mask(f, c))),
                |v| v.to_string()
            )
        )
        .unwrap();
        writeln!(
            out,
            "        bool[] primvars:clay:uvMapped = {} (interpolation = \"uniform\")",
            array(0..mesh.triangles.len(), |f| uv.mapped(f).to_string())
        )
        .unwrap();
        writeln!(
            out,
            "        custom int[] clay:seamVertexPairs = {}",
            array(uv.seams.iter().flatten(), |i| i.to_string())
        )
        .unwrap();
    }
    writeln!(
        out,
        "        rel material:binding = </Model/Materials/Base>"
    )
    .unwrap();
    if !maps.is_empty() {
        writeln!(out, "        uniform token subsetFamily:materialBind:familyType = \"nonOverlapping\"\n        def GeomSubset \"Painted\" (prepend apiSchemas = [\"MaterialBindingAPI\"]) {{\n            uniform token elementType = \"face\"\n            uniform token familyName = \"materialBind\"\n            int[] indices = {}\n            rel material:binding = </Model/Materials/Painted>\n        }}", array((0..mesh.triangles.len()).filter(|f| uv.mapped(*f)), |f| f.to_string())).unwrap();
    }
    out.push_str("    }\n    def Scope \"Materials\" {\n");
    for (name, textured) in [("Base", false), ("Painted", true)] {
        if textured && maps.is_empty() {
            continue;
        }
        check(cancel)?;
        let path = format!("/Model/Materials/{name}");
        writeln!(out, "        def Material \"{name}\" {{\n            token outputs:surface.connect = <{path}/Surface.outputs:surface>\n            def Shader \"Surface\" {{\n                uniform token info:id = \"UsdPreviewSurface\"\n                float inputs:opacity = 1\n                token outputs:surface").unwrap();
        for (channel, ty, input) in [
            (Channel::Color, "color3f", "diffuseColor"),
            (Channel::Roughness, "float", "roughness"),
            (Channel::Metallic, "float", "metallic"),
        ] {
            let output = if textured {
                if channel == Channel::Color {
                    "rgb"
                } else {
                    "r"
                }
            } else {
                "result"
            };
            writeln!(
                out,
                "                {ty} inputs:{input}.connect = <{path}/{}.outputs:{output}>",
                channel_name(channel)
            )
            .unwrap();
        }
        out.push_str("            }\n");
        if textured {
            writeln!(out, "            def Shader \"UV\" {{\n                uniform token info:id = \"UsdPrimvarReader_float2\"\n                string inputs:varname = \"st\"\n                float2 outputs:result\n            }}").unwrap();
            for map in maps {
                writeln!(out, "            def Shader \"{}\" {{\n                uniform token info:id = \"UsdUVTexture\"\n                asset inputs:file = @{}@\n                token inputs:sourceColorSpace = \"{}\"\n                token inputs:wrapS = \"clamp\"\n                token inputs:wrapT = \"clamp\"\n                float2 inputs:st.connect = <{path}/UV.outputs:result>\n                float3 outputs:rgb\n                float outputs:r\n            }}", channel_name(map.channel), map.path, if map.linear { "raw" } else { "sRGB" }).unwrap();
            }
        } else {
            for (channel, ty, primvar) in [
                (Channel::Color, "float3", "displayColor"),
                (Channel::Roughness, "float", "roughness"),
                (Channel::Metallic, "float", "metallic"),
            ] {
                writeln!(out, "            def Shader \"{}\" {{\n                uniform token info:id = \"UsdPrimvarReader_{ty}\"\n                string inputs:varname = \"{primvar}\"\n                {ty} outputs:result\n            }}", channel_name(channel)).unwrap();
            }
        }
        out.push_str("        }\n");
    }
    out.push_str("    }\n}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::Document,
        sculpt::{Geometry, Primitive, PrimitiveKind, texture::Pixels},
    };
    use std::sync::Arc;

    fn fixture() -> Project {
        let mut mesh = Primitive::for_kind(PrimitiveKind::Plane).mesh().unwrap();
        mesh.uv = mesh.unwrap_uv(true, 0.02, &|| false).unwrap();
        mesh.uv.masks = vec![[0.25, 0.5, 1.]; mesh.triangles.len()];
        mesh.uv.removed = vec![false; mesh.triangles.len()];
        mesh.uv.removed[0] = true;
        mesh.uv.seams = vec![crate::sculpt::uv::edge(
            mesh.triangles[0][0],
            mesh.triangles[0][1],
        )];
        for channel in Channel::ALL {
            let mut texture = Texture::new_with_precision(
                channel,
                16,
                &Material::default(),
                if channel == Channel::Color {
                    Precision::Float32
                } else {
                    Precision::U16
                },
            )
            .unwrap();
            if channel == Channel::Color {
                texture.pixels = Pixels::Float32(Arc::new([-0.5, 4., 8., 0.5].repeat(256)));
            }
            mesh.uv.textures.push(texture);
        }
        let mut document = Document::default();
        document.replace_geometry(Geometry::new(mesh)).unwrap();
        let mut project = document.project();
        project.model_name = "Artist's \"UV\" model".into();
        project
    }

    #[test]
    fn interchange_bundle_retains_uv_metadata_and_float_source_and_publishes_relative_assets() {
        let project = fixture();
        let original = project.clone();
        let bundle = prepare_export(&project, ExportFormat::OpenUsd, None, &|| false).unwrap();
        let text = std::str::from_utf8(&bundle.bytes).unwrap();
        assert!(text.contains("faceVarying") && text.contains("UsdPreviewSurface"));
        assert!(text.contains("clay:uvMask") && text.contains("clay:uvMapped"));
        let (temporary, name) = bundle.assets.as_ref().unwrap();
        let directory = temporary.0.join(name);
        let source = crate::persistence::texture::load(
            directory.join("source_color.exr").to_str().unwrap(),
            Channel::Color,
        )
        .unwrap();
        assert_eq!(source.pixels, project.mesh.uv.textures[0].pixels);
        let preview = crate::persistence::texture::load(
            directory.join("color.exr").to_str().unwrap(),
            Channel::Color,
        )
        .unwrap();
        assert_eq!(preview.pixels.precision(), Precision::Float32);
        let face = project
            .mesh
            .uv
            .corners
            .iter()
            .enumerate()
            .find(|(f, _)| project.mesh.uv.mapped(*f))
            .unwrap()
            .1;
        let center = [0, 1].map(|c| face.iter().map(|p| p[c]).sum::<f32>() / 3.);
        let pixel = (1. - center[1]).mul_add(16., 0.).floor() as usize * 16
            + (center[0] * 16.).floor() as usize;
        let expected = Material::default().base_color;
        let actual = preview.linear_pixel(pixel);
        for c in 0..3 {
            assert!((actual[c] - (expected[c] * 0.5 + [-0.5, 4., 8.][c] * 0.5)).abs() < 1e-5);
        }
        assert_eq!(actual[3], 1., "coverage must not make mesh transparent");
        let name = name.clone();
        let destination = Temporary::new().unwrap();
        let path = destination.0.join("model.usda");
        std::fs::write(&path, "previous").unwrap();
        bundle.write(&path).unwrap();
        assert!(destination.0.join(&name).join("color.exr").is_file());
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains(&format!("@{name}/color.exr@"))
        );
        let metadata: serde_json::Value = serde_json::from_slice(
            &std::fs::read(destination.0.join(&name).join("clay_metadata.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["uv"]["masks"][0][1], 0.5);
        assert_eq!(metadata["uv"]["removed"][0], true);
        assert!(project == original);
        assert!(prepare_export(&project, ExportFormat::OpenUsd, None, &|| true).is_err());
        let second = prepare_export(&project, ExportFormat::OpenUsd, None, &|| false).unwrap();
        let second_name = second.assets.as_ref().unwrap().1.clone();
        second.write(&path).unwrap();
        assert_ne!(name, second_name);
        assert!(
            destination.0.join(&name).join("color.exr").is_file(),
            "re-export never overwrites earlier companion files"
        );
    }

    #[test]
    fn interchange_refuses_stale_uvs_and_handles_unmapped_models() {
        let mut project = fixture();
        project.mesh.uv.topology[0].swap(0, 1);
        assert!(prepare_export(&project, ExportFormat::OpenUsd, None, &|| false).is_err());
        project.mesh.uv = Default::default();
        let bundle = prepare_export(&project, ExportFormat::OpenUsd, None, &|| false).unwrap();
        let text = std::str::from_utf8(&bundle.bytes).unwrap();
        assert!(!text.contains("primvars:st"));
        assert!(text.contains("UsdPrimvarReader_float3"));
    }

    #[test]
    #[ignore = "manual OpenUSD/FBX interoperability fixture; requires Blender"]
    fn interchange_manual_review() {
        let directory = std::path::PathBuf::from(
            std::env::var("CLAY_EXPORT_REVIEW")
                .unwrap_or_else(|_| "/tmp/clay-interchange-review".into()),
        );
        std::fs::create_dir_all(&directory).unwrap();
        let project = fixture();
        for format in [ExportFormat::OpenUsd, ExportFormat::Fbx] {
            let bundle = prepare_export(&project, format, None, &|| false).unwrap();
            if format == ExportFormat::Fbx {
                assert!(bundle.bytes.starts_with(b"Kaydara FBX Binary"));
            }
            bundle
                .write(&directory.join(format!("model.{}", format.extension())))
                .unwrap();
        }
        crate::persistence::save_project(directory.join("source.clay").to_str().unwrap(), &project)
            .unwrap();
    }
}
