use crate::sculpt::Material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct MaterialPreset {
    #[serde(default = "crate::sculpt::material::layers::fresh_id")]
    pub id: u64,
    pub name: String,
    pub material: Material,
}

/// Personal copies and colours persist independently of the open project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct MaterialLibrary {
    #[serde(default = "builtin_names", deserialize_with = "read_names")]
    pub names: Vec<String>,
    #[serde(default = "builtin_values", deserialize_with = "read_values")]
    pub values: Vec<Material>,
    pub copies: Vec<MaterialPreset>,
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
    fn material_library_migrates_six_presets_without_losing_custom_names_or_values() {
        let mut library = MaterialLibrary::default();
        library.names[4] = "My copper".into();
        library.values[4].roughness = 0.7;
        let mut old = serde_json::to_value(&library).unwrap();
        old["names"].as_array_mut().unwrap().truncate(6);
        old["values"].as_array_mut().unwrap().truncate(6);
        let migrated: MaterialLibrary = serde_json::from_value(old).unwrap();
        migrated.validate().unwrap();
        assert_eq!(migrated.names[4], "My copper");
        assert_eq!(migrated.values[4].roughness, 0.7);
        assert_eq!(migrated.names.len(), 11);
        assert_eq!(migrated.names[7], "Beeswax");
        assert!(migrated.values[9].subsurface > 0.);
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
}
