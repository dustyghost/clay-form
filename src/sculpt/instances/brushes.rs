use super::*;

pub(crate) const SHIP_GREEBLES_NAME: &str = "Ship greebles";

pub(crate) fn ship_greebles() -> &'static BrushBundle {
    static BRUSH: std::sync::OnceLock<BrushBundle> = std::sync::OnceLock::new();
    BRUSH.get_or_init(|| {
        let brush: BrushBundle = serde_json::from_str(include_str!("ship-greebles.objects.json"))
            .expect("Built-in ship greebles must deserialize");
        brush
            .validate()
            .expect("Built-in ship greebles must be valid");
        brush
    })
}

/// Named choices for an object brush. Meshes remain shared in the asset table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrushGroup {
    pub id: u64,
    pub name: String,
    pub objects: Vec<u64>,
}
impl BrushGroup {
    pub fn validate_name(name: &str) -> Result<(), String> {
        if name.trim().is_empty() || name.len() > 96 {
            return Err("Object brush names must contain 1–96 bytes".into());
        }
        Ok(())
    }
}

/// Portable one-brush file; includes each captured mesh once, no scene copies.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BrushBundle {
    pub version: u32,
    pub name: String,
    pub objects: Vec<Asset>,
}
impl BrushBundle {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported multi-object brush version".into());
        }
        BrushGroup::validate_name(&self.name)?;
        if self.objects.is_empty() {
            return Err("Capture at least one object before saving a brush".into());
        }
        Library {
            assets: self.objects.clone(),
            ..Default::default()
        }
        .validate(&[], &[])
    }
}
impl Library {
    pub(crate) fn new_brush_name(&self) -> String {
        (1..)
            .map(|n| {
                if n == 1 {
                    "Object brush".to_owned()
                } else {
                    format!("Object brush {n}")
                }
            })
            .find(|name| self.brushes.iter().all(|b| b.name != *name))
            .unwrap()
    }

    pub(crate) fn validate_brushes(&self) -> Result<(), String> {
        if self.brushes.len() > MAX_ASSETS {
            return Err("Projects are limited to 64 object brushes".into());
        }
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for brush in &self.brushes {
            BrushGroup::validate_name(&brush.name)?;
            if !names.insert(&brush.name) {
                return Err("Duplicate object brush name".into());
            }
            if brush.id == 0 || !ids.insert(brush.id) || brush.objects.len() > MAX_ASSETS {
                return Err("Invalid object brush identity or size".into());
            }
            let mut objects = HashSet::new();
            if brush
                .objects
                .iter()
                .any(|id| !objects.insert(*id) || !self.assets.iter().any(|a| a.id == *id))
            {
                return Err("Object brush contains a missing or duplicate object".into());
            }
        }
        Ok(())
    }
    pub fn brush_bundle(&self, id: u64) -> Result<BrushBundle, String> {
        let brush = self
            .brushes
            .iter()
            .find(|b| b.id == id)
            .ok_or("Choose an object brush first")?;
        let bundle = BrushBundle {
            version: 1,
            name: brush.name.clone(),
            objects: brush
                .objects
                .iter()
                .map(|id| {
                    self.assets
                        .iter()
                        .find(|a| a.id == *id)
                        .cloned()
                        .ok_or("Brush object is missing".to_owned())
                })
                .collect::<Result<_, _>>()?,
        };
        bundle.validate()?;
        Ok(bundle)
    }
}
