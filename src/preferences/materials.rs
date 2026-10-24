use crate::sculpt::Material;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct MaterialPreset {
    pub name: String,
    pub material: Material,
}

/// Personal copies and colours persist independently of the open project.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct MaterialLibrary {
    pub copies: Vec<MaterialPreset>,
    pub palette: Vec<[f32; 3]>,
}
impl Default for MaterialLibrary {
    fn default() -> Self {
        Self {
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
        self.copies.push(MaterialPreset { name, material });
        Some(self.copies.len() - 1)
    }
    pub fn add_colour(&mut self, colour: [f32; 3]) {
        if self.palette.len() < 32 && !self.palette.contains(&colour) {
            self.palette.push(colour);
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.copies.len() > 64 || self.palette.len() > 32 {
            return Err("Material library is too large".into());
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
    fn material_library_copies_and_palette_survive_preferences_round_trip() {
        let mut prefs = Preferences::default();
        let (name, material) = Material::examples()[4];
        let index = prefs.materials.copy(name, material).unwrap();
        prefs.materials.copies[index].material.roughness = 0.8;
        assert_ne!(
            prefs.materials.copies[index].material,
            Material::examples()[4].1
        );
        prefs.materials.copy(name, material).unwrap();
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
