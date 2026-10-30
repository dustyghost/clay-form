use crate::sculpt::Material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct MaterialPreset {
    #[serde(default = "crate::sculpt::material::layers::fresh_id")]
    pub id: u64,
    pub name: String,
    pub material: Material,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original: Option<Material>,
}

/// Personal copies and colours persist independently of the open project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct MaterialLibrary {
    #[serde(default = "builtin_names", deserialize_with = "read_names")]
    pub names: Vec<String>,
    #[serde(default = "builtin_values", deserialize_with = "read_values")]
    pub values: Vec<Material>,
    pub copies: Vec<MaterialPreset>,
    #[serde(default)]
    pub hidden_builtins: Vec<u64>,
    pub palette: Vec<[f32; 3]>,
}
fn builtin_values() -> Vec<Material> {
    Material::examples().map(|(_, m)| m).to_vec()
}
fn builtin_names() -> Vec<String> {
    Material::examples()
        .map(|(name, _)| name.to_owned())
        .to_vec()
}
fn read_names<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Vec<String>, D::Error> {
    let mut values = Vec::<String>::deserialize(de)?;
    values.extend(builtin_names().into_iter().skip(values.len()));
    Ok(values)
}
fn read_values<'de, D: serde::Deserializer<'de>>(de: D) -> Result<Vec<Material>, D::Error> {
    let mut values = Vec::<Material>::deserialize(de)?;
    values.extend(builtin_values().into_iter().skip(values.len()));
    Ok(values)
}
impl Default for MaterialLibrary {
    fn default() -> Self {
        Self {
            names: builtin_names(),
            values: builtin_values(),
            copies: Vec::new(),
            hidden_builtins: Vec::new(),
            palette: [
                [0.95, 0.94, 0.90],
                [0.07, 0.08, 0.09],
                [0.72, 0.045, 0.025],
                [0.95, 0.55, 0.12],
                [0.95, 0.82, 0.25],
                [0.20, 0.60, 0.32],
                [0.12, 0.55, 0.65],
                [0.25, 0.28, 0.72],
            ]
            .map(|c| Material::from_srgb(c, 0.5, 0.).base_color)
            .to_vec(),
        }
    }
}
impl MaterialLibrary {
    /// Explicitly publish a project material into the independent personal library.
    pub fn save_asset(
        &mut self,
        asset: &crate::sculpt::material::layers::Asset,
    ) -> Result<(), String> {
        crate::sculpt::material::layers::validate_assets(std::slice::from_ref(asset))?;
        if (1..=self.names.len() as u64).contains(&asset.id) {
            self.names[asset.id as usize - 1] = asset.name.clone();
            self.values[asset.id as usize - 1] = asset.material.unlinked();
        } else {
            let preset = MaterialPreset {
                id: asset.id,
                name: asset.name.clone(),
                material: asset.material.unlinked(),
                original: asset.original.clone(),
            };
            if let Some(existing) = self.copies.iter_mut().find(|p| p.id == asset.id) {
                *existing = preset;
            } else {
                if self.copies.len() >= 64 {
                    return Err("The personal material library holds up to 64 custom presets. Hide a preset before saving another.".into());
                }
                self.copies.push(preset);
            }
        }
        self.hidden_builtins.retain(|id| *id != asset.id);
        Ok(())
    }
    pub fn remove(&mut self, id: u64) {
        self.copies.retain(|p| p.id != id);
        if (1..=self.names.len() as u64).contains(&id) && !self.hidden_builtins.contains(&id) {
            self.hidden_builtins.push(id);
        }
    }
    #[cfg(test)]
    pub fn copy(&mut self, name: &str, material: Material) -> Option<usize> {
        if self.copies.len() >= 64 {
            return None;
        }
        let stem: String = name.chars().take(24).collect();
        let mut name = format!("{stem} copy");
        let mut number = 2;
        while self.copies.iter().any(|p| p.name == name) {
            name = format!("{stem} copy {number}");
            number += 1;
        }
        self.copies.push(MaterialPreset {
            id: crate::sculpt::material::layers::fresh_id(),
            name,
            original: Some(material.unlinked()),
            material: material.unlinked(),
        });
        Some(self.copies.len() - 1)
    }
    pub fn add_colour(&mut self, colour: [f32; 3]) {
        if self.palette.len() < 32 && !self.palette.contains(&colour) {
            self.palette.push(colour);
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.names.len() != Material::examples().len()
            || self.values.len() != self.names.len()
            || self.copies.len() > 64
            || self.palette.len() > 32
            || self.hidden_builtins.len() > self.names.len()
            || self
                .hidden_builtins
                .iter()
                .any(|id| !(1..=self.names.len() as u64).contains(id))
        {
            return Err("Material library is too large".into());
        }
        if self
            .names
            .iter()
            .any(|name| name.trim().is_empty() || name.len() > 128)
        {
            return Err("Invalid material name".into());
        }
        for material in &self.values {
            material.validate()?;
        }
        for preset in &self.copies {
            if preset.name.trim().is_empty() || preset.name.len() > 128 {
                return Err("Invalid material name".into());
            }
            preset.material.validate()?;
            if let Some(original) = &preset.original {
                original.validate()?;
                if !original.links.is_empty() {
                    return Err("Material reset settings cannot reference another material".into());
                }
            }
        }
        for colour in &self.palette {
            Material {
                base_color: *colour,
                ..Default::default()
            }
            .validate()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::{Action, Key, Modifiers, Preferences, Shortcut, load, save};
    #[test]
    fn focused_paint_workspace_restores_layout_after_preferences_reload() {
        let mut prefs = Preferences::default();
        let before = prefs.workspace("Before Paint focus".into());
        prefs.focus_paint_workspace();
        assert!(prefs.paint_workspace_backup.is_some());
        assert!(
            prefs
                .panels
                .iter()
                .filter(|panel| panel.dock != crate::preferences::Dock::Hidden)
                .all(|panel| panel.tool == crate::preferences::Tool::Brush)
        );
        prefs.validate().unwrap();
        let mut restored: Preferences =
            serde_json::from_slice(&serde_json::to_vec(&prefs).unwrap()).unwrap();
        restored.restore_paint_workspace();
        assert_eq!(restored.workspace("Before Paint focus".into()), before);
        assert!(restored.paint_workspace_backup.is_none());
        restored.validate().unwrap();
    }

    #[test]
    fn project_material_edits_and_undo_do_not_publish_personal_presets() {
        use crate::{document::Document, sculpt::material::layers::Asset};
        let mut library = MaterialLibrary::default();
        let original = library.clone();
        let mut document = Document::default();
        let mut asset = Asset {
            id: 2,
            name: "Project clay".into(),
            material: library.values[1].clone(),
            original: None,
        };
        document.save_material(asset.clone()).unwrap();
        document.end_edit();
        asset.material.roughness = 0.23;
        document.save_material(asset.clone()).unwrap();
        document.end_edit();
        assert_eq!(library, original);
        document.undo();
        assert_eq!(library, original);
        document.redo();
        assert_eq!(document.materials()[0].material.roughness, 0.23);
        library.save_asset(&asset).unwrap();
        document.undo();
        assert_eq!(library.values[1].roughness, 0.23);
        assert_eq!(library.names[1], "Project clay");
        assert_ne!(document.materials()[0].material.roughness, 0.23);
    }

    #[test]
    fn explicit_personal_save_updates_by_id_and_restores_hidden_preset() {
        use crate::sculpt::material::layers::{Asset, fresh_id};
        let mut library = MaterialLibrary::default();
        let mut asset = Asset {
            id: fresh_id(),
            name: "My paint".into(),
            material: Material::default(),
            original: None,
        };
        library.save_asset(&asset).unwrap();
        asset.material.metallic = 0.8;
        library.save_asset(&asset).unwrap();
        assert_eq!(library.copies.len(), 1);
        assert_eq!(library.copies[0].material.metallic, 0.8);
        asset.id = 1;
        library.remove(1);
        library.save_asset(&asset).unwrap();
        assert!(!library.hidden_builtins.contains(&1));
        assert_eq!(library.values[0].metallic, 0.8);
        library.validate().unwrap();
    }

    #[test]
    fn material_library_migrates_presets_without_losing_custom_names_or_values() {
        for count in [6, 11] {
            let mut library = MaterialLibrary::default();
            library.names[4] = "My copper".into();
            library.values[4].roughness = 0.7;
            library.hidden_builtins.push(3);
            let mut old = serde_json::to_value(&library).unwrap();
            old["names"].as_array_mut().unwrap().truncate(count);
            old["values"].as_array_mut().unwrap().truncate(count);
            let migrated: MaterialLibrary = serde_json::from_value(old).unwrap();
            migrated.validate().unwrap();
            assert_eq!(migrated.names[..count], library.names[..count]);
            assert_eq!(migrated.values[..count], library.values[..count]);
            assert_eq!(migrated.hidden_builtins, [3]);
            let index = Material::DEFAULT_PRESET_ID as usize - 1;
            assert_eq!(migrated.names[index], "Default clay");
            assert_eq!(migrated.values[index], Material::default());
        }
    }
    #[test]
    fn material_library_copies_and_palette_survive_preferences_round_trip() {
        let mut prefs = Preferences::default();
        prefs.materials.names[4] = "Warm copper".into();
        let (name, material) = Material::examples()[4].clone();
        let index = prefs.materials.copy(name, material.clone()).unwrap();
        prefs.materials.copies[index].material.roughness = 0.8;
        assert_ne!(
            prefs.materials.copies[index].material,
            Material::examples()[4].1.clone()
        );
        assert_eq!(
            prefs.materials.copies[index].original,
            Some(material.unlinked())
        );
        prefs.materials.copy(name, material.clone()).unwrap();
        assert_ne!(
            prefs.materials.copies[0].name,
            prefs.materials.copies[1].name
        );
        prefs.materials.add_colour(material.base_color);
        let count = prefs.materials.palette.len();
        prefs.materials.add_colour(material.base_color);
        assert_eq!(count, prefs.materials.palette.len());
        let path =
            std::env::temp_dir().join(format!("clay-material-library-{}.json", std::process::id()));
        save(&path, &prefs).unwrap();
        assert_eq!(load(&path).unwrap(), prefs);
        std::fs::remove_file(path).unwrap();
        prefs.materials.palette.push([f32::NAN, 0., 0.]);
        assert!(prefs.validate().is_err());
    }
    #[test]
    fn material_p_shortcut_migrates_once_without_stealing_custom_bindings() {
        let path =
            std::env::temp_dir().join(format!("clay-material-keys-{}.json", std::process::id()));
        let mut old = Preferences::default();
        old.clear_binding(&Action::PolygroupColors);
        let mut json = serde_json::to_value(&old).unwrap();
        json.as_object_mut()
            .unwrap()
            .remove("paint_shortcuts_version");
        crate::persistence::write_json(&path.to_string_lossy(), &json).unwrap();
        let mut migrated = load(&path).unwrap();
        assert_eq!(
            migrated.shortcut(&Action::PolygroupColors),
            Some(&Shortcut::new(Key::P, Modifiers::NONE))
        );
        migrated.clear_binding(&Action::PolygroupColors);
        save(&path, &migrated).unwrap();
        assert!(
            load(&path)
                .unwrap()
                .shortcut(&Action::PolygroupColors)
                .is_none()
        );
        old.paint_shortcuts_version = 0;
        old.bind(Action::Wireframe, Shortcut::new(Key::P, Modifiers::NONE))
            .unwrap();
        save(&path, &old).unwrap();
        let migrated = load(&path).unwrap();
        assert!(migrated.shortcut(&Action::PolygroupColors).is_none());
        assert_eq!(
            migrated.shortcut(&Action::Wireframe),
            old.shortcut(&Action::Wireframe)
        );
        old.bind(
            Action::PolygroupColors,
            Shortcut::new(Key::P, Modifiers::SHIFT),
        )
        .unwrap();
        save(&path, &old).unwrap();
        assert_eq!(
            load(&path).unwrap().shortcut(&Action::PolygroupColors),
            old.shortcut(&Action::PolygroupColors)
        );
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn colour_swap_shortcut_migrates_once_and_preserves_custom_keys() {
        let path =
            std::env::temp_dir().join(format!("clay-colour-swap-keys-{}.json", std::process::id()));
        for mode in 0..3 {
            let mut old = Preferences {
                paint_shortcuts_version: 1,
                ..Default::default()
            };
            old.clear_binding(&Action::SwapPaintColours);
            if mode == 1 {
                old.bind(Action::Wireframe, Shortcut::new(Key::Z, Modifiers::NONE))
                    .unwrap();
            } else if mode == 2 {
                old.bind(
                    Action::SwapPaintColours,
                    Shortcut::new(Key::Z, Modifiers::ALT),
                )
                .unwrap();
            }
            save(&path, &old).unwrap();
            let mut migrated = load(&path).unwrap();
            let expected = match mode {
                0 => Some(Shortcut::new(Key::Z, Modifiers::NONE)),
                1 => None,
                _ => Some(Shortcut::new(Key::Z, Modifiers::ALT)),
            };
            assert_eq!(
                migrated.shortcut(&Action::SwapPaintColours),
                expected.as_ref()
            );
            assert_eq!(
                migrated.shortcut(&Action::Wireframe),
                old.shortcut(&Action::Wireframe)
            );
            assert_eq!(
                migrated.shortcut(&Action::Undo),
                old.shortcut(&Action::Undo)
            );
            assert_eq!(
                migrated.shortcut(&Action::Redo),
                old.shortcut(&Action::Redo)
            );
            migrated.clear_binding(&Action::SwapPaintColours);
            save(&path, &migrated).unwrap();
            assert_eq!(load(&path).unwrap(), migrated);
        }
        std::fs::remove_file(path).unwrap();
    }
}
