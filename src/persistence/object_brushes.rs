//! One file per personal brush: listing names never deserializes source meshes.
use super::*;
use crate::sculpt::instances::{BrushBundle, BrushGroup};
use std::path::{Path, PathBuf};

const LIMIT: u64 = 128 * 1024 * 1024;
pub fn save(path: &str, brush: &BrushBundle) -> Result<(), String> {
    brush.validate()?;
    let bytes = json_bytes(brush)?;
    if bytes.len() as u64 > LIMIT {
        return Err("Object brush file exceeds 128 MiB".into());
    }
    write_bytes(Path::new(path), &bytes)
}
pub fn load(path: &str) -> Result<BrushBundle, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > LIMIT {
        return Err("Object brush file exceeds 128 MiB".into());
    }
    let brush: BrushBundle = read_json(path)?;
    brush.validate()?;
    Ok(brush)
}
pub fn personal_path(directory: &Path, name: &str) -> Result<PathBuf, String> {
    BrushGroup::validate_name(name)?;
    // Encoding the full UTF-8 name avoids path separators, platform-reserved
    // names and lossy sanitization collisions. Maximum filename: 213 bytes.
    let hex: String = name.bytes().map(|b| format!("{b:02x}")).collect();
    Ok(directory.join(format!("brush-{hex}.objects.json")))
}
pub fn save_personal(directory: &Path, brush: &BrushBundle) -> Result<(), String> {
    let path = personal_path(directory, &brush.name)?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    save(&path.to_string_lossy(), brush)
}
pub fn list_personal(directory: &Path) -> Result<Vec<String>, String> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let file = entry.file_name();
        let Some(hex) = file
            .to_str()
            .and_then(|s| s.strip_prefix("brush-"))
            .and_then(|s| s.strip_suffix(".objects.json"))
        else {
            continue;
        };
        if hex.is_empty()
            || hex.len() > 192
            || !hex.len().is_multiple_of(2)
            || !hex.bytes().all(|b| b.is_ascii_hexdigit())
        {
            continue;
        }
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        if let Ok(name) = String::from_utf8(bytes)
            && BrushGroup::validate_name(&name).is_ok()
        {
            names.push(name);
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}
