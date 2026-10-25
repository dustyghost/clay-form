//! Portable PSD exchange with a GIMP 3 compositing adapter for reload.
mod psd;
use crate::sculpt::{
    texture::{Exchange, Precision, Texture},
    uv::UvMap,
};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn directory() -> Result<PathBuf, String> {
    let path = std::env::temp_dir()
        .join("clay-form-texture-exchange")
        .join(format!("exchange-{}", Texture::new_id()));
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

fn run(
    executable: &str,
    config: serde_json::Value,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    run_script(executable, config, cancel, include_str!("gimp/exchange.py"))
}
fn run_script(
    executable: &str,
    mut config: serde_json::Value,
    cancel: &dyn Fn() -> bool,
    script: &str,
) -> Result<(), String> {
    let directory = directory()?;
    let result_path = directory.join("result.json");
    config["result"] = result_path.to_string_lossy().as_ref().into();
    // Hex-encode configuration so paths are data, never Python or shell source.
    let encoded: String = serde_json::to_vec(&config)
        .map_err(|e| e.to_string())?
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let batch = format!(
        "import json\nconfig=json.loads(bytes.fromhex('{encoded}'))\n{}",
        script
    );
    let mut child = Command::new(executable)
        .args([
            "--new-instance",
            "--no-interface",
            "--no-data",
            "--no-fonts",
            "--no-splash",
            "--no-shm",
            "--console-messages",
            "--stack-trace-mode=never",
            "--batch-interpreter=python-fu-eval",
        ])
        .arg(format!("--batch={batch}"))
        .arg("--quit")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| {
            format!("Could not run GIMP: {e}. Set a GIMP 3 executable in the texture controls")
        })?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if cancel() || started.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("GIMP exchange cancelled or timed out; current texture kept".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let result: serde_json::Value = std::fs::read(&result_path).ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or("GIMP exchange failed. Use GIMP 3 with its Python batch interpreter installed; current texture kept")?;
    let _ = std::fs::remove_file(result_path);
    let _ = std::fs::remove_dir(directory);
    if !status.success() || result["ok"] != true {
        return Err(result["error"]
            .as_str()
            .unwrap_or("GIMP exchange failed; current texture kept")
            .into());
    }
    Ok(())
}

pub(crate) fn prepare(
    mut texture: Texture,
    uv: &UvMap,
    cancel: &dyn Fn() -> bool,
) -> Result<Texture, String> {
    texture.validate()?;
    let directory = directory()?;
    let guide = directory.join("uv-guide.png");
    let xcf = directory.join("texture-with-uv.psd");
    let guide_pixels = guide_image(uv, texture.width, texture.height);
    guide_pixels.save(&guide).map_err(|e| e.to_string())?;
    super::write_bytes(&xcf, &psd::encode(&texture, &guide_pixels, cancel)?)?;
    texture.exchange = Some(Exchange {
        path: xcf.to_string_lossy().into(),
        baseline: texture.fingerprint(),
        layered_path: Some(xcf.to_string_lossy().into()),
    });
    Ok(texture)
}

pub(crate) fn reload(
    texture: &Texture,
    executable: &str,
    cancel: &dyn Fn() -> bool,
) -> Result<Texture, String> {
    let exchange = texture
        .exchange
        .as_ref()
        .ok_or("Send this texture to the external paint app first")?;
    let xcf = exchange
        .layered_path
        .as_ref()
        .ok_or("No layered working document")?;
    let metadata = std::fs::metadata(xcf)
        .map_err(|e| format!("Cannot open external paint working copy: {e}"))?;
    if !metadata.is_file() || metadata.len() > 2 * 1024 * 1024 * 1024 {
        return Err("Choose a regular layered working document smaller than 2 GiB".into());
    }
    let directory = directory()?;
    let float = texture.pixels.precision() == Precision::Float32;
    let output = directory.join(if float { "paint.exr" } else { "paint.png" });
    let result = (|| {
        run(
            executable,
            serde_json::json!({"operation":"reload", "xcf":xcf, "output":output, "float":float}),
            cancel,
        )?;
        let mut next = super::texture::load(&output.to_string_lossy(), texture.channel)?;
        next.id = texture.id;
        next.name = texture.name.clone();
        next.exchange = Some(Exchange {
            baseline: next.fingerprint(),
            ..exchange.clone()
        });
        Ok(next)
    })();
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir(directory);
    result
}

pub(crate) fn open(executable: &str, path: &Path) -> Result<(), String> {
    let mut child = Command::new(executable)
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not open external paint app: {e}"))?;
    let _ = std::thread::Builder::new()
        .name("clay-gimp-window".into())
        .spawn(move || {
            let _ = child.wait();
        });
    Ok(())
}

/// Transparent cyan wireframe with a dark rim, aligned to texture pixel rows.
fn guide_image(uv: &UvMap, width: u32, height: u32) -> image::RgbaImage {
    let mut coverage = vec![0u8; (width * height) as usize];
    let mut edges = std::collections::BTreeSet::new();
    for (_, triangle) in uv
        .corners
        .iter()
        .enumerate()
        .filter(|(face, _)| uv.mapped(*face))
    {
        for edge in 0..3 {
            let a = triangle[edge];
            let b = triangle[(edge + 1) % 3];
            if a.into_iter().chain(b).any(|v| !v.is_finite()) {
                continue;
            }
            let mut key = [a.map(f32::to_bits), b.map(f32::to_bits)];
            key.sort();
            if !edges.insert(key) {
                continue;
            }
            // Clip before sampling; tiled UVs must not cause unbounded work.
            let d = [b[0] - a[0], b[1] - a[1]];
            let (mut lo, mut hi) = (0f32, 1f32);
            for axis in 0..2 {
                if d[axis].abs() < f32::EPSILON {
                    if !(0. ..=1.).contains(&a[axis]) {
                        hi = -1.;
                    }
                } else {
                    let t0 = -a[axis] / d[axis];
                    let t1 = (1. - a[axis]) / d[axis];
                    lo = lo.max(t0.min(t1));
                    hi = hi.min(t0.max(t1));
                }
            }
            if lo > hi {
                continue;
            }
            let pixel = |t: f32| {
                [
                    (a[0] + d[0] * t) * (width - 1) as f32,
                    (1. - a[1] - d[1] * t) * (height - 1) as f32,
                ]
            };
            let start = pixel(lo);
            let end = pixel(hi);
            let steps = ((end[0] - start[0]).abs().max((end[1] - start[1]).abs()) * 2.)
                .ceil()
                .max(1.) as u32;
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let x = start[0] + (end[0] - start[0]) * t;
                let y = start[1] + (end[1] - start[1]) * t;
                for iy in (y.floor() as i32 - 2)..=(y.ceil() as i32 + 2) {
                    for ix in (x.floor() as i32 - 2)..=(x.ceil() as i32 + 2) {
                        if ix < 0 || iy < 0 || ix >= width as i32 || iy >= height as i32 {
                            continue;
                        }
                        let distance = ((ix as f32 - x).powi(2) + (iy as f32 - y).powi(2)).sqrt();
                        let value = ((2. - distance).clamp(0., 1.) * 255.).round() as u8;
                        let index = iy as usize * width as usize + ix as usize;
                        coverage[index] = coverage[index].max(value);
                    }
                }
            }
        }
    }
    image::RgbaImage::from_fn(width, height, |x, y| {
        let alpha = coverage[(y * width + x) as usize];
        image::Rgba(if alpha > 200 {
            [40, 220, 255, 230]
        } else {
            [8, 20, 28, alpha]
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sculpt::texture::{Channel, Pixels};
    use std::sync::Arc;

    #[test]
    fn uv_guide_uses_texture_orientation_and_omits_removed_faces() {
        let uv = UvMap {
            corners: vec![
                [[0.1, 0.9], [0.4, 0.9], [0.1, 0.6]],
                [[0.6, 0.4], [0.9, 0.4], [0.9, 0.1]],
                [[-100., 0.5], [100., 0.5], [100., 0.5]],
            ],
            removed: vec![false, true, false],
            ..Default::default()
        };
        let guide = guide_image(&uv, 512, 256);
        assert!(
            guide.get_pixel(100, 26)[3] > 0,
            "upper UV edge is near top of image"
        );
        assert_eq!(
            guide.get_pixel(400, 153)[3],
            0,
            "removed triangle has no guide"
        );
        assert_eq!(
            guide.get_pixel(100, 230)[3],
            0,
            "no vertically flipped guide"
        );
        assert!(
            guide.get_pixel(256, 128)[3] > 0,
            "outside edge clipped to image"
        );
        assert_eq!(
            guide.get_pixel(80, 55)[3],
            0,
            "island interiors stay transparent"
        );
        let old: Exchange =
            serde_json::from_str(r#"{"path":"texture.png","baseline":42}"#).unwrap();
        assert!(old.layered_path.is_none());
    }

    #[test]
    #[ignore = "requires installed GIMP 3 with Python; writes temporary layered documents"]
    fn layered_gimp_roundtrip_excludes_visible_guide_and_preserves_precision() {
        let uv = UvMap {
            corners: vec![[[0.1, 0.1], [0.85, 0.2], [0.25, 0.9]]],
            ..Default::default()
        };
        for precision in [Precision::U16, Precision::Float32] {
            let mut texture =
                Texture::new_with_precision(Channel::Color, 128, &Default::default(), precision)
                    .unwrap();
            texture.pixels = match precision {
                Precision::U16 => {
                    Pixels::U16(Arc::new([12345, 23456, 34567, 45678].repeat(128 * 128)))
                }
                Precision::Float32 => {
                    Pixels::Float32(Arc::new([-0.25, 4.5, 0.12345678, 0.5].repeat(128 * 128)))
                }
            };
            let sent = prepare(texture.clone(), &uv, &|| false).unwrap();
            let xcf = sent
                .exchange
                .as_ref()
                .unwrap()
                .layered_path
                .as_ref()
                .unwrap();
            let before = std::fs::read(xcf).unwrap();
            let loaded = reload(&sent, "gimp", &|| false).unwrap();
            assert_eq!(loaded.id, texture.id);
            assert_eq!(loaded.pixels.precision(), precision);
            let a = loaded.pixels.pixel(0);
            let b = texture.pixels.pixel(0);
            for (actual, expected) in a.into_iter().zip(b) {
                assert!(
                    (actual - expected).abs() < 0.0001,
                    "{precision:?}: {actual} != {expected}"
                );
            }
            for i in 0..128 * 128 {
                assert_eq!(
                    loaded.pixels.pixel(i),
                    loaded.pixels.pixel(0),
                    "visible guide leaked into texture at {i}"
                );
            }
            assert_eq!(
                std::fs::read(xcf).unwrap(),
                before,
                "reload must not change the layered source"
            );
            println!("Layered {precision:?} fixture: {xcf}");
            let edit = |operation: &str| {
                run_script(
                    "gimp",
                    serde_json::json!({"xcf":xcf,"edit":operation}),
                    &|| false,
                    include_str!("../../scripts/validate-psd-exchange.py"),
                )
                .unwrap();
            };
            if precision == Precision::U16 {
                edit("paint");
                let edited = reload(&sent, "gimp", &|| false).unwrap();
                for i in 0..128 * 128 {
                    let p = edited.linear_pixel(i);
                    assert!(
                        (p[0] - 0.8).abs() < 0.001
                            && (p[1] - 0.1).abs() < 0.001
                            && (p[2] - 0.2).abs() < 0.001,
                        "guide must be excluded after actual PSD save, including inside a group: {p:?}"
                    );
                }
                std::fs::write(xcf, &before).unwrap();
                edit("remove-guide");
                assert!(
                    reload(&sent, "gimp", &|| false)
                        .unwrap_err()
                        .contains("guide is missing")
                );
            } else {
                edit("downgrade");
                assert!(
                    reload(&sent, "gimp", &|| false)
                        .unwrap_err()
                        .contains("lost float precision")
                );
            }
            std::fs::write(xcf, &before).unwrap();
        }
    }
}
